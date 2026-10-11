//! 配置持久化：把"用户确认过的设置"记到磁盘，下次直接带上。
//!
//! ## 只记一份，按整机
//!
//! 目前只持久化容器布局（背包/箱子/合成箱尺寸、佣兵全装备）。
//! 这些是 **mod 级**的东西——同一个 mod 安装下所有角色共用同一套尺寸，
//! 实测数据层的布局表里各职业的格子数完全一致（都是 10×4），
//! "每行一个职业"是为了不同角色面板的 UI 定位，不是每角色不同大小。
//!
//! 所以**整机存一份**就够了，不按角色、也不按 mod 目录分：一台机器上的
//! 玩家通常就一套游戏 + 一套 mod。这样"设置一次，以后打开任何角色都不用再填"。
//!
//! ## 存放位置
//!
//! 固定放 `%LOCALAPPDATA%\D2RSaveEditor\settings.json`。
//!
//! **不要放 exe 同级**：一开始为了"绿色版便携"选了那里，结果是开发时每次
//! `cargo build` 都会重写 `target\debug\`、设置跟着被清掉；更新版本换目录也一样会丢。
//! 目录不存在会自动创建；读失败一律当作"没有配置"（返回 `None`），不挡住主流程。
//!
//! 老版本把设置写在 exe 同级，`load()` 里做了一次迁移（见 [`ConfigStore::legacy_path`]）。

use std::path::{Path, PathBuf};

use d2r_core::LayoutSettings;

/// 配置文件名。
const FILE_NAME: &str = "settings.json";

/// 配置内容。
///
/// 老配置文件缺字段时靠 `#[serde(default)]` 兜住，所以**加字段是兼容的**。
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    /// 容器布局。
    #[serde(default)]
    pub layout: LayoutSettings,
    /// 用户是否确认过布局（没确认过就不要当成事实）。
    #[serde(default)]
    pub layout_confirmed: bool,
    /// 最近打开的存档路径（最新的在最前）。
    ///
    /// 用途是给"打开存档"对话框定起始目录，免得每次都从默认位置重新找。
    #[serde(default)]
    pub recent_paths: Vec<String>,
}

/// 配置文件的读写。
///
/// 不带 `Clone`：`cached` 是内部可变状态，克隆它没有意义。
#[derive(Debug, Default)]
pub struct ConfigStore {
    /// 显式指定的路径（`None` 时按候选顺序探测）。
    explicit: Option<PathBuf>,
    /// 探测到的可写路径（进程内缓存，避免每次保存都去戳磁盘）。
    cached: std::cell::RefCell<Option<PathBuf>>,
}

impl ConfigStore {
    /// 用默认位置。
    pub fn new() -> Self {
        Self::default()
    }

    /// 用指定文件（测试与 CLI `--config` 用）。
    pub fn at(path: impl Into<PathBuf>) -> Self {
        Self {
            explicit: Some(path.into()),
            cached: Default::default(),
        }
    }

    /// 解析"首选"配置文件路径（只用于显示；真正写哪由 [`Self::writable_path`] 决定）。
    pub fn path(&self) -> PathBuf {
        self.candidates()
            .into_iter()
            .next()
            .unwrap_or_else(|| user_config_dir().join(FILE_NAME))
    }

    /// 读取配置；文件不存在或内容坏了都返回 `None`。
    ///
    /// 走 [`Self::candidates`]：哪个候选位置有可读的配置就用哪个。
    /// 这样既能读到常规位置，也能读到旧版本留在 exe 同级的配置，
    /// 用户升级后设置不会"消失"。
    pub fn load(&self) -> Option<Settings> {
        for path in self.candidates() {
            if let Ok(text) = std::fs::read_to_string(&path) {
                if let Ok(s) = serde_json::from_str::<Settings>(&text) {
                    return Some(s);
                }
            }
        }
        None
    }

    /// 写入配置，返回实际写入的路径。
    ///
    /// 走 [`Self::writable_path`]：按候选顺序找一个**真的能写**的目录，
    /// 而不是假定某个固定位置一定可写。受限环境（只读目录、安全软件、
    /// 受控文件夹、各种沙箱）里这一点很重要——否则设置会静默丢失。
    pub fn save(&self, settings: &Settings) -> std::io::Result<PathBuf> {
        let json = serde_json::to_string_pretty(settings)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        let path = self.writable_path().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                format!(
                    "候选目录都不可写：{}",
                    self.candidates()
                        .iter()
                        .map(|p| p.display().to_string())
                        .collect::<Vec<_>>()
                        .join("、")
                ),
            )
        })?;
        write_atomic(&path, &json)?;
        Ok(path)
    }

    /// 实际可写的配置路径（首次调用时探测并缓存）。
    ///
    /// 探测方式：在候选目录里真写一个空文件，成功即为可用。
    /// 只探测一次并缓存，避免每次保存都去戳磁盘。
    pub fn writable_path(&self) -> Option<PathBuf> {
        if let Some(p) = &self.explicit {
            return Some(p.clone());
        }
        if let Some(p) = self.cached.borrow().clone() {
            return Some(p);
        }
        for path in self.candidates() {
            let Some(dir) = path.parent() else { continue };
            if !dir.is_dir() && std::fs::create_dir_all(dir).is_err() {
                continue;
            }
            let probe = dir.join(".d2r-write-probe");
            if std::fs::write(&probe, b"").is_ok() {
                let _ = std::fs::remove_file(&probe);
                *self.cached.borrow_mut() = Some(path.clone());
                return Some(path);
            }
        }
        None
    }

    /// 候选配置路径，按优先级排列。
    ///
    /// 顺序：环境变量覆盖 → 用户配置目录 → exe 同级 → 系统临时目录。
    /// 前两个是常规位置；exe 同级给"绿色便携"与受限环境兜底；
    /// 临时目录最后兜底（至少让本次运行能记住设置）。
    pub fn candidates(&self) -> Vec<PathBuf> {
        if let Some(p) = &self.explicit {
            return vec![p.clone()];
        }
        let mut out = Vec::new();
        if let Some(dir) = std::env::var_os(DIR_ENV) {
            if !dir.is_empty() {
                out.push(PathBuf::from(dir).join(FILE_NAME));
            }
        }
        out.push(user_config_dir().join(FILE_NAME));
        if let Some(dir) = exe_dir() {
            out.push(dir.join(FILE_NAME));
        }
        out.push(std::env::temp_dir().join("D2RSaveEditor").join(FILE_NAME));
        out
    }
}

/// exe 所在目录（拿不到时 `None`）。
fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()?
        .parent()
        .map(Path::to_path_buf)
}

/// 用户配置目录：`%LOCALAPPDATA%\D2RSaveEditor`（非 Windows 用临时目录下的同名子目录）。
fn user_config_dir() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    base.join("D2RSaveEditor")
}

/// 覆盖配置目录的环境变量。
///
/// 两个用途：
/// 1. 需要"设置跟着 exe 走"的便携用法（指到 exe 同级目录即可）；
/// 2. 排查写盘失败——把配置目录指到一个确定可写的位置，就能区分
///    "代码逻辑问题"和"目标目录权限问题"。
const DIR_ENV: &str = "D2R_CONFIG_DIR";

/// 原子写：先写临时文件再改名，避免写一半被打断留下坏配置。
fn write_atomic(path: &Path, text: &str) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text)?;
    // Windows 上 rename 不能覆盖已存在的文件，先删再改名
    if path.exists() {
        let _ = std::fs::remove_file(path);
    }
    std::fs::rename(&tmp, path)
}

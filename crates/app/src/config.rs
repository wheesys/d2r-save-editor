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
#[derive(Debug, Clone, Default)]
pub struct ConfigStore {
    /// 显式指定的路径（`None` 时按默认位置解析）。
    explicit: Option<PathBuf>,
}

impl ConfigStore {
    /// 用默认位置。
    pub fn new() -> Self {
        Self { explicit: None }
    }

    /// 用指定文件（测试与 CLI `--config` 用）。
    pub fn at(path: impl Into<PathBuf>) -> Self {
        Self {
            explicit: Some(path.into()),
        }
    }

    /// 解析实际使用的配置文件路径。
    ///
    /// 只用一个位置：`%LOCALAPPDATA%\D2RSaveEditor\settings.json`。
    ///
    /// **不要放在 exe 同级**。一开始为了"绿色版便携"选了 exe 同级，结果是：
    /// 开发时每次 `cargo build` 都会重写 `target\debug\`，设置跟着被清掉；
    /// 将来更新版本换了目录也一样会丢。用户目录才是稳定的地方。
    /// （唯一例外是显式 `at()` 指定的路径，测试用。）
    pub fn path(&self) -> PathBuf {
        if let Some(p) = &self.explicit {
            return p.clone();
        }
        fallback_dir().join(FILE_NAME)
    }

    /// 读取配置；文件不存在或内容坏了都返回 `None`。
    ///
    /// 顺带做一次**旧位置迁移**：曾经把设置写在 exe 同级，
    /// 那里还有文件而用户目录没有时读它并搬过来，免得老用户升级后设置"消失"。
    pub fn load(&self) -> Option<Settings> {
        let text = match std::fs::read_to_string(self.path()) {
            Ok(t) => t,
            Err(_) => {
                let legacy = self.legacy_path()?;
                let t = std::fs::read_to_string(&legacy).ok()?;
                // 搬过来（写失败也无所谓，至少这次读到了）
                let _ = write_atomic(&self.path(), &t);
                t
            }
        };
        serde_json::from_str(&text).ok()
    }

    /// 写入配置，返回实际写入的路径。
    pub fn save(&self, settings: &Settings) -> std::io::Result<PathBuf> {
        let json = serde_json::to_string_pretty(settings)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        let path = self.path();
        write_atomic(&path, &json)?;
        Ok(path)
    }

    /// 旧位置（exe 同级）的配置文件，仅用于迁移。
    fn legacy_path(&self) -> Option<PathBuf> {
        if self.explicit.is_some() {
            return None;
        }
        exe_dir().map(|d| d.join(FILE_NAME))
    }
}

/// exe 所在目录（拿不到时 `None`）。
fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()?
        .parent()
        .map(Path::to_path_buf)
}

/// 配置目录：`%LOCALAPPDATA%\D2RSaveEditor`（非 Windows 用临时目录下的同名子目录）。
fn fallback_dir() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    base.join("D2RSaveEditor")
}

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

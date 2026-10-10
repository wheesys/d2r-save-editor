//! 数据集（Dataset）：一个"游戏 + mod 组合"对应的全部数据表与字符串表。
//!
//! 对应调研文档 §4.2。设计目标是**不写死任何游戏常量**：
//! 容器行列数（`inventory.txt`）、物品宽高（`Armor/Weapons/Misc.txt`）、
//! 属性位宽（`ItemStatCost.txt`）、词缀（`MagicPrefix/Suffix.txt`）……
//! 全部按需从数据集读取，mod 改了数据就能自动适配。

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::strings::Strings;
use crate::table::{DataError, Table};

/// 表集合：按需从目录加载并缓存。
#[derive(Debug, Default)]
pub struct TableStore {
    dir: Option<PathBuf>,
    /// 覆盖目录（如用户自己装的 mod 数据表）：同名表优先从这里读。
    override_dir: Option<PathBuf>,
    /// 内存里的表（内嵌数据包用；不落盘）。
    memory: HashMap<String, Table>,
    cache: HashMap<String, Option<Table>>,
}

impl TableStore {
    /// 指定表目录（例如 `<游戏>\d2rloader\data\compiler\3.2.0\data\global\excel`）。
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: Some(dir.into()), override_dir: None, memory: HashMap::new(), cache: HashMap::new() }
    }

    /// 空集合（尚未绑定目录）。
    pub fn empty() -> Self {
        Self { dir: None, override_dir: None, memory: HashMap::new(), cache: HashMap::new() }
    }

    /// 挂一个覆盖目录（一般是用户装 mod 后 loader 展开出来的表）：
    /// 同名表以覆盖目录里的为准。
    pub fn add_override_dir(&mut self, dir: impl Into<PathBuf>) {
        self.override_dir = Some(dir.into());
        self.cache.clear();
    }

    /// 覆盖目录。
    pub fn override_dir(&self) -> Option<&Path> {
        self.override_dir.as_deref()
    }

    /// 往内存里放一张表（内嵌数据包用，避免解包到磁盘）。
    pub fn insert_memory(&mut self, name: &str, text: &str) {
        let key = name.to_ascii_lowercase();
        let t = Table::parse_tsv(key.clone(), format!("<bundle>/{name}"), text);
        self.memory.insert(key.clone(), t);
        self.cache.remove(&key);
    }

    /// 内存表数量。
    pub fn memory_len(&self) -> usize {
        self.memory.len()
    }

    /// 表目录。
    pub fn dir(&self) -> Option<&Path> {
        self.dir.as_deref()
    }

    /// 取表（找不到返回 `None`，不报错；按名字缓存）。
    ///
    /// 查找顺序：`<dir>/<name>.txt`（大小写不敏感扫描一次目录列表）。
    pub fn get(&mut self, name: &str) -> Option<&Table> {
        let key = name.to_ascii_lowercase();
        if !self.cache.contains_key(&key) {
            let loaded = self.load(&key);
            self.cache.insert(key.clone(), loaded);
        }
        self.cache.get(&key).and_then(|t| t.as_ref())
    }

    /// 目录下实际存在的表名列表（`.txt`）。
    pub fn available(&self) -> Vec<String> {
        let mut names: Vec<String> = self.memory.keys().cloned().collect();
        let Some(dir) = &self.dir else {
            names.sort();
            return names;
        };
        let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
        names.extend(rd.filter_map(|e| e.ok()).filter_map(|e| {
            let p = e.path();
            if p.extension()?.to_str()?.eq_ignore_ascii_case("txt") {
                Some(p.file_stem()?.to_string_lossy().to_lowercase())
            } else {
                None
            }
        }));
        names.sort();
        names.dedup();
        names
    }

    fn load(&self, lowercase_name: &str) -> Option<Table> {
        // 覆盖目录优先
        if let Some(dir) = self.override_dir.as_ref() {
            if let Some(t) = load_from_dir(dir, lowercase_name) {
                return Some(t);
            }
        }
        // 磁盘目录
        if let Some(dir) = self.dir.as_ref() {
            if let Some(t) = load_from_dir(dir, lowercase_name) {
                return Some(t);
            }
        }
        // 内存表（内嵌数据包）
        self.memory.get(lowercase_name).cloned()
    }
}

/// 从目录里按表名（大小写不敏感）读一张表。
fn load_from_dir(dir: &Path, lowercase_name: &str) -> Option<Table> {
    let wanted = format!("{lowercase_name}.txt");
    let rd = std::fs::read_dir(dir).ok()?;
    for entry in rd.flatten() {
        let path = entry.path();
        let Some(file_name) = path.file_name().and_then(|s| s.to_str()) else { continue };
        if file_name.eq_ignore_ascii_case(&wanted) {
            return Table::from_file(&path).ok();
        }
    }
    None
}

/// 一个数据集：元信息 + 表集合 + 字符串表。
#[derive(Debug)]
pub struct Dataset {
    /// 显示名（如 "3.2 + 疯兔2"、"3.2 原版"）。
    pub name: String,
    /// 游戏版本（如 "3.2.0"）。
    pub game_version: Option<String>,
    /// 游戏构建号（如 "92777"）。
    pub game_build: Option<String>,
    /// excel 表集合（mod 合并后的版本）。
    pub excel: TableStore,
    /// 原版基准表集合（未合并 mod），用于判定 mod 改动范围。
    pub excel_base: Option<TableStore>,
    /// 字符串表。
    pub strings: Strings,
    /// 数据来源根目录。
    pub root: PathBuf,
}

impl Dataset {
    /// 字符串表条目数。
    pub fn string_count(&self) -> usize {
        self.strings.len()
    }

    /// 查字符串（快捷方法）。
    pub fn text(&self, key: &str) -> Option<&str> {
        self.strings.text(key)
    }

    /// 从"数据集根目录"装载。
    ///
    /// 期望结构（D2RLoader 编译产物即满足）：
    /// ```text
    /// <root>/data/global/excel/          合并后的表
    /// <root>/data/global/excel/base/     loader 自己的基准层（**不是原版表**，见文档 §3.7）
    /// <root>/data/local/lng/strings/     字符串表（可选，或用 with_strings_dir 追加）
    /// ```
    pub fn load(root: impl AsRef<Path>) -> Result<Self, DataError> {
        let root = root.as_ref().to_path_buf();
        if !root.is_dir() {
            return Err(DataError::Missing(root));
        }
        let excel_dir = root.join("data").join("global").join("excel");
        if !excel_dir.is_dir() {
            return Err(DataError::Missing(excel_dir));
        }
        let base_dir = excel_dir.join("base");
        let strings_dir = root.join("data").join("local").join("lng").join("strings");

        let strings = if strings_dir.is_dir() {
            Strings::from_dir(&strings_dir).map_err(|e| DataError::Io(strings_dir.clone(), e))?
        } else {
            Strings::default()
        };

        Ok(Self {
            name: root
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "dataset".into()),
            game_version: None,
            game_build: None,
            excel: TableStore::new(excel_dir),
            excel_base: base_dir.is_dir().then(|| TableStore::new(base_dir)),
            strings,
            root,
        })
    }

    /// 追加/覆盖字符串表目录（就地合并）。
    pub fn with_strings_dir(&mut self, dir: impl AsRef<Path>) -> Result<usize, DataError> {
        let dir = dir.as_ref();
        let s = Strings::from_dir(dir).map_err(|e| DataError::Io(dir.to_path_buf(), e))?;
        self.strings.merge(s);
        Ok(self.strings.len())
    }

    /// 数据集指纹：表名 → (行数, 文件长度)，用于判断"两份数据集是否同一套"。
    pub fn fingerprint(&mut self) -> Vec<(String, usize, u64)> {
        let names = self.excel.available();
        let mut out = Vec::with_capacity(names.len());
        for name in names {
            if let Some(t) = self.excel.get(&name) {
                let len = std::fs::metadata(&t.source).map(|m| m.len()).unwrap_or(0);
                out.push((name, t.len(), len));
            }
        }
        out
    }
}

/// mod 描述（`mods/<名>/modinfo.json`）。
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ModInfo {
    /// mod 名。
    pub name: String,
    /// 存档路径（相对 mod 目录；`"../"` 即游戏默认存档目录）。
    #[serde(default)]
    pub savepath: Option<String>,
}

impl ModInfo {
    /// 从 `modinfo.json` 读取。
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, DataError> {
        let path = path.as_ref();
        let bytes = std::fs::read(path).map_err(|e| DataError::Io(path.to_path_buf(), e))?;
        serde_json::from_slice(&bytes).map_err(|e| DataError::Json(path.to_path_buf(), e))
    }

    /// 扫描 `mods/*/modinfo.json`，返回 (mod 目录, 描述)。
    pub fn scan(game_root: impl AsRef<Path>) -> Vec<(PathBuf, ModInfo)> {
        let mods = game_root.as_ref().join("mods");
        let Ok(rd) = std::fs::read_dir(&mods) else { return Vec::new() };
        let mut out = Vec::new();
        for entry in rd.flatten() {
            let dir = entry.path();
            let info = dir.join("modinfo.json");
            if info.is_file() {
                if let Ok(m) = Self::from_file(&info) {
                    out.push((dir, m));
                }
            }
        }
        out.sort_by(|a, b| a.1.name.cmp(&b.1.name));
        out
    }
}

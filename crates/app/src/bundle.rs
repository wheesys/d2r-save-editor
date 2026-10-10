//! 随程序分发的数据包（built-in data bundle）。
//!
//! 目标：**用户机器上什么都不用找，除了要改的存档**。
//! 因此官方 excel 表与字符串表在构建期就被打进二进制，运行时不依赖：
//! - 本机是否装了 D2R
//! - 是否装了 CascView
//! - 是否有 D2RLoader 的编译产物
//!
//! 数据包由 `tools/make-bundle.py` 生成到 `crates/app/data-bundle/`，
//! 该目录**不入 git**（暴雪版权数据），但会随安装包分发。
//! 数据包缺失时（例如刚 clone 还没导出数据）编译仍能通过，只是退化为
//! "名字显示成物品代码"，并在运行时给出提示。
//!
//! 数据优先级（见 `Workspace`）：
//! ```text
//! 显式指定的目录  >  随程序分发的数据包  >  本机游戏目录里的 loader 产物
//! ```

use std::path::PathBuf;

use include_dir::{include_dir, Dir};

/// 内嵌的数据包目录。缺失时用 `data-bundle-stub`（只有一个 `.keep`）占位。
#[cfg(feature = "bundled-data")]
static BUNDLE: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/data-bundle");
#[cfg(not(feature = "bundled-data"))]
static BUNDLE: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/data-bundle-stub");

/// 数据包清单（`BUNDLE.json`）解析结果。
#[derive(Debug, Clone, serde::Deserialize)]
pub struct BundleManifest {
    /// 清单结构版本。
    pub schema: u32,
    /// 说明。
    #[serde(default)]
    pub note: String,
    /// excel 表数量。
    pub excel_files: usize,
    /// 字符串表数量。
    pub string_files: usize,
    /// 总字节数。
    pub total_bytes: u64,
    /// 逐文件记录。
    #[serde(default)]
    pub files: Vec<BundleFile>,
}

/// 数据包里的一个文件。
#[derive(Debug, Clone, serde::Deserialize)]
pub struct BundleFile {
    /// 包内相对路径。
    pub path: String,
    /// 字节数。
    pub size: u64,
    /// sha256。
    pub sha256: String,
    /// 来源（仓库内路径）。
    #[serde(default)]
    pub source: String,
}

/// 数据包是否可用（有 excel 表才算可用）。
pub fn available() -> bool {
    BUNDLE.get_file("excel/inventory.txt").is_some()
}

/// 读取数据包清单；不可用时返回 `None`。
pub fn manifest() -> Option<BundleManifest> {
    let f = BUNDLE.get_file("BUNDLE.json")?;
    serde_json::from_slice(f.contents()).ok()
}

/// 数据包里的 excel 表名（小写，不含扩展名）。
pub fn excel_tables() -> Vec<String> {
    let Some(dir) = BUNDLE.get_dir("excel") else { return Vec::new() };
    let mut names: Vec<String> = dir
        .files()
        .filter_map(|f| {
            let name = f.path().file_name()?.to_str()?;
            let stem = name.strip_suffix(".txt")?;
            Some(stem.to_ascii_lowercase())
        })
        .collect();
    names.sort();
    names.dedup();
    names
}

/// 按表名读取 excel 表内容（大小写不敏感；找不到返回 `None`）。
pub fn excel_table(name: &str) -> Option<String> {
    let wanted = format!("{}.txt", name.to_ascii_lowercase());
    let dir = BUNDLE.get_dir("excel")?;
    for f in dir.files() {
        let file_name = f.path().file_name()?.to_str()?;
        if file_name.to_ascii_lowercase() == wanted {
            return Some(String::from_utf8_lossy(f.contents()).to_string());
        }
    }
    None
}

/// 字符串表的全部内容（文件名 → 文本）。
pub fn string_files() -> Vec<(String, String)> {
    let Some(dir) = BUNDLE.get_dir("strings") else { return Vec::new() };
    let mut out: Vec<(String, String)> = dir
        .files()
        .filter_map(|f| {
            let name = f.path().file_name()?.to_str()?.to_string();
            Some((name, String::from_utf8_lossy(f.contents()).to_string()))
        })
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// 把数据包里的 excel 表装进 `TableStore`（内存，不落盘）。
///
/// 为什么不落盘：应用进程在当前受限环境下写不了 `%TEMP%`，
/// 而且内存读取更快、也不留下临时副本。
pub fn load_tables_into(store: &mut d2r_data::TableStore) -> usize {
    let mut n = 0;
    for name in excel_tables() {
        if let Some(text) = excel_table(&name) {
            store.insert_memory(&name, &text);
            n += 1;
        }
    }
    n
}

/// 数据包里的字符串表（合并成一个 `Strings`）。
pub fn load_strings() -> d2r_data::Strings {
    let mut out = d2r_data::Strings::default();
    for (_, text) in string_files() {
        out.merge(d2r_data::Strings::from_json_bytes(text.as_bytes()));
    }
    out
}

/// 物品尺寸表（`code → (invwidth, invheight)`），来自数据包的 Armor/Weapons/Misc。
pub fn item_size_map() -> std::collections::BTreeMap<String, (u32, u32)> {
    let mut map = std::collections::BTreeMap::new();
    for table in ["Armor", "Weapons", "Misc"] {
        let Some(text) = excel_table(table) else { continue };
        let t = d2r_data::Table::parse_tsv(table, format!("<bundle>/{table}.txt"), &text);
        for row in 0..t.len() {
            let Some(code) = t.cell_nonempty(row, "code") else { continue };
            let w = t.int(row, "invwidth").unwrap_or(1).max(1) as u32;
            let h = t.int(row, "invheight").unwrap_or(1).max(1) as u32;
            map.insert(code.to_string(), (w, h));
        }
    }
    map
}
/// 数据包概览（给界面/CLI 显示）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct BundleInfo {
    /// 是否可用。
    pub available: bool,
    /// excel 表数量。
    pub excel_tables: usize,
    /// 字符串表文件数。
    pub string_files: usize,
    /// 清单里的总字节数。
    pub total_bytes: u64,
}

/// 概览。
pub fn info() -> BundleInfo {
    let m = manifest();
    BundleInfo {
        available: available(),
        excel_tables: excel_tables().len(),
        string_files: string_files().len(),
        total_bytes: m.map(|m| m.total_bytes).unwrap_or(0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundle_is_usable() {
        // 数据包不入 git，CI 上可能不存在 —— 不存在时只提示，不失败
        if !available() {
            eprintln!("提示：数据包不存在（先运行 python tools/make-bundle.py）");
            return;
        }
        let mut store = d2r_data::TableStore::empty();
        let loaded = load_tables_into(&mut store);
        assert!(loaded > 80, "装进 TableStore 的表数量偏少：{loaded}");
        assert!(store.get("Inventory").is_some(), "应从数据包装到 Inventory 表");
        eprintln!("已从数据包装载 {loaded} 张表；字符串 {} 条", load_strings().len());
        assert!(!item_size_map().is_empty(), "物品尺寸表不应为空");
        let tables = excel_tables();
        assert!(tables.contains(&"inventory".to_string()), "应包含 inventory 表");
        assert!(tables.contains(&"itemstatcost".to_string()));
        assert!(tables.len() > 80, "表数量偏少：{}", tables.len());

        let text = excel_table("Inventory").expect("大小写不敏感查找");
        assert!(text.contains("gridX"), "inventory 表应有 gridX 列");

        let strings = string_files();
        assert!(strings.len() >= 15, "字符串表文件偏少：{}", strings.len());
        assert!(strings.iter().any(|(n, _)| n == "item-names.json"));
    }
}

//! 随程序分发的数据包（built-in data bundle）。
//!
//! 目标：**用户机器上什么都不用找，除了要改的存档**。
//! 因此程序运行所需的数据表与文本表在构建期就被打进二进制，运行时不依赖：
//! - 本机是否装了 D2R
//! - 是否装了任何资源查看/解包工具
//! - 是否有 D2RLoader 的编译产物
//!
//! 数据包由 `tools/make-bundle.py` 在本机生成到 `crates/app/data-bundle/`，
//! 该目录**不入 git**，但会随安装包分发（内嵌进可执行文件）。
//! 数据包缺失时（例如刚 clone 还没导出数据）编译仍能通过，只是退化为
//! "名字显示成物品代码"，并在运行时给出提示。
//!
//! 数据优先级（见 `Workspace`）：
//! ```text
//! 显式指定的目录  >  随程序分发的数据包  >  本机游戏目录里的 loader 产物
//! ```

/// 构建期生成的嵌入数据（预压缩，运行时解压）。见 `build.rs`。
#[cfg(feature = "bundled-data")]
mod embedded {
    include!(concat!(env!("OUT_DIR"), "/bundle_data.rs"));
}
#[cfg(not(feature = "bundled-data"))]
mod embedded {
    pub const AVAILABLE: bool = false;
    pub static FILES: &[(&str, &[u8], usize)] = &[];
}

/// 数据包是否可用（内嵌了表才算可用）。
pub fn available() -> bool {
    cfg!(feature = "bundled-data") && embedded::AVAILABLE && !embedded::FILES.is_empty()
}

/// 解压一个内嵌文件。
fn unpack(compressed: &[u8], raw_len: usize) -> Option<Vec<u8>> {
    use std::io::Read;
    let mut out = Vec::with_capacity(raw_len);
    flate2::read::ZlibDecoder::new(compressed).read_to_end(&mut out).ok()?;
    Some(out)
}

/// 解压成文本。
fn unpack_text(compressed: &[u8], raw_len: usize) -> Option<String> {
    Some(String::from_utf8_lossy(&unpack(compressed, raw_len)?).to_string())
}

/// 按包内相对路径取文本（路径大小写不敏感）。
fn text_of(rel: &str) -> Option<String> {
    let wanted = rel.to_ascii_lowercase();
    embedded::FILES
        .iter()
        .find(|(name, _, _)| name.to_ascii_lowercase() == wanted)
        .and_then(|(_, data, len)| unpack_text(data, *len))
}

/// 数据包里的 excel 表名（小写，不含扩展名）。
pub fn excel_tables() -> Vec<String> {
    let mut names: Vec<String> = embedded::FILES
        .iter()
        .filter_map(|(name, _, _)| {
            let rest = name.strip_prefix("excel/")?;
            let stem = rest.strip_suffix(".txt")?;
            Some(stem.to_ascii_lowercase())
        })
        .collect();
    names.sort();
    names.dedup();
    names
}

/// 按表名读取 excel 表内容（大小写不敏感；找不到返回 `None`）。
pub fn excel_table(name: &str) -> Option<String> {
    text_of(&format!("excel/{name}.txt"))
}

/// 文本表的全部内容（文件名 → 文本）。
pub fn string_files() -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = embedded::FILES
        .iter()
        .filter_map(|(name, data, len)| {
            let file = name.strip_prefix("strings/")?.to_string();
            Some((file, unpack_text(data, *len)?))
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
    BundleInfo {
        available: available(),
        excel_tables: excel_tables().len(),
        string_files: string_files().len(),
        total_bytes: embedded::FILES.iter().map(|(_, _, len)| *len as u64).sum(),
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

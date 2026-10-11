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

/// 物品代码 → 图标的 data URL（`data:image/png;base64,...`）。
///
/// 为什么用 data URL：界面是网页，直接从内嵌数据生成数据 URL 就不需要
/// 静态文件服务或额外的 Tauri 资源协议；图标很小（323 张共约 1 MB），
/// 一次全量返回比逐个请求更简单。
///
/// 数据包缺失或没有 icons 时返回空表（界面会退化成只显示文字）。
pub fn item_icons() -> Vec<(String, String)> {
    let Some(map) = text_of("icons/IMAGE-MAP.tsv") else {
        return Vec::new();
    };
    let mut out = Vec::with_capacity(map.lines().count());
    for line in map.lines() {
        let Some((code, file)) = line.split_once('\t') else {
            continue;
        };
        let code = code.trim();
        let file = file.trim();
        if code.is_empty() || file.is_empty() {
            continue;
        }
        let Some(bytes) = raw_of(&format!("icons/{file}")) else {
            continue;
        };
        out.push((code.to_string(), png_data_url(&bytes)));
    }
    out
}

/// 图标的原始字节（给需要自己处理的调用方）。
pub fn icon_bytes(file_name: &str) -> Option<Vec<u8>> {
    raw_of(&format!("icons/{file_name}"))
}

/// 按包内相对路径取原始字节（路径大小写不敏感）。
fn raw_of(rel: &str) -> Option<Vec<u8>> {
    let wanted = rel.to_ascii_lowercase();
    embedded::FILES
        .iter()
        .find(|(name, _, _)| name.to_ascii_lowercase() == wanted)
        .and_then(|(_, data, len)| unpack(data, *len))
}

/// 编码成 PNG 的 data URL。
fn png_data_url(bytes: &[u8]) -> String {
    format!("data:image/png;base64,{}", base64_encode(bytes))
}

/// 极简 base64 编码（避免为几十行代码再加一个依赖）。
fn base64_encode(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[(triple >> 18) as usize & 63] as char);
        out.push(TABLE[(triple >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            TABLE[(triple >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[triple as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

/// 解压一个内嵌文件。
fn unpack(compressed: &[u8], raw_len: usize) -> Option<Vec<u8>> {
    use std::io::Read;
    let mut out = Vec::with_capacity(raw_len);
    flate2::read::ZlibDecoder::new(compressed)
        .read_to_end(&mut out)
        .ok()?;
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
        let Some(text) = excel_table(table) else {
            continue;
        };
        let t = d2r_data::Table::parse_tsv(table, format!("<bundle>/{table}.txt"), &text);
        for row in 0..t.len() {
            let Some(code) = t.cell_nonempty(row, "code") else {
                continue;
            };
            let w = t.int(row, "invwidth").unwrap_or(1).max(1) as u32;
            let h = t.int(row, "invheight").unwrap_or(1).max(1) as u32;
            map.insert(code.to_string(), (w, h));
        }
    }
    map
}
/// 数据包概览（给界面/CLI 显示）。
///
/// 注意：必须 `camelCase` 序列化——界面按 `totalBytes` / `excelTables` 读，
/// 少了这个属性会让界面拿到 `undefined`（曾经显示成 `NaN MB`）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BundleInfo {
    /// 是否可用。
    pub available: bool,
    /// excel 表数量。
    pub excel_tables: usize,
    /// 字符串表文件数。
    pub string_files: usize,
    /// 图标数量。
    pub icons: usize,
    /// 清单里的总字节数。
    pub total_bytes: u64,
}

/// 概览。
pub fn info() -> BundleInfo {
    BundleInfo {
        available: available(),
        excel_tables: excel_tables().len(),
        string_files: string_files().len(),
        icons: embedded::FILES
            .iter()
            .filter(|(name, _, _)| name.starts_with("icons/") && name.ends_with(".png"))
            .count(),
        total_bytes: embedded::FILES.iter().map(|(_, _, len)| *len as u64).sum(),
    }
}

/// 测试用：极简 base64 解码，用来验证上面的编码。
#[cfg(test)]
fn base64_decode_for_test(input: &str) -> Vec<u8> {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut lookup = [255u8; 256];
    for (i, c) in TABLE.iter().enumerate() {
        lookup[*c as usize] = i as u8;
    }
    let mut out = Vec::new();
    let mut buffer = 0u32;
    let mut bits = 0u32;
    for c in input.bytes() {
        if c == b'=' {
            break;
        }
        let v = lookup[c as usize];
        if v == 255 {
            continue;
        }
        buffer = (buffer << 6) | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
        }
    }
    out
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
        assert!(
            store.get("Inventory").is_some(),
            "应从数据包装到 Inventory 表"
        );
        eprintln!(
            "已从数据包装载 {loaded} 张表；字符串 {} 条",
            load_strings().len()
        );
        assert!(!item_size_map().is_empty(), "物品尺寸表不应为空");
        let tables = excel_tables();
        assert!(
            tables.contains(&"inventory".to_string()),
            "应包含 inventory 表"
        );
        assert!(tables.contains(&"itemstatcost".to_string()));
        assert!(tables.len() > 80, "表数量偏少：{}", tables.len());

        let text = excel_table("Inventory").expect("大小写不敏感查找");
        assert!(text.contains("gridX"), "inventory 表应有 gridX 列");

        let strings = string_files();
        assert!(strings.len() >= 15, "字符串表文件偏少：{}", strings.len());
        assert!(strings.iter().any(|(n, _)| n == "item-names.json"));

        let icons = item_icons();
        assert!(icons.len() > 250, "图标数量偏少：{}", icons.len());
        assert!(icons.iter().any(|(c, _)| c == "hp1"), "应有治疗药水的图标");
        assert!(
            icons
                .iter()
                .all(|(_, url)| url.starts_with("data:image/png;base64,")),
            "图标应是 PNG data URL"
        );
        // 编码结果必须能解回 PNG（否则界面拿到的是坏图）
        let (_, url) = icons.iter().find(|(c, _)| c == "hp1").unwrap();
        let payload = url.trim_start_matches("data:image/png;base64,");
        let bytes = base64_decode_for_test(payload);
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n", "解出来应是 PNG 头");
    }
}

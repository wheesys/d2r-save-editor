//! 内置装备库：暗金与套装的**固定属性**，导入时直接复现。
//!
//! ## 只做暗金 / 套装
//!
//! 白装、蓝装、黄装的属性是随机生成的，没有"标准值"可言；而暗金与套装的每一条
//! 属性都写在数据表里，可以按**满值**复现——导入一件 Shako 就真的是那件 Shako。
//!
//! ## 数据从哪来、为什么不每次解析
//!
//! 条目由 `tools/make-library.py` 在**构建期**整理成 `library/items.tsv`，
//! 随数据包内嵌进二进制；这里在第一次用到时解析**一次**并缓存
//! （[`std::sync::OnceLock`]）。导入几百次也只解析一次。
//!
//! 每条属性是 `(属性 ID, 数值, 层)`：属性 ID 由属性代码经
//! `properties.txt` 的 `func / stat / val` 翻译而来，层来自 `val`（技能页、
//! 单一技能、光环这类参数位）。
//!
//! ## 显示名
//!
//! TSV 里只存**名字键**，显示名在运行时按当前语言解析，所以产物不含界面文案。

use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};

/// 装备库里的一条物品。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryItem {
    /// 稳定标识（`u<ID>` / `s<ID>`）。
    pub id: String,
    /// 表里的 `*ID`；写进 `quality_data.file_index` 用的就是这个值。
    pub file_index: u16,
    /// 名字键（运行时解析成显示名）。
    pub name_key: String,
    /// 显示名（解析不出时等于名字键）。
    pub name: String,
    /// 基型代码。
    pub code: String,
    /// 品质（`暗金` / `套装`）。
    pub quality: String,
    /// 分组（界面按它做类型筛选）。
    pub group: String,
    /// 所属套装名（暗金为空）。界面按"套装名 → 内部部件"两级展示。
    pub set_name: String,
    /// 需求等级。
    pub required_level: i32,
    /// 占格宽。
    pub width: u32,
    /// 占格高。
    pub height: u32,
    /// 属性：(属性 ID, 数值, 层)。
    pub stats: Vec<LibraryStat>,
}

/// 装备库里的一条属性。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryStat {
    /// 属性 ID（`ItemStatCost`）。
    pub stat: u16,
    /// 数值（满值）。
    pub value: i64,
    /// 层（参数位）。
    pub layer: i32,
}

/// 从装备库导入的结果（给界面显示"放到哪了、带了几条属性"）。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    /// 新物品在物品列表里的序号。
    pub index: usize,
    /// 物品显示名。
    pub name: String,
    /// 写入了多少条属性。
    pub stats: usize,
    /// 放置的列。
    pub col: u8,
    /// 放置的行。
    pub row: u8,
}
/// 空白暗金小护符在暗金表里借用的行号（`uniqueitems.txt` 的 `*ID`）。
///
/// 暗金物品的**名字由表行决定**，所以存档里这件物品游戏会显示成那一行的原名。
/// 我们至少在本程序里显示成 [`BLANK_CHARM_NAME`]，避免和装备库里的名字不一致。
pub const BLANK_CHARM_FILE_INDEX: u16 = 381;

/// 空白暗金小护符的自造名字。
pub const BLANK_CHARM_ID: &str = "custom:charm";

/// 判断一件物品是不是我们自造的空白护符（靠"暗金小护符 + 该表行 + 无属性"识别）。
///
/// 只认这三个条件同时成立的情况，不会误判正常暗金护符（它们都有属性）。
pub fn is_blank_charm(
    code: &str,
    quality: &str,
    file_index: Option<u16>,
    stat_count: usize,
) -> bool {
    code == "cm1"
        && quality == "Unique"
        && file_index == Some(BLANK_CHARM_FILE_INDEX)
        && stat_count == 0
}

/// 缓存的装备库（**按语言**缓存：显示名是本地化的，不同语言不能共用）。
/// 自造条目的多语言名字。
///
/// 装备库里绝大多数名字来自字符串表（生成期已本地化），只有我们自己追加的
/// 条目没有语言来源，所以在这里按语言给名字。键是条目 id 的 `custom:` 后缀。
///
/// `lang` 与本项目其它处一致，用小写语言码（`zhcn`/`zhtw`/`enus`…）。
pub fn blank_charm_name(lang: &str) -> &'static str {
    custom_name(BLANK_CHARM_ID, lang).unwrap_or("上帝法印")
}

fn custom_name(id: &str, lang: &str) -> Option<&'static str> {
    if id != "custom:charm" {
        return None;
    }
    // 上帝法印：无属性、无级别限制的空白暗金小护符
    Some(match lang.to_ascii_lowercase().as_str() {
        "zhtw" => "上帝法印",
        "enus" => "Sigil of God",
        "dede" => "Siegel Gottes",
        "eses" | "esmx" => "Sello de Dios",
        "frfr" => "Sceau de Dieu",
        "itit" => "Sigillo di Dio",
        "kokr" => "신의 인장",
        "plpl" => "Pieczęć Boga",
        "jajp" => "神の印",
        "ptbr" => "Selo de Deus",
        "ruru" => "Печать Бога",
        // zhcn 与未知语言都用简中
        _ => "上帝法印",
    })
}

static CACHE: OnceLock<Mutex<BTreeMap<String, Vec<LibraryItem>>>> = OnceLock::new();

/// 全部装备库条目。
///
/// 第一次用到时解析并缓存，之后直接返回缓存——导入几百次也只解析一次。
/// 缓存按语言分开：切换语言后会重新解析一次，拿到该语言的物品名。
pub fn items(lang: &str, name_of: &mut dyn FnMut(&str) -> Option<String>) -> Vec<LibraryItem> {
    let cache = CACHE.get_or_init(|| Mutex::new(BTreeMap::new()));
    let mut guard = match cache.lock() {
        Ok(g) => g,
        // 上个持有者 panic 过：缓存本身还是好的，继续用里面的数据
        Err(poisoned) => poisoned.into_inner(),
    };
    if let Some(hit) = guard.get(lang) {
        return hit.clone();
    }
    let parsed = crate::bundle::library_text()
        .map(|text| {
            let mut items = parse(&text, name_of);
            // 自造条目没有语言来源，按当前语言覆盖名字
            for item in &mut items {
                if let Some(name) = custom_name(&item.id, lang) {
                    item.name = name.to_string();
                }
            }
            items
        })
        .unwrap_or_default();
    guard.insert(lang.to_string(), parsed.clone());
    parsed
}

/// 解析 TSV。
///
/// 列：`id, file_index, name_key, code, quality, group, required_level,
/// width, height, stats`。`stats` 是 `属性ID:值:层` 的逗号分隔列表。
fn parse(text: &str, name_of: &mut dyn FnMut(&str) -> Option<String>) -> Vec<LibraryItem> {
    let mut out = Vec::new();
    for line in text.lines().skip(1) {
        if line.trim().is_empty() {
            continue;
        }
        let cells: Vec<&str> = line.split('\t').collect();
        if cells.len() < 11 {
            continue;
        }
        let Ok(file_index) = cells[1].parse::<u16>() else {
            continue;
        };
        let name_key = cells[2].to_string();
        let stats: Vec<LibraryStat> = cells[10]
            .split(',')
            .filter(|s| !s.is_empty())
            .filter_map(|s| {
                let mut parts = s.split(':');
                let stat = parts.next()?.parse::<u16>().ok()?;
                let value = parts.next()?.parse::<i64>().ok()?;
                let layer = parts
                    .next()
                    .and_then(|v| v.parse::<i32>().ok())
                    .unwrap_or(0);
                Some(LibraryStat { stat, value, layer })
            })
            .collect();
        out.push(LibraryItem {
            id: cells[0].to_string(),
            file_index,
            name: name_of(&name_key).unwrap_or_else(|| name_key.clone()),
            name_key,
            code: cells[3].to_string(),
            quality: cells[4].to_string(),
            group: cells[5].to_string(),
            set_name: cells[6].to_string(),
            required_level: cells[7].parse().unwrap_or(0),
            width: cells[8].parse().unwrap_or(1),
            height: cells[9].parse().unwrap_or(1),
            stats,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_one_row() {
        let text = "id\tfile_index\tname_key\tcode\tquality\tgroup\tset_name\trequired_level\twidth\theight\tstats\n\
                    u123\t123\tHarlequin Crest\tuap\t暗金\t头盔\t\t69\t2\t2\t127:2:0,80:50:0";
        let items = parse(text, &mut |key| Some(format!("[{key}]")));
        assert_eq!(items.len(), 1);
        let item = &items[0];
        assert_eq!(item.id, "u123");
        assert_eq!(item.file_index, 123);
        assert_eq!(item.name, "[Harlequin Crest]");
        assert_eq!(item.code, "uap");
        assert_eq!(item.quality, "暗金");
        assert_eq!(item.group, "头盔");
        assert_eq!(item.required_level, 69);
        assert_eq!((item.width, item.height), (2, 2));
        assert_eq!(
            item.stats,
            vec![
                LibraryStat {
                    stat: 127,
                    value: 2,
                    layer: 0
                },
                LibraryStat {
                    stat: 80,
                    value: 50,
                    layer: 0
                },
            ]
        );
    }

    #[test]
    fn malformed_rows_are_skipped() {
        let text = "id\tfile_index\tname_key\tcode\tquality\tgroup\tset_name\trequired_level\twidth\theight\tstats\n\
                    短行\t1\n\
                    u1\tnot-a-number\ta\tb\tc\td\t\t1\t1\t1\t\n\
                    u2\t2\tname\thax\t暗金\t武器\t\t7\t1\t3\t0:8:0";
        let items = parse(text, &mut |_| None);
        assert_eq!(items.len(), 1, "只有最后一行合法");
        assert_eq!(items[0].stats.len(), 1);
    }
}

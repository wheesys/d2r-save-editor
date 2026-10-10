//! 物品显示名解析：把存档里的 4 字符物品代码 + 品质数据，变成玩家看得懂的名字。
//!
//! 命名规则（已用本地文本表实证，见 `docs/next-steps-materials.md`）：
//!
//! | 物品 | 字符串键 | 来源 |
//! |---|---|---|
//! | 普通物品/底材 | 物品代码（如 `rvl`） | `item-names.json` |
//! | 暗金 | `UniqueItems.txt` 的 `index` 列（如 `The Gnasher`） | `item-names.json` |
//! | 套装 | `Sets.txt` 的 `index` 列（如 `Angelical Raiment`） | `item-names.json` |
//! | 劣质/精良 | `Low Quality`/`Damaged`/`Cracked`/`Hiquality` | `item-nameaffixes.json` |
//!
//! 符文之语/词缀的键形态尚未实证（`Runeword*` 键不存在），因此这些暂时只给 ID，
//! **不猜**：宁可显示原始 ID，也不显示一个可能错的"名字"。

use crate::dataset::TableStore;
use crate::strings::Strings;

/// 物品名解析结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemName {
    /// 基础名（底材名，取不到时回退为物品代码）。
    pub base: String,
    /// 是否取到了真正的名字（false 表示回退成了代码）。
    pub resolved: bool,
    /// 装饰性前缀（如"精良的"），用于拼显示。
    pub quality_prefix: Option<String>,
    /// 说明后缀（如"（暗金 #123）"），名字取不到时给出线索。
    pub note: Option<String>,
}

impl ItemName {
    /// 拼成一行显示文本。
    pub fn display(&self) -> String {
        match &self.quality_prefix {
            Some(p) => format!("{p}{}", self.base),
            None => self.base.clone(),
        }
    }
}

/// 物品名解析器：把字符串表 + 表集合包起来，提供按物品取的接口。
pub struct ItemNamer<'a> {
    strings: &'a Strings,
    tables: &'a mut TableStore,
    lang: String,
}

impl<'a> ItemNamer<'a> {
    /// 新建解析器。`lang` 为语言代码（如 `zhCN`/`enUS`）。
    pub fn new(strings: &'a Strings, tables: &'a mut TableStore, lang: impl Into<String>) -> Self {
        Self {
            strings,
            tables,
            lang: lang.into(),
        }
    }

    /// 查字符串（按当前语言，失败回退首选语言链）。
    fn text(&self, key: &str) -> Option<&str> {
        self.strings
            .get(key, &self.lang)
            .or_else(|| self.strings.text(key))
    }

    /// 普通物品/底材名：键就是物品代码。
    pub fn base_name(&self, code: &str) -> Option<&str> {
        self.text(code)
    }

    /// 暗金物品名。
    ///
    /// 存档里存的是 `UniqueItems.txt` 的**行号**，而该表第 1 列 `index` 是字符串名
    /// （第 2 列 `*ID` 才是数字 ID），所以用 `*ID` 反查行号再取 `index` 当字符串键。
    pub fn unique_name(&mut self, file_index: u16) -> Option<String> {
        let key = {
            let t = self.tables.get("UniqueItems")?;
            let row = t
                .find_row_by("*ID", &file_index.to_string())
                .unwrap_or(file_index as usize);
            t.cell_nonempty(row, "index")?.to_string()
        };
        self.text(&key).map(|s| s.to_string())
    }

    /// 套装物品名：存档存的是 `Sets.txt` 的**行号**，该表第 1 列 `index` 本身就是字符串名。
    pub fn set_name(&mut self, file_index: u16) -> Option<String> {
        let key = {
            let t = self.tables.get("Sets")?;
            t.cell_nonempty(file_index as usize, "index")?.to_string()
        };
        self.text(&key).map(|s| s.to_string())
    }

    /// 劣质/精良的类型名（`file_index` 0-7）。
    pub fn inferior_prefix(&self, file_index: u8) -> Option<String> {
        const KEYS: [&str; 8] = [
            "Low Quality",
            "Damaged",
            "Cracked",
            "Crude",
            "Hiquality",
            "",
            "",
            "",
        ];
        KEYS.get(file_index as usize)
            .filter(|k| !k.is_empty())
            .and_then(|k| self.text(k))
            .map(|s| s.to_string())
    }

    /// 组装一个物品的显示名。
    ///
    /// `quality` 用调试字符串（如 `"Unique"`、`"Set"`、`"Magic"`），
    /// `file_index` 来自品质数据（暗金/套装/劣质的行号）。
    pub fn resolve(&mut self, code: &str, quality: &str, file_index: Option<u16>) -> ItemName {
        let mut quality_prefix = None;
        let mut note = None;

        // 先用 4 字符物品代码查底材名（普通/魔法/稀有/劣质/精良/套装的底材都靠它）
        let base_name = self.text(code).map(|s| s.to_string());

        let mut base = match quality {
            "Unique" => match file_index.and_then(|i| self.unique_name(i).map(|n| (i, n))) {
                Some((_, n)) => n,
                None => match &base_name {
                    Some(n) => n.clone(),
                    None => {
                        note = Some(match file_index {
                            Some(i) => format!("暗金行号 {i} 与代码 {code} 都查不到名字"),
                            None => format!("字符串表缺 {code}"),
                        });
                        code.to_string()
                    }
                },
            },
            "Set" => match file_index.and_then(|i| self.set_name(i).map(|n| (i, n))) {
                Some((_, n)) => n,
                None => match &base_name {
                    Some(n) => n.clone(),
                    None => {
                        note = Some(match file_index {
                            Some(i) => format!("套装行号 {i} 查不到名字"),
                            None => format!("字符串表缺 {code}"),
                        });
                        code.to_string()
                    }
                },
            },
            "Inferior" => {
                quality_prefix = file_index
                    .and_then(|i| self.inferior_prefix(i as u8))
                    .map(|p| format!("{p}的"));
                base_name.clone().unwrap_or_else(|| code.to_string())
            }
            "Superior" => {
                quality_prefix = self.text("Hiquality").map(|p| format!("{p}的"));
                base_name.clone().unwrap_or_else(|| code.to_string())
            }
            _ => base_name.clone().unwrap_or_else(|| code.to_string()),
        };

        if base.is_empty() {
            base = code.to_string();
        }
        // 走到这里 base 仍是代码，说明既没专有名也没底材名 → 给出可定位的提示，
        // 绝不静默"当作正常"（界面据此提示用户数据集可能不匹配）。
        if base == code && note.is_none() {
            note = Some(format!("字符串表缺条目：{code}"));
        }

        let resolved = base != code || quality_prefix.is_some();
        ItemName {
            base,
            resolved,
            quality_prefix,
            note,
        }
    }
}

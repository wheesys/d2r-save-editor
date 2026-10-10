//! 属性名解析：属性 ID → 人类可读名称。
//!
//! 映射链（已实测确认）：
//!
//! ```text
//! 存档属性 ID  ──→  ItemStatCost.txt 的 *ID 行
//!                        │
//!                        ├─ Stat          内部名（如 maxhp）
//!                        ├─ descstrpos    描述字符串键（如 ModStr1u）
//!                        ├─ Save Bits / Save Add / ValShift   编码参数
//!                        └─ descfunc == 0 表示"没有描述字符串"
//!                        │
//!                        └──→  文本表里的 Key（item-modifiers 等）→ 中文名
//! ```
//!
//! 界面上要让人"看得懂"就靠这一层；解析不出来时退回内部名，绝不编造。

use std::collections::BTreeMap;

use crate::strings::Strings;
use crate::table::Table;

/// 一个属性的展示信息。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatName {
    /// 属性 ID（`*ID`）。
    pub id: u16,
    /// 内部名（`Stat` 列，如 `maxhp`）。
    pub internal: String,
    /// 显示名（优先中文；解析不到时等于内部名）。
    pub display: String,
    /// 显示名是否成功解析（`false` 表示用的是内部名）。
    pub resolved: bool,
    /// 值的最小小步长（`1 << ValShift`，只对没有参数位的属性有意义）。
    pub step: i64,
    /// 是否带参数位（`Save Param Bits > 0`）。
    pub has_param_bits: bool,
    /// 游戏会不会为这个属性生成描述串（`descfunc != 0` 且描述键能在文本表里找到）。
    ///
    /// 这是"**值不值得让用户改**"最可靠的一条判据：游戏不为它生成描述的属性，
    /// 基本是引擎内部记账字段，改了不会在物品说明里体现，也不保证影响玩法。
    pub user_visible: bool,
    /// 描述串用的文字（洗掉格式占位符后的模板，保留下来做参考）。
    pub desc_template: Option<String>,
}

impl StatName {
    /// 是否适合作为"可编辑属性"展示给用户。
    ///
    /// 条件：游戏会为它生成描述（`user_visible`），并且描述串确实解析出了名字。
    pub fn is_editable(&self) -> bool {
        self.user_visible && self.resolved
    }
}

/// 属性名解析器。
#[derive(Debug, Clone, Default)]
pub struct StatNamer {
    by_id: BTreeMap<u16, StatName>,
}

impl StatNamer {
    /// 从 `ItemStatCost` 表与文本表构建。
    pub fn new(table: &Table, strings: &Strings) -> Self {
        let mut by_id = BTreeMap::new();
        for r in 0..table.len() {
            let Some(id) = table.int(r, "*ID").or_else(|| table.int(r, "ID")) else {
                continue;
            };
            let internal = table.cell_nonempty(r, "Stat").unwrap_or("").to_string();
            if internal.is_empty() {
                continue;
            }
            // 没有描述字符串的属性（descfunc == 0）直接用内部名
            let descfunc = table.int(r, "descfunc").unwrap_or(0);
            let key = table.cell_nonempty(r, "descstrpos").unwrap_or("");
            let mut template = None;
            let (display, resolved) = if descfunc != 0 && !key.is_empty() {
                // text() 按首选语言（zhCN）自动回退
                match strings.text(key) {
                    Some(text) if !text.is_empty() => {
                        template = Some(text.to_string());
                        let label = clean_label(text);
                        if label.is_empty() {
                            (internal.clone(), false)
                        } else {
                            (label, true)
                        }
                    }
                    _ => (internal.clone(), false),
                }
            } else {
                (internal.clone(), false)
            };
            let shift = table.int(r, "ValShift").unwrap_or(0).clamp(0, 31) as u32;
            // 游戏会为它生成描述，且描述串确实解析出了名字 → 值得让用户改
            let user_visible = descfunc != 0 && template.is_some();
            by_id.insert(
                id as u16,
                StatName {
                    id: id as u16,
                    internal,
                    display,
                    resolved,
                    step: 1i64 << shift,
                    has_param_bits: table.int(r, "Save Param Bits").unwrap_or(0) > 0,
                    user_visible,
                    desc_template: template,
                },
            );
        }
        Self { by_id }
    }

    /// 空表。
    pub fn empty() -> Self {
        Self::default()
    }

    /// 解析出的属性数量。
    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }

    /// 查一个属性。
    pub fn get(&self, id: u16) -> Option<&StatName> {
        self.by_id.get(&id)
    }

    /// 显示名；查不到时返回 `None`。
    pub fn name_of(&self, id: u16) -> Option<&str> {
        self.by_id.get(&id).map(|s| s.display.as_str())
    }

    /// 显示名，查不到时退回 `属性#<id>`。
    pub fn display_or_id(&self, id: u16) -> String {
        match self.by_id.get(&id) {
            Some(s) => s.display.clone(),
            None => format!("属性#{id}"),
        }
    }

    /// 值的最小小步长（没有信息时返回 1）。
    pub fn step_of(&self, id: u16) -> i64 {
        self.by_id.get(&id).map(|s| s.step).unwrap_or(1)
    }

    /// 全部属性（按 ID 升序），供界面做"添加属性"下拉。
    pub fn all(&self) -> Vec<&StatName> {
        self.by_id.values().collect()
    }

    /// 只要解析出显示名的属性（界面下拉优先用这个）。
    pub fn named(&self) -> Vec<&StatName> {
        self.by_id.values().filter(|s| s.resolved).collect()
    }

    /// **适合让用户编辑**的属性：游戏会为它生成描述，且描述串解析出了名字。
    ///
    /// 判据来自数据表本身：`descfunc != 0` 且 `descstrpos` 能在文本表里找到。
    /// 实测那 148 个 `descfunc == 0` 的属性**都不带描述串**，所以它们本来就没有
    /// 解析出名字、也不会出现在 [`Self::named`] 里；这个方法把"可编辑"的判据
    /// 写成显式条件，避免以后靠"有没有名字"这种间接推断。
    ///
    /// 将来若某个无描述属性被赋予名字（例如有人补了文本表），它**不会**进入这里，
    /// 因为游戏不会为它生成物品说明、改了也不会体现出来。
    pub fn editable(&self) -> Vec<&StatName> {
        self.by_id.values().filter(|s| s.is_editable()).collect()
    }

    /// 可编辑属性的数量。
    pub fn editable_count(&self) -> usize {
        self.by_id.values().filter(|s| s.is_editable()).count()
    }
}

/// 把描述模板洗成适合当标签的名字。
///
/// 文本表里存的是**格式串**（例如 `%+d 生命`、`火焰抗性 %+d%%`），
/// 直接当标签又丑又费解，所以去掉格式占位符与多余标点。
///
/// 规则：`%` 后跟着的可选标志/宽度/精度一律丢掉；`%%` 保留成一个 `%`。
/// 洗完之后如果什么都不剩（例如整条就是 `%d`），返回空串，调用方退回内部名。
fn clean_label(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        // 看过 `%` 之后的第一个字符：
        // - `%`  → 字面百分号，保留一个
        // - 数字/`+`/`-`/`.`/空格  → 标志与宽度，丢掉，继续找转换字符
        // - 字母（如 i/d/s/u/x）   → 转换字符，丢掉并结束
        #[allow(clippy::while_let_on_iterator)]
        while let Some(n) = chars.next() {
            if n == '%' {
                out.push('%');
                break;
            }
            if n.is_ascii_alphabetic() {
                break; // 转换字符，整段丢掉
            }
            // 标志/宽度/精度：继续往后看
        }
    }
    // 统一标点与空白：冒号变空格，压缩连续空格
    let unified: String = out
        .chars()
        .map(|c| if c == '：' || c == ':' { ' ' } else { c })
        .collect();
    let mut s = unified.split_whitespace().collect::<Vec<_>>().join(" ");
    // 去掉残留的两端分隔符（例如 "耐久度 /"）
    s = s
        .trim_matches(|c: char| c == '/' || c == '-' || c == '%' || c.is_whitespace())
        .to_string();
    s = s.trim().to_string();
    // 只剩标点符号的不算名字
    if !s.chars().any(|c| c.is_alphanumeric() || !c.is_ascii()) {
        return String::new();
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn real() -> Option<(StatNamer, usize)> {
        let mut store = crate::dataset::TableStore::empty();
        // 数据包不入库：找不到就跳过
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()?
            .join("app")
            .join("data-bundle");
        if !dir.join("excel").is_dir() {
            return None;
        }
        store.add_override_dir(dir.join("excel"));
        let mut strings = Strings::default();
        if let Ok(rd) = std::fs::read_dir(dir.join("strings")) {
            for e in rd.flatten() {
                if let Ok(text) = std::fs::read_to_string(e.path()) {
                    strings.merge(Strings::from_json_bytes(text.as_bytes()));
                }
            }
        }
        let table = store.get("ItemStatCost")?;
        let n = StatNamer::new(table, &strings);
        let count = n.len();
        Some((n, count))
    }

    #[test]
    fn resolves_real_stat_names() {
        let Some((namer, count)) = real() else {
            eprintln!("跳过：没有数据包");
            return;
        };
        assert!(count > 100, "属性数量偏少：{count}");

        // maxhp（ID 7）：descstrpos = ModStr1u
        let hp = namer.get(7).expect("maxhp 应在表里");
        assert_eq!(hp.internal, "maxhp");
        assert_eq!(hp.step, 256, "ValShift 8 → 步长 256");
        assert!(hp.resolved, "应解析出中文名：{hp:?}");
        assert!(!hp.display.is_empty());

        // 常见属性都应解析出名字
        let named = namer.named();
        assert!(named.len() > 50, "解析出名字的属性偏少：{}", named.len());
        assert!(namer.name_of(0).is_some(), "力量应有名字");
    }

    #[test]
    fn label_cleaning() {
        assert_eq!(clean_label("%+d 力量"), "力量");
        assert_eq!(clean_label("火焰抗性 %+d%%"), "火焰抗性");
        assert_eq!(clean_label("耐久度: %i / %i"), "耐久度");
        assert_eq!(clean_label("%d"), "");
        assert_eq!(clean_label("  生命  "), "生命");
        assert_eq!(clean_label("%+d%% 提升攻击速度"), "提升攻击速度");
    }

    #[test]
    fn unknown_stat_falls_back_to_id() {
        let namer = StatNamer::empty();
        assert!(namer.is_empty());
        assert_eq!(namer.display_or_id(4242), "属性#4242");
        assert_eq!(namer.step_of(4242), 1);
    }
}

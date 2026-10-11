//! 属性取值上限（来自数据表 `ItemStatCost.txt` 的位宽列）。
//!
//! 为什么需要：属性是**按位存储**的。写一个超出位宽的值不会报错，而是被静默截断，
//! 例如 `maxhp`（Save Bits 9 / Save Add 32）只能表示 `[-32, 479]`，
//! 写 1234 会被截成 1024 —— 用户看到"改动没生效"，或者更糟，写进一个错误数值。
//!
//! 因此编辑属性前先按位宽校验，超范围直接拒绝。
//!
//! 映射用 `*ID` 列（不是 `Stat` 列）：实测属性在存档里的 ID 就是 `*ID`，
//! 例如 `maxhp` 的 `*ID` 是 7。
//!
//! 注意：这只是**单字段编码**的边界。带 `Save Param Bits` 的属性（按等级/数量参数化）
//! 编码更复杂，这里只保证"值落在位宽能表示的范围内"，不做语义校验。

use std::collections::BTreeMap;

use crate::dataset::TableStore;
use crate::table::Table;

/// 把每级加成格式化成给用户看的文本。
///
/// 是整数就不带小数点（`8` 而不是 `8.000`），否则保留必要位数
/// （生命每级是 `7.875`，砍成 `7.88` 会让人对不上游戏里的数）。
pub fn format_rate(rate: f64) -> String {
    if rate.fract() == 0.0 {
        format!("{}", rate as i64)
    } else {
        // 去掉末尾多余的 0，同时保留足够精度
        let s = format!("{rate:.3}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// 一个属性的取值上限。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatLimit {
    /// 属性 ID（`ItemStatCost.txt` 的 `*ID`）。
    pub id: u16,
    /// 位宽。
    pub save_bits: u8,
    /// 偏移量。
    pub save_add: u32,
    /// 是否带参数位。
    ///
    /// 带参数位的属性，存档里会**另外写一段 layer 位**，值本身仍是这段位宽，
    /// 所以范围检查照样适用（参数不作为值的一部分）。
    pub has_param_bits: bool,
    /// 是否带编码（`Encode` 非 0，表示值是枚举/索引语义）。**保留位宽检查**。
    pub has_encoding: bool,
    /// 是不是"按等级加成"的属性（`ItemStatCost` 的 `Stat` 以 `perlevel` 结尾）。
    ///
    /// 这类属性的**数值部分是系数**，不是直接加成：游戏按
    /// `值 / 2^op_param × 角色等级` 算出实际加成。
    /// 只显示一个裸数字会让人误以为是总值（用户实际提过这个问题）。
    pub per_level: bool,
    /// 按等级加成时的除法因子指数（`ItemStatCost` 的 `op param`）。
    ///
    /// 官方公式（见 `itemstatcost.txt` 的 `op` / `op param` / `op base`）：
    /// `实际加成 = 值 / 2^op_param × op_base`，`op base` 是 `level`（角色等级）。
    /// "`2^op param` = 每级 1 点需要多少值"。
    /// 实测本表里 38 个 `*perlevel` 属性中，除命中类是 1 之外**全是 3**（即除以 8）。
    pub op_param: u8,
    /// 移位量（`ValShift`）。
    ///
    /// 存档里的写法是 `写成 << ValShift`，所以**界面上的值必须是
    /// `2^val_shift` 的倍数**，否则写盘时会因右移而静默变值
    /// （例如 maxhp 的 ValShift=8，值必须是 256 的倍数）。
    pub val_shift: u8,
}

impl StatLimit {
    /// 值的最小可表示单位（`2^ValShift`）。
    ///
    /// 例如 maxhp 的 `ValShift=8` → 256：只能存 0、256、512…
    pub fn step(&self) -> i64 {
        if self.val_shift >= 62 {
            return i64::MAX;
        }
        1i64 << self.val_shift
    }

    /// 位宽能表示的最小值（移位前的原始值）。
    fn raw_min(&self) -> i64 {
        -(self.save_add as i64)
    }

    /// 位宽能表示的最大值（移位前的原始值）。
    fn raw_max(&self) -> i64 {
        if self.save_bits >= 62 {
            return i64::MAX;
        }
        (1i64 << self.save_bits) - 1 - self.save_add as i64
    }

    /// 可表示的最小值（**存档原始值**）。
    pub fn min(&self) -> i64 {
        if self.val_shift > 0 {
            self.step()
        } else {
            self.raw_min()
        }
    }

    /// 可表示的最大值（**存档原始值**）。
    pub fn max(&self) -> i64 {
        if self.save_bits >= 62 {
            return i64::MAX;
        }
        self.raw_max() << self.val_shift
    }

    /// 值是否可表示（范围对 + 落在步长上）。
    pub fn accepts(&self, value: i64) -> bool {
        value >= self.min() && value <= self.max() && value % self.step() == 0
    }

    /// 值是否会被 d2s **整个跳过**（写盘后属性消失）。
    ///
    /// 写侧是 `shifted = value >> val_shift; if shifted == 0 { continue }`，
    /// 所以非正的值都会被丢掉——这是最容易踩、也最难看出的坑。
    pub fn is_dropped(&self, value: i64) -> bool {
        (value >> self.val_shift.min(62)) == 0
    }

    /// 人类可读的范围，例如 `0–107`、`1–479`。
    ///
    /// 用的是**游戏里显示的值**（已换算），不是存档里的原始数——
    /// 用户不该看到 -27 这种内部值。
    pub fn range_text(&self) -> String {
        let (lo, hi) = self.display_range();
        format!("{}–{}", format_rate(lo), format_rate(hi))
    }

    /// 按等级加成属性的**每级实际加成**（游戏里"每级 +X"的那个 X）。
    ///
    /// 官方公式（`ItemStatCost.txt` 的 `op` / `op param` / `op base`）：
    /// `实际加成 = 值 × 角色等级 / 2^op_param`，所以
    /// **每级加成 = 值 / 2^op_param**（`op param` 就是"每级 1 点需要多少值"的指数）。
    ///
    /// 用 `f64` 是因为结果可能不是整数：生命 `item_hp_perlevel` 的系数是 63、
    /// `op param` 是 3，所以每级是 `63/8 = 7.875`。界面直接显示这个数，
    /// **不要再乘角色等级**（用户明确要求只看每级多少）。
    /// 界面上给用户看的**每级系数**（仅 `*perlevel` 属性有意义）。
    ///
    /// 这类属性的值在游戏里要按 `值 × 角色等级 / 2^op_param` 算，
    /// 但**编辑的仍是存档里的原始值**。这个数字只用于"说明"，
    /// 让用户知道这条属性是按等级加成的、每级大概多少。
    pub fn per_level_rate(&self, value: i64) -> f64 {
        let divisor = (1i64 << self.op_param.min(62)) as f64;
        value as f64 / divisor
    }

    /// 界面上的值 = **存档里的值本身**。
    ///
    /// 用户决定（2026-10）：**不做游戏显示值换算**，直接按数据大小处理，
    /// 只把每条属性的有效范围说明清楚。原因是各属性的处理方式不同
    /// （`ItemStatCost` 里还有 `Encode` / `Add` / `Multiply` 这些列，
    /// 没有可靠的公开定义），与其猜错，不如把原始数据如实呈现。
    pub fn to_user(&self, stored: i64) -> f64 {
        stored as f64
    }

    /// [`Self::to_user`] 的逆运算：界面值就是存档值。
    pub fn to_stored(&self, display: f64) -> i64 {
        display.round() as i64
    }

    /// 界面上给用户看的数：就是存档里的值。
    pub fn display_value(&self, stored: i64) -> f64 {
        stored as f64
    }

    /// [`Self::display_value`] 的逆运算。
    pub fn stored_from_display(&self, display: f64) -> i64 {
        display.round() as i64
    }

    /// 界面上的有效范围 = **存档里的有效范围**。
    pub fn display_range(&self) -> (f64, f64) {
        (self.min() as f64, self.max() as f64)
    }

    /// 界面输入框的最小变化量。
    ///
    /// 带移位（`ValShift`）的属性在存档里按 `2^ValShift` 的精度存，
    /// 值必须是它的倍数，所以界面步长就是 [`Self::step`]。
    pub fn user_step(&self) -> f64 {
        self.step() as f64
    }
}

/// 全部属性的上限表（按属性 ID）。
#[derive(Debug, Clone, Default)]
pub struct StatLimits {
    by_id: BTreeMap<u16, StatLimit>,
}

impl StatLimits {
    /// 从 `ItemStatCost` 表构建。
    pub fn from_table(table: &Table) -> Self {
        let mut by_id = BTreeMap::new();
        for r in 0..table.len() {
            let Some(id) = table.int(r, "*ID").or_else(|| table.int(r, "ID")) else {
                continue;
            };
            let Some(bits) = table.int(r, "Save Bits") else {
                continue;
            };
            if bits <= 0 || bits > 62 {
                continue;
            }
            let add = table.int(r, "Save Add").unwrap_or(0).max(0) as u32;
            let param = table.int(r, "Save Param Bits").unwrap_or(0) > 0;
            let encoding = table.int(r, "Encode").unwrap_or(0) != 0;
            let val_shift = table.int(r, "ValShift").unwrap_or(0).clamp(0, 62) as u8;
            // 按等级加成的属性：内部名以 perlevel 结尾。
            // 数据表里它们 DESC 用的都是 descfunc=19（按等级增强），可以交叉印证，
            // 但直接用名字判断更稳（mod 可能改 descfunc）。
            let per_level = table
                .cell(r, "Stat")
                .map(|n| n.to_ascii_lowercase().ends_with("perlevel"))
                .unwrap_or(false);
            // 按等级加成的除法因子：官方公式是 值 / 2^op_param × 角色等级
            let op_param = table.int(r, "op param").unwrap_or(0).clamp(0, 62) as u8;
            by_id.insert(
                id as u16,
                StatLimit {
                    id: id as u16,
                    save_bits: bits as u8,
                    save_add: add,
                    has_param_bits: param,
                    has_encoding: encoding,
                    per_level,
                    op_param,
                    val_shift,
                },
            );
        }
        Self { by_id }
    }

    /// 从表集合构建（表名大小写不敏感）。表不存在时返回空表（不限制）。
    pub fn from_store(store: &mut TableStore) -> Self {
        match store.get("ItemStatCost") {
            Some(t) => Self::from_table(t),
            None => Self::default(),
        }
    }

    /// 空表（不限制任何属性）。
    pub fn empty() -> Self {
        Self::default()
    }

    /// 表里有几个属性。
    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }

    /// 查某个属性的上限。
    pub fn get(&self, id: u16) -> Option<&StatLimit> {
        self.by_id.get(&id)
    }

    /// 硬校验一个**界面值**（游戏里看到的值）：不可表示就拒绝。
    ///
    /// 逐条属性各按自己的规格判（`ItemStatCost.txt` 的 `Save Bits` /
    /// `Save Add` / `ValShift`），因为**不同属性的有效值完全不同**：
    /// `strength` 是 -32–223，`maxhp` 是 0–130816 且必须是 256 的倍数，
    /// `item_allskills` 只有 0–7。
    ///
    /// `label` 是给用户看的属性名——**有名字就用名字**，别丢一个 ID 让人猜。
    ///
    /// 三类拒因分开说，避免用户猜：
    /// 1. 值右移后为 0 会被 d2s 整个跳过（写盘后属性**消失**）；
    /// 2. 不在步长上会被右移截断（静默变值）；
    /// 3. 超出位宽能表示的范围。
    pub fn check(&self, id: u16, value: i64) -> Result<(), String> {
        self.check_named(id, value, None)
    }

    /// 与 [`Self::check`] 相同，但可以带一个给用户看的名字。
    pub fn check_named(&self, id: u16, value: i64, label: Option<&str>) -> Result<(), String> {
        let Some(limit) = self.get(id) else {
            return Ok(()); // 表里没有的属性不限制（可能是 mod 自定义）
        };
        let who = match label {
            Some(n) if !n.is_empty() => format!("「{n}」"),
            _ => format!("属性 {id}"),
        };

        // 会被整个跳过：写侧 `shifted = value >> val_shift; if shifted == 0 { continue }`
        if limit.is_dropped(value) {
            return Err(format!(
                "{who}的值 {value} 会让这条属性**写盘后消失**：\
                 它按 {step} 的精度存储，这个值右移后为 0。有效范围 {range}",
                step = limit.step(),
                range = limit.range_text()
            ));
        }

        if value % limit.step() != 0 {
            return Err(format!(
                "{who}的值 {value} 存不进去：它按 {step} 的精度存储，\
                 只能是 {step} 的倍数，否则会被截断成 {truncated}。有效范围 {range}",
                step = limit.step(),
                truncated = (value >> limit.val_shift) << limit.val_shift,
                range = limit.range_text()
            ));
        }

        if !limit.accepts(value) {
            return Err(format!(
                "{who}的值 {value} 超出有效范围 {range}，写进去会被静默截断",
                range = limit.range_text()
            ));
        }
        Ok(())
    }

    /// 把界面提交的**用户可见值**换算回**存档里的整数**。
    ///
    /// 按等级加成的属性，界面是"每级加多少"（`7.875`），存档是系数（`63`）；
    /// 移位类界面是 63、存档是 16128。**换算完全在内部完成。**
    pub fn display_to_raw(&self, id: u16, display: f64) -> i64 {
        match self.get(id) {
            Some(l) => l.stored_from_display(display),
            None => display.round() as i64,
        }
    }

    /// 软提示：值虽然可表示，但可能不是用户想表达的意思。
    ///
    /// 带 `Save Param Bits` 的属性，参数（layer）是**另外一段位**，
    /// 所以这里只提示一句，不阻止。
    pub fn advise(&self, id: u16, value: i64) -> Option<String> {
        let limit = self.get(id)?;
        if limit.has_param_bits {
            return Some(format!(
                "属性 ID {id} 带参数位（按等级/数量参数化），参数存在单独一段位里；\
                 这里改的是数值部分，有效范围 {}",
                limit.range_text()
            ));
        }
        if limit.has_encoding {
            return Some(format!(
                "属性 ID {id} 带编码（Encode），值是索引语义而非直接数值；有效范围 {}",
                limit.range_text()
            ));
        }
        let _ = value;
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 造一个上限，方便逐条验证。
    fn limit(id: u16, bits: u8, add: u32, val_shift: u8) -> StatLimit {
        StatLimit {
            id,
            save_bits: bits,
            save_add: add,
            has_param_bits: false,
            has_encoding: false,
            per_level: false,
            op_param: 0,
            val_shift,
        }
    }

    #[test]
    fn plain_stat_range_math() {
        // strength：Save Bits 8 / Save Add 32 / 无移位 → [-32, 223]
        let l = limit(0, 8, 32, 0);
        assert_eq!(l.step(), 1);
        assert_eq!((l.min(), l.max()), (-32, 223));
        assert!(l.accepts(-32));
        assert!(l.accepts(223));
        assert!(!l.accepts(224), "超出位宽会被截断");
        assert!(!l.accepts(-33));
    }

    /// 带移位的属性：值必须是 2^ValShift 的倍数，且不能为 0（会被丢弃）。
    #[test]
    fn shifted_stat_requires_multiples() {
        // maxhp：Save Bits 9 / Save Add 32 / ValShift 8
        let l = limit(7, 9, 32, 8);
        assert_eq!(l.step(), 256);
        // 原始可表示 [-32, 479] → 界面值 [256, 122624]
        assert_eq!(l.min(), 256, "0 会被跳过，所以最小有效值是 256");
        assert_eq!(l.max(), 479 << 8);
        assert!(l.accepts(256));
        assert!(l.accepts(512));
        assert!(l.accepts(479 << 8));
        assert!(!l.accepts(100), "不是 256 的倍数");
        assert!(!l.accepts(0), "会被写侧跳过");
        assert!(!l.accepts((479 << 8) + 256), "超出位宽");
    }

    /// 值右移后为 0 会被 d2s 整个跳过——属性直接消失，必须拦住。
    ///
    /// 注意负数**不属于**这一类：Rust 的 `>>` 是算术右移，
    /// `-1 >> 8 == -1`，所以它不会被跳过，而是写成 `0xFFFFFFFF` 再被截成
    /// 9 位——变成垃圾值。负数由范围检查挡住（带移位的属性 `min()` 是正的）。
    #[test]
    fn zero_shifted_value_is_reported_as_dropped() {
        let l = limit(7, 9, 32, 8);
        assert!(l.is_dropped(0));
        assert!(l.is_dropped(255), "255 >> 8 == 0，会被整个跳过");
        assert!(!l.is_dropped(256));
        // 负数是"写成垃圾值"而不是"被跳过"，走范围检查
        assert!(!l.is_dropped(-1), "算术右移不会变成 0");
        assert!(!l.accepts(-1), "但负数仍应被范围检查拒绝");
    }

    /// 三个拒因分别给出可读的原因，别让用户猜。
    #[test]
    fn check_explains_each_failure() {
        let mut limits = StatLimits::empty();
        limits.by_id.insert(7, limit(7, 9, 32, 8)); // maxhp
        limits.by_id.insert(0, limit(0, 8, 32, 0)); // strength
        limits.by_id.insert(127, limit(127, 3, 0, 0)); // item_allskills → 0..7
        limits.by_id.insert(96, limit(96, 7, 20, 0)); // 更快速跑步/步行

        // 消失
        let e = limits.check(7, 0).unwrap_err();
        assert!(e.contains("消失"), "{e}");
        // 步长（措辞要让人看懂是"精度"，不是内部的"步长"）
        let e = limits.check(7, 100).unwrap_err();
        assert!(e.contains("精度"), "{e}");
        assert!(e.contains("256"), "{e}");
        // 超范围
        let e = limits.check(127, 9).unwrap_err();
        assert!(e.contains("超出有效范围"), "{e}");
        // 有名字时用名字，不要丢一个 ID 让用户猜
        let e = limits
            .check_named(96, 500, Some("更快速跑步/步行"))
            .unwrap_err();
        assert!(e.contains("更快速跑步/步行"), "{e}");
        assert!(!e.contains("属性 96"), "有名字就不该再显示 ID：{e}");

        // 合法值放行
        assert!(limits.check(7, 256).is_ok());
        assert!(limits.check(0, 100).is_ok());
        assert!(limits.check(127, 7).is_ok());
        // 表里没有的不限制
        assert!(limits.check(9999, i64::MAX).is_ok());
    }

    /// **按数据大小处理**：界面显示/编辑的值就是存档里的值本身。
    ///
    /// 用户决定（2026-10）：不做游戏显示值换算（各属性处理方式不同，
    /// `ItemStatCost` 里还有 `Encode` / `Add` / `Multiply` 没有可靠公开定义），
    /// 只把每条属性的**有效范围**说明清楚。
    #[test]
    fn display_is_the_stored_value_itself() {
        for (id, bits, add, vs) in [
            (76u16, 6u8, 10u32, 0u8), // item_maxhp_percent
            (96, 7, 20, 0),           // item_fastermovevelocity
            (127, 3, 0, 0),           // item_allskills
            (7, 9, 32, 8),            // maxhp
        ] {
            let l = limit(id, bits, add, vs);
            // 往返恒等：界面值 == 存档值
            for v in [l.min(), 0, 1, l.max()] {
                assert_eq!(l.display_value(v), v as f64, "ID {id} 显示值应等于存档值");
                assert_eq!(l.stored_from_display(v as f64), v, "ID {id} 往返应恒等");
            }
        }
    }

    /// 范围按**存档值**给出（位宽 + 偏移 + 移位算出来的那个区间）。
    #[test]
    fn range_is_in_stored_units() {
        // item_maxhp_percent：6 位、SaveAdd 10 → -10..53
        let hpp = limit(76, 6, 10, 0);
        assert_eq!(hpp.range_text(), "-10–53");
        // item_allskills：3 位、无偏移 → 0..7
        assert_eq!(limit(127, 3, 0, 0).range_text(), "0–7");
        // item_fastermovevelocity：7 位、SaveAdd 20 → -20..107
        assert_eq!(limit(96, 7, 20, 0).range_text(), "-20–107");
        // maxhp：9 位、SaveAdd 32、ValShift 8 → 步长 256，区间 256..122624
        let hp = limit(7, 9, 32, 8);
        assert_eq!(hp.range_text(), "256–122624");
        assert_eq!(hp.user_step(), 256.0, "取值须是 256 的倍数");
    }

    /// 按等级加成的属性：**编辑的仍是存档值**，per_level_rate 只用于说明。
    #[test]
    fn per_level_stats_edit_stored_value() {
        let mut strl = limit(220, 6, 0, 0);
        strl.per_level = true;
        strl.op_param = 3;
        // 编辑值就是存档值 61，不做任何除法
        assert_eq!(strl.display_value(61), 61.0);
        assert_eq!(strl.stored_from_display(61.0), 61);
        // 说明用的"每级"是 61/8 = 7.625（游戏内按 值×等级/8 生效）
        assert_eq!(format_rate(strl.per_level_rate(61)), "7.625");

        // 同时带移位（SaveAdd 0、ValShift 8）：存档值 16128，步长 256
        let mut hpl = limit(216, 6, 0, 8);
        hpl.per_level = true;
        hpl.op_param = 3;
        assert_eq!(hpl.display_value(16128), 16128.0, "按数据大小处理");
        assert_eq!(hpl.stored_from_display(16128.0), 16128);
        assert_eq!(hpl.user_step(), 256.0);
    }
    /// 参数位是**另算一段位**的，所以值仍按位宽判，只给提示。
    #[test]
    fn param_bits_still_check_value_range() {
        let mut limits = StatLimits::empty();
        let mut l = limit(83, 3, 0, 0); // item_addclassskills
        l.has_param_bits = true;
        limits.by_id.insert(83, l);

        assert!(
            limits.check(83, 3).is_ok(),
            "参数不是值的一部分，值照样判范围"
        );
        assert!(limits.check(83, 9).is_err(), "值本身超出 3 位");
        assert!(limits.advise(83, 3).unwrap().contains("参数位"));
    }

    /// 按等级加成属性的**每级**加成：`值 / 2^op_param`。
    ///
    /// 官方来源：`ItemStatCost.txt` 的 `op` / `op param` / `op base`，
    /// 公式见 Phrozen Keep 的说明（`2^op param` = 每级 1 点需要多少值，
    /// `op base` 在这里是角色等级）。
    ///
    /// 这条测试同时钉住**运算顺序**：必须"先乘后除"。
    /// 我先写成先整除再乘，52 级时得出 357 而不是 409（用户实测是 409）。
    #[test]
    fn per_level_uses_character_level() {
        let mut hp = limit(216, 6, 0, 8); // item_hp_perlevel
        hp.per_level = true;
        hp.op_param = 3; // 除以 8

        // 用户实测：52 级角色，这件装备的生命加成是 409（= 7.875×52）。
        // 界面只显示每级多少，所以这里断言"每级"。
        assert_eq!(hp.per_level_rate(63), 7.875, "63/8");
        assert_eq!(format_rate(hp.per_level_rate(63)), "7.875");
        // 与用户实测的合计对得上：7.875 × 52 = 409.5 → 409
        assert_eq!((hp.per_level_rate(63) * 52.0) as i64, 409);

        // 力量系数 61 → 每级 7.625；61 × 52 / 8 = 396.5 → 396（用户实测 396）
        let mut strl = limit(220, 6, 0, 0);
        strl.per_level = true;
        strl.op_param = 3;
        assert_eq!(format_rate(strl.per_level_rate(61)), "7.625");
        assert_eq!((strl.per_level_rate(61) * 52.0) as i64, 396);

        // **同时带移位与按等级加成**（item_hp_perlevel：op param 3 + ValShift 8）
        // 按数据大小处理：编辑的就是存档里的 16128（步长 256）。
        // 说明用的"每级"= 16128 / 256 / 8 = 7.875（先按精度还原再除以 8）。
        let mut hpl = limit(216, 6, 0, 8);
        hpl.per_level = true;
        hpl.op_param = 3;
        assert_eq!(hpl.display_value(16128), 16128.0, "编辑值=存档值");
        assert_eq!(hpl.stored_from_display(16128.0), 16128);
        assert_eq!(hpl.user_step(), 256.0, "取值须是 256 的倍数");
        assert_eq!(
            format_rate(hpl.per_level_rate(16128 / 256)),
            "7.875",
            "说明用的每级 = (16128/256)/8"
        );

        // 命中类属性是 op param = 1（每级 1 点需要 2 点值）
        let mut ar = limit(224, 6, 0, 0); // item_tohit_perlevel
        ar.per_level = true;
        ar.op_param = 1;
        assert_eq!(ar.per_level_rate(10), 5.0, "10/2");
        assert_eq!(format_rate(ar.per_level_rate(10)), "5", "整数不带小数点");
    }
}

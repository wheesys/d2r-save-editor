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
    pub fn per_level_rate(&self, value: i64) -> f64 {
        let divisor = (1i64 << self.op_param.min(62)) as f64;
        value as f64 / divisor
    }

    /// 界面上的值（**位字段原值**）→ `d2s` 期望的存储值。
    ///
    /// `d2s` 的编码是 `value = (field - SaveAdd) << ValShift`，直接照这个来。
    /// 顺序不能换：`(field << vs) - add` 在 `SaveAdd` 非零时会算错
    /// （`maxhp` 字段 798 会得到 204256 而不是 196096，用穷举验过）。
    pub fn to_stored(&self, display: f64) -> i64 {
        let field = display.round() as i64;
        (field - self.save_add as i64) << self.val_shift.min(62)
    }

    /// `d2s` 的存储值 → 界面上显示的**位字段原值**。
    ///
    /// `d2s` 解码是 `field = (value >> ValShift) + SaveAdd`，照这个来。
    ///
    /// ## 为什么游戏显示的是字段值
    ///
    /// 验证（`crates/cli/src/bin/fieldcheck.rs`）：全部 263 条属性的实测取值
    /// 按这个假设算出的字段都放得进位宽，没有一条超宽；区间也与用户在游戏里
    /// 看到的对上：
    /// - `item_maxhp_percent`（6 位）→ 用户说范围 **0–63**，
    ///   而把字段再减 `SaveAdd` 会得到 -10..53（那个负数就是假的）；
    /// - `item_fastermovevelocity`（7 位）→ 用户说最大 **127**，
    ///   减 `SaveAdd` 会变成 107。
    ///
    /// 即 `SaveAdd` 是**位字段的内部偏置**，游戏界面显示的是加回偏置后的字段值，
    /// 修改器也要这样做才和游戏里一致。
    pub fn to_user(&self, stored: i64) -> f64 {
        ((stored >> self.val_shift.min(62)) + self.save_add as i64) as f64
    }

    /// 界面输入框的最小变化量（`step`）。
    ///
    /// 按等级加成的属性：每级 1 点需要 `2^op_param` 点值，
    /// 所以界面上最小变化是 `1 / 2^op_param`（生命是 `0.125`）。
    pub fn user_step(&self) -> f64 {
        if self.per_level {
            1.0 / (1i64 << self.op_param.min(62)) as f64
        } else {
            1.0
        }
    }

    /// 界面上给用户看的数。
    ///
    /// - 按等级加成的属性（`*perlevel`）：**每级加多少** = 字段值 / `2^op_param`
    ///   （生命字段 63、`op param` 3 → `7.875`）。用户明确要求只显示每级多少。
    /// - 其它属性：就是**字段值**本身（`item_maxhp_percent` 显示 31 之类）。
    pub fn display_value(&self, stored: i64) -> f64 {
        let field = self.to_user(stored);
        if self.per_level {
            field / (1i64 << self.op_param.min(62)) as f64
        } else {
            field
        }
    }

    /// [`Self::display_value`] 的逆运算：把界面上填的数换回 `d2s` 的存储形态。
    pub fn stored_from_display(&self, display: f64) -> i64 {
        let field = if self.per_level {
            display * (1i64 << self.op_param.min(62)) as f64
        } else {
            display
        };
        self.to_stored(field)
    }

    /// 界面上的有效范围（与 [`Self::display_value`] 同单位）。
    pub fn display_range(&self) -> (f64, f64) {
        let divisor = if self.per_level {
            (1i64 << self.op_param.min(62)) as f64
        } else {
            1.0
        };
        (
            self.to_user(self.min()) / divisor,
            self.to_user(self.max()) / divisor,
        )
    }

    /// 把**存档里的值**换算成**游戏里显示的值**。
    ///
    /// 只需要处理移位（`ValShift`）：带移位的属性存的是放大过的量，
    /// 生命/法力 按 1/256 精度存（存档 16128 → 游戏显示 **63**）。
    ///
    /// **不加/减 `Save Add`**：实测存档里的攻速/施法/跑速类值本身就是
    /// 游戏显示值（10/20/30/40 都是标准词缀值，`item_allskills` 是 7），
    /// 所以 `Save Add` 只是**位字段的内部偏移**，不是显示偏移。
    /// 一开始按"减去 Save Add"换算，得出"存档 50 → 显示 30"，
    /// 但那样 127 会变成 107，和用户看到的字段满值 127 对不上。
    pub fn to_display(&self, raw: i64) -> i64 {
        if self.val_shift > 0 {
            raw / self.step()
        } else {
            raw
        }
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

    /// 显示值 = **存档位字段的原值**（游戏里显示的就是它）。
    ///
    /// 用户要求"修改器展示的和游戏内一致"。验证见
    /// `crates/cli/src/bin/fieldcheck.rs`：全部 263 条属性的实测取值按这个
    /// 假设都能落进位宽，且区间与用户在游戏里看到的吻合
    /// （`item_maxhp_percent` → 0–63、`item_fastermovevelocity` → 最大 127）。
    #[test]
    fn display_is_the_raw_field_value() {
        // item_maxhp_percent：6 位、SaveAdd 10 → d2s 给的 31 对应字段 41
        let hpp = limit(76, 6, 10, 0);
        assert_eq!(hpp.to_user(31), 41.0, "字段 = d2s值 + SaveAdd");
        assert_eq!(hpp.to_stored(41.0), 31, "存回 d2s 形态");
        assert_eq!(hpp.display_range().1, 63.0, "6 位满值 63");

        // item_allskills：SaveAdd 0 → 字段值就是 d2s 值
        let sk = limit(127, 3, 0, 0);
        assert_eq!(sk.to_user(7), 7.0);
        assert_eq!(sk.range_text(), "0–7");

        // item_fastermovevelocity：7 位、SaveAdd 20 → 字段上限 127
        let frw = limit(96, 7, 20, 0);
        assert_eq!(frw.to_user(30), 50.0);
        assert_eq!(frw.to_stored(50.0), 30);
        assert_eq!(frw.display_range().1, 127.0);
    }

    /// 按等级加成：界面只给"每级加多少" = 字段值 / 2^op_param。
    #[test]
    fn display_value_for_per_level_stats() {
        // 无位移：字段 61、op param 3 → 每级 7.625
        let mut strl = limit(220, 6, 0, 0);
        strl.per_level = true;
        strl.op_param = 3;
        assert_eq!(strl.to_user(61), 61.0);
        assert_eq!(strl.display_value(61), 7.625);
        assert_eq!(strl.stored_from_display(7.625), 61);
        assert!((strl.user_step() - 0.125).abs() < 1e-9);

        // 同时带位移（item_hp_perlevel：SaveAdd 0、ValShift 8）：
        // d2s 值 16128 → 字段 (16128 >> 8) + 0 = 63 → 每级 63/8 = 7.875
        let mut hpl = limit(216, 6, 0, 8);
        hpl.per_level = true;
        hpl.op_param = 3;
        assert_eq!(hpl.to_user(16128), 63.0, "先还原字段");
        assert_eq!(hpl.display_value(16128), 7.875, "再除以 8 得每级");
        assert_eq!(hpl.stored_from_display(7.875), 16128, "存回 d2s 形态");
    }

    /// 显示值 ↔ 原始值的换算（**用户只看游戏值**）。
    ///
    /// 这条是用户明确要求的："我们修改器给用户看的要是游戏里面展示的，
    /// 换算是我们自己完成的，不要让用户感知到"。
    #[test]
    fn display_conversion_matches_game() {
        // 生命：ValShift 8 / SaveAdd 32 → d2s 的 16128 对应字段 95
        let hp = limit(7, 9, 32, 8);
        assert_eq!(hp.to_user(16128), 95.0, "(16128>>8)+32");
        assert_eq!(hp.to_stored(95.0), 16128, "存回 d2s 形态");
        assert_eq!(hp.to_display(16128), 63, "仅在只有移位、无偏置时成立");

        // 跑速：**不换算**（存档里的值就是字段值再减偏置）
        let frw = limit(96, 7, 20, 0);
        assert_eq!(frw.to_user(30), 50.0);
        assert_eq!(frw.to_stored(50.0), 30);

        // 所有技能：3 位 → 0..7
        let sk = limit(127, 3, 0, 0);
        assert_eq!(sk.range_text(), "0–7");
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
        // 先把 d2s 值还原成字段（`16128 >> 8 = 63`），再除以 8 得每级 7.875。
        // 只做其中一个会得到 2016 或 63（都踩过）。
        let mut hpl = limit(216, 6, 0, 8);
        hpl.per_level = true;
        hpl.op_param = 3;
        assert_eq!(hpl.to_user(16128), 63.0, "先还原字段");
        assert_eq!(hpl.display_value(16128), 7.875, "再除以 8 得每级");
        assert_eq!(hpl.stored_from_display(7.875), 16128, "存回 d2s 形态");
        assert_eq!(format_rate(hpl.display_value(16128)), "7.875");
        // 界面步长是 1/8 = 0.125（填 7.875 → 存 63）
        assert!((hpl.user_step() - 0.125).abs() < 1e-9);

        // 命中类属性是 op param = 1（每级 1 点需要 2 点值）
        let mut ar = limit(224, 6, 0, 0); // item_tohit_perlevel
        ar.per_level = true;
        ar.op_param = 1;
        assert_eq!(ar.per_level_rate(10), 5.0, "10/2");
        assert_eq!(format_rate(ar.per_level_rate(10)), "5", "整数不带小数点");
    }
}

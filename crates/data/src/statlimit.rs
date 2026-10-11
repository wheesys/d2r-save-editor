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

    /// 可表示的最小**界面值**（游戏里看到的值）。
    ///
    /// 有移位时原始值 `raw` 会左移成 `raw << val_shift`；而 d2s 在写盘时
    /// **值为 0 的属性会被整个跳过**（属性直接消失），所以最小有效值是
    /// `step()`（即第一个非零可表示值）。无移位时就是 `raw_min()`。
    pub fn min(&self) -> i64 {
        if self.val_shift > 0 {
            self.step()
        } else {
            self.raw_min()
        }
    }

    /// 可表示的最大**界面值**。
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

    /// 人类可读的范围与步长，例如 `0–2560（步长 256）`。
    pub fn range_text(&self) -> String {
        if self.val_shift == 0 {
            format!("{}–{}", self.min(), self.max())
        } else {
            format!("{}–{}（步长 {}）", self.min(), self.max(), self.step())
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
            by_id.insert(
                id as u16,
                StatLimit {
                    id: id as u16,
                    save_bits: bits as u8,
                    save_add: add,
                    has_param_bits: param,
                    has_encoding: encoding,
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
    /// 三类拒因分开说，避免用户猜：
    /// 1. 非正的值会被 d2s 整个跳过（写盘后属性**消失**）；
    /// 2. 不在步长上会被右移截断（静默变值）；
    /// 3. 超出位宽能表示的范围。
    pub fn check(&self, id: u16, value: i64) -> Result<(), String> {
        let Some(limit) = self.get(id) else {
            return Ok(()); // 表里没有的属性不限制（可能是 mod 自定义）
        };

        // 会被整个跳过：写侧 `shifted = value >> val_shift; if shifted == 0 { continue }`
        if limit.is_dropped(value) {
            return Err(format!(
                "属性 ID {id} 的值 {value} 会导致这条属性**写盘后消失**：\
                 存的是「值右移 {} 位」，而这个值右移后为 0，程序会跳过它。\
                 有效范围 {}",
                limit.val_shift,
                limit.range_text()
            ));
        }

        if value % limit.step() != 0 {
            return Err(format!(
                "属性 ID {id} 的值 {value} 存不进去：该属性在存档里带 {} 位移位，\
                 只能是步长 {} 的倍数，否则会被截断成 {}。有效范围 {}",
                limit.val_shift,
                limit.step(),
                (value >> limit.val_shift) << limit.val_shift,
                limit.range_text()
            ));
        }

        if !limit.accepts(value) {
            return Err(format!(
                "属性 ID {id} 的值 {value} 超出可表示范围 {}（Save Bits {} / Save Add {} / ValShift {}），\
                 写进去会被静默截断",
                limit.range_text(),
                limit.save_bits,
                limit.save_add,
                limit.val_shift
            ));
        }
        Ok(())
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

        // 消失
        let e = limits.check(7, 0).unwrap_err();
        assert!(e.contains("消失"), "{e}");
        // 步长
        let e = limits.check(7, 100).unwrap_err();
        assert!(e.contains("步长 256"), "{e}");
        // 超范围
        let e = limits.check(127, 9).unwrap_err();
        assert!(e.contains("超出可表示范围"), "{e}");

        // 合法值放行
        assert!(limits.check(7, 256).is_ok());
        assert!(limits.check(0, 100).is_ok());
        assert!(limits.check(127, 7).is_ok());
        // 表里没有的不限制
        assert!(limits.check(9999, i64::MAX).is_ok());
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
}

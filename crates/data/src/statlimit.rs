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
    /// 是否带参数位（这类属性的语义校验超出本模块范围）。
    pub has_param_bits: bool,
    /// 是否带编码/移位（`Encode` / `ValShift` 非 0）。
    ///
    /// 这类属性在存档里**不是直接存值**：例如 `item_hp_perlevel`（*ID 216）
    /// 的 `ValShift=8`，存档里的 16128 其实是 `63 << 8`。
    /// 所以它们不能按"单字段 + 偏移"判范围。
    pub has_encoding: bool,
}

impl StatLimit {
    /// 是否可以用"位宽 + 偏移"硬判范围。
    pub fn is_plain(&self) -> bool {
        !self.has_param_bits && !self.has_encoding
    }

    /// 可表示的最小值。
    pub fn min(&self) -> i64 {
        -(self.save_add as i64)
    }

    /// 可表示的最大值。
    pub fn max(&self) -> i64 {
        if self.save_bits >= 62 {
            return i64::MAX; // 位宽异常大时不做限制
        }
        (1i64 << self.save_bits) - 1 - self.save_add as i64
    }

    /// 值是否可表示。
    pub fn accepts(&self, value: i64) -> bool {
        value >= self.min() && value <= self.max()
    }

    /// 人类可读的范围。
    pub fn range_text(&self) -> String {
        format!("{}–{}", self.min(), self.max())
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
            let encoding = table.int(r, "Encode").unwrap_or(0) != 0
                || table.int(r, "ValShift").unwrap_or(0) != 0;
            by_id.insert(
                id as u16,
                StatLimit {
                    id: id as u16,
                    save_bits: bits as u8,
                    save_add: add,
                    has_param_bits: param,
                    has_encoding: encoding,
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

    /// 硬校验一个值：超范围就拒绝。
    ///
    /// **只对"纯单字段编码"的属性成立**。带 `Save Param Bits` 或 `Encode`/`ValShift`
    /// 的属性在存档里不是直接存值（例如 `item_hp_perlevel` 的 16128 是 `63 << 8`），
    /// 用单字段范围判会误伤，所以这类属性直接放行，改用 [`Self::advise`] 提示。
    pub fn check(&self, id: u16, value: i64) -> Result<(), String> {
        let Some(limit) = self.get(id) else {
            return Ok(()); // 表里没有的属性不限制（可能是 mod 自定义）
        };
        if !limit.is_plain() || limit.accepts(value) {
            return Ok(());
        }
        Err(format!(
            "属性 ID {id} 的值 {value} 超出位宽可表示范围 {}（Save Bits {} / Save Add {}），写进去会被静默截断",
            limit.range_text(),
            limit.save_bits,
            limit.save_add
        ))
    }

    /// 软提示：带参数/编码的属性，值看起来是"裸值"时提醒一句（**不阻止**）。
    pub fn advise(&self, id: u16, value: i64) -> Option<String> {
        let limit = self.get(id)?;
        if limit.is_plain() {
            return None;
        }
        if limit.accepts(value) && value.abs() < 256 {
            return Some(format!(
                "属性 ID {id} 带参数位或编码（Save Param Bits / Encode / ValShift）：\
                 值 {value} 看起来是裸值，这类属性在存档里通常要把参数或移位算进去（例如 值<<8）"
            ));
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limit_range_math() {
        // maxhp：Save Bits 9 / Save Add 32 → [-32, 479]
        let l = StatLimit {
            id: 7,
            save_bits: 9,
            save_add: 32,
            has_param_bits: false,
            has_encoding: false,
        };
        assert_eq!(l.min(), -32);
        assert_eq!(l.max(), 479);
        assert!(l.accepts(-32));
        assert!(l.accepts(479));
        assert!(!l.accepts(480));
        assert!(!l.accepts(1234), "1234 会被截断成 1024");
    }

    #[test]
    fn check_reports_truncation() {
        let mut limits = StatLimits::empty();
        limits.by_id.insert(
            7,
            StatLimit {
                id: 7,
                save_bits: 9,
                save_add: 32,
                has_param_bits: false,
                has_encoding: false,
            },
        );
        assert!(limits.check(7, 100).is_ok());
        let err = limits.check(7, 1234).unwrap_err();
        assert!(err.contains("超出位宽"), "{err}");
        // 表里没有的属性不限制
        assert!(limits.check(9999, i64::MAX).is_ok());
    }
}

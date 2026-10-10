//! 物品导入/导出（里程碑 5b）：把物品存成 JSON 蓝图，再导入存档。
//!
//! ## 设计取舍
//!
//! 存档里的物品没有"物品 ID 表"这种简单结构：属性带位宽编码、孔位里还能嵌套物品、
//! 耳语/无形/符文之语各自有独立字段。凭空构造一件物品风险很高，所以这里采用
//! **模板克隆**：从存档里已有的一件物品复制一份，再按蓝图改属性/耐久/数量。
//!
//! ```text
//! 导出：某件物品 → JSON（含它自己的属性，可当模板用）
//! 导入：JSON + 模板序号 → 克隆模板 → 覆盖属性/耐久/数量 → 追加到物品列表
//! ```
//!
//! 好处是"能导出就能导入"：你看到的任何一件装备都可以存成蓝图再复制出来。
//! 代价是必须指定同存档里的一个模板序号，不能凭空生成。

use serde::{Deserialize, Serialize};

use crate::edit::{EditError, EditSession};
use crate::items::{ItemDetail, ItemEdit};

/// 一件物品的 JSON 蓝图。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemBlueprint {
    /// 蓝图说明（可选，纯备注）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// 原始物品代码（仅备注用；实际形状以模板为准）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    /// 原始品质（仅备注用）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quality: Option<String>,
    /// 属性列表：`{ "stat": 属性ID, "value": 数值, "layer": 层 }`。
    /// 给了就**整体替换**物品自带属性。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stats: Option<Vec<BlueprintStat>>,
    /// 当前耐久（可选）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub durability: Option<i32>,
    /// 最大耐久（可选）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_durability: Option<i32>,
    /// 堆叠数量（可选）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quantity: Option<i32>,
    /// 防御值（可选）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub defense: Option<i32>,
    /// 目标位置：存储页名（`Inventory`/`Stash`/`Cube`/`Belt`，可选）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub store_page: Option<String>,
    /// 目标格子列（可选）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inv_x: Option<u8>,
    /// 目标格子行（可选）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inv_y: Option<u8>,
}

/// 蓝图里的一条属性。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlueprintStat {
    /// 属性 ID（`ItemStatCost` 语义 ID）。
    pub stat: u16,
    /// 数值。
    pub value: i64,
    /// 层（多数属性为 0）。
    #[serde(default)]
    pub layer: i32,
}

/// 导入结果。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportOutcome {
    /// 新物品在物品列表里的序号。
    pub index: usize,
    /// 使用的模板序号。
    pub template: usize,
    /// 覆盖的属性条数。
    pub stats_replaced: usize,
    /// 是否改了耐久。
    pub durability_set: bool,
    /// 是否改了数量。
    pub quantity_set: bool,
}

/// 蓝图解析失败。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlueprintError(pub String);

impl std::fmt::Display for BlueprintError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for BlueprintError {}

impl From<BlueprintError> for EditError {
    fn from(e: BlueprintError) -> Self {
        EditError::Invalid(format!("蓝图不合法：{e}"))
    }
}

/// 从 JSON 文本解析蓝图。
pub fn parse_blueprint(json: &str) -> Result<ItemBlueprint, BlueprintError> {
    serde_json::from_str(json).map_err(|e| BlueprintError(e.to_string()))
}

/// 把一件物品导出成蓝图。
pub fn export_blueprint(session: &EditSession, index: usize) -> Result<ItemBlueprint, EditError> {
    let d: ItemDetail = session.item_detail(index)?;
    Ok(ItemBlueprint {
        note: Some(format!("由 {} 导出（模板序号 {index}）", d.code)),
        code: Some(d.code),
        quality: Some(d.quality),
        stats: Some(
            d.stats
                .iter()
                .map(|(stat, value, layer)| BlueprintStat {
                    stat: *stat,
                    value: *value,
                    layer: *layer,
                })
                .collect(),
        ),
        durability: d.durability,
        max_durability: d.max_durability,
        quantity: d.quantity,
        defense: d.defense,
        store_page: Some(d.store_page),
        inv_x: Some(d.inv_x),
        inv_y: Some(d.inv_y),
    })
}

impl EditSession {
    /// 批量导出：把全部物品写成蓝图数组。
    pub fn export_blueprints(&self) -> Vec<ItemBlueprint> {
        (0..self.item_count())
            .filter_map(|i| self.export_one(i).ok())
            .collect()
    }

    /// 导出单件（内部用，避免借用冲突）。
    fn export_one(&self, index: usize) -> Result<ItemBlueprint, EditError> {
        export_blueprint(self, index)
    }

    /// 导入一件物品：克隆 `template` 指定的物品，再按蓝图覆盖。
    ///
    /// 顺序很重要：先克隆（追加到末尾），再在新物品上改属性/耐久/数量，
    /// 最后按蓝图移动格子。任何一步失败都会整批回滚。
    pub fn import_blueprint(
        &mut self,
        blueprint: &ItemBlueprint,
        template: usize,
    ) -> Result<ImportOutcome, EditError> {
        if template >= self.item_count() {
            return Err(EditError::Invalid(format!(
                "模板序号 {template} 越界（共 {} 件）",
                self.item_count()
            )));
        }

        let mut edits: Vec<ItemEdit> = Vec::new();
        // 克隆出来的新物品一定在末尾
        let new_index = self.item_count();
        edits.push(ItemEdit::CloneItem { template });

        let mut stats_replaced = 0usize;
        if let Some(stats) = &blueprint.stats {
            stats_replaced = stats.len();
            edits.push(ItemEdit::ReplaceStats {
                index: new_index,
                stats: stats.iter().map(|s| (s.stat, s.value, s.layer)).collect(),
            });
        }
        if let Some(v) = blueprint.durability {
            edits.push(ItemEdit::SetDurability {
                index: new_index,
                value: v,
            });
        }
        if let (Some(max), Some(cur)) = (blueprint.max_durability, blueprint.durability) {
            if cur > max {
                return Err(EditError::Invalid(format!(
                    "蓝图里当前耐久 {cur} 大于最大耐久 {max}"
                )));
            }
        }
        if let Some(v) = blueprint.quantity {
            edits.push(ItemEdit::SetQuantity {
                index: new_index,
                value: v,
            });
        }
        if let Some(v) = blueprint.defense {
            edits.push(ItemEdit::SetDefense {
                index: new_index,
                value: v,
            });
        }
        if let (Some(x), Some(y)) = (blueprint.inv_x, blueprint.inv_y) {
            edits.push(ItemEdit::MoveTo {
                index: new_index,
                col: x,
                row: y,
            });
        }

        self.apply_items(&edits)?;

        // 目标存储页（不在 ItemEdit 里，直接改）
        if let Some(page) = &blueprint.store_page {
            let raw = store_page_raw(page)
                .ok_or_else(|| EditError::Invalid(format!("未知的存储页：{page}")))?;
            if let Some(item) = self.file.save.items.get_mut(new_index) {
                item.position.store_page = d2s::StorePage::from_raw(raw);
            }
        }

        Ok(ImportOutcome {
            index: new_index,
            template,
            stats_replaced,
            durability_set: blueprint.durability.is_some(),
            quantity_set: blueprint.quantity.is_some(),
        })
    }

    /// 把某件物品的属性整体替换。
    pub fn replace_stats(
        &mut self,
        index: usize,
        stats: Vec<(u16, i64, i32)>,
    ) -> Result<(), EditError> {
        self.apply_item(ItemEdit::ReplaceStats { index, stats })
    }
}

/// 存储页名 → 原始值（与 `d2s::StorePage` 的变体一致）。
pub fn store_page_raw(name: &str) -> Option<u8> {
    Some(match name {
        "Inventory" => 0,
        "Equip" => 1,
        "Trade" => 2,
        "Cube" => 3,
        "Stash" => 4,
        "Belt" => 5,
        "Null" => 255,
        _ => return None,
    })
}

/// 原始值 → 存储页名。
pub fn store_page_name(raw: u8) -> String {
    match raw {
        0 => "Inventory".into(),
        1 => "Equip".into(),
        2 => "Trade".into(),
        3 => "Cube".into(),
        4 => "Stash".into(),
        5 => "Belt".into(),
        255 => "Null".into(),
        other => format!("Unknown({other})"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_page_names_roundtrip() {
        for name in [
            "Inventory",
            "Equip",
            "Trade",
            "Cube",
            "Stash",
            "Belt",
            "Null",
        ] {
            let raw = store_page_raw(name).expect(name);
            assert_eq!(store_page_name(raw), name);
        }
        assert!(store_page_raw("Nope").is_none());
    }

    #[test]
    fn blueprint_json_shapes() {
        // 最简蓝图：只给属性
        let json = r#"{"stats":[{"stat":7,"value":100}]}"#;
        let b: ItemBlueprint = serde_json::from_str(json).expect("解析最简蓝图");
        assert_eq!(b.stats.as_ref().unwrap().len(), 1);
        assert_eq!(b.stats.as_ref().unwrap()[0].layer, 0, "layer 默认为 0");
        assert!(b.durability.is_none());

        // 完整蓝图：驼峰字段 + 位置
        let json = r#"{
            "note": "test",
            "stats": [{"stat": 7, "value": 100, "layer": 0}],
            "durability": 5,
            "maxDurability": 10,
            "quantity": 3,
            "storePage": "Stash",
            "invX": 2,
            "invY": 4
        }"#;
        let b: ItemBlueprint = serde_json::from_str(json).expect("解析完整蓝图");
        assert_eq!(b.durability, Some(5));
        assert_eq!(b.quantity, Some(3));
        assert_eq!(b.store_page.as_deref(), Some("Stash"));
        assert_eq!((b.inv_x, b.inv_y), (Some(2), Some(4)));

        // 非法 JSON 应报错而不是 panic
        assert!(parse_blueprint("{").is_err());

        // 导出再导入：JSON 形态一致
        let b2: ItemBlueprint =
            serde_json::from_str(&serde_json::to_string(&b).unwrap()).expect("往返");
        assert_eq!(b, b2);
    }
}

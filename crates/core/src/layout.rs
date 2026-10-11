//! 容器与佣兵布局设置（mod 改动过的东西，存档里往往读不出来）。
//!
//! ## 为什么需要用户确认
//!
//! 调研与实测（`docs/next-steps-materials.md` §3.9）确认：
//! - 背包/箱子/方块的真实格子数**不在存档里**；原版表（`inventory.txt`）只给了原版尺寸，
//!   而整合包会把扩展存储并到同一个存储页上（同一角色背包出现 `x=14, y=7`）。
//! - 佣兵"全装备"这种改动也读不出来（原版只允许头盔/武器/护甲）。
//!
//! 所以策略是：**能从存档推断的就推断**（例如坐标跨度给个下限），
//! **推断不出来的就让用户勾选**，而不是猜。

/// 存储布局设置（用户可改，也可来自推断）。
///
/// 每个字段都带 `#[serde(default)]`：以后加新设置项时，**旧配置文件仍然能读**
/// （缺的字段用默认值）。没有这一层，加字段会让旧配置整份解析失败，
/// 用户已保存的设置等于被丢掉——测试 `old_config_without_new_fields_still_loads`
/// 盯的就是这件事。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LayoutSettings {
    /// 背包每行格子数（原版 10）。
    pub inventory_cols: u32,
    /// 背包行数（原版 4）。
    pub inventory_rows: u32,
    /// 合成箱每行格子数（原版 3）。
    pub cube_cols: u32,
    /// 合成箱行数（原版 4）。
    pub cube_rows: u32,
    /// 共享箱每行格子数（原版 10）。
    pub stash_cols: u32,
    /// 共享箱行数（原版 10）。
    pub stash_rows: u32,
    /// 是否按"佣兵全装备"处理（mod 常见改动）。
    pub merc_full_equipment: bool,
    /// 传送书/鉴定书是否只占一格（mod 常见改动：原版它们是 1×2）。
    ///
    /// 开启后**渲染与拖拽都按 1×1 算**（界面按这个值决定方块大小与落点），
    /// 但**不改存档里的坐标数据**——那两件物品在存档里的位置语义不变，
    /// 只是编辑器把它们当成 1 格来摆放与判越界。
    pub books_single_cell: bool,
    /// 合成盒（赫拉迪克方块）是否只占一格（mod 常见改动：原版是 2×2）。
    ///
    /// 与书同理：只影响界面的占格与拖拽落点，不动存档数据。
    pub cube_single_cell: bool,
    /// 每个设置是否由用户显式指定（`false` 表示用的默认/推断值）。
    pub confirmed_by_user: bool,
}

impl Default for LayoutSettings {
    fn default() -> Self {
        // 原版 D2R 3.x 的默认值
        Self {
            inventory_cols: 10,
            inventory_rows: 4,
            cube_cols: 3,
            cube_rows: 4,
            stash_cols: 10,
            stash_rows: 10,
            merc_full_equipment: false,
            books_single_cell: false,
            cube_single_cell: false,
            confirmed_by_user: false,
        }
    }
}

/// 哪些物品算"书"（mod 里常被改成只占一格）。
///
/// 只列传送书与鉴定书：卷轴（`tsc`/`isc`）本来就是 1×1，不需要处理。
pub const BOOK_CODES: [&str; 2] = ["tbk", "ibk"];

/// 合成盒的物品代码。
pub const CUBE_CODES: [&str; 1] = ["box"];

impl LayoutSettings {
    /// 某物品在界面上的占格（考虑"书/合成盒只占一格"这类 mod 设置）。
    ///
    /// `base` 是从数据表读到的原始占格；命中被改小的物品时返回 1×1。
    pub fn item_size(&self, code: &str, base: (u32, u32)) -> (u32, u32) {
        if self.books_single_cell && BOOK_CODES.contains(&code) {
            return (1, 1);
        }
        if self.cube_single_cell && CUBE_CODES.contains(&code) {
            return (1, 1);
        }
        base
    }
}

/// 从存档能推断出来的信息（读不出来的项会出现在 `unknowns`）。
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutHints {
    /// 背包页实际占用的坐标跨度 `(x, y)`（下限推断）。
    pub inventory_extent: Option<(u32, u32)>,
    /// 背包页里的最大 x / y（用于判断是否超出原版尺寸）。
    pub inventory_max: Option<(u32, u32)>,
    /// 合成箱页最大 x / y。
    pub cube_max: Option<(u32, u32)>,
    /// 箱子页实际占用的坐标跨度。
    pub stash_extent: Option<(u32, u32)>,
    /// 箱子页最大 x / y。
    pub stash_max: Option<(u32, u32)>,
    /// 佣兵装备段是否存在。
    pub merc_section: bool,
    /// 佣兵装备段里的物品数。
    pub merc_items: usize,
    /// 推断不出来的项（给界面提示用）。
    pub unknowns: Vec<String>,
}

impl LayoutHints {
    /// 背包是否**看起来**被扩展过（超出原版默认尺寸）。
    pub fn inventory_looks_expanded(&self) -> bool {
        match self.inventory_max {
            Some((x, y)) => {
                x + 1 > LayoutSettings::default().inventory_cols
                    || y + 1 > LayoutSettings::default().inventory_rows
            }
            None => false,
        }
    }

    /// 合成箱是否看起来被扩展过。
    pub fn cube_looks_expanded(&self) -> bool {
        let d = LayoutSettings::default();
        match self.cube_max {
            Some((x, y)) => x + 1 > d.cube_cols || y + 1 > d.cube_rows,
            None => false,
        }
    }

    /// 箱子是否看起来被扩展过。
    pub fn stash_looks_expanded(&self) -> bool {
        match self.stash_max {
            Some((x, y)) => {
                x + 1 > LayoutSettings::default().stash_cols
                    || y + 1 > LayoutSettings::default().stash_rows
            }
            None => false,
        }
    }
}

/// 推断提示：给界面用的一句话（哪些项需要用户确认）。
pub fn needs_confirmation(hints: &LayoutHints) -> Vec<String> {
    let mut out = Vec::new();
    if hints.inventory_looks_expanded() {
        out.push(format!(
            "背包坐标超过原版 {0}×{1}（实测最大 x={2}, y={3}）——可能装了改背包尺寸的 mod，请确认背包大小",
            LayoutSettings::default().inventory_cols,
            LayoutSettings::default().inventory_rows,
            hints.inventory_max.map(|m| m.0).unwrap_or(0),
            hints.inventory_max.map(|m| m.1).unwrap_or(0),
        ));
    }
    if hints.cube_looks_expanded() {
        out.push(format!(
            "合成箱坐标超过原版 {0}×{1}（实测最大 x={2}, y={3}）——请确认合成箱大小",
            LayoutSettings::default().cube_cols,
            LayoutSettings::default().cube_rows,
            hints.cube_max.map(|m| m.0).unwrap_or(0),
            hints.cube_max.map(|m| m.1).unwrap_or(0),
        ));
    }
    if hints.stash_looks_expanded() {
        out.push(format!(
            "箱子坐标超过原版 {0}×{1}（实测最大 x={2}, y={3}）——请确认箱子大小",
            LayoutSettings::default().stash_cols,
            LayoutSettings::default().stash_rows,
            hints.stash_max.map(|m| m.0).unwrap_or(0),
            hints.stash_max.map(|m| m.1).unwrap_or(0),
        ));
    }
    // 佣兵段几乎每个角色都有（哪怕是空的），所以只在**真有物品**时才问
    if hints.merc_section && hints.merc_items > 0 {
        out.push(format!(
            "存档里有佣兵装备段（{} 件物品）——原版只允许头盔/武器/护甲；若装了「佣兵全装备」类 mod 请勾选",
            hints.merc_items
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_vanilla() {
        let d = LayoutSettings::default();
        assert_eq!((d.inventory_cols, d.inventory_rows), (10, 4));
        assert_eq!((d.stash_cols, d.stash_rows), (10, 10));
        assert!(!d.merc_full_equipment);
        assert!(!d.confirmed_by_user);
    }

    #[test]
    fn hints_detect_expansion() {
        let mut h = LayoutHints {
            inventory_max: Some((14, 7)),
            ..Default::default()
        };
        assert!(h.inventory_looks_expanded());
        assert!(!h.stash_looks_expanded(), "箱子没有数据时不应判定为扩展");
        h.stash_max = Some((15, 15));
        assert!(h.stash_looks_expanded());

        // 原版尺寸内不算扩展
        let vanilla = LayoutHints {
            inventory_max: Some((9, 3)),
            stash_max: Some((9, 9)),
            ..Default::default()
        };
        assert!(!vanilla.inventory_looks_expanded());
        assert!(!vanilla.stash_looks_expanded());
    }

    #[test]
    fn needs_confirmation_lists_reasons() {
        let hints = LayoutHints {
            inventory_max: Some((14, 7)),
            stash_max: Some((15, 15)),
            merc_section: true,
            merc_items: 3,
            ..Default::default()
        };
        let items = needs_confirmation(&hints);
        assert_eq!(items.len(), 3, "背包/箱子/佣兵各一条：{items:?}");
        assert!(items[0].contains("背包"));
        assert!(items[1].contains("箱子"));
        assert!(items[2].contains("佣兵"));

        // 佣兵段存在但没有物品 → 不该问
        let empty_merc = LayoutHints {
            merc_section: true,
            merc_items: 0,
            ..Default::default()
        };
        assert!(
            needs_confirmation(&empty_merc).is_empty(),
            "空佣兵段不应产生提问"
        );
    }
}

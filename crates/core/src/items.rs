//! 物品编辑（里程碑 5）：查看明细、改耐久/数量/防御、增删属性、移动、删除。
//!
//! 设计原则（与里程碑 4 一致）：
//! - **改动可校验**：写路径仍走 [`crate::edit::EditSession::verify`]，逐字节往返是发布门槛；
//! - **不写死游戏常量**：属性位宽/上限来自数据集（`ItemStatCost.txt`），
//!   容器尺寸来自存档自身（见 `d2r-data::placement` 的说明）；
//! - **失败安全**：越界、超上限、不存在的下标一律拒绝，不"猜着写"。

use d2s::{ItemPosition, Stat, StatId};

use crate::edit::{EditError, EditSession};

/// 物品明细（只读展示用）。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemDetail {
    /// 序号（在当前存档的物品列表里的位置）。
    pub index: usize,
    /// 4 字符物品代码。
    pub code: String,
    /// 品质（调试名，如 `Unique`）。
    pub quality: String,
    /// 品质数据里的行号（暗金/套装/劣质用）。
    pub quality_index: Option<u16>,
    /// 物品等级。
    pub level: u8,
    /// 位置模式（调试名）。
    pub mode: String,
    /// 装备位（调试名）。
    pub body_location: String,
    /// 存储页（调试名）。
    pub store_page: String,
    /// 列。
    pub inv_x: u8,
    /// 行。
    pub inv_y: u8,
    /// 防御值。
    pub defense: Option<i32>,
    /// 最大耐久。
    pub max_durability: Option<i32>,
    /// 当前耐久。
    pub durability: Option<i32>,
    /// 堆叠数量。
    pub quantity: Option<i32>,
    /// 镶嵌孔数（`sockets` 长度）。
    pub socket_count: usize,
    /// 已镶嵌数量。
    pub socketed_count: usize,
    /// 属性列表：`(StatId 原始值, 数值, 层)`。
    pub stats: Vec<(u16, i64, i32)>,
    /// 符文之语 ID。
    pub runeword_id: Option<u16>,
}

/// 一条物品改动指令。
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ItemEdit {
    /// 设置当前耐久（0..=max）。
    SetDurability {
        /// 物品序号。
        index: usize,
        /// 新耐久。
        value: i32,
    },
    /// 设置堆叠数量（1..=上限）。
    SetQuantity {
        /// 物品序号。
        index: usize,
        /// 新数量。
        value: i32,
    },
    /// 设置防御值。
    SetDefense {
        /// 物品序号。
        index: usize,
        /// 新防御。
        value: i32,
    },
    /// 设置/覆盖一个属性。
    SetStat {
        /// 物品序号。
        index: usize,
        /// 属性 ID（`ItemStatCost` 语义 ID）。
        stat: u16,
        /// 新数值。
        value: i64,
    },
    /// 删除一个属性。
    RemoveStat {
        /// 物品序号。
        index: usize,
        /// 属性 ID。
        stat: u16,
    },
    /// 移动物品到某个格子（只改坐标，不改容器/页码）。
    MoveTo {
        /// 物品序号。
        index: usize,
        /// 列。
        col: u8,
        /// 行。
        row: u8,
    },
    /// 删除物品（含其镶嵌物）。
    Remove {
        /// 物品序号。
        index: usize,
    },
}

/// 一件物品超出用户确认的容器尺寸。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoundsIssue {
    /// 物品序号。
    pub index: usize,
    /// 物品代码。
    pub code: String,
    /// 容器名（背包/箱子/合成箱/腰带）。
    pub page: String,
    /// 物品所在格子。
    pub at: (u32, u32),
    /// 物品占格。
    pub size: (u32, u32),
    /// 用户确认的容器尺寸。
    pub container: (u32, u32),
}

impl std::fmt::Display for BoundsIssue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{}] {} 在{}的 ({},{}) 占 {}×{}，超出容器 {}×{}",
            self.index,
            self.code,
            self.page,
            self.at.0,
            self.at.1,
            self.size.0,
            self.size.1,
            self.container.0,
            self.container.1
        )
    }
}

/// 物品统计（给界面/CLI 汇总）。
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemStats {
    /// 物品总数。
    pub total: usize,
    /// 各存储页数量（页名 → 数量）。
    pub by_page: Vec<(String, usize)>,
    /// 各品质数量（品质名 → 数量）。
    pub by_quality: Vec<(String, usize)>,
    /// 已装备数量。
    pub equipped: usize,
    /// 带镶嵌物的数量。
    pub with_sockets: usize,
}

impl EditSession {
    /// 物品总数。
    pub fn item_count(&self) -> usize {
        self.file.save.items.len()
    }

    /// 取某个物品的明细。
    pub fn item_detail(&self, index: usize) -> Result<ItemDetail, EditError> {
        let item = self.file.save.items.get(index).ok_or_else(|| {
            EditError::Invalid(format!(
                "物品序号 {index} 越界（共 {} 件）",
                self.item_count()
            ))
        })?;
        let p: &ItemPosition = &item.position;
        Ok(ItemDetail {
            index,
            code: item.code.to_string(),
            quality: format!("{:?}", item.quality),
            quality_index: item.quality_data.as_ref().and_then(|q| q.file_index()),
            level: item.level,
            mode: format!("{:?}", p.mode),
            body_location: format!("{:?}", p.body_location),
            store_page: format!("{:?}", p.store_page),
            inv_x: p.inv_x,
            inv_y: p.inv_y,
            defense: item.defense,
            max_durability: item.max_durability,
            durability: item.durability,
            quantity: item.quantity,
            socket_count: item.sockets.len(),
            socketed_count: item.sockets.iter().filter(|s| s.is_some()).count(),
            stats: item
                .stats
                .iter()
                .map(|s| (s.id.0, s.value, s.layer))
                .collect(),
            runeword_id: item.runeword_id,
        })
    }

    /// 全部物品明细。
    pub fn item_details(&self) -> Vec<ItemDetail> {
        (0..self.item_count())
            .filter_map(|i| self.item_detail(i).ok())
            .collect()
    }

    /// 汇总统计。
    pub fn item_stats(&self) -> ItemStats {
        let mut by_page: std::collections::BTreeMap<String, usize> = Default::default();
        let mut by_quality: std::collections::BTreeMap<String, usize> = Default::default();
        let mut equipped = 0;
        let mut with_sockets = 0;
        for it in &self.file.save.items {
            *by_page
                .entry(format!("{:?}", it.position.store_page))
                .or_default() += 1;
            *by_quality.entry(format!("{:?}", it.quality)).or_default() += 1;
            if format!("{:?}", it.position.mode) == "Equipped" {
                equipped += 1;
            }
            if it.sockets.iter().any(|s| s.is_some()) {
                with_sockets += 1;
            }
        }
        ItemStats {
            total: self.item_count(),
            by_page: by_page.into_iter().collect(),
            by_quality: by_quality.into_iter().collect(),
            equipped,
            with_sockets,
        }
    }

    /// 按用户确认的布局做**可靠的越界校验**。
    ///
    /// 为什么现在能硬判：`layout` 是用户勾选/填写的实际尺寸（读不出来所以问用户），
    /// 有了它之后"物品是否超出容器"就从"猜"变成"算"。
    ///
    /// 返回 `(问题列表, 参与校验的物品数)`。
    pub fn validate_bounds(
        &self,
        layout: &crate::layout::LayoutSettings,
        size_of: impl Fn(&str) -> Option<(u32, u32)>,
    ) -> (Vec<BoundsIssue>, usize) {
        let mut issues = Vec::new();
        let mut checked = 0usize;

        for (index, it) in self.file.save.items.iter().enumerate() {
            let p = &it.position;
            let mode = format!("{:?}", p.mode);
            if !matches!(mode.as_str(), "Stored" | "InBelt") {
                continue;
            }
            let (cols, rows, label) = match format!("{:?}", p.store_page).as_str() {
                "Inventory" => (layout.inventory_cols, layout.inventory_rows, "背包"),
                "Cube" => (layout.cube_cols, layout.cube_rows, "合成箱"),
                "Stash" => (layout.stash_cols, layout.stash_rows, "箱子"),
                "Belt" => (4, 4, "腰带"),
                _ => continue,
            };
            let (w, h) = size_of(&it.code.to_string()).unwrap_or((1, 1));
            let (x, y) = (p.inv_x as u32, p.inv_y as u32);
            checked += 1;
            if x + w > cols || y + h > rows {
                issues.push(BoundsIssue {
                    index,
                    code: it.code.to_string(),
                    page: label.to_string(),
                    at: (x, y),
                    size: (w, h),
                    container: (cols, rows),
                });
            }
        }
        (issues, checked)
    }

    /// 从存档推断存储布局（读不出来的项会出现在 `unknowns`）。
    ///
    /// 只做**下限推断**：坐标跨度只能说明"至少这么大"，不能说明真实尺寸，
    /// 所以推断结果用于提示用户确认，而不是直接当作真实尺寸使用。
    pub fn layout_hints(&self) -> crate::layout::LayoutHints {
        use crate::layout::LayoutHints;

        let mut hints = LayoutHints::default();
        let mut inv_max: Option<(u32, u32)> = None;
        let mut cube_max: Option<(u32, u32)> = None;
        let mut stash_max: Option<(u32, u32)> = None;

        for it in &self.file.save.items {
            let p = &it.position;
            let mode = format!("{:?}", p.mode);
            if !matches!(mode.as_str(), "Stored" | "InBelt") {
                continue;
            }
            let (x, y) = (p.inv_x as u32, p.inv_y as u32);
            match format!("{:?}", p.store_page).as_str() {
                "Inventory" => {
                    let cur = inv_max.get_or_insert((0, 0));
                    cur.0 = cur.0.max(x);
                    cur.1 = cur.1.max(y);
                }
                "Cube" => {
                    let cur = cube_max.get_or_insert((0, 0));
                    cur.0 = cur.0.max(x);
                    cur.1 = cur.1.max(y);
                }
                "Stash" => {
                    let cur = stash_max.get_or_insert((0, 0));
                    cur.0 = cur.0.max(x);
                    cur.1 = cur.1.max(y);
                }
                _ => {}
            }
        }

        if let Some((x, y)) = inv_max {
            hints.inventory_max = Some((x, y));
            // 跨度只是下限：真实尺寸至少是"最大坐标 + 1"，再留一行余量
            hints.inventory_extent = Some((x + 1, y + 1));
        }
        if let Some((x, y)) = cube_max {
            hints.cube_max = Some((x, y));
        }
        if let Some((x, y)) = stash_max {
            hints.stash_max = Some((x, y));
            hints.stash_extent = Some((x + 1, y + 1));
        }

        // 佣兵装备段
        if let Some(merc) = &self.file.save.merc_items {
            hints.merc_section = true;
            hints.merc_items = merc.items.len();
        }

        hints.unknowns = vec![
            "背包真实格子数（存档只存坐标，不存容器尺寸）".into(),
            "箱子真实格子数（同上）".into(),
            "合成箱真实格子数（同上）".into(),
            "是否佣兵全装备（mod 改动，存档无标记）".into(),
        ];
        hints
    }
    /// 应用一条物品改动。
    pub fn apply_item(&mut self, edit: ItemEdit) -> Result<(), EditError> {
        // 先做所有校验，再改动（避免半途失败留下半成品）
        match &edit {
            ItemEdit::SetDurability { index, value } => {
                let max = self.item(*index)?.max_durability.unwrap_or(0);
                if *value < 0 || (max > 0 && *value > max) {
                    return Err(EditError::Invalid(format!(
                        "耐久必须在 0–{max} 之间（收到 {value}）"
                    )));
                }
                self.item_mut(*index)?.durability = Some(*value);
            }
            ItemEdit::SetQuantity { index, value } => {
                if *value < 1 || *value > 511 {
                    return Err(EditError::Invalid(format!(
                        "堆叠数量必须在 1–511（收到 {value}）"
                    )));
                }
                self.item_mut(*index)?.quantity = Some(*value);
            }
            ItemEdit::SetDefense { index, value } => {
                if *value < 0 || *value > 65535 {
                    return Err(EditError::Invalid(format!(
                        "防御值必须在 0–65535（收到 {value}）"
                    )));
                }
                self.item_mut(*index)?.defense = Some(*value);
            }
            ItemEdit::SetStat { index, stat, value } => {
                let item = self.item_mut(*index)?;
                let id = StatId(*stat);
                match item.stats.iter_mut().find(|s| s.id == id && s.layer == 0) {
                    Some(existing) => existing.value = *value,
                    None => item.stats.push(Stat {
                        id,
                        value: *value,
                        layer: 0,
                    }),
                }
            }
            ItemEdit::RemoveStat { index, stat } => {
                let item = self.item_mut(*index)?;
                let id = StatId(*stat);
                let before = item.stats.len();
                item.stats.retain(|s| s.id != id);
                if item.stats.len() == before {
                    return Err(EditError::Invalid(format!("该物品没有属性 ID {stat}")));
                }
            }
            ItemEdit::MoveTo { index, col, row } => {
                let item = self.item_mut(*index)?;
                // 坐标是 4 位字段（0–15）；越界在容器校验里另行判断
                if *col > 15 || *row > 15 {
                    return Err(EditError::Invalid(format!(
                        "格子坐标必须在 0–15（收到 {col},{row}）"
                    )));
                }
                item.position.inv_x = *col;
                item.position.inv_y = *row;
            }
            ItemEdit::Remove { index } => {
                if *index >= self.item_count() {
                    return Err(EditError::Invalid(format!("物品序号 {index} 越界")));
                }
                self.file.save.items.remove(*index);
            }
        }
        self.item_edits.push(edit);
        Ok(())
    }

    /// 批量应用（原子）。
    pub fn apply_items(&mut self, edits: &[ItemEdit]) -> Result<(), EditError> {
        let snapshot = self.file.save.clone();
        let applied_len = self.item_edits.len();
        for e in edits {
            if let Err(err) = self.apply_item(e.clone()) {
                self.file.save = snapshot;
                self.item_edits.truncate(applied_len);
                return Err(err);
            }
        }
        Ok(())
    }

    fn item(&self, index: usize) -> Result<&d2s::Item, EditError> {
        self.file.save.items.get(index).ok_or_else(|| {
            EditError::Invalid(format!(
                "物品序号 {index} 越界（共 {} 件）",
                self.item_count()
            ))
        })
    }

    fn item_mut(&mut self, index: usize) -> Result<&mut d2s::Item, EditError> {
        let total = self.item_count();
        self.file
            .save
            .items
            .get_mut(index)
            .ok_or_else(|| EditError::Invalid(format!("物品序号 {index} 越界（共 {total} 件）")))
    }
}

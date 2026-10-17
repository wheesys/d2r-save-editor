//! 物品编辑（里程碑 5）：查看明细、改耐久/数量/防御/孔数、增删属性、移动、删除、克隆、导入。
//!
//! 设计原则（与里程碑 4 一致）：
//! - **改动可校验**：写路径仍走 [`crate::edit::EditSession::verify`]，逐字节往返是发布门槛；
//! - **不写死游戏常量**：属性位宽/步长来自数据表，容器尺寸由用户确认（见 [`crate::layout`]）；
//! - **失败安全**：越界、超上限、不存在的下标一律拒绝，不"猜着写"。

use d2s::{ItemMode, ItemPosition, Stat, StatId};

use crate::edit::{EditError, EditSession};

/// 存档里孔数字段是 3 位，所以一件物品最多 7 孔（游戏内核也是这个上限）。
pub const MAX_SOCKETS: usize = 7;

/// 储物页名（与 `d2s::StorePage` 的变体一一对应）。
///
/// 用枚举而不是裸字符串：界面/CLI 与存档之间的映射只在这一处，
/// 拼错名字会被 `parse` 立刻拒绝，而不是静默落到 `Unknown`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum StorePageName {
    /// 背包页。
    Inventory,
    /// 装备页（穿戴中的物品）。
    Equip,
    /// 交易页。
    Trade,
    /// 赫拉迪姆方块。
    Cube,
    /// 储物箱。
    Stash,
    /// 腰带。
    Belt,
    /// 无（未放置）。
    Null,
}

impl StorePageName {
    /// 对应的原始值。
    pub fn raw(self) -> u8 {
        match self {
            StorePageName::Inventory => 0,
            StorePageName::Equip => 1,
            StorePageName::Trade => 2,
            StorePageName::Cube => 3,
            StorePageName::Stash => 4,
            StorePageName::Belt => 5,
            StorePageName::Null => 255,
        }
    }

    /// 从原始值还原（未知值返回 `None`）。
    pub fn from_raw(raw: u8) -> Option<Self> {
        Some(match raw {
            0 => StorePageName::Inventory,
            1 => StorePageName::Equip,
            2 => StorePageName::Trade,
            3 => StorePageName::Cube,
            4 => StorePageName::Stash,
            5 => StorePageName::Belt,
            255 => StorePageName::Null,
            _ => return None,
        })
    }

    /// 名称（存档里的调试名，与本枚举变体同名）。
    pub fn as_str(self) -> &'static str {
        match self {
            StorePageName::Inventory => "Inventory",
            StorePageName::Equip => "Equip",
            StorePageName::Trade => "Trade",
            StorePageName::Cube => "Cube",
            StorePageName::Stash => "Stash",
            StorePageName::Belt => "Belt",
            StorePageName::Null => "Null",
        }
    }

    /// 解析名称（大小写不敏感）。
    pub fn parse(name: &str) -> Option<Self> {
        Some(match name.to_ascii_lowercase().as_str() {
            "inventory" => StorePageName::Inventory,
            "equip" => StorePageName::Equip,
            "trade" => StorePageName::Trade,
            "cube" => StorePageName::Cube,
            "stash" => StorePageName::Stash,
            "belt" => StorePageName::Belt,
            "null" => StorePageName::Null,
            _ => return None,
        })
    }
}

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
    /// 是否带 `SOCKETED` 标志（孔数字段只有在带这个标志时才写入）。
    pub socketed_flag: bool,
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
    /// 设置堆叠数量（1..=511）。
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
    /// 设置孔数（0–7）。
    ///
    /// 孔数在存档里就是"`sockets` 的长度"，写进 3 位字段（因此上限 7），
    /// 且只在物品带 `SOCKETED` 标志时写入；孔数为 0 时同时清掉该标志。
    /// 已镶嵌的宝石/符文不会被静默丢弃：孔数小于已镶数量时**直接拒绝**。
    SetSockets {
        /// 物品序号。
        index: usize,
        /// 新孔数。
        count: usize,
    },
    /// 移动到另一个容器/格子（会同时处理存储页与放置模式）。
    Relocate {
        /// 物品序号。
        index: usize,
        /// 目标存储页。
        page: StorePageName,
        /// 目标列。
        col: u8,
        /// 目标行。
        row: u8,
    },
    /// 装备到某个装备位（`None`/`"none"` 表示卸下，回到背包）。
    SetEquipSlot {
        /// 物品序号。
        index: usize,
        /// 装备位名（`BodyLocation` 的变体名，如 `Head`）。
        slot: String,
    },
    /// 设置/覆盖一个属性。
    SetStat {
        /// 物品序号。
        index: usize,
        /// 属性 ID（`ItemStatCost` 语义 ID）。
        stat: u16,
        /// 新数值。
        value: i64,
        /// 层（参数位）。带参数位的属性同一 ID 会有多条记录，靠它区分：
        /// `item_addclassskills` 的层是职业行号、`item_aura` 的层是光环 ID。
        ///
        /// 不给时按 0 处理（普通属性都是 0）。
        #[serde(default)]
        layer: Option<i32>,
    },
    /// 删除一个属性。
    RemoveStat {
        /// 物品序号。
        index: usize,
        /// 属性 ID。
        stat: u16,
        /// 层（参数位），含义同 [`Self::SetStat::layer`]。
        ///
        /// 不给时删掉该 ID 的**全部**记录（保持旧的调用行为）。
        #[serde(default)]
        layer: Option<i32>,
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
    /// 克隆一件物品并追加到列表末尾（物品导入用：模板克隆）。
    CloneItem {
        /// 模板物品序号。
        template: usize,
    },
    /// 从零创建一件物品并追加到列表末尾（装备库导入用）。
    ///
    /// 与 [`Self::CloneItem`] 的区别：克隆要求存档里**已经有**同基型的物品
    /// 当模板，而装备库要能导入存档里没有的物品，所以必须能从基型代码直接构造。
    CreateItem {
        /// 基型代码（如 `uap`、`rin`）。
        code: String,
        /// 品质：`暗金` / `套装` / `普通`。
        quality: String,
        /// 表行索引（暗金/套装的 `*ID`），普通物品给 `None`。
        #[serde(default)]
        file_index: Option<u16>,
        /// 属性：(属性 ID, 数值, 层)。会**整体替换**新物品的属性。
        #[serde(default)]
        stats: Vec<(u16, i64, i32)>,
        /// 目标存储页名（`Inventory`/`Stash`/`Cube`/`Belt`）。
        #[serde(default = "default_store_page")]
        store_page: String,
        /// 目标列。
        #[serde(default)]
        col: u8,
        /// 目标行。
        #[serde(default)]
        row: u8,
        /// 物品等级（影响部分公式）。
        #[serde(default = "default_item_level")]
        level: u8,
    },
    /// 整体替换物品的属性列表（物品导入用）。
    ReplaceStats {
        /// 物品序号。
        index: usize,
        /// 新属性：(属性 ID, 数值, 层)。
        stats: Vec<(u16, i64, i32)>,
    },
}

/// 佣兵装备段里的一件物品（与主物品列表分开存储）。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MercenaryItem {
    /// 在佣兵段里的序号。
    pub index: usize,
    /// 物品代码。
    pub code: String,
    /// 品质（调试名）。
    pub quality: String,
    /// 物品等级。
    pub level: u8,
    /// 装备位（调试名）。
    pub body_location: String,
    /// 属性条数。
    pub stats_count: usize,
    /// 孔数。
    pub socket_count: usize,
    /// 已镶嵌数。
    pub socketed_count: usize,
    /// 当前耐久。
    pub durability: Option<i32>,
    /// 最大耐久。
    pub max_durability: Option<i32>,
    /// 堆叠数量。
    pub quantity: Option<i32>,
    /// 属性列表。
    pub stats: Vec<(u16, i64, i32)>,
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

impl EditSession {
    /// 物品总数。
    pub fn item_count(&self) -> usize {
        self.file.save.items.len()
    }

    /// 取某个物品的明细（主物品列表）。
    pub fn item_detail(&self, index: usize) -> Result<ItemDetail, EditError> {
        self.item_detail_on(ItemTarget::Inventory, index)
    }

    /// 取某件物品的明细（可指定列表）。
    ///
    /// 佣兵段与主列表序号体系不同，所以界面按佣兵序号取明细时必须带目标。
    pub fn item_detail_on(
        &self,
        target: ItemTarget,
        index: usize,
    ) -> Result<ItemDetail, EditError> {
        let item = self.item_on(target, index)?;
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
            socketed_flag: item.flags.contains(d2s::ItemFlags::SOCKETED),
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

    /// 佣兵装备段里的物品。
    ///
    /// 存档的佣兵段与主物品列表是**分开存的**，所以这里的 `index` 是它在佣兵段里的位置。
    pub fn mercenary_items(&self) -> Vec<MercenaryItem> {
        let Some(merc) = &self.file.save.merc_items else {
            return Vec::new();
        };
        merc.items
            .iter()
            .enumerate()
            .map(|(index, it)| MercenaryItem {
                index,
                code: it.code.to_string(),
                quality: format!("{:?}", it.quality),
                level: it.level,
                body_location: format!("{:?}", it.position.body_location),
                stats_count: it.stats.len(),
                socket_count: it.sockets.len(),
                socketed_count: it.sockets.iter().filter(|s| s.is_some()).count(),
                durability: it.durability,
                max_durability: it.max_durability,
                quantity: it.quantity,
                stats: it
                    .stats
                    .iter()
                    .map(|s| (s.id.0, s.value, s.layer))
                    .collect(),
            })
            .collect()
    }

    /// 佣兵物品的完整明细（序号是佣兵段内的序号）。
    ///
    /// 界面按佣兵序号打开属性弹窗时用它——**不能**拿主列表的 `item_details()`
    /// 里的序号去查佣兵段，两边序号体系不同。
    pub fn mercenary_item_details(&self) -> Vec<ItemDetail> {
        let total = self.mercenary_items().len();
        (0..total)
            .filter_map(|i| self.item_detail_on(ItemTarget::Mercenary, i).ok())
            .collect()
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
            // 跨度只是下限：真实尺寸至少是"最大坐标 + 1"
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

    /// 应用一条物品改动（作用于主物品列表）。
    ///
    /// 佣兵段要用 [`Self::apply_item_on`]：两处序号体系不同，必须显式给目标。
    pub fn apply_item(&mut self, edit: ItemEdit) -> Result<(), EditError> {
        self.apply_item_on(ItemTarget::Inventory, edit)
    }

    /// 对**指定列表**施加一条物品指令。
    ///
    /// 记录改动时会带上目标，撤销/重放才不会串到另一个列表。
    pub fn apply_item_on(&mut self, target: ItemTarget, edit: ItemEdit) -> Result<(), EditError> {
        // 先做所有校验，再改动（避免半途失败留下半成品）
        match &edit {
            ItemEdit::SetDurability { index, value } => {
                let max = self.item_on(target, *index)?.max_durability.unwrap_or(0);
                if *value < 0 || (max > 0 && *value > max) {
                    return Err(EditError::Invalid(format!(
                        "耐久必须在 0–{max} 之间（收到 {value}）"
                    )));
                }
                self.item_mut_on(target, *index)?.durability = Some(*value);
            }
            ItemEdit::SetQuantity { index, value } => {
                if *value < 1 || *value > 511 {
                    return Err(EditError::Invalid(format!(
                        "堆叠数量必须在 1–511（收到 {value}）"
                    )));
                }
                self.item_mut_on(target, *index)?.quantity = Some(*value);
            }
            ItemEdit::SetDefense { index, value } => {
                if *value < 0 || *value > 65535 {
                    return Err(EditError::Invalid(format!(
                        "防御值必须在 0–65535（收到 {value}）"
                    )));
                }
                self.item_mut_on(target, *index)?.defense = Some(*value);
            }
            ItemEdit::SetSockets { index, count } => {
                if *count > MAX_SOCKETS {
                    return Err(EditError::Invalid(format!(
                        "孔数最多 {MAX_SOCKETS}（存档里是 3 位字段），收到 {count}"
                    )));
                }
                let item = self.item_mut_on(target, *index)?;
                let filled = item.sockets.iter().filter(|s| s.is_some()).count();
                if *count < filled {
                    return Err(EditError::Invalid(format!(
                        "这件物品已经镶了 {filled} 个，孔数不能减到 {count}（先把镶嵌物取出来）"
                    )));
                }
                // 缩：只裁掉空的孔；扩：补 None
                item.sockets.truncate(*count);
                while item.sockets.len() < *count {
                    item.sockets.push(None);
                }
                // 孔数为 0 时清掉标志，否则孔数字段不会被写入
                if *count == 0 {
                    item.flags.remove(d2s::ItemFlags::SOCKETED);
                } else {
                    item.flags.insert(d2s::ItemFlags::SOCKETED);
                }
            }
            ItemEdit::Relocate {
                index,
                page,
                col,
                row,
            } => {
                // 坐标是 4 位字段；容器行列数由用户设置决定，界面会先把越界挡掉
                if *col > 15 || *row > 15 {
                    return Err(EditError::Invalid(format!(
                        "格子坐标必须在 0–15（收到 {col},{row}）"
                    )));
                }
                // 注意：这里的局部名不要叫 target——那个名字是"物品列表"参数
                let store_page = d2s::StorePage::from_raw(page.raw());
                let item = self.item_mut_on(target, *index)?;
                item.position.store_page = store_page;
                item.position.inv_x = *col;
                item.position.inv_y = *row;
                // 放置模式跟着目标页走：装备页以外的都算"存放"
                item.position.mode = match page {
                    StorePageName::Equip => ItemMode::Equipped,
                    StorePageName::Belt => ItemMode::InBelt,
                    _ => ItemMode::Stored,
                };
            }
            ItemEdit::SetEquipSlot { index, slot } => {
                let item = self.item_mut_on(target, *index)?;
                if slot.eq_ignore_ascii_case("none") {
                    // 卸下：回到背包、模式改回"存放"
                    item.position.body_location = d2s::BodyLocation::from_raw(0);
                    item.position.store_page =
                        d2s::StorePage::from_raw(StorePageName::Inventory.raw());
                    item.position.mode = ItemMode::Stored;
                } else {
                    let Some(loc) = body_location_from_name(slot) else {
                        return Err(EditError::Invalid(format!("未知的装备位：{slot}")));
                    };
                    item.position.body_location = loc;
                    item.position.store_page = d2s::StorePage::from_raw(StorePageName::Equip.raw());
                    item.position.mode = ItemMode::Equipped;
                }
            }
            ItemEdit::SetStat {
                index,
                stat,
                value,
                layer,
            } => {
                let item = self.item_mut_on(target, *index)?;
                set_stat_on(&mut item.stats, StatId(*stat), *value, *layer)?;
            }
            ItemEdit::RemoveStat { index, stat, layer } => {
                let item = self.item_mut_on(target, *index)?;
                let id = StatId(*stat);
                let before = item.stats.len();
                // 指定了层就只删那一条；没指定则删该 ID 的全部（保持旧行为）
                match layer {
                    Some(wanted) => item.stats.retain(|s| !(s.id == id && s.layer == *wanted)),
                    None => item.stats.retain(|s| s.id != id),
                }
                if item.stats.len() == before {
                    return Err(EditError::Invalid(format!("该物品没有属性 ID {stat}")));
                }
            }
            ItemEdit::MoveTo { index, col, row } => {
                if *col > 15 || *row > 15 {
                    return Err(EditError::Invalid(format!(
                        "格子坐标必须在 0–15（收到 {col},{row}）"
                    )));
                }
                let item = self.item_mut_on(target, *index)?;
                item.position.inv_x = *col;
                item.position.inv_y = *row;
            }
            ItemEdit::Remove { index } => {
                if *index >= self.item_count() {
                    return Err(EditError::Invalid(format!("物品序号 {index} 越界")));
                }
                self.file.save.items.remove(*index);
            }
            ItemEdit::CloneItem { template } => {
                let src = self
                    .file
                    .save
                    .items
                    .get(*template)
                    .ok_or_else(|| {
                        EditError::Invalid(format!(
                            "模板序号 {template} 越界（共 {} 件）",
                            self.item_count()
                        ))
                    })?
                    .clone();
                self.file.save.items.push(src);
            }
            ItemEdit::CreateItem {
                code,
                quality,
                file_index,
                stats,
                store_page,
                col,
                row,
                level,
            } => {
                let raw_page = crate::blueprint::store_page_raw(store_page)
                    .ok_or_else(|| EditError::Invalid(format!("未知的存储页：{store_page}")))?;
                // 物品的 `version` 必须与存档版本对齐：写盘时是按**物品自身的
                // version** 决定位宽编码的，留成默认的 0 会写出别的格式，
                // 读回来直接报错（不是"能打开但显示异常"，是打不开）。
                let version = self.file.header.version as u16;
                let mut item = build_item(code, quality, *file_index, *level, version)?;
                item.position.store_page = d2s::StorePage::from_raw(raw_page);
                item.position.inv_x = *col;
                item.position.inv_y = *row;
                item.position.mode = match crate::blueprint::store_page_name(raw_page).as_str() {
                    "Belt" => ItemMode::InBelt,
                    "Equip" => ItemMode::Equipped,
                    _ => ItemMode::Stored,
                };
                item.stats = stats
                    .iter()
                    .map(|(id, value, layer)| Stat {
                        id: StatId(*id),
                        value: *value,
                        layer: *layer,
                    })
                    .collect();
                self.file.save.items.push(item);
            }
            ItemEdit::ReplaceStats { index, stats } => {
                let list: Vec<Stat> = stats
                    .iter()
                    .map(|(id, value, layer)| Stat {
                        id: StatId(*id),
                        value: *value,
                        layer: *layer,
                    })
                    .collect();
                self.item_mut_on(target, *index)?.stats = list;
            }
        }
        self.item_edits.push((target, edit));
        Ok(())
    }

    /// 批量应用（原子，作用于主物品列表）。
    pub fn apply_items(&mut self, edits: &[ItemEdit]) -> Result<(), EditError> {
        self.apply_items_on(ItemTarget::Inventory, edits)
    }

    /// 对**指定列表**批量应用（原子）。
    pub fn apply_items_on(
        &mut self,
        target: ItemTarget,
        edits: &[ItemEdit],
    ) -> Result<(), EditError> {
        let snapshot = self.file.save.clone();
        let applied_len = self.item_edits.len();
        for e in edits {
            if let Err(err) = self.apply_item_on(target, e.clone()) {
                self.file.save = snapshot;
                self.item_edits.truncate(applied_len);
                return Err(err);
            }
        }
        Ok(())
    }

    /// 撤销全部挂起的物品修改，回到打开时的状态。
    ///
    /// 用途：应用层做"写入后读回校验"发现值被截断时回滚。
    pub fn revert_item_edits(&mut self) {
        if self.item_edits.is_empty() {
            return;
        }
        // 重新从原始字节加载，最干净
        let original = self.original.clone();
        let kind = self.file.kind;
        if let Ok(file) = crate::SaveFile::from_bytes(kind, original) {
            let path = self.path.clone();
            self.file = file;
            self.file.path = path;
        }
        self.item_edits.clear();
    }

    /// 取某个列表里的物品（只读）。
    fn item_on(&self, target: ItemTarget, index: usize) -> Result<&d2s::Item, EditError> {
        let list = self.item_list(target)?;
        let total = list.len();
        list.get(index).ok_or_else(|| {
            EditError::Invalid(format!(
                "{}里的物品序号 {index} 越界（共 {total} 件）",
                target.label()
            ))
        })
    }

    /// 取某个列表里的物品（可变）。
    fn item_mut_on(
        &mut self,
        target: ItemTarget,
        index: usize,
    ) -> Result<&mut d2s::Item, EditError> {
        let total = self.item_list(target)?.len();
        self.item_list_mut(target)?.get_mut(index).ok_or_else(|| {
            EditError::Invalid(format!(
                "{}里的物品序号 {index} 越界（共 {total} 件）",
                target.label()
            ))
        })
    }

    /// 某个目标对应的物品列表（只读）。
    ///
    /// `ItemsSection` 是 `Vec<Item>` 的新类型，所以主列表与佣兵列表统一成切片。
    fn item_list(&self, target: ItemTarget) -> Result<&[d2s::Item], EditError> {
        match target {
            ItemTarget::Inventory => Ok(&self.file.save.items.0),
            ItemTarget::Mercenary => match &self.file.save.merc_items {
                Some(m) => Ok(&m.items.0),
                None => Err(EditError::Invalid("这件存档没有佣兵段".into())),
            },
        }
    }

    /// 某个目标对应的物品列表（可变）。
    fn item_list_mut(&mut self, target: ItemTarget) -> Result<&mut Vec<d2s::Item>, EditError> {
        match target {
            ItemTarget::Inventory => Ok(&mut self.file.save.items.0),
            ItemTarget::Mercenary => match &mut self.file.save.merc_items {
                Some(m) => Ok(&mut m.items.0),
                None => Err(EditError::Invalid("这件存档没有佣兵段".into())),
            },
        }
    }
}

/// 物品指令作用的列表。
///
/// 存档里物品分两处存：主背包/箱子列表与**佣兵段**。两处的 `Item` 结构一样，
/// 但**序号体系互不相干**（佣兵段的 #0 不是主列表的 #0），所以指令必须带上目标，
/// 否则会改错物品。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ItemTarget {
    /// 主物品列表（背包/箱子/方块/腰带/身上装备）。
    #[default]
    Inventory,
    /// 佣兵装备段（`save.merc_items`）。
    Mercenary,
}

impl ItemTarget {
    /// 中文名（给错误信息与界面用）。
    pub fn label(self) -> &'static str {
        match self {
            Self::Inventory => "物品栏",
            Self::Mercenary => "佣兵",
        }
    }
}

/// 装备位名 → `BodyLocation`（大小写不敏感）。
///
/// `BodyLocation` 是开放枚举，没有内置的按名字查找，所以按原始值逐个比对变体的调试名。
pub fn body_location_from_name(name: &str) -> Option<d2s::BodyLocation> {
    for raw in 0u8..=20 {
        let loc = d2s::BodyLocation::from_raw(raw);
        if format!("{loc:?}").eq_ignore_ascii_case(name.trim()) {
            return Some(loc);
        }
    }
    None
}

/// 从基型代码构造一件物品（装备库导入用）。
///
/// 与 [`ItemEdit::CloneItem`] 的区别：克隆要求存档里已经有同基型的物品当模板，
/// 而装备库要导入存档里**没有**的物品，所以必须能从代码直接构造。
///
/// `version` 必须传存档版本：写盘时按物品自身的 version 决定位宽编码，
/// 用默认值会写出别的格式，读回来直接报错。
/// 新建物品的默认存储页。
fn default_store_page() -> String {
    "Inventory".to_string()
}

/// 新建物品的默认等级（1 级总比 0 级安全：0 级会让部分按等级公式算出 0）。
fn default_item_level() -> u8 {
    1
}

fn build_item(
    code: &str,
    quality: &str,
    file_index: Option<u16>,
    level: u8,
    version: u16,
) -> Result<d2s::Item, EditError> {
    let code = code.trim();
    if code.is_empty() {
        return Err(EditError::Invalid("物品代码不能为空".into()));
    }
    let mut item = d2s::Item::new(code);
    item.version = version;
    item.level = level;
    // 品质：暗金/套装靠 `quality_data` 里的表行号区分，普通物品不需要额外数据。
    // 这里只构造"能装进存档"的最小结构，属性由调用方整体写入。
    match quality {
        "暗金" | "Unique" => {
            item.quality = d2s::ItemQuality::Unique;
            // `file_index` 是表里的 `*ID`：指错就会变成别的暗金（或读出越界）。
            item.quality_data = Some(d2s::QualityData::SetUnique {
                file_index: file_index
                    .ok_or_else(|| EditError::Invalid("暗金物品必须指定表行索引".to_string()))?,
            });
        }
        "套装" | "Set" => {
            item.quality = d2s::ItemQuality::Set;
            item.quality_data = Some(d2s::QualityData::SetUnique {
                file_index: file_index
                    .ok_or_else(|| EditError::Invalid("套装物品必须指定表行索引".to_string()))?,
            });
        }
        "普通" | "Normal" | "" => {
            item.quality = d2s::ItemQuality::Normal;
            item.quality_data = None;
        }
        other => {
            return Err(EditError::Invalid(format!("不支持的品质：{other}")));
        }
    }
    Ok(item)
}

/// 在属性列表上设置一条属性（**按 `id + layer` 精确定位**）。
///
/// 抽成独立函数是为了能单测这条最容易出错的匹配逻辑。
///
/// ## 为什么必须带 layer
///
/// 带参数位的属性（`ItemStatCost` 里 `Save Param Bits > 0`，共 17 条）
/// 同一 ID 会有多条记录，靠层区分：`item_addclassskills` 的层是职业行号、
/// `item_aura` 的层是光环 ID。
///
/// 以前无条件匹配 `layer == 0`，层非 0 的属性永远找不到，于是每次
/// "应用修改"都**新增一条**——用户改了值，存档里却多出一条，
/// 越改越乱（这就是"小型神符值不对"的真正原因）。
///
/// 语义：
/// - `layer = Some(n)`：只改层 `n` 那一条；**找不到就报错**，不静默新增
///   （否则用户以为改好了，其实多了一条）。
/// - `layer = None`：按层 0 找；找不到就新增一条层 0 的记录
///   （这是"添加属性"按钮的用法）。
fn set_stat_on(
    stats: &mut Vec<Stat>,
    id: StatId,
    value: i64,
    layer: Option<i32>,
) -> Result<(), EditError> {
    let wanted = layer.unwrap_or(0);
    if let Some(existing) = stats.iter_mut().find(|s| s.id == id && s.layer == wanted) {
        existing.value = value;
        return Ok(());
    }
    if layer.is_some() {
        return Err(EditError::Invalid(format!(
            "该物品没有属性 ID {}（层 {wanted}）",
            id.0
        )));
    }
    stats.push(Stat {
        id,
        value,
        layer: wanted,
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_page_names_roundtrip() {
        for page in [
            StorePageName::Inventory,
            StorePageName::Equip,
            StorePageName::Trade,
            StorePageName::Cube,
            StorePageName::Stash,
            StorePageName::Belt,
            StorePageName::Null,
        ] {
            assert_eq!(StorePageName::from_raw(page.raw()), Some(page));
            assert_eq!(StorePageName::parse(page.as_str()), Some(page));
        }
        assert!(StorePageName::parse("Nope").is_none());
        assert!(StorePageName::from_raw(9).is_none());
    }

    #[test]
    fn equip_slot_names() {
        assert!(body_location_from_name("Head").is_some());
        assert!(body_location_from_name("head").is_some(), "大小写不敏感");
        assert!(body_location_from_name("nope").is_none());
    }

    /// **按层精确定位**：带参数位的属性同一 ID 有多条。
    ///
    /// 这是"小型神符值不对"的根因：以前无条件匹配 `layer == 0`，
    /// 层非 0 的属性永远找不到，于是每次应用都新增一条。
    #[test]
    fn set_stat_matches_by_layer() {
        // item_aura（151）两条记录，层不同：122 和 123
        let mut stats = vec![
            Stat {
                id: StatId(151),
                value: 10,
                layer: 122,
            },
            Stat {
                id: StatId(151),
                value: 20,
                layer: 123,
            },
        ];
        // 改层 123 那条：只动它，不能新增
        set_stat_on(&mut stats, StatId(151), 31, Some(123)).unwrap();
        assert_eq!(stats.len(), 2, "不该新增记录");
        assert_eq!(stats[1].value, 31);
        assert_eq!(stats[0].value, 10, "层 122 那条不受影响");

        // 改层 122 那条
        set_stat_on(&mut stats, StatId(151), 7, Some(122)).unwrap();
        assert_eq!(stats.len(), 2);
        assert_eq!(stats[0].value, 7);

        // 指定一个不存在的层 → 报错，而不是新增
        let err = set_stat_on(&mut stats, StatId(151), 5, Some(999));
        assert!(err.is_err(), "指定了不存在的层应当报错");
        assert_eq!(stats.len(), 2, "报错时不能留下多余记录");

        // 不改层（layer = None）→ 找不到就新增一条层 0 的记录
        set_stat_on(&mut stats, StatId(151), 3, None).unwrap();
        assert_eq!(stats.len(), 3);
        assert_eq!(stats[2].layer, 0);
        // 再来一次要改那一条，而不是再加
        set_stat_on(&mut stats, StatId(151), 9, None).unwrap();
        assert_eq!(stats.len(), 3);
        assert_eq!(stats[2].value, 9);
    }

    /// 普通属性（层恒为 0）反复设置不能越改越多。
    #[test]
    fn set_stat_is_idempotent_for_layer_zero() {
        let mut stats = vec![Stat {
            id: StatId(7),
            value: 100,
            layer: 0,
        }];
        for v in [101, 102, 103] {
            set_stat_on(&mut stats, StatId(7), v, Some(0)).unwrap();
        }
        assert_eq!(stats.len(), 1, "同一条属性反复改不该变多");
        assert_eq!(stats[0].value, 103);
    }
}

//! 容器与物品摆放校验（里程碑 3）。
//!
//! ## 两条实测结论（2026-10-10，用本机 7 个真实存档验证）
//!
//! ### 1. 不能拿 `inventory.txt` 的尺寸判"越界"
//!
//! 疯兔2 这类整合包把**扩展存储并到 `Inventory` 页**，存档里出现 `x=14, y=7` 这种坐标，
//! **连游戏自建的空白角色模板都有**（`blank-testnew.d2s`：`maxX=14 / maxY=7`），
//! 而 `inventory.txt` 原版行写的是 `Amazon → gridX=10, gridY=4`。
//! 拿原版表直接判越界会把**正常存档全判成非法**。所以尺寸基准取**存档自身**：
//! 同一区域的坐标跨度即该区域的实际尺寸，只把原版尺寸作为参照打印。
//!
//! ### 2. 合成箱（`StorePage::Cube`）用的是**局部坐标**
//!
//! `小五.d2s` 里 `Inventory` 页有 18 件、`Cube` 页有 15 件，两者坐标区间重叠，
//! 直接合并会报 3 处"重叠"。把合成箱原点偏移到 **(12,0)** 后，
//! 22 件物品在 16×8 网格里**零冲突**（实测），说明合成箱的坐标是相对自己的面板的。
//! 未完全证实：偏移量是否恒为 (12,0)（也可能随 mod 变化）。
//! 因此本模块即使已知偏移也**只用于报告**，默认不把两页合并判重叠，
//! 避免把正常存档误判为非法。
//!
//! 可靠的判定保留如下：
//! - **同页内重叠**：同 `(模式, 存储页)` 区域里两件物品占同一格；
//! - **同一装备位重复**：两件都装备在 Head/Neck/…；
//! - **未知物品代码**：数据集里没有该代码（mod 新增或数据集不匹配）。
//!
//! "容器有几格、mod 是否扩容"这类需要 mod 元数据的判定，等拿到 mod 的扩展尺寸定义
//! 之后再收紧，见 `docs/next-steps-materials.md`。

use std::collections::BTreeMap;

use crate::dataset::TableStore;

/// 单个存储区域的校验结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionReport {
    /// 区域名（如 `背包`、`储物箱 页4`、`腰带`）。
    pub label: String,
    /// 参与校验的物品数。
    pub checked: usize,
    /// 存档里该区域的坐标跨度 `(max_x+1, max_y+1)`。
    pub extent: (u32, u32),
    /// 实际被占用的格数（重叠格只算一次）。
    pub used_cells: u32,
    /// 检出问题。
    pub issues: Vec<Issue>,
}

impl RegionReport {
    /// 是否有问题。
    pub fn is_ok(&self) -> bool {
        self.issues.is_empty()
    }
}

/// 全部校验结果。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ValidationReport {
    /// 逐区域结果。
    pub regions: Vec<RegionReport>,
    /// 装备位重复问题（无网格，单独报）。
    pub equipment_issues: Vec<Issue>,
}

impl ValidationReport {
    /// 问题总数（含提示）。
    pub fn issue_count(&self) -> usize {
        self.regions.iter().map(|r| r.issues.len()).sum::<usize>() + self.equipment_issues.len()
    }

    /// 仅"确定问题"的数量（[`Severity::Error`]）。
    pub fn error_count(&self) -> usize {
        self.regions
            .iter()
            .flat_map(|r| r.issues.iter())
            .chain(self.equipment_issues.iter())
            .filter(|i| i.severity() == Severity::Error)
            .count()
    }

    /// 提示数量（[`Severity::Warning`]）。
    pub fn warning_count(&self) -> usize {
        self.issue_count() - self.error_count()
    }

    /// 是否没有"确定问题"（提示不算失败）。
    pub fn is_ok(&self) -> bool {
        self.error_count() == 0
    }
}

/// 网格里的一件物品。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacedItem {
    /// 物品代码。
    pub code: String,
    /// 显示名。
    pub name: String,
    /// 左上角列。
    pub col: u32,
    /// 左上角行。
    pub row: u32,
    /// 占用宽（格）。
    pub w: u32,
    /// 占用高（格）。
    pub h: u32,
    /// 是否来自"坐标语义待确认"的面板（如合成箱的局部坐标）。
    ///
    /// 与其它面板的物品发生重叠时只报 [`Severity::Warning`]，避免误判正常存档。
    pub cross_panel: bool,
}

/// 问题严重级别。
///
/// `Warning` 用于**坐标语义尚未完全证实**的情况（合成箱局部坐标、mod 扩展存储），
/// 只提示不判失败——宁可漏报也不能把正常存档判成非法。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// 确定的问题（会导致存档损坏/行为异常）。
    Error,
    /// 需要人工确认的提示。
    Warning,
}

/// 校验问题。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Issue {
    /// 网格内重叠。
    Overlap {
        /// 物品序号。
        index: usize,
        /// 物品名。
        name: String,
        /// 冲突物品序号。
        other_index: usize,
        /// 冲突物品名。
        other_name: String,
        /// 冲突格。
        cell: (u32, u32),
        /// 严重级别（跨面板坐标不确定性 → Warning）。
        severity: Severity,
    },
    /// 取不到尺寸（数据集里没有这个代码）。
    UnknownSize {
        /// 物品序号。
        index: usize,
        /// 物品代码。
        code: String,
    },
    /// 同一装备位被两件物品占用。
    DuplicateBodySlot {
        /// 装备位名。
        slot: String,
        /// 物品名。
        first: String,
        /// 物品名。
        second: String,
    },
}

impl Issue {
    /// 严重级别。
    pub fn severity(&self) -> Severity {
        match self {
            Self::Overlap { severity, .. } => *severity,
            // 未知代码只影响显示，不判失败
            Self::UnknownSize { .. } => Severity::Warning,
            Self::DuplicateBodySlot { .. } => Severity::Error,
        }
    }
}

impl std::fmt::Display for Issue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Overlap {
                index,
                name,
                other_index,
                other_name,
                cell,
                severity,
            } => {
                let tag = match severity {
                    Severity::Error => "重叠",
                    Severity::Warning => "疑似重叠（跨面板坐标待确认）",
                };
                write!(
                    f,
                    "[{index}] {name} 与 [{other_index}] {other_name} 在格 ({},{}) {tag}",
                    cell.0, cell.1
                )
            }
            Self::UnknownSize { index, code } => {
                write!(f, "[{index}] 物品代码 {code} 取不到尺寸（数据集不匹配？）")
            }
            Self::DuplicateBodySlot {
                slot,
                first,
                second,
            } => {
                write!(f, "装备位 {slot} 被两件物品占用：{first} / {second}")
            }
        }
    }
}

/// 校验单个网格区域。
///
/// `size_lookup` 用于取物品占用宽高（来自 `Armor/Weapons/Misc` 表）；
/// 取不到时回退到 `PlacedItem` 自带的 `w/h` 并报 [`Issue::UnknownSize`]。
///
/// 区域尺寸**由传入物品的坐标范围决定**（见模块文档），不做外部尺寸判定。
///
/// `strict`：同区域重叠是否判为 Error。含“坐标语义待确认”面板时传 false。
pub fn validate_region(
    label: impl Into<String>,
    items: &[PlacedItem],
    size_lookup: impl Fn(&str) -> Option<(u32, u32)>,
    strict: bool,
) -> RegionReport {
    // 先算区域跨度
    let mut extent = (0u32, 0u32);
    for it in items {
        let (w, h) = size_lookup(&it.code).unwrap_or((it.w.max(1), it.h.max(1)));
        extent.0 = extent.0.max(it.col + w);
        extent.1 = extent.1.max(it.row + h);
    }

    let mut issues = Vec::new();
    let mut grid: BTreeMap<(u32, u32), usize> = BTreeMap::new();

    for (i, it) in items.iter().enumerate() {
        let size = size_lookup(&it.code);
        let (w, h) = size.unwrap_or((it.w.max(1), it.h.max(1)));
        if size.is_none() {
            issues.push(Issue::UnknownSize {
                index: i,
                code: it.code.clone(),
            });
        }
        let mut reported = std::collections::HashSet::new();
        for dy in 0..h {
            for dx in 0..w {
                let cell = (it.col + dx, it.row + dy);
                match grid.get(&cell) {
                    Some(&other) => {
                        // 同一对物品只报一次，避免大物品刷屏
                        if reported.insert(other) {
                            // 跨面板（坐标语义未证实）只报提示
                            let severity = if !strict || it.cross_panel || items[other].cross_panel
                            {
                                Severity::Warning
                            } else {
                                Severity::Error
                            };
                            issues.push(Issue::Overlap {
                                index: i,
                                name: it.name.clone(),
                                other_index: other,
                                other_name: items[other].name.clone(),
                                cell,
                                severity,
                            });
                        }
                    }
                    None => {
                        grid.insert(cell, i);
                    }
                }
            }
        }
    }

    RegionReport {
        label: label.into(),
        checked: items.len(),
        extent,
        used_cells: grid.len() as u32,
        issues,
    }
}

/// 校验装备位是否重复占用。
///
/// `equipped` 为 `(装备位名, 物品名)` 列表。
pub fn validate_body_slots(equipped: &[(String, String)]) -> Vec<Issue> {
    let mut seen: BTreeMap<&str, &str> = BTreeMap::new();
    let mut issues = Vec::new();
    for (slot, name) in equipped {
        match seen.get(slot.as_str()) {
            Some(first) => issues.push(Issue::DuplicateBodySlot {
                slot: slot.clone(),
                first: (*first).to_string(),
                second: name.clone(),
            }),
            None => {
                seen.insert(slot.as_str(), name.as_str());
            }
        }
    }
    issues
}

/// 物品尺寸查询（`Armor`/`Weapons`/`Misc` 的 `invwidth`/`invheight`）。
///
/// 一次读齐成 map，避免逐件 IO；同时可用于界面显示占格。
pub fn build_size_map(excel_dir: &std::path::Path) -> BTreeMap<String, (u32, u32)> {
    let mut map = BTreeMap::new();
    let Ok(rd) = std::fs::read_dir(excel_dir) else {
        return map;
    };
    let wanted = ["armor", "weapons", "misc"];
    for entry in rd.flatten() {
        let path = entry.path();
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if !wanted.contains(&stem.to_ascii_lowercase().as_str()) {
            continue;
        }
        let Ok(t) = crate::table::Table::from_file(&path) else {
            continue;
        };
        for row in 0..t.len() {
            let Some(code) = t.cell_nonempty(row, "code") else {
                continue;
            };
            let w = t.int(row, "invwidth").unwrap_or(1).max(1) as u32;
            let h = t.int(row, "invheight").unwrap_or(1).max(1) as u32;
            map.insert(code.to_string(), (w, h));
        }
    }
    map
}

/// 数据集里的容器规格（供界面显示"原版是多大"，**不用于判越界**）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContainerSpec {
    /// 每行格子数。
    pub cols: u32,
    /// 行数。
    pub rows: u32,
    /// 格子边长（像素）。
    pub cell_w: u32,
    /// 格子高（像素）。
    pub cell_h: u32,
}

impl ContainerSpec {
    /// 总格数。
    pub fn cells(&self) -> u32 {
        self.cols * self.rows
    }
}

/// 从 `inventory.txt` 取某行（如 `Amazon`、`Big Bank Page 1`）的网格规格。
pub fn container_spec_from_tables(
    tables: &mut TableStore,
    row_name: &str,
) -> Option<ContainerSpec> {
    let t = tables.get("inventory")?;
    let row = t.find_row_by("class", row_name)?;
    let get = |col: &str| {
        t.cell_nonempty(row, col)
            .and_then(|v| v.parse::<u32>().ok())
    };
    Some(ContainerSpec {
        cols: get("gridX")?,
        rows: get("gridY")?,
        cell_w: get("gridBoxWidth").unwrap_or(1),
        cell_h: get("gridBoxHeight").unwrap_or(1),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(code: &str, col: u32, row: u32, w: u32, h: u32) -> PlacedItem {
        PlacedItem {
            code: code.into(),
            name: code.into(),
            col,
            row,
            w,
            h,
            cross_panel: false,
        }
    }

    #[test]
    fn accepts_valid_layout_and_reports_extent() {
        let items = vec![
            item("a", 0, 0, 1, 1),
            item("b", 2, 0, 2, 1),
            item("c", 0, 1, 1, 3),
        ];
        let sizes = |c: &str| match c {
            "a" => Some((1, 1)),
            "b" => Some((2, 1)),
            "c" => Some((1, 3)),
            _ => None,
        };
        let r = validate_region("背包", &items, sizes, true);
        assert!(r.is_ok(), "{:?}", r.issues);
        assert_eq!(r.checked, 3);
        assert_eq!(r.extent, (4, 4));
        assert_eq!(r.used_cells, 1 + 2 + 3);
    }

    #[test]
    fn detects_overlap_once_per_pair() {
        // 2x2 物品压住另一件 2x2，重叠 1 格
        let items = vec![item("a", 0, 0, 2, 2), item("b", 1, 1, 2, 2)];
        let r = validate_region("背包", &items, |_| Some((2, 2)), true);
        assert_eq!(r.issues.len(), 1, "同一对物品只报一次：{:?}", r.issues);
        match &r.issues[0] {
            Issue::Overlap { cell, .. } => assert_eq!(*cell, (1, 1)),
            other => panic!("期望重叠，得到 {other:?}"),
        }
        // 重叠格不重复计入
        assert_eq!(r.used_cells, 4 + 3);
        assert_eq!(r.extent, (3, 3));
    }

    #[test]
    fn overlapping_pair_reported_per_pair_not_per_cell() {
        // 1x4 压 1x4，重叠 3 格，但只应报 1 条（同一对）
        let items = vec![item("a", 0, 0, 1, 4), item("b", 0, 1, 1, 4)];
        let r = validate_region("背包", &items, |_| Some((1, 4)), true);
        assert_eq!(r.issues.len(), 1, "{:?}", r.issues);
    }

    #[test]
    fn reports_unknown_item_code() {
        let items = vec![item("zzz", 0, 0, 1, 1)];
        let r = validate_region("背包", &items, |_| None, true);
        assert!(matches!(r.issues[0], Issue::UnknownSize { .. }));
    }

    #[test]
    fn size_lookup_overrides_item_fields() {
        let items = vec![item("a", 0, 0, 1, 1)];
        let r = validate_region("背包", &items, |_| Some((3, 1)), true);
        assert_eq!(r.used_cells, 3);
        assert_eq!(r.extent, (3, 1));
    }

    #[test]
    fn detects_duplicate_body_slot() {
        let equipped = vec![
            ("Head".to_string(), "帽子A".to_string()),
            ("Neck".to_string(), "项链".to_string()),
            ("Head".to_string(), "帽子B".to_string()),
        ];
        let issues = validate_body_slots(&equipped);
        assert_eq!(issues.len(), 1);
        match &issues[0] {
            Issue::DuplicateBodySlot {
                slot,
                first,
                second,
            } => {
                assert_eq!(slot, "Head");
                assert_eq!(first, "帽子A");
                assert_eq!(second, "帽子B");
            }
            other => panic!("期望装备位重复，得到 {other:?}"),
        }
    }
}

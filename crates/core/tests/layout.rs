//! 布局推断与"用户确认"集成测试（里程碑 4/5 的配套能力）。
//!
//! 样本不入库，找不到时跳过。

use std::path::{Path, PathBuf};

use d2r_core::{needs_confirmation, EditSession, LayoutSettings};

fn samples_root() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("D2R_SAMPLES_DIR") {
        let p = PathBuf::from(p);
        if p.is_dir() {
            return Some(p);
        }
    }
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo = manifest.parent()?.parent()?;
    let p = repo.join("samples").join("saves");
    p.is_dir().then_some(p)
}

fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(dir) else {
        return out;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            out.extend(walk(&p));
        } else if p.extension().map(|x| x == "d2s").unwrap_or(false) {
            out.push(p);
        }
    }
    out
}

#[test]
fn hints_are_produced_for_real_saves() {
    let Some(root) = samples_root() else {
        eprintln!("跳过：没有样本");
        return;
    };
    let files = walk(&root);
    if files.is_empty() {
        eprintln!("跳过：没有 .d2s 样本");
        return;
    }
    let mut with_coords = 0;
    for p in &files {
        let Ok(session) = EditSession::open(p) else {
            continue;
        };
        let h = session.layout_hints();
        // 只要存档里有存放类物品，就应该推断出坐标上限
        if h.inventory_max.is_some() || h.stash_max.is_some() {
            with_coords += 1;
        }
        // unknowns 必须明确列出"读不出来"的项
        assert_eq!(h.unknowns.len(), 4, "{}: 应列出 4 项未知", p.display());
    }
    assert!(with_coords > 0, "至少应有一个样本能推断出坐标");
}

#[test]
fn user_confirmed_layout_makes_bounds_reliable() {
    let Some(root) = samples_root() else { return };
    let files = walk(&root);
    let Some(target) = files.iter().find(|p| {
        EditSession::open(p)
            .map(|s| s.layout_hints().stash_max.is_some())
            .unwrap_or(false)
    }) else {
        eprintln!("跳过：没有带箱子坐标的样本");
        return;
    };
    let session = EditSession::open(target).expect("打开");
    let hints = session.layout_hints();
    let size = |_code: &str| Some((1u32, 1u32));

    // 1) 原版默认尺寸下：扩展过的存档必然报越界（这说明"不能拿原版表硬判"）
    let vanilla = LayoutSettings::default();
    let (issues_vanilla, checked) = session.validate_bounds(&vanilla, size);
    assert!(checked > 0, "应有物品参与校验");
    assert!(
        !issues_vanilla.is_empty(),
        "{}: 扩展过的存档在原版尺寸下应报越界",
        target.display()
    );

    // 2) 按推断出的跨度设置尺寸后：不该再报越界
    let (sx, sy) = hints.stash_max.unwrap();
    let (ix, iy) = hints.inventory_max.unwrap_or((9, 3));
    let fitted = LayoutSettings {
        books_single_cell: false,
        cube_single_cell: false,
        // 留足余量（推断只给下限）：按最大坐标 + 1 再加一点
        inventory_cols: ix + 2,
        inventory_rows: iy + 2,
        cube_cols: hints.cube_max.map(|c| c.0 + 2).unwrap_or(3),
        cube_rows: hints.cube_max.map(|c| c.1 + 2).unwrap_or(4),
        stash_cols: sx + 2,
        stash_rows: sy + 2,
        merc_full_equipment: hints.merc_items > 3,
        confirmed_by_user: true,
    };
    let (issues_fitted, _) = session.validate_bounds(&fitted, size);
    assert!(
        issues_fitted.is_empty(),
        "{}: 按用户确认的尺寸不该报越界：{:?}",
        target.display(),
        issues_fitted
    );

    // 3) 确认之后不该再问同样的问题
    let questions_when_unconfirmed = needs_confirmation(&hints);
    assert!(
        !questions_when_unconfirmed.is_empty(),
        "扩展存档应产生待确认项"
    );
    let confirmed = LayoutSettings {
        confirmed_by_user: true,
        ..fitted.clone()
    };
    assert!(confirmed.confirmed_by_user);
}

#[test]
fn empty_merc_section_does_not_ask() {
    let Some(root) = samples_root() else { return };
    for p in walk(&root) {
        let Ok(session) = EditSession::open(&p) else {
            continue;
        };
        let h = session.layout_hints();
        if h.merc_section && h.merc_items == 0 {
            let qs = needs_confirmation(&h);
            assert!(
                !qs.iter().any(|q| q.contains("佣兵")),
                "{}: 空佣兵段不该产生佣兵提问：{qs:?}",
                p.display()
            );
        }
    }
}

/// "书只占一格"的设置：只在开关打开时把传送书/鉴定书算成 1×1。
#[test]
fn books_single_cell_only_affects_books() {
    let mut s = LayoutSettings::default();
    assert!(!s.books_single_cell, "默认应是原版行为（书占 1×2）");

    // 关闭时保持原始占格
    assert_eq!(s.item_size("tbk", (1, 2)), (1, 2), "传送书默认 1×2");
    assert_eq!(s.item_size("ibk", (1, 2)), (1, 2), "鉴定书默认 1×2");
    assert_eq!(s.item_size("hp1", (1, 1)), (1, 1));

    // 打开后书变 1×1，其它物品不受影响
    s.books_single_cell = true;
    assert_eq!(s.item_size("tbk", (1, 2)), (1, 1), "传送书应只占一格");
    assert_eq!(s.item_size("ibk", (1, 2)), (1, 1), "鉴定书应只占一格");
    assert_eq!(
        s.item_size("amu", (1, 1)),
        (1, 1),
        "非书物品不该被改（护身符本来就 1×1）"
    );
    assert_eq!(
        s.item_size("box", (2, 2)),
        (2, 2),
        "非书物品不该被改（盒子 2×2 要保持）"
    );
    // 卷轴本来就是 1×1，开不开都一样
    assert_eq!(s.item_size("tsc", (1, 1)), (1, 1));
}

/// 旧配置文件缺字段也要能读（每加一个设置项都要保住这条）。
#[test]
fn layout_settings_ignore_missing_fields() {
    // 只有 4 个字段的老配置
    let json = r#"{"inventoryCols":16,"inventoryRows":8,"cubeCols":8,"cubeRows":8}"#;
    let s: LayoutSettings = serde_json::from_str(json).expect("缺字段的旧配置应能读");
    assert_eq!(
        (s.inventory_cols, s.inventory_rows),
        (16, 8),
        "给出的字段要生效"
    );
    assert!(!s.books_single_cell, "缺的字段用默认值");
    assert!(!s.merc_full_equipment);
    assert_eq!((s.stash_cols, s.stash_rows), (10, 10));
}

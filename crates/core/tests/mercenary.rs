//! 佣兵装备编辑：`ItemTarget::Mercenary` 的读写。
//!
//! 核心风险是**序号串台**：佣兵段与主物品列表是分开存的两个 `ItemsSection`，
//! 序号体系互不相干（佣兵段的 #0 不是主列表的 #0）。所以这里重点验证
//! "改佣兵不动主列表、改主列表不动佣兵"。

use std::path::{Path, PathBuf};

use d2r_core::items::ItemTarget;
use d2r_core::{EditSession, ItemEdit};

fn sample() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("D2R_SAMPLES_DIR") {
        let p = PathBuf::from(p).join("d2r-105").join("杜仲.d2s");
        if p.is_file() {
            return Some(p);
        }
    }
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo = manifest.parent()?.parent()?;
    let dir = repo.join("samples").join("saves").join("d2r-105");
    ["杜仲.d2s", "小五.d2s", "杜老五.d2s"]
        .iter()
        .map(|n| dir.join(n))
        .find(|p| p.is_file())
}

fn work_copy(src: &Path, tag: &str) -> PathBuf {
    let mut dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    dir.push("merc");
    let _ = std::fs::create_dir_all(&dir);
    let dest = dir.join(format!("{tag}.d2s"));
    std::fs::copy(src, &dest).expect("复制样本");
    dest
}

#[test]
fn merc_edit_does_not_touch_inventory() {
    let Some(src) = sample() else {
        eprintln!("跳过：没有样本");
        return;
    };
    let work = work_copy(&src, "no_crosstalk");
    let mut session = EditSession::open(&work).expect("打开");

    let before_inv: Vec<i32> = session
        .item_details()
        .iter()
        .map(|d| d.durability.unwrap_or(-1))
        .collect();
    let before_merc: Vec<i32> = session
        .mercenary_items()
        .iter()
        .map(|m| m.durability.unwrap_or(-1))
        .collect();
    assert!(!before_merc.is_empty(), "样本应有佣兵装备");

    // 把佣兵 #0 的耐久改成一个新值（改成和原值不同，才能看出串台）
    let original = before_merc[0];
    let new_value = if original > 5 {
        original - 5
    } else {
        original + 1
    };
    session
        .apply_item_on(
            ItemTarget::Mercenary,
            ItemEdit::SetDurability {
                index: 0,
                value: new_value,
            },
        )
        .expect("改佣兵耐久");

    let after_merc: Vec<i32> = session
        .mercenary_items()
        .iter()
        .map(|m| m.durability.unwrap_or(-1))
        .collect();
    let after_inv: Vec<i32> = session
        .item_details()
        .iter()
        .map(|d| d.durability.unwrap_or(-1))
        .collect();

    assert_eq!(after_merc[0], new_value, "佣兵 #0 应被改动");
    assert_eq!(
        after_inv, before_inv,
        "主物品列表一个字节都不该被改（序号串台）"
    );

    let (_, ok) = session.verify().expect("写前校验");
    assert!(ok);
    session.save().expect("保存");

    let reopened = EditSession::open(&work).expect("重新打开");
    assert_eq!(
        reopened.mercenary_items()[0].durability,
        Some(new_value),
        "改动应落盘"
    );
    assert!(
        reopened.file.check_roundtrip().identical,
        "改佣兵后仍应逐字节往返一致"
    );
}

#[test]
fn inventory_edit_does_not_touch_merc() {
    let Some(src) = sample() else { return };
    let work = work_copy(&src, "reverse");
    let mut session = EditSession::open(&work).expect("打开");

    let before_merc: Vec<u16> = session
        .mercenary_items()
        .iter()
        .map(|m| m.stats.first().map(|s| s.0).unwrap_or(0))
        .collect();

    session
        .apply_item(ItemEdit::SetQuantity { index: 0, value: 3 })
        .expect("改主列表数量");

    let after_merc: Vec<u16> = session
        .mercenary_items()
        .iter()
        .map(|m| m.stats.first().map(|s| s.0).unwrap_or(0))
        .collect();
    assert_eq!(after_merc, before_merc, "佣兵段不该被主列表的改动影响");
}

#[test]
fn merc_stat_edit_and_replay_are_target_aware() {
    let Some(src) = sample() else { return };
    let work = work_copy(&src, "replay");
    let mut session = EditSession::open(&work).expect("打开");

    let stat = session.mercenary_items()[0].stats[0].0;
    session
        .apply_item_on(
            ItemTarget::Mercenary,
            ItemEdit::SetStat {
                index: 0,
                stat,
                value: 7,
                layer: None,
            },
        )
        .expect("改佣兵属性");

    // 记录里必须带上目标，否则撤销/重放会作用到主列表
    assert!(
        session
            .item_edits
            .iter()
            .all(|(t, _)| *t == ItemTarget::Mercenary),
        "佣兵改动的记录应标记为佣兵目标"
    );

    let (_, ok) = session.verify().expect("写前校验");
    assert!(ok);
}

#[test]
fn merc_edit_on_save_without_merc_section_is_rejected() {
    // 用一个没有佣兵段的存档（无样本时跳过）
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo = manifest.parent().unwrap().parent().unwrap();
    let dir = repo.join("samples").join("saves").join("d2r-105");
    let Some(path) = std::fs::read_dir(&dir).ok().and_then(|rd| {
        rd.flatten().map(|e| e.path()).find(|p| {
            p.extension().and_then(|e| e.to_str()) == Some("d2s")
                && d2r_core::SaveFile::open(p)
                    .map(|f| f.save.merc_items.is_none())
                    .unwrap_or(false)
        })
    }) else {
        eprintln!("跳过：没有「无佣兵段」的样本");
        return;
    };
    let work = work_copy(&path, "no_merc");
    let mut session = EditSession::open(&work).expect("打开");
    assert!(session.mercenary_items().is_empty());
    let err = session
        .apply_item_on(
            ItemTarget::Mercenary,
            ItemEdit::SetDurability { index: 0, value: 1 },
        )
        .unwrap_err();
    assert!(format!("{err}").contains("佣兵段"), "{err}");
}

//! 容器网格与换容器/装备位（里程碑 5/7）集成测试。
//!
//! 这些是界面网格视图依赖的底层操作：网格里拖动物品 = `Relocate`，
//! 拖到装备槽 = `SetEquipSlot`。样本不入库，找不到时跳过。

use std::path::{Path, PathBuf};

use d2r_core::{EditSession, ItemEdit, StorePageName};

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
    dir.push("grid");
    let _ = std::fs::create_dir_all(&dir);
    let dest = dir.join(format!("{tag}.d2s"));
    std::fs::copy(src, &dest).expect("复制样本");
    dest
}

#[test]
fn relocate_between_pages_roundtrips() {
    let Some(src) = sample() else {
        eprintln!("跳过：没有样本");
        return;
    };
    let work = work_copy(&src, "relocate");
    let mut session = EditSession::open(&work).expect("打开");
    let Some(detail) = session.item_details().into_iter().next() else {
        eprintln!("跳过：没有物品");
        return;
    };
    let index = detail.index;

    // 背包 → 箱子
    session
        .apply_item(ItemEdit::Relocate {
            index,
            page: StorePageName::Stash,
            col: 3,
            row: 4,
        })
        .expect("移动到箱子");
    let after = session.item_detail(index).expect("读明细");
    assert_eq!(after.store_page, "Stash");
    assert_eq!((after.inv_x, after.inv_y), (3, 4));
    assert_eq!(after.mode, "Stored", "普通容器里的物品模式应是 Stored");

    // 箱子 → 腰带（模式应变成 InBelt）
    session
        .apply_item(ItemEdit::Relocate {
            index,
            page: StorePageName::Belt,
            col: 1,
            row: 0,
        })
        .expect("移动到腰带");
    assert_eq!(session.item_detail(index).unwrap().mode, "InBelt");

    // 保存后重新打开：位置必须保持，且仍逐字节往返
    let (_, ok) = session.verify().expect("写前校验");
    assert!(ok);
    session.save().expect("保存");
    let reopened = EditSession::open(&work).expect("重新打开");
    let re = reopened.item_detail(index).expect("读明细");
    assert_eq!(re.store_page, "Belt");
    assert_eq!((re.inv_x, re.inv_y), (1, 0));
    assert!(
        reopened.file.check_roundtrip().identical,
        "换容器后仍应逐字节往返一致"
    );
}

#[test]
fn relocate_rejects_out_of_range_coordinates() {
    let Some(src) = sample() else { return };
    let work = work_copy(&src, "relocate_bad");
    let mut session = EditSession::open(&work).expect("打开");
    let index = session.item_details()[0].index;

    // 坐标是 4 位字段：16 装不下
    let err = session
        .apply_item(ItemEdit::Relocate {
            index,
            page: StorePageName::Inventory,
            col: 16,
            row: 0,
        })
        .unwrap_err();
    assert!(format!("{err}").contains("0–15"), "{err}");

    // 未知物品号
    assert!(session
        .apply_item(ItemEdit::Relocate {
            index: 99999,
            page: StorePageName::Stash,
            col: 0,
            row: 0,
        })
        .is_err());
}

#[test]
fn equip_slot_moves_item_in_and_out() {
    let Some(src) = sample() else { return };
    let work = work_copy(&src, "equip");
    let mut session = EditSession::open(&work).expect("打开");
    let index = session.item_details()[0].index;

    // 戴到头盔位
    session
        .apply_item(ItemEdit::SetEquipSlot {
            index,
            slot: "Head".into(),
        })
        .expect("装备到头");
    let d = session.item_detail(index).expect("读明细");
    assert_eq!(d.body_location, "Head");
    assert_eq!(d.mode, "Equipped");
    assert_eq!(d.store_page, "Equip");

    // 戴上取下（"none" 表示卸下）
    session
        .apply_item(ItemEdit::SetEquipSlot {
            index,
            slot: "none".into(),
        })
        .expect("卸下");
    let d = session.item_detail(index).expect("读明细");
    assert_eq!(d.body_location, "None");
    assert_eq!(d.mode, "Stored");
    assert_eq!(d.store_page, "Inventory");

    // 未知槽位被拒
    assert!(session
        .apply_item(ItemEdit::SetEquipSlot {
            index,
            slot: "Nope".into(),
        })
        .is_err());

    let (_, ok) = session.verify().expect("写前校验");
    assert!(ok);
}

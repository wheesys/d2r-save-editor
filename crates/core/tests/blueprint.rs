//! 物品导入/导出（里程碑 5b）集成测试：用真实存档跑通"导出 → 导入 → 往返"。
//!
//! 样本不入库，找不到时跳过。

use std::path::{Path, PathBuf};

use d2r_core::{parse_blueprint, EditSession, ItemEdit, SaveFile};

fn sample() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("D2R_SAMPLES_DIR") {
        let p = PathBuf::from(p);
        if p.is_dir() {
            let f = p.join("d2r-105").join("杜仲.d2s");
            if f.is_file() {
                return Some(f);
            }
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
    dir.push("blueprint");
    let _ = std::fs::create_dir_all(&dir);
    let dest = dir.join(format!("{tag}.d2s"));
    std::fs::copy(src, &dest).expect("复制样本");
    dest
}

/// 找一件有属性的物品当模板。
fn template_index(session: &EditSession) -> Option<usize> {
    session
        .item_details()
        .into_iter()
        .find(|d| !d.stats.is_empty())
        .map(|d| d.index)
}

#[test]
fn export_then_import_roundtrips() {
    let Some(src) = sample() else {
        eprintln!("跳过：没有样本");
        return;
    };
    let work = work_copy(&src, "export_import");
    let mut session = EditSession::open(&work).expect("打开");
    let Some(template) = template_index(&session) else {
        eprintln!("跳过：没有带属性的物品");
        return;
    };
    let before_count = session.item_count();

    // 1) 导出蓝图 → JSON 文本 → 解析回来（模拟存盘/读盘）
    let blueprint = d2r_core::export_blueprint(&session, template).expect("导出蓝图");
    assert!(blueprint
        .stats
        .as_ref()
        .map(|s| !s.is_empty())
        .unwrap_or(false));
    let json = serde_json::to_string_pretty(&blueprint).expect("序列化");
    let parsed = parse_blueprint(&json).expect("解析蓝图");
    assert_eq!(parsed, blueprint, "导出再解析必须一致");

    // 2) 导入：克隆模板并覆盖属性
    let outcome = session.import_blueprint(&parsed, template).expect("导入");
    assert_eq!(outcome.index, before_count, "新物品应追加在末尾");
    assert_eq!(session.item_count(), before_count + 1);
    assert_eq!(
        outcome.stats_replaced,
        blueprint.stats.as_ref().unwrap().len()
    );

    // 3) 新物品的属性应与蓝图一致
    let detail = session.item_detail(outcome.index).expect("读新物品");
    let want: Vec<(u16, i64, i32)> = blueprint
        .stats
        .as_ref()
        .unwrap()
        .iter()
        .map(|s| (s.stat, s.value, s.layer))
        .collect();
    assert_eq!(detail.stats, want, "导入后的属性应与蓝图一致");
    assert_eq!(
        detail.code,
        blueprint.code.as_deref().unwrap_or(&detail.code)
    );

    // 4) 写前校验 + 保存 + 重新打开
    let (_bytes, ok) = session.verify().expect("写前校验");
    assert!(ok, "写前校验必须通过");
    session.save().expect("保存");

    let reopened = EditSession::open(&work).expect("重新打开");
    assert_eq!(reopened.item_count(), before_count + 1);
    assert_eq!(reopened.item_detail(outcome.index).unwrap().stats, want);

    // 5) 导入后仍必须逐字节往返
    assert!(
        reopened.file.check_roundtrip().identical,
        "导入物品后仍应逐字节往返一致"
    );

    // 6) 删除该物品后也应能往返
    let mut session = reopened;
    session
        .apply_item(ItemEdit::Remove {
            index: outcome.index,
        })
        .expect("删除");
    let (_, ok) = session.verify().expect("删除后校验");
    assert!(ok);
}

#[test]
fn import_rejects_out_of_range_template() {
    let Some(src) = sample() else { return };
    let work = work_copy(&src, "bad_template");
    let mut session = EditSession::open(&work).expect("打开");
    let bp = d2r_core::ItemBlueprint {
        note: None,
        code: None,
        quality: None,
        stats: None,
        durability: None,
        max_durability: None,
        quantity: None,
        defense: None,
        store_page: None,
        inv_x: None,
        inv_y: None,
    };
    let before = session.item_count();
    assert!(
        session.import_blueprint(&bp, 99999).is_err(),
        "越界模板必须被拒"
    );
    assert_eq!(session.item_count(), before, "失败不应改变物品数");
    assert!(!session.is_dirty(), "失败不应留下挂起修改");
}

#[test]
fn import_rejects_bad_durability() {
    let Some(src) = sample() else { return };
    let work = work_copy(&src, "bad_dur");
    let mut session = EditSession::open(&work).expect("打开");
    let Some(template) = template_index(&session) else {
        return;
    };
    let max = session
        .item_detail(template)
        .unwrap()
        .max_durability
        .unwrap_or(0);
    if max == 0 {
        eprintln!("跳过：模板没有耐久概念");
        return;
    }
    let bp = d2r_core::ItemBlueprint {
        note: None,
        code: None,
        quality: None,
        stats: None,
        durability: Some(max + 1000),
        max_durability: Some(max),
        quantity: None,
        defense: None,
        store_page: None,
        inv_x: None,
        inv_y: None,
    };
    let before = session.item_count();
    assert!(
        session.import_blueprint(&bp, template).is_err(),
        "当前耐久大于最大耐久必须被拒"
    );
    assert_eq!(session.item_count(), before);
}

#[test]
fn store_page_moves_item_to_target_page() {
    let Some(src) = sample() else { return };
    let work = work_copy(&src, "page");
    let mut session = EditSession::open(&work).expect("打开");
    let Some(template) = template_index(&session) else {
        return;
    };
    let bp = d2r_core::ItemBlueprint {
        note: None,
        code: None,
        quality: None,
        stats: None,
        durability: None,
        max_durability: None,
        quantity: None,
        defense: None,
        store_page: Some("Stash".into()),
        inv_x: Some(1),
        inv_y: Some(1),
    };
    let outcome = session.import_blueprint(&bp, template).expect("导入");
    let d = session.item_detail(outcome.index).expect("读回");
    assert_eq!(d.store_page, "Stash");
    assert_eq!((d.inv_x, d.inv_y), (1, 1));
    let (_, ok) = session.verify().expect("校验");
    assert!(ok);
    // 保存后样品仍逐字节一致
    session.save().expect("保存");
    let f = SaveFile::open(&work).expect("打开");
    assert!(f.check_roundtrip().identical);
}

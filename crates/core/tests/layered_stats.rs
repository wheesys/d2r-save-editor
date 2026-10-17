//! 带参数位属性（"层"）的编辑：**必须按 `id + layer` 精确定位**。
//!
//! ## 为什么专门测这个
//!
//! 用户报"小型神符的值不对"。根因是 `ItemEdit::SetStat` 的匹配条件写成了
//! `s.id == id && s.layer == 0`，而带参数位的属性层通常非 0
//! （`item_aura` 的层是光环 ID 123、`item_addclassskills` 的层是职业行号）。
//! 于是：
//! - 改这类属性**永远匹配不上**，每次都走"新增一条"；
//! - 用户每点一次"应用修改"，存档里就多一条重复属性，值看着也不对。
//!
//! 这个测试把"改值不新增、按层区分、落盘后重开仍然正确"钉死。

use d2r_core::{EditSession, ItemEdit, ItemTarget};
use std::path::{Path, PathBuf};

/// 找一个**带参数位属性**的物品（层非 0 才算，否则测不到这条路径）。
fn sample_with_layered_stat() -> Option<(PathBuf, usize, u16, i32)> {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo = manifest.parent()?.parent()?;
    let dir = repo.join("samples").join("saves").join("d2r-105");
    for entry in std::fs::read_dir(&dir).ok()?.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("d2s") {
            continue;
        }
        let Ok(session) = EditSession::open(&path) else {
            continue;
        };
        for (index, detail) in session.item_details().iter().enumerate() {
            // 只认真正的物品序号（`item_details` 的序就是 `item_details()[i]`）
            for (stat, value, layer) in &detail.stats {
                if *layer != 0 && *value > 0 {
                    return Some((path, index, *stat, *layer));
                }
            }
        }
    }
    None
}

fn work_copy(src: &Path, tag: &str) -> PathBuf {
    let mut dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    dir.push("layer");
    let _ = std::fs::create_dir_all(&dir);
    let dest = dir.join(format!("{tag}.d2s"));
    std::fs::copy(src, &dest).expect("复制样本");
    dest
}

/// 统计某物品上某属性 ID 的记录条数。
fn count_stats(session: &EditSession, index: usize, stat: u16) -> usize {
    session.item_details()[index]
        .stats
        .iter()
        .filter(|(id, _, _)| *id == stat)
        .count()
}

/// 取某物品上 `id + layer` 那条记录的值。
fn value_of(session: &EditSession, index: usize, stat: u16, layer: i32) -> Option<i64> {
    session.item_details()[index]
        .stats
        .iter()
        .find(|(id, _, l)| *id == stat && *l == layer)
        .map(|(_, v, _)| *v)
}

#[test]
fn editing_layered_stat_updates_in_place_and_persists() {
    let Some((src, index, stat, layer)) = sample_with_layered_stat() else {
        eprintln!("跳过：样本里没有带层的属性");
        return;
    };
    let work = work_copy(&src, "layered");
    let mut session = EditSession::open(&work).expect("打开");

    let before_count = count_stats(&session, index, stat);
    let before_value = value_of(&session, index, stat, layer).expect("应有这条属性");
    let new_value = if before_value > 1 {
        before_value - 1
    } else {
        before_value + 7
    };

    session
        .apply_item(ItemEdit::SetStat {
            index,
            stat,
            value: new_value,
            layer: Some(layer),
        })
        .expect("按层改属性");

    assert_eq!(
        count_stats(&session, index, stat),
        before_count,
        "改一条已有属性**不该新增记录**（这正是之前的 bug：越改越多）"
    );
    assert_eq!(
        value_of(&session, index, stat, layer),
        Some(new_value),
        "应改的正是那一层"
    );

    let (_, ok) = session.verify().expect("写前校验");
    assert!(ok, "写前校验应通过");
    session.save().expect("保存");

    // 重新打开：改动必须落盘，且条数不变
    let reopened = EditSession::open(&work).expect("重开");
    assert_eq!(
        count_stats(&reopened, index, stat),
        before_count,
        "落盘后条数不该变"
    );
    assert_eq!(
        value_of(&reopened, index, stat, layer),
        Some(new_value),
        "改动应落盘"
    );
}

#[test]
fn editing_unknown_layer_is_rejected() {
    let Some((src, index, stat, layer)) = sample_with_layered_stat() else {
        eprintln!("跳过：样本里没有带层的属性");
        return;
    };
    let work = work_copy(&src, "badlayer");
    let mut session = EditSession::open(&work).expect("打开");

    let before = count_stats(&session, index, stat);
    // 指定一个不存在的层：应报错，而不是悄悄新增一条
    let err = session
        .apply_item(ItemEdit::SetStat {
            index,
            stat,
            value: 1,
            layer: Some(layer + 1000),
        })
        .unwrap_err();
    assert!(
        format!("{err}").contains("层"),
        "错误信息应说明是层的问题：{err}"
    );
    assert_eq!(
        count_stats(&session, index, stat),
        before,
        "报错时不能留下多余记录"
    );
}

#[test]
fn repeated_edits_do_not_grow_the_stat_list() {
    // 用户会反复点"应用修改"，这条保证存档不会越改越乱
    let Some((src, index, stat, layer)) = sample_with_layered_stat() else {
        eprintln!("跳过：样本里没有带层的属性");
        return;
    };
    let work = work_copy(&src, "repeat");
    let mut session = EditSession::open(&work).expect("打开");
    let before_total: usize = session.item_details()[index].stats.len();

    for value in [1i64, 2, 3, 4, 5] {
        session
            .apply_item(ItemEdit::SetStat {
                index,
                stat,
                value,
                layer: Some(layer),
            })
            .expect("反复改同一层");
    }

    assert_eq!(
        session.item_details()[index].stats.len(),
        before_total,
        "反复改同一条属性，属性总数不该变化"
    );
    assert_eq!(value_of(&session, index, stat, layer), Some(5));
}

/// 删除**一条**层属性时，同 ID 的其他层必须留下。
///
/// 用户报"删除的时候，有两条技能等级就一起删了"：前端对层 0 的那条传了
/// "不带层"，而后端"不带层"的语义是"删该 ID 全部"，于是层 1 那条也没了。
/// 现在前端总是带上层；这条测试把后端的**按层删除**行为钉住。
#[test]
fn removing_one_layer_keeps_the_other() {
    let Some((src, index, stat, layer)) = sample_with_layered_stat() else {
        eprintln!("跳过：样本里没有带层的属性");
        return;
    };
    let work = work_copy(&src, "remove_layer");
    let mut session = EditSession::open(&work).expect("打开");
    let before = count_stats(&session, index, stat);
    if before < 2 {
        eprintln!("跳过：这条属性只有一层，测不到区分度");
        return;
    }

    session
        .apply_item(ItemEdit::RemoveStat {
            index,
            stat,
            layer: Some(layer),
        })
        .expect("按层删除");

    assert_eq!(
        count_stats(&session, index, stat),
        before - 1,
        "只该少一条（删哪层就只删哪层）"
    );
    assert_eq!(
        value_of(&session, index, stat, layer),
        None,
        "被指定的那一层应已删除"
    );
    let others = session.item_details()[index]
        .stats
        .iter()
        .filter(|(id, _, l)| *id == stat && *l != layer)
        .count();
    assert_eq!(others, before - 1, "其他层不该被牵连");
}

#[test]
fn target_defaults_to_inventory() {
    // 不使用 `apply_item_on` 时目标应是主物品列表
    let Some((src, index, stat, layer)) = sample_with_layered_stat() else {
        eprintln!("跳过：样本里没有带层的属性");
        return;
    };
    let work = work_copy(&src, "target");
    let mut session = EditSession::open(&work).expect("打开");
    session
        .apply_item(ItemEdit::SetStat {
            index,
            stat,
            value: 3,
            layer: Some(layer),
        })
        .expect("默认目标改属性");
    assert!(
        session
            .item_edits
            .iter()
            .all(|(t, _)| *t == ItemTarget::Inventory),
        "默认目标应是主物品列表"
    );
}

/// **添加属性**必须能新增一个物品上原本没有的 `ID + 层`。
///
/// 用户实测：在空白护符上加"职业技能等级（德鲁伊层 6）"被拒绝，
/// 报"该物品没有属性 ID 83（层 6）"。根因是"添加"走了 `SetStat`，
/// 而 `SetStat` 带层时找不到就报错（那是给"改已有值"用的语义）。
#[test]
fn add_stat_creates_a_missing_layer() {
    let Some((src, index, _, _)) = sample_with_layered_stat() else {
        eprintln!("跳过：样本里没有带层的属性");
        return;
    };
    let work = work_copy(&src, "add_layer");
    let mut session = EditSession::open(&work).expect("打开");
    // 先看这件物品现在有哪些 `ID + 层`
    let existing = session
        .item_detail(index)
        .expect("读明细")
        .stats
        .iter()
        .map(|(id, _, layer)| (*id, *layer))
        .collect::<Vec<_>>();
    let (stat, layer) = (83u16, 6i32); // item_addclassskills / 德鲁伊
    if existing.contains(&(stat, layer)) {
        eprintln!("跳过：样本第 0 件已经有该属性");
        return;
    }
    session
        .apply_item(ItemEdit::AddStat {
            index,
            stat,
            value: 1,
            layer: Some(layer),
        })
        .expect("添加属性应成功（即使原本没有）");
    let after = session.item_detail(index).expect("读明细");
    assert!(
        after
            .stats
            .iter()
            .any(|(id, _, l)| *id == stat && *l == layer),
        "新属性应出现在物品上"
    );
    // 再添加一次同一层：应覆盖而不是加出两条
    session
        .apply_item(ItemEdit::AddStat {
            index,
            stat,
            value: 2,
            layer: Some(layer),
        })
        .expect("重复添加应覆盖");
    let again = session.item_detail(index).expect("读明细");
    let hits = again
        .stats
        .iter()
        .filter(|(id, _, l)| *id == stat && *l == layer)
        .count();
    assert_eq!(hits, 1, "同一 ID+层 只应有一条");
}

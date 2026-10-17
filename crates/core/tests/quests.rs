//! 任务编辑（里程碑：任务进度）。
//!
//! 两个容易出错的地方，各有一条测试盯着：
//! 1. 掩码里的位必须与解析内核 `d2s::QuestFlags` 的定义一致（两边是独立声明的）；
//! 2. "全解锁"只能动真任务槽（按幕的槽位范围），不能碰序章、收尾槽、
//!    Act IV 的 NPC 对话槽、Act V 的黑暗 Wanderer / 闲聊 / 洗点槽。

use std::path::{Path, PathBuf};

use d2r_core::{quest_slot_range_of_act, Edit, EditSession, QUEST_COMPLETE_MASK};

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
    dir.push("quests");
    let _ = std::fs::create_dir_all(&dir);
    let dest = dir.join(format!("{tag}.d2s"));
    std::fs::copy(src, &dest).expect("复制样本");
    dest
}

/// 掩码里的位必须都真实存在于 `d2s::QuestFlags` 中。
#[test]
fn complete_mask_bits_exist_in_kernel() {
    let mut remaining = QUEST_COMPLETE_MASK;
    let mut seen = 0;
    while remaining != 0 {
        let bit = remaining & remaining.wrapping_neg();
        let flags = d2s::QuestFlags::from_bits(bit);
        assert!(
            flags.is_some(),
            "掩码里的位 0x{bit:04X} 在 QuestFlags 里不存在"
        );
        remaining &= !bit;
        seen += 1;
    }
    assert_eq!(seen, 2, "掩码应由 2 个位组成");
    // 反向：确认就是这两个位
    assert_eq!(
        QUEST_COMPLETE_MASK,
        d2s::QuestFlags::REWARD_GRANTED.bits() | d2s::QuestFlags::QUEST_LOG.bits()
    );
    assert_eq!(
        QUEST_COMPLETE_MASK & d2s::QuestFlags::COMPLETED_NOW.bits(),
        0,
        "COMPLETED_NOW 在所有样本里从不出现，不能用来判完成"
    );
    assert_eq!(
        QUEST_COMPLETE_MASK & d2s::QuestFlags::COMPLETED_BEFORE.bits(),
        0,
        "COMPLETED_BEFORE 只在少数槽出现，用它判完成会大量漏判"
    );
}

#[test]
fn complete_one_quest_sets_bits_and_keeps_others() {
    let Some(src) = sample() else {
        eprintln!("跳过：没有样本");
        return;
    };
    let work = work_copy(&src, "complete_one");
    let mut session = EditSession::open(&work).expect("打开");

    // 找一个当前**没有**完成任务位的槽，避免"本来就完成"导致断言无意义
    let before = session.quests();
    let (act, slot) = (0..5)
        .flat_map(|a| {
            let (start, count) = quest_slot_range_of_act(a);
            (start..start + count).map(move |s| (a, s))
        })
        .find(|(a, s)| before[0][*a][*s] & QUEST_COMPLETE_MASK != QUEST_COMPLETE_MASK)
        .expect("应能找到未完成的槽");

    let original = before[0][act][slot];
    session
        .apply(Edit::CompleteQuest {
            difficulty: 0,
            act,
            slot,
        })
        .expect("标记完成");

    let after = session.quests()[0][act][slot];
    assert_eq!(
        after & QUEST_COMPLETE_MASK,
        QUEST_COMPLETE_MASK,
        "完成位都应置上"
    );
    assert_eq!(
        after & !QUEST_COMPLETE_MASK,
        original & !QUEST_COMPLETE_MASK,
        "其它状态位不应被改动（原值 0x{original:04X} → 0x{after:04X}）"
    );

    // 写盘后重新打开，位要还在，且仍逐字节往返
    let (_, ok) = session.verify().expect("写前校验");
    assert!(ok);
    session.save().expect("保存");
    let reopened = EditSession::open(&work).expect("重新打开");
    assert_eq!(
        reopened.quests()[0][act][slot] & QUEST_COMPLETE_MASK,
        QUEST_COMPLETE_MASK
    );
    assert!(
        reopened.file.check_roundtrip().identical,
        "改任务后仍应逐字节往返一致"
    );
}

#[test]
fn clear_quest_zeroes_the_slot() {
    let Some(src) = sample() else { return };
    let work = work_copy(&src, "clear_one");
    let mut session = EditSession::open(&work).expect("打开");

    session
        .apply(Edit::ClearQuest {
            difficulty: 0,
            act: 0,
            slot: 1,
        })
        .expect("清空");
    assert_eq!(session.quests()[0][0][1], 0);

    let (_, ok) = session.verify().expect("写前校验");
    assert!(ok);
}

#[test]
fn complete_all_touches_only_real_quest_slots() {
    let Some(src) = sample() else { return };
    let work = work_copy(&src, "complete_all");
    let mut session = EditSession::open(&work).expect("打开");

    let before = session.quests()[0].clone();
    session
        .apply(Edit::CompleteAllQuests { difficulty: 0 })
        .expect("全部完成");
    let after = session.quests()[0].clone();

    for (act, (before_act, after_act)) in before.iter().zip(after.iter()).enumerate() {
        let (start, count) = quest_slot_range_of_act(act);
        for (slot, (b, a)) in before_act.iter().zip(after_act.iter()).enumerate() {
            let is_real = (start..start + count).contains(&slot);
            if is_real {
                assert_eq!(
                    a & QUEST_COMPLETE_MASK,
                    QUEST_COMPLETE_MASK,
                    "Act{} 槽{slot} 应被标记完成",
                    act + 1
                );
            } else {
                assert_eq!(
                    a,
                    b,
                    "Act{} 槽{slot} 不是真任务槽，不应被改动（0x{b:04X} → 0x{a:04X}）",
                    act + 1
                );
            }
        }
    }

    let (_, ok) = session.verify().expect("写前校验");
    assert!(ok);
}

#[test]
fn out_of_range_quest_indices_are_rejected() {
    let Some(src) = sample() else { return };
    let work = work_copy(&src, "quest_bad");
    let mut session = EditSession::open(&work).expect("打开");

    assert!(session
        .apply(Edit::CompleteQuest {
            difficulty: 0,
            act: 5,
            slot: 1
        })
        .is_err());
    assert!(session
        .apply(Edit::CompleteQuest {
            difficulty: 0,
            act: 0,
            slot: 99
        })
        .is_err());
    assert!(session
        .apply(Edit::CompleteQuest {
            difficulty: 9,
            act: 0,
            slot: 1
        })
        .is_err());
}

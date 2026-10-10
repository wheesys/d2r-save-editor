//! 物品编辑（里程碑 5）集成测试：用真实存档验证改动 + 校验 + 保留其它物品。
//!
//! 样本不入库，找不到时跳过。

use std::path::{Path, PathBuf};

use d2r_core::{EditSession, ItemEdit, SaveFile};

fn samples_root() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("D2R_SAMPLES_DIR") {
        let p = PathBuf::from(p);
        if p.is_dir() {
            return Some(p);
        }
    }
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).parent()?.parent()?;
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
        } else {
            out.push(p);
        }
    }
    out
}

/// 找一个既能逐字节往返、又有可编辑物品（数量或耐久）的 v105 存档。
fn pick_sample() -> Option<PathBuf> {
    let root = samples_root()?;
    let mut candidates: Vec<PathBuf> = walk(&root)
        .into_iter()
        .filter(|p| p.extension().map(|e| e == "d2s").unwrap_or(false))
        .collect();
    candidates.sort();
    for p in candidates {
        let Ok(f) = SaveFile::open(&p) else { continue };
        if !f.check_roundtrip().identical {
            continue;
        }
        let has_editable = f
            .save
            .items
            .iter()
            .any(|i| i.quantity.is_some() || i.durability.is_some());
        if has_editable {
            return Some(p);
        }
    }
    None
}

fn work_copy(src: &Path, tag: &str) -> PathBuf {
    let mut dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    dir.push("items");
    let _ = std::fs::create_dir_all(&dir);
    let dest = dir.join(format!("{tag}.d2s"));
    std::fs::copy(src, &dest).expect("复制样本");
    dest
}

#[test]
fn item_edits_persist_and_verify() {
    let Some(src) = pick_sample() else {
        eprintln!("跳过：找不到合适的样本");
        return;
    };
    let work = work_copy(&src, "item_edits");
    let mut session = EditSession::open(&work).expect("打开");

    let details = session.item_details();
    assert!(!details.is_empty());
    let total_before = details.len();

    // 找一件有数量的物品和一件有耐久的物品
    let qty_idx = details
        .iter()
        .find(|d| d.quantity.is_some())
        .map(|d| d.index);
    let dur_idx = details
        .iter()
        .find(|d| d.max_durability.unwrap_or(0) > 0)
        .map(|d| d.index);

    let mut edits = Vec::new();
    if let Some(i) = qty_idx {
        edits.push(ItemEdit::SetQuantity {
            index: i,
            value: 123,
        });
    }
    if let Some(i) = dur_idx {
        edits.push(ItemEdit::SetDurability { index: i, value: 1 });
    }
    if edits.is_empty() {
        eprintln!("跳过：该样本没有数量/耐久可改");
        return;
    }
    session.apply_items(&edits).expect("应用物品修改");
    assert!(session.is_dirty());

    let (bytes, ok) = session.verify().expect("写前校验");
    assert!(ok && !bytes.is_empty());
    session.save().expect("保存");

    // 重新打开：改动持久化，其它物品数量不变
    let reopened = EditSession::open(&work).expect("重新打开");
    let after = reopened.item_details();
    assert_eq!(after.len(), total_before, "物品数量不应变化");
    if let Some(i) = qty_idx {
        assert_eq!(after[i].quantity, Some(123));
    }
    if let Some(i) = dur_idx {
        assert_eq!(after[i].durability, Some(1));
    }
    // 保存后仍逐字节往返
    assert!(
        reopened.file.check_roundtrip().identical,
        "物品改动后仍应逐字节往返一致"
    );
}

#[test]
fn item_edits_are_atomic_and_bounded() {
    let Some(src) = pick_sample() else { return };
    let work = work_copy(&src, "item_atomic");
    let mut session = EditSession::open(&work).expect("打开");
    let before = session.item_details();

    // 第二条非法（耐久超上限）→ 整批回滚
    let dur = before
        .iter()
        .find(|d| d.max_durability.unwrap_or(0) > 0)
        .map(|d| (d.index, d.max_durability.unwrap()))
        .unwrap_or((0, 0));
    let r = session.apply_items(&[
        ItemEdit::SetDefense {
            index: 0,
            value: 42,
        },
        ItemEdit::SetDurability {
            index: dur.0,
            value: dur.1 + 1000,
        },
    ]);
    assert!(r.is_err(), "超上限必须被拒绝");

    let after = session.item_details();
    assert_eq!(before, after, "整批应回滚");
    assert!(!session.is_dirty(), "回滚后不应有挂起修改");

    // 越界序号也要被拒绝
    assert!(session
        .apply_item(ItemEdit::SetQuantity {
            index: 99999,
            value: 2
        })
        .is_err());
}

#[test]
fn item_stats_summary_matches_list() {
    let Some(src) = pick_sample() else { return };
    let session = EditSession::open(&src).expect("打开");
    let stats = session.item_stats();
    let details = session.item_details();
    assert_eq!(stats.total, details.len());
    let sum: usize = stats.by_page.iter().map(|(_, n)| *n).sum();
    assert_eq!(sum, details.len(), "各存储页数量之和应等于总数");
    let qsum: usize = stats.by_quality.iter().map(|(_, n)| *n).sum();
    assert_eq!(qsum, details.len(), "各品质数量之和应等于总数");
}

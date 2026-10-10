//! 物品孔数编辑（里程碑 5）集成测试：用真实存档验证"孔数 = sockets.len()"这条编码规则。
//!
//! 样本不入库，找不到时跳过。

use std::path::{Path, PathBuf};

use d2r_core::{EditSession, ItemEdit, MAX_SOCKETS};

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
    dir.push("sockets");
    let _ = std::fs::create_dir_all(&dir);
    let dest = dir.join(format!("{tag}.d2s"));
    std::fs::copy(src, &dest).expect("复制样本");
    dest
}

/// 找一件**没有镶嵌物**的物品（可以随便改孔数）。
fn empty_socket_item(session: &EditSession) -> Option<usize> {
    session
        .item_details()
        .into_iter()
        .find(|d| d.socketed_count == 0)
        .map(|d| d.index)
}

#[test]
fn socket_count_roundtrips_through_save() {
    let Some(src) = sample() else {
        eprintln!("跳过：没有样本");
        return;
    };
    let work = work_copy(&src, "roundtrip");
    let mut session = EditSession::open(&work).expect("打开");
    let Some(index) = empty_socket_item(&session) else {
        eprintln!("跳过：没有可用的物品");
        return;
    };
    let before = session.item_detail(index).expect("读明细");

    // 0 孔 → 3 孔
    session
        .apply_item(ItemEdit::SetSockets { index, count: 3 })
        .expect("设置 3 孔");
    let after = session.item_detail(index).expect("读明细");
    assert_eq!(after.socket_count, 3);
    assert_eq!(after.socketed_count, 0, "新孔是空的");
    assert!(
        after.socketed_flag,
        "孔数 > 0 时必须带 SOCKETED 标志，否则字段不会被写入"
    );
    if before.socket_count == 0 {
        assert!(!before.socketed_flag, "原物品无孔时不该带标志");
    }

    // 写盘 → 重新打开：孔数必须保持
    let (_, ok) = session.verify().expect("写前校验");
    assert!(ok);
    session.save().expect("保存");

    let reopened = EditSession::open(&work).expect("重新打开");
    let re = reopened.item_detail(index).expect("读明细");
    assert_eq!(re.socket_count, 3, "孔数应持久化");
    assert!(re.socketed_flag);
    assert!(
        reopened.file.check_roundtrip().identical,
        "改孔数后仍应逐字节往返一致"
    );
}

#[test]
fn socket_bounds_and_protection() {
    let Some(src) = sample() else { return };
    let work = work_copy(&src, "bounds");
    let mut session = EditSession::open(&work).expect("打开");
    let Some(index) = empty_socket_item(&session) else {
        return;
    };
    let snapshot = session.item_detail(index).expect("读明细");

    // 上限：存档里是 3 位字段
    let err = session
        .apply_item(ItemEdit::SetSockets {
            index,
            count: MAX_SOCKETS + 1,
        })
        .unwrap_err();
    assert!(format!("{err}").contains("最多"), "{err}");
    assert_eq!(
        session.item_detail(index).unwrap().socket_count,
        snapshot.socket_count
    );

    // 越界物品号
    assert!(session
        .apply_item(ItemEdit::SetSockets {
            index: 99999,
            count: 1
        })
        .is_err());

    // 设 0 孔：清掉标志
    session
        .apply_item(ItemEdit::SetSockets { index, count: 0 })
        .expect("设为 0 孔");
    let d = session.item_detail(index).expect("读明细");
    assert_eq!(d.socket_count, 0);
    assert!(!d.socketed_flag, "0 孔时应清掉 SOCKETED 标志");

    // 再设回上限
    session
        .apply_item(ItemEdit::SetSockets {
            index,
            count: MAX_SOCKETS,
        })
        .expect("设为上限");
    assert_eq!(
        session.item_detail(index).unwrap().socket_count,
        MAX_SOCKETS
    );
    let (_, ok) = session.verify().expect("校验");
    assert!(ok);
}

#[test]
fn reducing_below_filled_is_rejected() {
    let Some(src) = sample() else { return };
    let work = work_copy(&src, "filled");
    let mut session = EditSession::open(&work).expect("打开");
    // 找一件**已经镶了东西**的物品
    let Some(d) = session
        .item_details()
        .into_iter()
        .find(|d| d.socketed_count > 0)
    else {
        eprintln!("跳过：样本里没有已镶嵌的物品");
        return;
    };
    let index = d.index;
    let filled = d.socketed_count;

    let err = session
        .apply_item(ItemEdit::SetSockets {
            index,
            count: filled - 1,
        })
        .unwrap_err();
    assert!(format!("{err}").contains("已经镶了"), "{err}");
    assert_eq!(
        session.item_detail(index).unwrap().socketed_count,
        filled,
        "被拒后镶嵌物不能丢"
    );

    // 加空孔是允许的（只要不超过上限）
    if filled < MAX_SOCKETS {
        session
            .apply_item(ItemEdit::SetSockets {
                index,
                count: filled + 1,
            })
            .expect("加一个空孔");
        let d = session.item_detail(index).unwrap();
        assert_eq!(d.socket_count, filled + 1);
        assert_eq!(d.socketed_count, filled, "原镶嵌物应保留");
    }
}

//! 结构性改动（加孔数会改变文件长度）之后，写出的文件是否**自洽**。
//!
//! 为什么不能只看逐字节往返：加一个孔会让物品数据变长，后面所有字节位移、
//! 文件长度与校验和都必须重算，所以"逐字节一致"对这类编辑本来就不适用。
//! 该验的是：长度字段对得上、校验和能通过、重新解析后改动还在。
use d2r_core::items::ItemTarget;
use d2r_core::{EditSession, ItemEdit, SaveFile};

fn work(tag: &str) -> std::path::PathBuf {
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let mut dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    dir.push("sockets");
    let _ = std::fs::create_dir_all(&dir);
    let dest = dir.join(format!("{tag}.d2s"));
    std::fs::copy(repo.join("samples/saves/d2r-105/杜仲.d2s"), &dest).unwrap();
    dest
}

fn assert_self_consistent(path: &std::path::Path, tag: &str) {
    let bytes = std::fs::read(path).unwrap();
    // 头部偏移 8 是文件长度
    let declared = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]) as usize;
    println!(
        "{tag}: 文件 {} 字节，头部声明 {} 字节",
        bytes.len(),
        declared
    );
    assert_eq!(
        declared,
        bytes.len(),
        "{tag}: 头部声明的长度必须等于实际长度"
    );
    assert!(
        d2s::checksum::verify(&bytes),
        "{tag}: 写出的文件校验和必须能通过"
    );
    // 重新解析：改动要还在
    assert!(SaveFile::open(path).is_ok(), "{tag}: 自己必须能重新打开");
}

#[test]
fn socket_change_produces_valid_file() {
    let path = work("valid");
    let mut s = EditSession::open(&path).unwrap();
    let before = s.mercenary_items()[0].socket_count;
    s.apply_item_on(
        ItemTarget::Mercenary,
        ItemEdit::SetSockets {
            index: 0,
            count: before + 2,
        },
    )
    .unwrap();
    let (_, ok) = s.verify().unwrap();
    assert!(ok, "写前校验应通过");
    s.save().unwrap();

    let re = EditSession::open(&path).unwrap();
    assert_eq!(
        re.mercenary_items()[0].socket_count,
        before + 2,
        "加上的孔数应落盘"
    );
    assert_self_consistent(&path, "佣兵加孔");
}

#[test]
fn socket_change_on_inventory_produces_valid_file() {
    let path = work("valid_inv");
    let mut s = EditSession::open(&path).unwrap();
    let idx = s
        .item_details()
        .iter()
        .position(|d| d.socket_count == 0)
        .expect("找一个没孔的物品");
    s.apply_item(ItemEdit::SetSockets {
        index: idx,
        count: 3,
    })
    .unwrap();
    let (_, ok) = s.verify().unwrap();
    assert!(ok);
    s.save().unwrap();

    let re = EditSession::open(&path).unwrap();
    assert_eq!(re.item_detail(idx).unwrap().socket_count, 3);
    assert_self_consistent(&path, "物品栏加孔");
}

#[test]
fn non_structural_edit_keeps_file_length() {
    // 对照：改耐久不改长度，所以长度必须一字不差（长度一变说明写路径有噪声）。
    // 注意这里**不**断言逐字节一致——内容改了，校验和必然跟着变。
    let path = work("identity");
    let mut s = EditSession::open(&path).unwrap();
    s.apply_item(ItemEdit::SetDurability { index: 0, value: 1 })
        .unwrap();
    let rt = s.file.check_roundtrip();
    assert_eq!(rt.original_len, rt.rewritten_len, "改耐久不应改变文件长度");
    assert_self_consistent(&path, "改耐久前（未落盘，仅结构）");
}

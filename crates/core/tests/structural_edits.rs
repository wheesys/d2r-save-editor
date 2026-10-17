//! 结构性改动（加孔数会改变文件长度）之后，写出的文件是否**自洽**。
//!
//! 为什么不能只看逐字节往返：加一个孔会让物品数据变长，后面所有字节位移、
//! 文件长度与校验和都必须重算，所以"逐字节一致"对这类编辑本来就不适用。
//! 该验的是：长度字段对得上、校验和能通过、重新解析后改动还在。
use d2r_core::items::ItemTarget;
use d2r_core::{EditSession, ItemEdit, SaveFile};

/// 找一个可用的样本存档；**没有样本时返回 `None` 让测试跳过**。
///
/// 这些测试需要真实存档，而样本文件**不入库**（见 `.gitignore`），
/// 所以 CI 上一定没有。逐个测试都做跳过判断，不要在 `copy().unwrap()`
/// 上 panic——那会让整个 CI 红掉，而实际代码并没有问题。
///
/// 优先用 `D2R_SAMPLES_DIR`，其次在仓库的 `samples/saves/` 里挑一个存在的。
fn sample_save() -> Option<std::path::PathBuf> {
    let candidates = |dir: &std::path::Path| -> Vec<std::path::PathBuf> {
        ["杜仲.d2s", "小五.d2s", "杜老五.d2s", "杜老二.d2s"]
            .iter()
            .map(|n| dir.join("d2r-105").join(n))
            .collect()
    };
    if let Ok(dir) = std::env::var("D2R_SAMPLES_DIR") {
        if let Some(p) = candidates(std::path::Path::new(&dir))
            .into_iter()
            .find(|p| p.is_file())
        {
            return Some(p);
        }
    }
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()?
        .parent()?;
    candidates(&repo.join("samples").join("saves"))
        .into_iter()
        .find(|p| p.is_file())
}

/// 复制样本到可写目录；没有样本时返回 `None`。
fn work(tag: &str) -> Option<std::path::PathBuf> {
    let src = sample_save()?;
    let mut dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    dir.push("sockets");
    let _ = std::fs::create_dir_all(&dir);
    let dest = dir.join(format!("{tag}.d2s"));
    std::fs::copy(&src, &dest).ok()?;
    Some(dest)
}

/// 跳过测试时统一打印一句，方便在日志里看出"不是失败，是没样本"。
macro_rules! skip_without_sample {
    ($tag:expr) => {
        match work($tag) {
            Some(p) => p,
            None => {
                eprintln!("跳过：没有样本存档（样本不入库，CI 上属正常）");
                return;
            }
        }
    };
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
    let path = skip_without_sample!("valid");
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
    let path = skip_without_sample!("valid_inv");
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
    let path = skip_without_sample!("identity");
    let mut s = EditSession::open(&path).unwrap();
    s.apply_item(ItemEdit::SetDurability { index: 0, value: 1 })
        .unwrap();
    let rt = s.file.check_roundtrip();
    assert_eq!(rt.original_len, rt.rewritten_len, "改耐久不应改变文件长度");
    assert_self_consistent(&path, "改耐久前（未落盘，仅结构）");
}

#[test]
fn rename_adds_personalized_name_and_grows_record() {
    // 重命名会往物品记录里**追加一个字符串**，属于改长度的编辑：
    // 后面所有字节都位移，所以必须重新算头部长度与校验和，否则文件报废。
    let path = skip_without_sample!("rename");
    let mut s = EditSession::open(&path).unwrap();
    let before = s.file.check_roundtrip();
    s.apply_item(ItemEdit::RenameItem {
        index: 0,
        name: "上帝法印".to_string(),
    })
    .unwrap();
    let (_, ok) = s.verify().unwrap();
    assert!(ok, "写前校验应通过");
    s.save().unwrap();

    let re = EditSession::open(&path).unwrap();
    assert_eq!(
        re.item_detail(0).unwrap().personalized_name,
        "上帝法印",
        "自定义名字应落盘"
    );
    let after = re.file.check_roundtrip();
    assert!(
        after.original_len > before.original_len,
        "加了名字，记录变长，文件总长应变大"
    );
    assert_self_consistent(&path, "重命名");
}

#[test]
fn clearing_rename_removes_flag_and_shrinks_back() {
    // 清除命名要**同时**清字符串与 PERSONALIZED 标志：
    // 只清字符串会写出一个空字符串段，物品记录仍带着"已命名"的位。
    let path = skip_without_sample!("rename_clear");
    let mut s = EditSession::open(&path).unwrap();
    let baseline = s.file.check_roundtrip().original_len;
    s.apply_item(ItemEdit::RenameItem {
        index: 0,
        name: "临时".to_string(),
    })
    .unwrap();
    s.apply_item(ItemEdit::RenameItem {
        index: 0,
        name: String::new(),
    })
    .unwrap();
    s.save().unwrap();

    let re = EditSession::open(&path).unwrap();
    assert_eq!(re.item_detail(0).unwrap().personalized_name, "");
    assert_eq!(
        re.file.check_roundtrip().original_len,
        baseline,
        "清除命名后应回到原长度"
    );
    assert_self_consistent(&path, "清除命名");
}

#[test]
fn rename_rejects_overlong_name() {
    // 游戏侧的字名有长度上限，超长会被截断；这里先拦，不写坏数据。
    let path = skip_without_sample!("rename_long");
    let mut s = EditSession::open(&path).unwrap();
    let err = s
        .apply_item(ItemEdit::RenameItem {
            index: 0,
            name: "一二三四五六七八九十一二三四五六".to_string(),
        })
        .unwrap_err();
    assert!(err.to_string().contains("15"), "应提示长度上限：{err}");
}

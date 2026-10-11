//! 逐条核对每个属性的有效值范围（真实数据）。
//!
//! 用户明确要求："不同属性的有效值和保存值不一样，都有数一下"。
//! 所以这里对 `ItemStatCost.txt` 里**每一个**属性算一遍范围，并且：
//!
//! 1. 把结果写成一份报告 `target/stat-ranges.txt`（便于直接翻看）；
//! 2. 断言几条已知属性的范围（拿数据表交叉验证公式没写反）；
//! 3. 断言带移位（`ValShift`）的属性最小有效值**不是 0**——
//!    因为 d2s 写盘时 `shifted == 0` 的属性会被整个跳过，属性会消失。

use d2r_data::statlimit::StatLimits;
use d2r_data::{Strings, TableStore};

fn load_store() -> Option<TableStore> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()?
        .join("app")
        .join("data-bundle")
        .join("excel");
    if !dir.is_dir() {
        return None;
    }
    let mut store = TableStore::empty();
    store.add_override_dir(dir);
    Some(store)
}

fn limits() -> Option<StatLimits> {
    let mut store = load_store()?;
    let limits = StatLimits::from_store(&mut store);
    if limits.is_empty() {
        return None;
    }
    Some(limits)
}

/// 算出全部属性的范围并写出报告，同时返回 (属性数, 带移位的数量)。
fn write_report() -> Option<(usize, usize, usize)> {
    let mut store = load_store()?;
    let table = store.get("ItemStatCost")?.clone();
    let limits = StatLimits::from_store(&mut store);
    if limits.is_empty() {
        return None;
    }
    let name_of = |id: i64| -> String {
        for r in 0..table.len() {
            if table.int(r, "*ID") == Some(id) {
                return table.cell(r, "Stat").unwrap_or("?").to_string();
            }
        }
        "?".into()
    };

    let mut out =
        String::from("属性 ID\t内部名\t最小值\t最大值\t步长\t位宽\t偏移\t移位\t参数位\t编码\n");
    let mut shifted = 0usize;
    let mut with_encoding = 0usize;
    for r in 0..table.len() {
        let Some(id) = table.int(r, "*ID").or_else(|| table.int(r, "ID")) else {
            continue;
        };
        let Some(limit) = limits.get(id as u16) else {
            continue;
        };
        if limit.val_shift > 0 {
            shifted += 1;
        }
        if limit.has_encoding {
            with_encoding += 1;
        }
        out.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            id,
            name_of(id),
            limit.min(),
            limit.max(),
            limit.step(),
            limit.save_bits,
            limit.save_add,
            limit.val_shift,
            limit.has_param_bits,
            limit.has_encoding,
        ));
    }

    let count = out.lines().count() - 1;
    if let Some(dir) = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(|p| p.join("..").join("target"))
    {
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(dir.join("stat-ranges.txt"), &out);
    }
    Some((count, shifted, with_encoding))
}

#[test]
fn every_stat_gets_a_range() {
    let Some((count, shifted, encoded)) = write_report() else {
        eprintln!("跳过：没有数据包");
        return;
    };
    // 报告写在 target/stat-ranges.txt，方便直接翻看
    eprintln!("已算出 {count} 个属性的范围（带移位 {shifted} 个，带编码 {encoded} 个）");
    assert!(count > 200, "只算出 {count} 个属性的范围，数据表应该更多");
}

/// 拿数据表里的确定值交叉验证范围公式（防止位宽/偏移抄错）。
#[test]
fn known_stat_ranges_match_data() {
    let Some(limits) = limits() else {
        eprintln!("跳过：没有数据包");
        return;
    };

    // 力量：Save Bits 8 / Save Add 32 / 无移位 → -32..223
    let s = limits.get(0).expect("应有力量(*ID 0)");
    assert_eq!((s.min(), s.max()), (-32, 223), "力量范围不对: {s:?}");

    // 生命：Save Bits 9 / Save Add 32 / ValShift 8 → 步长 256，位宽上限 479 → 上限 479*256
    let hp = limits.get(7).expect("应有生命(*ID 7)");
    assert_eq!(hp.val_shift, 8, "生命应有 ValShift 8");
    assert_eq!(hp.step(), 256);
    assert_eq!(hp.min(), 256, "0 会被写侧跳过，最小有效值是 256");
    assert_eq!(hp.max(), 479 << 8);

    // 所有技能：Save Bits 3 / Save Add 0 → 0..7
    let sk = limits.get(127).expect("应有所有技能(*ID 127)");
    assert_eq!((sk.min(), sk.max()), (0, 7), "所有技能应是 0..7");
}

/// 带移位的属性，最小有效值必须 > 0，否则用户填 0 时属性会整体消失。
#[test]
fn shifted_stats_never_allow_zero() {
    let Some((count, shifted, _)) = write_report() else {
        eprintln!("跳过：没有数据包");
        return;
    };
    assert!(count > 0);
    if shifted == 0 {
        eprintln!("注意：数据里没有带移位的属性（预期应有 生命/法力/耐力 等）");
        return;
    }
    let Some(limits) = limits() else {
        return;
    };
    let ptr = &limits;
    let mut bad = Vec::new();
    for id in 0u16..=u16::MAX {
        let Some(l) = ptr.get(id) else { break };
        if l.val_shift > 0 && l.min() <= 0 {
            bad.push(format!(
                "ID {id}: val_shift={} 但 min={}",
                l.val_shift,
                l.min()
            ));
        }
    }
    assert!(bad.is_empty(), "带移位的属性允许了 0：\n{}", bad.join("\n"));
}

/// 空表（没有数据表时）不应限制任何东西，免得把用户卡死。
#[test]
fn empty_limits_allow_everything() {
    let limits = StatLimits::empty();
    assert!(limits.get(7).is_none());
    assert!(limits.check(7, 123456).is_ok());
    assert!(limits.check(0, -99999).is_ok());
}

/// `Strings` 只是为了让本测试文件与其它数据测试的依赖一致，避免未使用告警。
#[test]
fn strings_type_is_available() {
    let s = Strings::default();
    let _ = s;
}

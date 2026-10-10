//! 传送点（小站）名称解析：`Levels.txt` 的 `Waypoint` 列 → 关卡名 → 中文名。
//!
//! 这套编号是**存档里那 39 个位的下标**，所以必须保证：
//! 1. `Waypoint` 列取值是 0..=38 连续无缺失（否则界面按序号取名字会错位）；
//! 2. 每个 `LevelName` 都能在文本表里解析出中文名（否则界面只能显示英文/数字）。

use d2r_data::{Strings, TableStore};

/// 加载内置数据包里的表与文本表（数据包缺失时返回 `None`）。
fn load() -> Option<(TableStore, Strings)> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()?
        .join("app")
        .join("data-bundle");
    if !dir.join("excel").is_dir() {
        return None;
    }
    let mut store = TableStore::empty();
    for group in ["excel", "strings"] {
        if let Ok(rd) = std::fs::read_dir(dir.join(group)) {
            for e in rd.flatten() {
                let _ = e;
            }
        }
    }
    // 表与文本表都用数据包里的（与运行时同一来源）
    store.add_override_dir(dir.join("excel"));
    let mut strings = Strings::default();
    for e in std::fs::read_dir(dir.join("strings")).ok()?.flatten() {
        if let Ok(text) = std::fs::read_to_string(e.path()) {
            strings.merge(Strings::from_json_bytes(text.as_bytes()));
        }
    }
    Some((store, strings))
}

#[test]
fn waypoint_indices_are_contiguous_and_named() {
    let Some((mut store, strings)) = load() else {
        eprintln!("跳过：没有数据包");
        return;
    };
    let levels = store.get("Levels").expect("应有 Levels 表");

    // 收集 Waypoint 列（0..=38）对应的关卡名。
    // 注意：表里没有小站的行是 `255`，空白单元格会被解析成别的值，
    // 所以必须**显式限制在 0..=38**——存档里那 39 个位就是这个范围。
    let mut named: Vec<(i64, String)> = Vec::new();
    for row in 0..levels.len() {
        let Some(wp) = levels.int(row, "Waypoint") else {
            continue;
        };
        if !(0..=38).contains(&wp) {
            continue;
        }
        let Some(name) = levels.cell_nonempty(row, "LevelName") else {
            continue;
        };
        named.push((wp, name.to_string()));
    }
    named.sort();
    assert_eq!(named.len(), 39, "小站数量应为 39，实际 {}", named.len());

    // 编号必须连续无缺失，且从 0 开始——存档里的位下标就是它
    for (i, (wp, name)) in named.iter().enumerate() {
        assert_eq!(
            *wp, i as i64,
            "第 {i} 个的 Waypoint 值应为 {i}，实际 {wp}（{name}）"
        );
    }

    // 每个关卡名都要能解析出名字（中文优先）
    let mut unresolved = Vec::new();
    for (wp, name) in &named {
        match strings.text(name) {
            Some(text) if !text.is_empty() => {}
            _ => unresolved.push(format!("wp={wp} {name}")),
        }
    }
    assert!(
        unresolved.is_empty(),
        "有 {} 个小站解析不出名字：{unresolved:?}",
        unresolved.len()
    );

    // 抽查几个已知名字
    let name_of = |idx: i64| -> String {
        let (_, name) = named.iter().find(|(w, _)| *w == idx).expect("索引存在");
        strings.text(name).unwrap_or_default().to_string()
    };
    assert!(
        name_of(0).contains("流亡者营地") || name_of(0) == "流亡者营地",
        "0 号：{}",
        name_of(0)
    );
    println!("0 号小站 = {}", name_of(0));
    println!("16 号小站 = {}", name_of(16));
    println!("38 号小站 = {}", name_of(38));
}

//! 文件系统相关测试放在集成测试里：`CARGO_TARGET_TMPDIR` 指向构建目录下的可写临时区，
//! 比 `%TEMP%` 更可靠（不依赖用户临时目录权限）。

use std::path::PathBuf;

use d2r_data::{ModInfo, TableStore};

fn tmp_dir(tag: &str) -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    p.push(tag);
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).expect("创建临时目录");
    p
}

#[test]
fn modinfo_parses_fengtu_style_json() {
    let dir = tmp_dir("modinfo");
    let f = dir.join("modinfo.json");
    std::fs::write(&f, br#"{ "name": "fengtu2", "savepath": "../" }"#).unwrap();
    let m = ModInfo::from_file(&f).unwrap();
    assert_eq!(m.name, "fengtu2");
    assert_eq!(m.savepath.as_deref(), Some("../"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn table_store_lists_and_caches() {
    let dir = tmp_dir("tablestore");
    std::fs::write(dir.join("inventory.txt"), "index\tinvwidth\n0\t1\n").unwrap();
    std::fs::write(dir.join("ItemStatCost.txt"), "Stat\tSave Bits\n0\t9\n").unwrap();
    std::fs::write(dir.join("notes.md"), "ignored").unwrap();

    let mut store = TableStore::new(&dir);
    // 只列 .txt，名字小写化并排序
    assert_eq!(
        store.available(),
        vec!["inventory".to_string(), "itemstatcost".to_string()]
    );
    // 大小写不敏感查找
    assert_eq!(
        store.get("ItemStatCost").unwrap().int(0, "save bits"),
        Some(9)
    );
    assert_eq!(store.get("inventory").unwrap().int(0, "invwidth"), Some(1));
    // 缺失返回 None（并缓存该结果）
    assert!(store.get("MagicPrefix").is_none());
    assert!(store.get("MagicPrefix").is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn mod_scan_finds_mods_directory() {
    let game = tmp_dir("game");
    let mod_dir = game.join("mods").join("fengtu2");
    std::fs::create_dir_all(&mod_dir).unwrap();
    std::fs::write(
        mod_dir.join("modinfo.json"),
        br#"{ "name": "fengtu2", "savepath": "../" }"#,
    )
    .unwrap();

    let found = ModInfo::scan(&game);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].1.name, "fengtu2");
    let _ = std::fs::remove_dir_all(&game);
}

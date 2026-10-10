//! 物品名解析测试（需要可写临时目录，故放在集成测试里）。

use std::path::PathBuf;

use d2r_data::{ItemNamer, Strings, TableStore};

const STRINGS: &[u8] = br#"[
  { "id": 1, "Key": "rvl", "zhCN": "\u5b8c\u5168\u590d\u82cf\u836f\u6c34", "enUS": "Full Rejuvenation Potion" },
  { "id": 2, "Key": "The Gnasher", "zhCN": "\u5916\u58f3\u6495\u54ac", "enUS": "The Gnasher" },
  { "id": 3, "Key": "Hiquality", "zhCN": "\u7cbe\u826f", "enUS": "Superior" },
  { "id": 4, "Key": "Cracked", "zhCN": "\u88c2\u7eb9", "enUS": "Cracked" }
]"#;

fn tmp_dir(tag: &str) -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    p.push(tag);
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).expect("创建临时目录");
    p
}

fn setup(tag: &str) -> (Strings, TableStore) {
    let strings = Strings::from_json_bytes(STRINGS);
    let dir = tmp_dir(tag);
    // 与真实表一致：UniqueItems 第 1 列 index 是字符串名，第 2 列才是数字 *ID
    std::fs::write(
        dir.join("UniqueItems.txt"),
        "index\t*ID\tcode\nThe Gnasher\t0\thax\nSecond Unique\t1\taxe\n",
    )
    .unwrap();
    // Sets 第 1 列 index 本身就是字符串名（真实文件即如此）
    std::fs::write(
        dir.join("Sets.txt"),
        "index\tname\nAngelical Raiment\tAngelical Raiment\n",
    )
    .unwrap();
    (strings, TableStore::new(dir))
}

#[test]
fn resolves_normal_item_by_code() {
    let (strings, mut tables) = setup("resolves_normal_item_by_code");
    let mut n = ItemNamer::new(&strings, &mut tables, "zhCN");
    let r = n.resolve("rvl", "Normal", None);
    assert_eq!(r.base, "完全复苏药水");
    assert!(r.resolved);
    assert_eq!(r.note, None);
}

#[test]
fn resolves_unique_through_table_index() {
    let (strings, mut tables) = setup("resolves_unique_through_table_index");
    let mut n = ItemNamer::new(&strings, &mut tables, "zhCN");
    // UniqueItems 第 0 行的 index = "The Gnasher" → 字符串表里有
    let r = n.resolve("cm1", "Unique", Some(0));
    assert_eq!(r.base, "外壳撕咬");
    assert!(r.resolved);
    // 第 1 行的 index 不在字符串表里，且 cm1 也没底材名 → 回退代码并给线索
    let r2 = n.resolve("cm1", "Unique", Some(1));
    assert_eq!(r2.base, "cm1");
    assert!(!r2.resolved);
    assert!(r2.note.unwrap().contains("查不到名字"));
}

#[test]
fn unique_falls_back_to_base_name_when_index_unknown() {
    let (strings, mut tables) = setup("unique_falls_back_to_base_name_when_index_unknown");
    let mut n = ItemNamer::new(&strings, &mut tables, "zhCN");
    // Some(1) 的名字查不到，但 rvl 有底材名 → 用底材名，且不该报 note
    let r = n.resolve("rvl", "Unique", Some(1));
    assert_eq!(r.base, "完全复苏药水");
    assert!(r.resolved);
    assert_eq!(r.note, None);
}

#[test]
fn quality_prefix_is_localized() {
    let (strings, mut tables) = setup("quality_prefix_is_localized");
    let mut n = ItemNamer::new(&strings, &mut tables, "zhCN");
    let r = n.resolve("rvl", "Superior", None);
    assert_eq!(r.quality_prefix.as_deref(), Some("精良的"));
    assert_eq!(r.display(), "精良的完全复苏药水");

    let r2 = n.resolve("rvl", "Inferior", Some(2)); // Cracked
    assert_eq!(r2.display(), "裂纹的完全复苏药水");
}

#[test]
fn language_falls_back_to_preferred_chain() {
    let (strings, mut tables) = setup("language_falls_back_to_preferred_chain");
    let mut n = ItemNamer::new(&strings, &mut tables, "deDE"); // 表里没有德语
    let r = n.resolve("rvl", "Normal", None);
    assert_eq!(r.base, "完全复苏药水", "应回退到 zhCN→zhTW→enUS");
}

#[test]
fn missing_string_is_reported_not_faked() {
    let (strings, mut tables) = setup("missing_string_is_reported_not_faked");
    let mut n = ItemNamer::new(&strings, &mut tables, "zhCN");
    let r = n.resolve("zzz", "Normal", None);
    assert_eq!(r.base, "zzz");
    assert!(!r.resolved);
    assert!(r.note.unwrap().contains("zzz"));
}

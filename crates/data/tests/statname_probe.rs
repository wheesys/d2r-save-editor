use d2r_data::{StatNamer, Strings, TableStore};

#[test]
fn print_names() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("app")
        .join("data-bundle");
    if !dir.join("excel").is_dir() {
        eprintln!("跳过：没有数据包");
        return;
    }
    let mut store = TableStore::empty();
    store.add_override_dir(dir.join("excel"));
    let mut strings = Strings::default();
    for e in std::fs::read_dir(dir.join("strings")).unwrap().flatten() {
        if let Ok(t) = std::fs::read_to_string(e.path()) {
            strings.merge(Strings::from_json_bytes(t.as_bytes()));
        }
    }
    let table = store.get("ItemStatCost").unwrap();
    let n = StatNamer::new(table, &strings);
    println!("属性 {}/{} 个解析出显示名", n.named().len(), n.len());
    for id in [0u16, 1, 2, 3, 7, 27, 31, 39, 41, 43, 76, 89, 216] {
        match n.get(id) {
            Some(s) => println!(
                "  ID {id:>4}  {:<28} → {:<20} 步长 {:<6} 参数位 {}",
                s.internal, s.display, s.step, s.has_param_bits
            ),
            None => println!("  ID {id:>4}  （无）"),
        }
    }
}

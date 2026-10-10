//! 图标接口自测：确认数据包里的图标表可用、能编成 PNG data URL。

#[test]
fn icons_are_available() {
    let icons = d2r_app::bundle::item_icons();
    if !d2r_app::bundle::available() {
        eprintln!("跳过：没有数据包");
        return;
    }
    assert!(icons.len() > 250, "图标数量偏少：{}", icons.len());
    assert!(icons.iter().any(|(c, _)| c == "hp1"), "应有治疗药水图标");
    assert!(
        icons.iter().all(|(_, url)| url.starts_with("data:image/png;base64,")),
        "图标应是 PNG data URL"
    );
    println!("图标 {} 张，例如 hp1 -> {} 字节", icons.len(), icons[0].1.len());
}

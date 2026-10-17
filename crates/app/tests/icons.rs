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
        icons
            .iter()
            .all(|(_, url)| url.starts_with("data:image/png;base64,")),
        "图标应是 PNG data URL"
    );
    println!(
        "图标 {} 张，例如 hp1 -> {} 字节",
        icons.len(),
        icons[0].1.len()
    );
}

#[test]
fn skill_icons_are_available() {
    if !d2r_app::bundle::available() {
        eprintln!("跳过：没有数据包");
        return;
    }
    for class in ["ama", "sor", "nec", "pal", "bar", "dru", "ass", "war"] {
        for icon in 1..=30u8 {
            let url = d2r_app::bundle::skill_icon(class, icon)
                .unwrap_or_else(|| panic!("缺少技能图标 {class}-{icon:02}"));
            assert!(url.starts_with("data:image/png;base64,"));
        }
    }
    // 0 是空格标记，不应有对应文件
    assert!(d2r_app::bundle::skill_icon("ama", 0).is_none());
    assert!(d2r_app::bundle::skill_icon("nope", 1).is_none());
}

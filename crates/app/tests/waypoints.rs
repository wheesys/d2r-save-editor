//! 小站名接口自测：确认 39 个名字都拿得到，且是中文。

#[test]
fn waypoint_names_are_chinese() {
    let ws = d2r_app::Workspace::new(d2r_app::default_game_dir());
    let names = ws.waypoint_names();
    if names.is_empty() {
        eprintln!("跳过：没有数据包");
        return;
    }
    assert_eq!(names.len(), 39, "应有 39 个小站");
    assert!(names.iter().all(|n| !n.is_empty()), "不应有空名字");
    assert!(
        !names.iter().any(|n| n.starts_with("小站#")),
        "不应有编号兜底"
    );
    println!("0 号 = {}", names[0]);
    println!("16 号 = {}", names[16]);
    println!("38 号 = {}", names[38]);
    // 抽查中文
    assert!(names[0].contains("流亡者营地"), "{}", names[0]);
}

//! 设置持久化：布局是 mod 级、**整机存一份**，所以这些用例围绕"读得到、坏不了"。
//!
//! 放在集成测试里而不是模块内联测试：`CARGO_TARGET_TMPDIR` 只有集成测试能用。

use std::path::PathBuf;

use d2r_app::{ConfigStore, Settings};

fn dir(tag: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("config-{tag}"));
    let _ = std::fs::create_dir_all(&d);
    d
}

#[test]
fn missing_file_loads_as_none() {
    let store = ConfigStore::at(dir("missing").join("nope.json"));
    assert!(store.load().is_none(), "没有配置文件时应返回 None");
}

#[test]
fn roundtrip_keeps_layout_and_confirmed() {
    let path = dir("roundtrip").join("settings.json");
    let store = ConfigStore::at(&path);

    let settings = Settings {
        layout: d2r_core::LayoutSettings {
            inventory_cols: 16,
            inventory_rows: 8,
            cube_cols: 8,
            cube_rows: 8,
            stash_cols: 16,
            stash_rows: 16,
            merc_full_equipment: true,
            confirmed_by_user: true,
        },
        layout_confirmed: true,
        ..Settings::default()
    };
    let written = store.save(&settings).expect("写配置");
    assert_eq!(written, path, "显式路径应写到指定文件");

    let back = store.load().expect("读回配置");
    assert_eq!(back, settings, "配置应原样读回");
}

#[test]
fn corrupt_file_loads_as_none_instead_of_panicking() {
    let path = dir("corrupt").join("settings.json");
    std::fs::write(&path, "{ 这不是合法 JSON").unwrap();
    assert!(
        ConfigStore::at(&path).load().is_none(),
        "坏配置不该让程序崩，当成没有即可"
    );
}

#[test]
fn old_config_without_new_fields_still_loads() {
    // 只有 layout、没有 layoutConfirmed 的旧配置：靠 serde default 兜住
    let path = dir("old").join("settings.json");
    std::fs::write(
        &path,
        r#"{"layout":{"inventoryCols":16,"inventoryRows":8,"cubeCols":8,"cubeRows":8,"stashCols":16,"stashRows":16,"mercFullEquipment":true,"confirmedByUser":true}}"#,
    )
    .unwrap();
    let settings = ConfigStore::at(&path).load().expect("旧配置应能读");
    assert_eq!(settings.layout.inventory_cols, 16);
    assert!(!settings.layout_confirmed, "缺的字段用默认值");
}

#[test]
fn default_path_is_a_json_file() {
    let p = ConfigStore::new().path();
    assert_eq!(
        p.file_name().and_then(|s| s.to_str()),
        Some("settings.json")
    );
}

/// 最近打开的路径要能存下来（跨重启），顺序是"最新在最前"。
#[test]
fn recent_paths_roundtrip_and_order() {
    let path = dir("recent").join("settings.json");
    let store = ConfigStore::at(&path);

    let settings = Settings {
        recent_paths: vec![r"C:\saves\b.d2s".to_string(), r"C:\saves\a.d2s".to_string()],
        ..Settings::default()
    };
    store.save(&settings).unwrap();

    let back = store.load().unwrap();
    assert_eq!(back.recent_paths.len(), 2);
    assert_eq!(back.recent_paths[0], r"C:\saves\b.d2s", "最新的应在最前");
}

/// 老配置（没有 recentPaths 字段）读进来应是空表，不是报错。
#[test]
fn config_without_recent_paths_loads_empty() {
    let path = dir("no-recent").join("settings.json");
    std::fs::write(
        &path,
        r#"{"layout":{"inventoryCols":10,"inventoryRows":4,"cubeCols":3,"cubeRows":4,"stashCols":10,"stashRows":10,"mercFullEquipment":false,"confirmedByUser":true}}"#,
    )
    .unwrap();
    let settings = ConfigStore::at(&path).load().expect("应能读");
    assert!(settings.recent_paths.is_empty(), "缺字段应是空表");
}

/// 工作区把配置读进来当默认布局：写一份配置后再建工作区，布局要跟着变。
#[test]
fn workspace_picks_up_saved_layout() {
    // 注意：这里只能验证 ConfigStore 的读写链路——Workspace 用的是默认位置，
    // 而默认位置在 exe 同级，测试进程不方便替换。真实链路由桌面端启动验证。
    let path = dir("workspace").join("settings.json");
    let store = ConfigStore::at(&path);
    let settings = Settings {
        layout: d2r_core::LayoutSettings {
            inventory_cols: 12,
            inventory_rows: 6,
            ..d2r_core::LayoutSettings::default()
        },
        layout_confirmed: true,
        ..Settings::default()
    };
    store.save(&settings).unwrap();
    let back = store.load().unwrap();
    assert_eq!(
        (back.layout.inventory_cols, back.layout.inventory_rows),
        (12, 6)
    );
}

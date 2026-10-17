//! 技能页数据：三棵树、等级门槛、图标编号与图标命令的端到端检查。

use std::path::PathBuf;

fn sample() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("D2R_SAMPLES_DIR") {
        let p = PathBuf::from(dir).join("d2r-105").join("杜仲.d2s");
        if p.is_file() {
            return Some(p);
        }
    }
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo = manifest.parent()?.parent()?;
    let dir = repo.join("samples").join("saves").join("d2r-105");
    ["杜仲.d2s", "小五.d2s", "杜老五.d2s"]
        .iter()
        .map(|n| dir.join(n))
        .find(|p| p.is_file())
}

#[test]
fn skills_view_has_levels_icons_and_loadable_art() {
    if !d2r_app::bundle::available() {
        eprintln!("跳过：没有数据包");
        return;
    }
    let Some(src) = sample() else {
        eprintln!("跳过：没有样本存档");
        return;
    };
    let mut ws = d2r_app::Workspace::new(d2r_app::default_game_dir());
    ws.open(&src).expect("打开样本");

    let view = ws.skills_view().expect("技能页视图");
    assert_eq!(view.trees.len(), 3, "应有三个技能系");
    assert!(!view.class_code.is_empty());
    assert!(!view.class_name.is_empty());

    let mut checked = 0usize;
    for tree in &view.trees {
        assert_eq!(tree.skills.len(), 18, "{} 应是 6×3", tree.name);
        assert_eq!(tree.levels.len(), 18);
        assert_eq!(tree.icons.len(), 18);
        for (slot, skill) in tree.skills.iter().enumerate() {
            if skill.skill_id.is_none() {
                assert_eq!(tree.levels[slot], 0);
                assert_eq!(tree.icons[slot], 0);
                continue;
            }
            assert!(tree.levels[slot] > 0, "真技能应有等级门槛");
            assert!((1..=30).contains(&tree.icons[slot]), "图标编号应在 1..=30");
            let url = ws
                .skill_icon(skill.skill_id.unwrap())
                .expect("真技能应能取到图标");
            assert!(url.starts_with("data:image/png;base64,"));
            checked += 1;
        }
    }
    assert_eq!(checked, 30, "每个职业应有 30 个技能图标");
}

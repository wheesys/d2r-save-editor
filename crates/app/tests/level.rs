//! 改等级必须同步总经验：只写等级，游戏会按旧经验重新判定，等于没改。
//!
//! 经验表来自数据包（`experience.txt`），所以没有数据包时跳过。

use std::path::{Path, PathBuf};

use d2r_core::Edit;

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

fn work_copy(src: &Path, tag: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("level");
    let _ = std::fs::create_dir_all(&dir);
    let dest = dir.join(format!("{tag}.d2s"));
    std::fs::copy(src, &dest).expect("复制样本");
    dest
}

#[test]
fn set_level_writes_matching_experience() {
    if !d2r_app::bundle::available() {
        eprintln!("跳过：没有数据包");
        return;
    }
    let Some(src) = sample() else {
        eprintln!("跳过：没有样本存档");
        return;
    };
    let work = work_copy(&src, "level-xp");
    let mut ws = d2r_app::Workspace::new(d2r_app::default_game_dir());
    ws.open(&work).expect("打开样本");

    // 99 级的总经验（D2 经验表固定值；与职业无关）
    ws.apply(&[Edit::SetLevel {
        level: 99,
        experience: None,
    }])
    .expect("改到 99 级");
    let c = ws.character().expect("读角色");
    assert_eq!(c.level, 99);
    assert_eq!(c.experience, 3_520_485_254, "99 级应写入对应总经验");

    // 2 级是 500
    ws.apply(&[Edit::SetLevel {
        level: 2,
        experience: None,
    }])
    .expect("改到 2 级");
    let c = ws.character().expect("读角色");
    assert_eq!(c.level, 2);
    assert_eq!(c.experience, 500, "2 级应写入 500 经验");

    // 1 级是 0
    ws.apply(&[Edit::SetLevel {
        level: 1,
        experience: None,
    }])
    .expect("改到 1 级");
    assert_eq!(ws.character().expect("读角色").experience, 0);
}

#[test]
fn out_of_range_character_stat_is_rejected() {
    if !d2r_app::bundle::available() {
        eprintln!("跳过：没有数据包");
        return;
    }
    let Some(src) = sample() else {
        eprintln!("跳过：没有样本存档");
        return;
    };
    let work = work_copy(&src, "level-limit");
    let mut ws = d2r_app::Workspace::new(d2r_app::default_game_dir());
    ws.open(&work).expect("打开样本");

    // newskills 只有 8 位：300 会被 d2s 静默截成 255，必须提前拒绝
    let too_big = ws.apply(&[Edit::SetStat {
        stat: d2s::StatId::SKILL_POINTS.0 as u32,
        value: 300,
    }]);
    assert!(too_big.is_err(), "技能点 300 应被位宽校验拒绝");

    // 255 是能存下的上限
    ws.apply(&[Edit::SetStat {
        stat: d2s::StatId::SKILL_POINTS.0 as u32,
        value: 255,
    }])
    .expect("技能点 255 应能写入");
    assert_eq!(ws.character().expect("读角色").skill_points, 255);
}

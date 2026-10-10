//! 编辑会话（里程碑 4）集成测试：用真实存档验证"改 → 校验 → 保存 → 重新打开"闭环。
//!
//! 找不到样本时**跳过**（样本不入库）。

use std::path::{Path, PathBuf};

use d2r_core::{Edit, EditSession, SaveFile};

fn samples_root() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("D2R_SAMPLES_DIR") {
        let p = PathBuf::from(p);
        if p.is_dir() {
            return Some(p);
        }
    }
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).parent()?.parent()?;
    let p = repo.join("samples").join("saves");
    p.is_dir().then_some(p)
}

/// 找一个能逐字节往返的 v105 角色存档（编辑测试要在可靠样本上做）。
fn pick_sample() -> Option<PathBuf> {
    let root = samples_root()?;
    let mut candidates: Vec<PathBuf> = walk(&root)
        .into_iter()
        .filter(|p| p.extension().map(|e| e == "d2s").unwrap_or(false))
        .collect();
    candidates.sort();
    for p in candidates {
        if let Ok(f) = SaveFile::open(&p) {
            if f.check_roundtrip().identical && f.save.character.level >= 1 {
                return Some(p);
            }
        }
    }
    None
}

fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(dir) else {
        return out;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            out.extend(walk(&p));
        } else {
            out.push(p);
        }
    }
    out
}

fn work_copy(src: &Path, tag: &str) -> PathBuf {
    let mut dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    dir.push("edit");
    let _ = std::fs::create_dir_all(&dir);
    let dest = dir.join(format!("{tag}.d2s"));
    std::fs::copy(src, &dest).expect("复制样本");
    dest
}

#[test]
fn edit_save_reopen_roundtrip() {
    let Some(src) = pick_sample() else {
        eprintln!("跳过：找不到可用的 v105 样本");
        return;
    };
    let work = work_copy(&src, "edit_save_reopen");

    let mut session = EditSession::open(&work).expect("打开");
    let before = session.character();

    // 改等级 + 两个属性 + 一个技能 + 普通难度传送点全开
    let new_level = if before.level == 99 { 98 } else { 99 };
    session
        .apply_all(&[
            Edit::SetLevel { level: new_level },
            Edit::SetStat {
                stat: 0,
                value: 321,
            }, // 力量
            Edit::SetStat {
                stat: 2,
                value: 234,
            }, // 敏捷
            Edit::SetSkill { index: 0, level: 7 },
            Edit::UnlockWaypoints { difficulty: 0 },
        ])
        .expect("应用修改");

    let after = session.character();
    assert_eq!(after.level, new_level);
    assert_eq!(after.strength, 321);
    assert_eq!(after.dexterity, 234);
    assert!(session.is_dirty());

    // 写前校验：写出并重新解析必须通过
    let (bytes, verified) = session.verify().expect("写前校验");
    assert!(verified);
    assert!(!bytes.is_empty());

    // 保存（会创建 .bak）
    let saved = session.save().expect("保存");
    assert_eq!(saved, work);
    assert!(work.with_extension("d2s.bak").exists(), "应生成备份");

    // 重新打开：修改应持久化
    let reopened = EditSession::open(&work).expect("重新打开");
    let v = reopened.character();
    assert_eq!(v.level, new_level);
    assert_eq!(v.strength, 321);
    assert_eq!(v.dexterity, 234);
    assert_eq!(reopened.file.save.skills.level(0), 7);
    assert!(reopened
        .file
        .save
        .waypoints
        .get(d2s::Difficulty::Normal)
        .unwrap()
        .is_active(0));

    // 保存后的文件仍要能逐字节往返（写路径质量门槛）
    assert!(
        reopened.file.check_roundtrip().identical,
        "保存后的文件应仍逐字节往返一致"
    );
}

#[test]
fn invalid_edits_are_rejected_atomically() {
    let Some(src) = pick_sample() else { return };
    let work = work_copy(&src, "edit_invalid");

    let mut session = EditSession::open(&work).expect("打开");
    let before = session.character();

    // 第二条非法（等级 0）→ 整批回滚，第一条也不能生效
    let r = session.apply_all(&[Edit::SetLevel { level: 42 }, Edit::SetLevel { level: 0 }]);
    assert!(r.is_err(), "非法修改必须报错");
    let after = session.character();
    assert_eq!(after.level, before.level, "整批应回滚");
    assert!(!session.is_dirty(), "回滚后不应有挂起修改");
}

#[test]
fn verify_does_not_touch_disk() {
    let Some(src) = pick_sample() else { return };
    let work = work_copy(&src, "edit_verify_only");

    let mut session = EditSession::open(&work).expect("打开");
    session.apply(Edit::SetLevel { level: 50 }).expect("改等级");
    let mtime_before = std::fs::metadata(&work).unwrap().modified().unwrap();

    // verify 只写出到内存
    session.verify().expect("校验");
    let mtime_after = std::fs::metadata(&work).unwrap().modified().unwrap();
    assert_eq!(mtime_before, mtime_after, "verify 不应写盘");

    // 磁盘上的内容仍是原样
    let on_disk = SaveFile::open(&work).expect("重新读盘");
    assert_ne!(on_disk.save.character.level, 50, "verify 不应改动磁盘文件");
}

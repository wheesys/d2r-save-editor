//! 导出样本的任务段原始 flags：用来判断哪些位真正代表"已完成/已接到"。
//!
//! 为什么要导原始位而不是直接猜：任务槽是 16 位位集，语义没有官方文档，
//! 猜错会让"勾选完成"写出游戏不认的值。先用真实存档看**实际出现哪些位**，
//! 再据此外推。

use d2r_core::SaveFile;
use d2s::Difficulty;

fn main() {
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let dir = repo.join("samples").join("saves").join("d2r-105");
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .map(|rd| rd.flatten().map(|e| e.path()).collect())
        .unwrap_or_default();
    files.sort();

    for path in files {
        if path.extension().and_then(|e| e.to_str()) != Some("d2s") {
            continue;
        }
        let Ok(file) = SaveFile::open(&path) else {
            continue;
        };
        println!("=== {} ===", path.file_name().unwrap().to_string_lossy());
        for (diff_name, diff) in [
            ("Normal", Difficulty::Normal),
            ("Nightmare", Difficulty::Nightmare),
            ("Hell", Difficulty::Hell),
        ] {
            let Some(q) = file.save.quests.get(diff) else {
                continue;
            };
            println!("  {diff_name}");
            let acts: [(&str, Vec<u16>); 5] = [
                ("Act1", q.act1.slots().iter().map(|f| f.bits()).collect()),
                ("Act2", q.act2.slots().iter().map(|f| f.bits()).collect()),
                ("Act3", q.act3.slots().iter().map(|f| f.bits()).collect()),
                ("Act4", q.act4.slots().iter().map(|f| f.bits()).collect()),
                ("Act5", q.act5.slots().iter().map(|f| f.bits()).collect()),
            ];
            for (act, slots) in acts {
                let row: Vec<String> = slots
                    .iter()
                    .enumerate()
                    .map(|(i, v)| {
                        if *v == 0 {
                            format!("{i}:0")
                        } else {
                            format!("{i}:0x{v:04X}")
                        }
                    })
                    .collect();
                println!("    {act}  {}", row.join(" "));
            }
        }
        println!();
    }
}

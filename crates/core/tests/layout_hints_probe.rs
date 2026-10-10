//! 布局推断的实测探针（打印各样本推断结果，供人工核对）。
//!
//! 运行：`cargo test -p d2r-core --test layout_hints_probe -- --nocapture`

use std::path::PathBuf;

#[test]
fn print_hints_for_samples() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo = manifest.parent().unwrap().parent().unwrap();
    let dir = repo.join("samples").join("saves").join("d2r-105");
    for name in ["杜仲", "杜老五", "小五", "blank-testnew", "aruisi"] {
        let p = dir.join(format!("{name}.d2s"));
        let Ok(session) = d2r_core::EditSession::open(&p) else {
            continue;
        };
        let h = session.layout_hints();
        let questions = d2r_core::needs_confirmation(&h);
        println!(
            "{name}: 背包最大={:?} 箱子最大={:?} 佣兵段={} 佣兵物品={} 需确认={}",
            h.inventory_max,
            h.stash_max,
            h.merc_section,
            h.merc_items,
            questions.len()
        );
        for q in &questions {
            println!("    - {q}");
        }
    }
}

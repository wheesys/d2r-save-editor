//! 验证"游戏显示值 = 存档字段原值"这个假设（方案 B）在**全部属性**上是否成立。
//!
//! 背景：`d2s` 解码时会做 `value = (field - SaveAdd) << ValShift`。
//! 用户坚持"修改器要显示游戏里的值"，而截图显示 `item_maxhp_percent` 在
//! 游戏里是 31（范围 0–63），暗示游戏显示的是**字段原值**而不是 d2s 的值。
//! 如果真是这样，那么换算就是 `field = (d2s.value >> ValShift) + SaveAdd`。
//!
//! 硬约束：`field` 必须放得进 `Save Bits` 位宽。任何一条超宽，
//! 就说明 B 不成立（或该属性另有规则）。
//!
//! 输出还包含每条属性在两种假设下的**有效范围**，便于逐条与游戏核对。

use d2r_data::dataset::TableStore;
use d2s::data::ExternalData;

fn main() {
    let dir = std::path::Path::new(r"D:\code\d2r-save-editor\crates\app\data-bundle\excel");
    let mut store = TableStore::empty();
    store.add_override_dir(dir);
    let Some(table) = store.get("ItemStatCost").cloned() else {
        println!("没有 ItemStatCost 表");
        return;
    };
    let data = d2s::data::default_data();

    // 从存档里收集每条属性出现过的 d2s 值，用来做"字段是否超宽"的实测
    let game_dir = d2r_app::default_game_dir();
    let mut ws = d2r_app::Workspace::new(game_dir);
    let mut samples: Vec<(u16, i64)> = Vec::new();
    let saves_dir = std::path::Path::new(r"D:\code\d2r-save-editor\samples\saves\d2r-105");
    if let Ok(rd) = std::fs::read_dir(saves_dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().and_then(|s| s.to_str()) != Some("d2s") {
                continue;
            }
            if ws.open(&p).is_err() {
                continue;
            }
            let Ok(items) = ws.item_details() else {
                continue;
            };
            for it in &items {
                let Ok(d) = ws.item_detail_dto(it.index) else {
                    continue;
                };
                for s in &d.stats {
                    samples.push((s.id, s.value));
                }
            }
        }
    }
    println!("从存档收集到 {} 条属性取值", samples.len());

    let mut over: Vec<String> = Vec::new();
    let mut ok_count = 0usize;
    let mut checked = 0usize;

    println!();
    println!(
        "{:<30}{:>10}{:>9}{:>8}{:>8}{:>10}{:>14}",
        "内部名", "SaveAdd", "ValShift", "位宽", "位上限", "A范围", "B范围(字段原值)"
    );
    for row in 0..table.len() {
        let Some(id) = table.int(row, "*ID") else {
            continue;
        };
        let Ok(si) = data.stat_info(d2s::StatId(id as u16), 105) else {
            continue;
        };
        if si.save_bits == 0 || si.save_bits >= 62 {
            continue;
        }
        let name = table.cell(row, "Stat").unwrap_or("?");
        let bits_max = (1i64 << si.save_bits) - 1;
        let shift = si.val_shift as i64;
        let add = si.save_add as i64;

        // A：游戏显示 = d2s 值 → d2s 的有效范围
        let (a_lo, a_hi) = (-add << shift, (bits_max - add) << shift);
        // B：游戏显示 = 字段原值 → 字段范围
        let (b_lo, b_hi) = (0, bits_max);

        // 用真实取值验证 B：字段 = (value >> shift) + add 必须在 [0, bits_max]
        let mut bad = None;
        for (_sid, value) in samples.iter().filter(|(s, _)| *s == id as u16) {
            let field = (value >> shift) + add;
            if field < 0 || field > bits_max {
                bad = Some(format!(
                    "ID {id}({name}) d2s值={value} → 字段={field} 超出 0..{bits_max}"
                ));
                break;
            }
        }
        if let Some(b) = bad {
            over.push(b);
        } else {
            ok_count += 1;
        }
        checked += 1;

        // 只打印常见/关心的属性，避免 400 行刷屏
        let interesting = name.ends_with("perlevel")
            || matches!(
                name,
                "strength"
                    | "dexterity"
                    | "vitality"
                    | "energy"
                    | "maxhp"
                    | "maxmana"
                    | "maxstamina"
                    | "item_maxhp_percent"
                    | "item_maxmana_percent"
                    | "item_fastermovevelocity"
                    | "item_fastercastrate"
                    | "item_allskills"
                    | "item_cannotbefrozen"
                    | "fireresist"
            );
        if interesting {
            println!(
                "{:<30}{:>10}{:>9}{:>8}{:>8}{:>10}{:>14}",
                name,
                add,
                shift,
                si.save_bits,
                bits_max,
                format!("{a_lo}..{a_hi}"),
                format!("{b_lo}..{b_hi}")
            );
        }
    }

    println!();
    println!("检查了 {checked} 条属性；假设 B（字段原值即显示值）下字段未超宽的：{ok_count}");
    if over.is_empty() {
        println!("结论：**全部属性的字段都放得下**，假设 B 未被实测数据否证。");
    } else {
        println!("发现 {} 条超宽（假设 B 不成立的具体属性）：", over.len());
        for o in over.iter().take(20) {
            println!("  {o}");
        }
    }
}

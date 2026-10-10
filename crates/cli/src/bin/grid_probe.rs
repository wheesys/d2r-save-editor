//! 容器网格布局校验：把"某个存档在界面上会画成什么样"算出来，便于核对。
//!
//! 为什么需要它：界面是网格视图，**位置/占格算错在截图里很难看出来**
//! （块大小只差一格、越界物品会被挪到别的区域）。这个工具用与界面同一套规则
//! （页名 + 格子坐标 + 占格宽高 + 用户确认的容器尺寸）算出每个块的位置，
//! 于是"界面画出来的格子"可以直接在终端里逐条核对。
//!
//! 运行：`cargo run -p d2r-cli --bin grid-probe -- <存档> [--inv 16x8] [--stash 16x16] [--cube 16x8]`

use std::process::ExitCode;

use d2r_core::SaveFile;

/// 解析 `列x行`。
fn parse_size(text: &str) -> Option<(u32, u32)> {
    let (a, b) = text.split_once(['x', 'X', '*'])?;
    Some((a.trim().parse().ok()?, b.trim().parse().ok()?))
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let value_of = |flag: &str| -> Option<String> {
        args.iter()
            .position(|a| a == flag)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let Some(target) = args.iter().find(|a| !a.starts_with("--")) else {
        eprintln!(
            "用法: grid-probe <存档> [--inv 16x8] [--stash 16x16] [--cube 16x8] [--belt 4x4]"
        );
        return ExitCode::from(2);
    };

    let inv = value_of("--inv")
        .and_then(|v| parse_size(&v))
        .unwrap_or((10, 4));
    let stash = value_of("--stash")
        .and_then(|v| parse_size(&v))
        .unwrap_or((10, 10));
    let cube = value_of("--cube")
        .and_then(|v| parse_size(&v))
        .unwrap_or((3, 4));
    let belt_arg = value_of("--belt").and_then(|v| parse_size(&v));

    let file = match SaveFile::open(target) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("打开失败：{e}");
            return ExitCode::from(1);
        }
    };

    // 尺寸表：从内置数据包取（Armor/Weapons/Misc 的 invwidth/invheight）
    let sizes = d2r_app::bundle::item_size_map();

    // 腰带尺寸：界面按"实际用到的坐标"自适应（mod 会把它加宽，实测出现 x=15 的药水格），
    // 没显式传 --belt 时这里用同一规则，否则两边结论会不一致。
    let belt = match belt_arg {
        Some(s) => s,
        None => {
            let mut cols = 4u32;
            let mut rows = 4u32;
            for it in &file.save.items {
                let mode = format!("{:?}", it.position.mode);
                let page = format!("{:?}", it.position.store_page);
                if mode != "InBelt" && !(mode == "Stored" && page == "Belt") {
                    continue;
                }
                let (w, h) = sizes.get(&it.code.to_string()).copied().unwrap_or((1, 1));
                cols = cols.max(it.position.inv_x as u32 + w);
                rows = rows.max(it.position.inv_y as u32 + h);
            }
            (cols, rows)
        }
    };

    println!("存档      : {target}");
    println!(
        "容器设置  : 背包 {}×{}｜合成箱 {}×{}｜箱子 {}×{}｜腰带 {}×{}",
        inv.0, inv.1, cube.0, cube.1, stash.0, stash.1, belt.0, belt.1
    );
    println!();

    let mut counts: std::collections::BTreeMap<String, (usize, usize)> = Default::default();
    for it in &file.save.items {
        let page = format!("{:?}", it.position.store_page);
        let mode = format!("{:?}", it.position.mode);
        let code = it.code.to_string();
        let (w, h) = sizes.get(&code).copied().unwrap_or((1, 1));
        let (cols, rows, label) = match (mode.as_str(), page.as_str()) {
            // 装备位不按网格判越界：它按 body_location 落到固定槽位
            ("Equipped", _) => {
                let slot = format!("{:?}", it.position.body_location);
                let index = file
                    .save
                    .items
                    .iter()
                    .position(|o| std::ptr::eq(o, it))
                    .unwrap_or(0);
                println!("    Equip       #{index:<3} {code:<5} 槽位 {slot:<12} {mode}");
                continue;
            }
            ("InBelt", _) | (_, "Belt") => (belt.0, belt.1, "Belt"),
            (_, "Inventory") => (inv.0, inv.1, "Inventory"),
            (_, "Cube") => (cube.0, cube.1, "Cube"),
            (_, "Stash") => (stash.0, stash.1, "Stash"),
            _ => continue,
        };
        let (x, y) = (it.position.inv_x as u32, it.position.inv_y as u32);
        let fits = x + w <= cols && y + h <= rows;
        let index = file
            .save
            .items
            .iter()
            .position(|o| std::ptr::eq(o, it))
            .unwrap_or(0);
        let slot = format!("{:?}", it.position.body_location);
        let slot_note = if slot == "None" {
            String::new()
        } else {
            format!("·{slot}")
        };
        let entry = counts.entry(label.to_string()).or_insert((0, 0));
        if fits {
            entry.0 += 1;
        } else {
            entry.1 += 1;
        }
        let flag = if fits { "  " } else { "越界" };
        println!("{flag} {label:<11} #{index:<3} {code:<5} 格({x:>2},{y:>2}) 占 {w}×{h}  {mode}{slot_note}");
    }

    println!();
    for (page, (inside, outside)) in counts {
        println!("{page:<9} 容器内 {inside} 件，越界 {outside} 件");
    }
    ExitCode::SUCCESS
}

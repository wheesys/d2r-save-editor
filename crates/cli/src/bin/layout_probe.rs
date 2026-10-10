//! 布局诊断（临时工具，用于用"改动前后样本"反推容器坐标语义）。
//!
//! 打印某个存档在 `Inventory` / `Cube` 页上的物品矩形与重叠情况，
//! 并支持对合成箱做不同的原点偏移试探。

use std::collections::BTreeMap;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(target) = args.iter().find(|a| !a.starts_with("--")) else {
        eprintln!("用法: layout-probe <文件.d2s> [--dx N]");
        return ExitCode::from(2);
    };
    let dx: i32 = args
        .iter()
        .position(|a| a == "--dx")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(12);
    let dy: i32 = args
        .iter()
        .position(|a| a == "--dy")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

    let file = match d2r_core::SaveFile::open(target) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("打开失败：{e}");
            return ExitCode::from(1);
        }
    };

    // 尺寸表：从 loader 数据集读 Armor/Weapons/Misc 的 invwidth/invheight
    let mut size_map: BTreeMap<String, (i32, i32)> = d2r_app::bundle::item_size_map()
        .into_iter()
        .map(|(k, (w, h))| (k, (w as i32, h as i32)))
        .collect();
    if let Some(root) = d2r_data::find_loader_dataset(d2r_app::default_game_dir()) {
        let excel = root.join("data").join("global").join("excel");
        for (k, v) in d2r_data::build_size_map(&excel) {
            size_map.insert(k, (v.0 as i32, v.1 as i32));
        }
    }

    let mut cells: BTreeMap<(i32, i32), String> = BTreeMap::new();
    let mut overlaps = 0usize;
    let mut rows: Vec<(String, String, String, i32, i32, i32, i32)> = Vec::new();

    for it in &file.save.items {
        let p = &it.position;
        let page = format!("{:?}", p.store_page);
        let mode = format!("{:?}", p.mode);
        if !matches!(mode.as_str(), "Stored" | "InBelt") {
            continue;
        }
        if page != "Inventory" && page != "Cube" {
            continue;
        }
        let code = it.code.to_string();
        let (w, h) = size_map.get(&code).copied().unwrap_or((1, 1));
        let (mut x, y) = (p.inv_x as i32, p.inv_y as i32);
        if page == "Cube" {
            x += dx;
            // y += dy;
        }
        rows.push((page.clone(), code.clone(), format!("{:?}", it.quality), x, y, w, h));
        for ddy in 0..h {
            for ddx in 0..w {
                let key = (x + ddx, y + ddy);
                if let Some(other) = cells.get(&key) {
                    if other != &code {
                        println!("  重叠 @({},{}): {} vs {}", key.0, key.1, other, code);
                        overlaps += 1;
                    }
                } else {
                    cells.insert(key, code.clone());
                }
            }
        }
    }

    println!("文件      : {target}");
    println!("合成箱偏移: (+{dx},+{dy})");
    println!("参与物品  : {} 件（Inventory + Cube）", rows.len());
    println!("\n页       代码    品质      位置    占格");
    for (page, code, quality, x, y, w, h) in &rows {
        println!("  {page:<9} {code:<6} {quality:<8} ({x:>2},{y:>2})  {w}×{h}");
    }
    println!("\n占用格数: {}", cells.len());
    println!("重叠次数: {overlaps}");
    let max_x = cells.keys().map(|k| k.0).max().unwrap_or(-1) + 1;
    let max_y = cells.keys().map(|k| k.1).max().unwrap_or(-1) + 1;
    println!("边界跨度: {max_x} × {max_y}");

    // 打印占用网格
    println!("\n占用网格（. 空，字母为物品序号）");
    let order: Vec<String> = {
        let mut seen = Vec::new();
        for (_, code, _, _, _, _, _) in &rows {
            if !seen.contains(code) {
                seen.push(code.clone());
            }
        }
        seen
    };
    let label = |code: &str| -> char {
        order.iter().position(|c| c == code).map(|i| (b'A' + (i % 26) as u8) as char).unwrap_or('?')
    };
    for y in 0..max_y {
        let mut line = format!("  {y:>2} ");
        for x in 0..max_x {
            match cells.get(&(x, y)) {
                Some(c) => line.push(label(c)),
                None => line.push('.'),
            }
        }
        println!("{line}");
    }
    println!("\n图例: {}", order.iter().enumerate().map(|(i, c)| format!("{}={c}", (b'A' + (i % 26) as u8) as char)).collect::<Vec<_>>().join(" "));

    ExitCode::SUCCESS
}

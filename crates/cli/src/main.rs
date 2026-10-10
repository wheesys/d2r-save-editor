//! `d2r` 命令行工具。
//!
//! ```text
//! d2r roundtrip [目录或文件...]     读入 → 原样写出 → 逐字节比对（写路径门槛）
//! d2r inspect   <文件.d2s|.d2i>     打印头部、版本、角色、物品统计
//! d2r dataset   [--game <游戏目录>] 检查数据集：表数量、是否与原版基准有差异、字符串表
//! ```
//!
//! 退出码：0 全部通过；1 有失败；2 用法错误。

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use d2r_core::{SaveFile, SaveKind};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        usage();
        return ExitCode::from(2);
    }
    match args[0].as_str() {
        "roundtrip" => cmd_roundtrip(&args[1..]),
        "inspect" => cmd_inspect(&args[1..]),
        "dataset" => cmd_dataset(&args[1..]),
        "strings" => cmd_strings(&args[1..]),
        "-h" | "--help" | "help" => {
            usage();
            ExitCode::SUCCESS
        }
        other => {
            eprintln!("未知命令：{other}");
            usage();
            ExitCode::from(2)
        }
    }
}

fn usage() {
    eprintln!(
        "d2r {}\n\n\
         用法:\n  \
         d2r roundtrip [目录或文件...]      往返一致性校验（默认 samples/saves）\n  \
         d2r inspect   <文件.d2s|.d2i>      打印存档信息（--json 输出 JSON）\n  \
         d2r dataset   [--game <游戏目录>]  检查数据集\n\n\
         环境变量:\n  \
         D2R_GAME_DIR      游戏目录（默认 D:\\Games\\d2r92777\\Diablo II Resurrected Infernal Edition）\n  \
         D2R_SAMPLES_DIR   样本目录（默认 <仓库>/samples/saves）",
        d2r_core::VERSION
    );
}

fn default_game_dir() -> PathBuf {
    if let Ok(p) = std::env::var("D2R_GAME_DIR") {
        return PathBuf::from(p);
    }
    PathBuf::from(r"D:\Games\d2r92777\Diablo II Resurrected Infernal Edition")
}

fn default_samples_dir() -> PathBuf {
    if let Ok(p) = std::env::var("D2R_SAMPLES_DIR") {
        return PathBuf::from(p);
    }
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    // 从当前目录向上找 samples/saves
    let mut dir = Some(cwd.as_path());
    while let Some(d) = dir {
        let cand = d.join("samples").join("saves");
        if cand.is_dir() {
            return cand;
        }
        dir = d.parent();
    }
    cwd.join("samples").join("saves")
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    if dir.is_file() {
        if SaveKind::from_path(dir).is_some() {
            out.push(dir.to_path_buf());
        }
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = rd.filter_map(|e| e.ok()).map(|e| e.path()).collect();
    entries.sort();
    for e in entries {
        collect(&e, out);
    }
}

fn cmd_roundtrip(args: &[String]) -> ExitCode {
    let targets: Vec<PathBuf> = if args.is_empty() {
        vec![default_samples_dir()]
    } else {
        args.iter().map(PathBuf::from).collect()
    };
    let mut files = Vec::new();
    for t in &targets {
        collect(t, &mut files);
    }
    if files.is_empty() {
        eprintln!("没有找到 .d2s/.d2i 文件（查找：{:?}）", targets);
        return ExitCode::from(2);
    }

    let mut pass = 0usize;
    let mut fail = 0usize;
    for f in &files {
        let name = f.file_name().unwrap_or_default().to_string_lossy().to_string();
        match SaveFile::open(f) {
            Ok(file) => {
                let rt = file.check_roundtrip();
                let items = file.save.items.len();
                let who = if file.kind == SaveKind::Character {
                    format!("{} Lv{} {:?}", file.save.name(), file.save.character.level, file.save.character.class)
                } else {
                    "共享箱".to_string()
                };
                if rt.identical {
                    pass += 1;
                    println!("[一致] {name:<30} {:<22} {who:<32} 物品={items:<3} {}", file.version().to_string(), rt);
                } else {
                    fail += 1;
                    println!("[失败] {name:<30} {:<22} {who:<32} 物品={items:<3} {}", file.version().to_string(), rt);
                }
            }
            Err(e) => {
                fail += 1;
                println!("[失败] {name:<30} {e}");
            }
        }
    }
    println!("\n合计 {}：一致 {pass}，失败 {fail}", files.len());
    if fail == 0 { ExitCode::SUCCESS } else { ExitCode::from(1) }
}

fn cmd_inspect(args: &[String]) -> ExitCode {
    let json = args.iter().any(|a| a == "--json");
    let Some(target) = args.iter().find(|a| !a.starts_with("--")) else {
        eprintln!("用法: d2r inspect <文件.d2s|.d2i> [--json]");
        return ExitCode::from(2);
    };
    let path = Path::new(target);
    let file = match SaveFile::open(path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("打开失败：{e}");
            return ExitCode::from(1);
        }
    };

    if json {
        let items: Vec<serde_json::Value> = file
            .save
            .items
            .iter()
            .map(|it| {
                let p = &it.position;
                serde_json::json!({
                    "code": it.code.to_string(),
                    "quality": format!("{:?}", it.quality),
                    "level": it.level,
                    "mode": format!("{:?}", p.mode),
                    "body_location": format!("{:?}", p.body_location),
                    "inv_x": p.inv_x,
                    "inv_y": p.inv_y,
                    "store_page": format!("{:?}", p.store_page),
                    "stats": it.stats.iter().map(|s| serde_json::json!({
                        "id": format!("{:?}", s.id),
                        "value": s.value
                    })).collect::<Vec<_>>(),
                })
            })
            .collect();
        let out = serde_json::json!({
            "path": file.path.as_ref().map(|p| p.display().to_string()),
            "kind": format!("{:?}", file.kind),
            "header": {
                "magic": format!("0x{:08X}", file.header.magic),
                "version": file.header.version,
                "file_size": file.header.file_size,
                "checksum": format!("0x{:08X}", file.header.checksum),
                "size_matches": file.header.size_matches(file.original.len()),
            },
            "actual_len": file.original.len(),
            "character": {
                "name": file.save.name(),
                "level": file.save.character.level,
                "class": format!("{:?}", file.save.character.class),
            },
            "item_count": file.save.items.len(),
            "items": items,
            "roundtrip_identical": file.check_roundtrip().identical,
        });
        println!("{}", serde_json::to_string_pretty(&out).unwrap());
    } else {
        println!("文件      : {}", file.path.as_ref().map(|p| p.display().to_string()).unwrap_or_default());
        println!("类型      : {:?}", file.kind);
        println!("魔数      : 0x{:08X}", file.header.magic);
        println!("版本号    : {} (还原值 {})", file.version(), file.header.version);
        println!("长度字段  : {}（实际 {}，{}）",
            file.header.file_size, file.original.len(),
            if file.header.size_matches(file.original.len()) { "一致" } else { "不一致" });
        println!("校验和    : 0x{:08X}", file.header.checksum);
        println!("角色      : {} Lv{} {:?}", file.save.name(), file.save.character.level, file.save.character.class);
        println!("物品数    : {}", file.save.items.len());
        for (i, it) in file.save.items.iter().enumerate().take(20) {
            let p = &it.position;
            println!("  [{i:>2}] {:<5} {:?} Lv{:<3} 模式={:?} 装备位={:?} 格=({},{}) 页={:?} 属性={}",
                it.code, it.quality, it.level, p.mode, p.body_location, p.inv_x, p.inv_y, p.store_page, it.stats.len());
        }
        if file.save.items.len() > 20 {
            println!("  ...（共 {} 件）", file.save.items.len());
        }
        let rt = file.check_roundtrip();
        println!("往返一致  : {}", if rt.identical { "是" } else { "否" });
        if !rt.identical {
            println!("            {rt}");
        }
    }
    ExitCode::SUCCESS
}

fn cmd_dataset(args: &[String]) -> ExitCode {
    let game = args
        .iter()
        .position(|a| a == "--game")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from)
        .unwrap_or_else(default_game_dir);

    println!("游戏目录  : {}", game.display());
    let Some(ds_root) = d2r_data::find_loader_dataset(&game) else {
        eprintln!("没找到 loader 数据集（期望 <游戏>\\d2rloader\\data\\compiler\\<版本>\\data\\global\\excel）");
        return ExitCode::from(1);
    };
    println!("数据集根  : {}", ds_root.display());

    // 数据集元信息（loader 的 manifest.json 在上一层）
    let manifest = ds_root.join("manifest.json");
    if manifest.is_file() {
        println!("manifest  : {}（{} 字节）", manifest.display(),
            std::fs::metadata(&manifest).map(|m| m.len()).unwrap_or(0));
    }

    let mut ds = match d2r_data::Dataset::load(&ds_root) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("装载数据集失败：{e}");
            return ExitCode::from(1);
        }
    };
    let table_names = ds.excel.available();
    println!("excel 表  : {} 张", table_names.len());
    if let Some(base) = &ds.excel_base {
        println!("原版基准  : {} 张（{}）", base.available().len(), base.dir().map(|d| d.display().to_string()).unwrap_or_default());
    }

    // 关键表抽查：容器尺寸与属性位宽（数据驱动方案的两块基石）
    for name in ["inventory", "itemstatcost", "armor", "weapons", "misc"] {
        match ds.excel.get(name) {
            Some(t) => println!("  {name:<14} {:<6} 行  {} 列  源={}", t.len(), t.columns.len(),
                t.source.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default()),
            None => println!("  {name:<14} 缺失"),
        }
    }

    // 与基准表逐表比对，列出 mod 改动的表（数据驱动适配的目标）
    // 注意：这里的"基准"应指向**官方 CASC 导出**；loader 自带的 base\ 不是原版表
    // （实测它与官方原表差 36 张），所以标签必须写清楚，避免误判 mod 改动范围。
    if let Some(base_dir) = ds.excel_base.as_ref().and_then(|b| b.dir()).map(Path::to_path_buf) {
        let mut changed = Vec::new();
        for name in &table_names {
            let main = ds_root.join("data").join("global").join("excel").join(format!("{name}.txt"));
            let Some(base) = find_ci(&base_dir, name) else { continue };
            let a = std::fs::read(&main).ok();
            let b = std::fs::read(&base).ok();
            if a != b {
                changed.push(name.clone());
            }
        }
        println!("loader base 层差异: {} 张（注意：这是 loader 自己的基准层，不是官方原版表）", changed.len());
        if !changed.is_empty() {
            let show: Vec<_> = changed.iter().take(30).cloned().collect();
            println!("  {}", show.join(", "));
            if changed.len() > show.len() {
                println!("  ...（共 {} 张）", changed.len());
            }
        }
        println!("  提示：判定 mod 改动请在「官方 CASC 导出」上比，可用 `compare-dirs` 子命令");
    }

    println!("字符串表  : {} 条", ds.string_count());
    if ds.string_count() > 0 {
        println!("  语言    : {}", ds.strings.languages().join(", "));
    }

    // 官方字符串表（CascView 导出的 json 目录）默认从 samples/game-data/lng/strings 加载
    let strings_dir = args
        .iter()
        .position(|a| a == "--strings")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from)
        .or_else(|| {
            std::env::current_dir()
                .ok()
                .map(|c| c.join("samples").join("game-data").join("lng").join("strings"))
                .filter(|p| p.is_dir())
        });
    if let Some(dir) = strings_dir {
        match ds.with_strings_dir(&dir) {
            Ok(n) => {
                println!("官方字符串: {n} 条（{}）", dir.display());
                match (ds.text("rvl"), ds.text("amu")) {
                    (Some(a), Some(b)) => println!("  抽查 rvl={a}  amu={b}"),
                    _ => println!("  抽查失败：未查到 rvl/amu"),
                }
            }
            Err(e) => println!("官方字符串: 加载失败 {e}"),
        }
    } else {
        println!("官方字符串: 未找到目录（用 --strings <目录> 指定）");
    }
    ExitCode::SUCCESS
}

fn cmd_strings(args: &[String]) -> ExitCode {
    let dir = args
        .iter()
        .position(|a| a == "--dir")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join("samples")
                .join("game-data")
                .join("lng")
                .join("strings")
        });
    let lang = args
        .iter()
        .position(|a| a == "--lang")
        .and_then(|i| args.get(i + 1))
        .cloned()
        .unwrap_or_else(|| "zhCN".to_string());
    let probes: Vec<&String> = args.iter().skip(1).filter(|a| !a.starts_with("--") && *a != &lang).collect();

    if !dir.is_dir() {
        eprintln!("字符串表目录不存在：{}", dir.display());
        return ExitCode::from(1);
    }
    let strings = match d2r_data::Strings::from_dir(&dir) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("读取失败：{e}");
            return ExitCode::from(1);
        }
    };
    println!("目录    : {}", dir.display());
    println!("条目数  : {}", strings.len());
    println!("语言    : {}", strings.languages().join(", "));
    if strings.is_empty() {
        eprintln!("没有解析到任何条目（检查 JSON 结构）");
        return ExitCode::from(1);
    }
    if probes.is_empty() {
        // 抽查几个常见键
        for key in ["qf1", "qf2", "cap", "rvl", "hp1", "cm1", "key", "amu", "zlb"] {
            let zh = strings.get(key, &lang).unwrap_or("<缺失>");
            let en = strings.get(key, "enUS").unwrap_or("<缺失>");
            println!("  {key:<6} {lang}={zh:<18} enUS={en}");
        }
    } else {
        for key in probes {
            let v = strings.get(key, &lang).unwrap_or("<缺失>");
            let en = strings.get(key, "enUS").unwrap_or("<缺失>");
            println!("  {key:<10} {lang}={v}  | enUS={en}");
        }
    }
    ExitCode::SUCCESS
}

fn find_ci(dir: &Path, stem: &str) -> Option<PathBuf> {    let wanted = format!("{stem}.txt");
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .find(|p| p.file_name().and_then(|s| s.to_str()).map(|s| s.eq_ignore_ascii_case(&wanted)).unwrap_or(false))
}

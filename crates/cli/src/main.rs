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
        "validate" => cmd_validate(&args[1..]),
        "edit" => cmd_edit(&args[1..]),
        "view" => cmd_view(&args[1..]),
        "items" => cmd_items(&args[1..]),
        "export-items" => cmd_export_items(&args[1..]),
        "import-item" => cmd_import_item(&args[1..]),
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
         d2r validate  <文件.d2s> [--layout <JSON>]  容器与摆放校验（越界/重叠/未知物品）\n  \
         d2r edit      <文件.d2s> --set <JSON> [--out <文件>]  编辑并保存（先校验后落盘）\n  \
         d2r view      <文件.d2s> [--json]                  角色/技能/传送点/物品总览（界面同源数据）\n  \
         d2r items     <文件.d2s> [--filter 关键字] [--json] 物品清单/明细\n  \
         d2r export-items <文件.d2s> [--index N] [--out 文件.json]  导出物品蓝图\n  \
         d2r import-item  <文件.d2s> --blueprint 文件.json --template N [--dry-run]  导入物品\n  \
         d2r dataset   [--game <游戏目录>]  检查数据集\n  \
         d2r strings   [--dir <目录>]       字符串表抽查\n\n\
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

/// 取 `--key value` 形式的参数值。
fn arg_of(args: &[String], key: &str) -> Option<String> {
    let i = args.iter().position(|a| a == key)?;
    args.get(i + 1).cloned()
}

/// 默认字符串表目录：仓库内 `samples/game-data/lng/strings`（从当前目录逐级向上找）。
fn default_strings_dir() -> Option<PathBuf> {
    let mut dir = std::env::current_dir().ok()?;
    loop {
        let cand = dir
            .join("samples")
            .join("game-data")
            .join("lng")
            .join("strings");
        if cand.is_dir() {
            return Some(cand);
        }
        if !dir.pop() {
            return None;
        }
    }
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
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
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
        let name = f
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        match SaveFile::open(f) {
            Ok(file) => {
                let rt = file.check_roundtrip();
                let items = file.save.items.len();
                let who = if file.kind == SaveKind::Character {
                    format!(
                        "{} Lv{} {:?}",
                        file.save.name(),
                        file.save.character.level,
                        file.save.character.class
                    )
                } else if let Some(st) = &file.stash {
                    format!(
                        "共享箱 结构v{} 存档v{} 物品{}",
                        st.structure_version, st.save_version, st.item_count
                    )
                } else {
                    "共享箱".to_string()
                };
                if rt.identical {
                    pass += 1;
                    println!(
                        "[一致] {name:<30} {:<22} {who:<32} 物品={items:<3} {}",
                        file.version().to_string(),
                        rt
                    );
                } else {
                    fail += 1;
                    println!(
                        "[失败] {name:<30} {:<22} {who:<32} 物品={items:<3} {}",
                        file.version().to_string(),
                        rt
                    );
                }
            }
            Err(e) => {
                fail += 1;
                println!("[失败] {name:<30} {e}");
            }
        }
    }
    println!("\n合计 {}：一致 {pass}，失败 {fail}", files.len());
    if fail == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

fn cmd_inspect(args: &[String]) -> ExitCode {
    let json = args.iter().any(|a| a == "--json");
    let lang = arg_of(args, "--lang").unwrap_or_else(|| "zhCN".to_string());
    let Some(target) = args.iter().find(|a| !a.starts_with("--")) else {
        eprintln!("用法: d2r inspect <文件.d2s|.d2i> [--json] [--lang zhCN] [--strings <目录>]");
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

    // 名称解析：字符串表 + 暗金/套装表
    let strings_dir = arg_of(args, "--strings")
        .map(PathBuf::from)
        .or_else(default_strings_dir);
    let mut tables = d2r_data::TableStore::empty();
    if let Some(game) = d2r_data::find_loader_dataset(default_game_dir()) {
        tables = d2r_data::TableStore::new(game.join("data").join("global").join("excel"));
    }
    let strings = match &strings_dir {
        Some(dir) => d2r_data::Strings::from_dir(dir).unwrap_or_default(),
        None => d2r_data::Strings::default(),
    };
    let mut namer = d2r_data::ItemNamer::new(&strings, &mut tables, &lang);
    let has_names = !strings.is_empty();

    // 预先解析所有物品名（namer 需要可变借用表集合）
    let names: Vec<Option<d2r_data::ItemName>> = if has_names {
        file.save
            .items
            .iter()
            .map(|it| {
                let quality = format!("{:?}", it.quality);
                let idx = it.quality_data.as_ref().and_then(|q| q.file_index());
                Some(namer.resolve(&it.code.to_string(), &quality, idx))
            })
            .collect()
    } else {
        vec![None; file.save.items.len()]
    };

    // 共享箱：结构不同，单独输出
    if let Some(st) = &file.stash {
        println!(
            "文件      : {}",
            file.path
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_default()
        );
        println!("类型      : 共享储物箱（.d2i）");
        println!("结构版本  : {}", st.structure_version);
        println!(
            "存档版本  : {} (还原值 {})",
            file.version(),
            st.save_version
        );
        println!(
            "头部声明长度: {}（实际 {}）",
            st.declared_size,
            file.original.len()
        );
        println!("名称      : {:?}", st.name);
        println!("金币      : {}", st.gold);
        println!(
            "物品段    : 偏移 {}，物品数 {}，{} 字节",
            st.items_offset(),
            st.item_count,
            st.items_bytes.len()
        );
        println!("校验和    : 无（.d2i 不存 CRC，已实测确认）");
        let rt = file.check_roundtrip();
        println!("往返一致  : {}", if rt.identical { "是" } else { "否" });
        if json {
            let out = serde_json::json!({
                "path": file.path.as_ref().map(|p| p.display().to_string()),
                "kind": "stash",
                "structure_version": st.structure_version,
                "save_version": st.save_version,
                "declared_size": st.declared_size,
                "name": st.name,
                "gold": st.gold,
                "items_offset": st.items_offset(),
                "item_count": st.item_count,
                "items_bytes": st.items_bytes.len(),
                "roundtrip_identical": rt.identical,
            });
            println!("{}", serde_json::to_string_pretty(&out).unwrap());
        }
        return ExitCode::SUCCESS;
    }

    if json {
        let items: Vec<serde_json::Value> = file
            .save
            .items
            .iter()
            .enumerate()
            .map(|(i, it)| {
                let p = &it.position;
                let nm = names.get(i).and_then(|n| n.as_ref());
                serde_json::json!({
                    "code": it.code.to_string(),
                    "name": nm.map(|n| n.display()),
                    "name_resolved": nm.map(|n| n.resolved),
                    "name_note": nm.and_then(|n| n.note.clone()),
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
        println!(
            "文件      : {}",
            file.path
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_default()
        );
        println!("类型      : {:?}", file.kind);
        println!("魔数      : 0x{:08X}", file.header.magic);
        println!(
            "版本号    : {} (还原值 {})",
            file.version(),
            file.header.version
        );
        println!(
            "长度字段  : {}（实际 {}，{}）",
            file.header.file_size,
            file.original.len(),
            if file.header.size_matches(file.original.len()) {
                "一致"
            } else {
                "不一致"
            }
        );
        println!("校验和    : 0x{:08X}", file.header.checksum);
        println!(
            "角色      : {} Lv{} {:?}",
            file.save.name(),
            file.save.character.level,
            file.save.character.class
        );
        println!("物品数    : {}", file.save.items.len());
        for (i, it) in file.save.items.iter().enumerate().take(30) {
            let p = &it.position;
            let display = names
                .get(i)
                .and_then(|n| n.as_ref())
                .map(|n| n.display())
                .unwrap_or_else(|| it.code.to_string());
            let note = names
                .get(i)
                .and_then(|n| n.as_ref())
                .and_then(|n| n.note.clone())
                .map(|s| format!("  ← {s}"))
                .unwrap_or_default();
            println!(
                "  [{i:>2}] {:<5} {:<22} {:?} Lv{:<3} {:?} 格=({},{}) 页={:?} 属性={}{note}",
                it.code.to_string(),
                display,
                it.quality,
                it.level,
                p.mode,
                p.inv_x,
                p.inv_y,
                p.store_page,
                it.stats.len()
            );
        }
        if file.save.items.len() > 30 {
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
        println!(
            "manifest  : {}（{} 字节）",
            manifest.display(),
            std::fs::metadata(&manifest).map(|m| m.len()).unwrap_or(0)
        );
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
        println!(
            "原版基准  : {} 张（{}）",
            base.available().len(),
            base.dir()
                .map(|d| d.display().to_string())
                .unwrap_or_default()
        );
    }

    // 关键表抽查：容器尺寸与属性位宽（数据驱动方案的两块基石）
    for name in ["inventory", "itemstatcost", "armor", "weapons", "misc"] {
        match ds.excel.get(name) {
            Some(t) => println!(
                "  {name:<14} {:<6} 行  {} 列  源={}",
                t.len(),
                t.columns.len(),
                t.source
                    .file_name()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default()
            ),
            None => println!("  {name:<14} 缺失"),
        }
    }

    // 与基准表逐表比对，列出 mod 改动的表（数据驱动适配的目标）
    // 注意：这里的"基准"应指向本地导出结果；loader 自带的 base\ 不是原版表
    // （实测它与本地导出结果有差异），所以标签必须写清楚，避免误判 mod 改动范围。
    if let Some(base_dir) = ds
        .excel_base
        .as_ref()
        .and_then(|b| b.dir())
        .map(Path::to_path_buf)
    {
        let mut changed = Vec::new();
        for name in &table_names {
            let main = ds_root
                .join("data")
                .join("global")
                .join("excel")
                .join(format!("{name}.txt"));
            let Some(base) = find_ci(&base_dir, name) else {
                continue;
            };
            let a = std::fs::read(&main).ok();
            let b = std::fs::read(&base).ok();
            if a != b {
                changed.push(name.clone());
            }
        }
        println!(
            "loader base 层差异: {} 张（注意：这是 loader 自己的基准层，不是原版表）",
            changed.len()
        );
        if !changed.is_empty() {
            let show: Vec<_> = changed.iter().take(30).cloned().collect();
            println!("  {}", show.join(", "));
            if changed.len() > show.len() {
                println!("  ...（共 {} 张）", changed.len());
            }
        }
        println!("  提示：判定 mod 改动请在本地导出结果上比，可用 `compare-dirs` 子命令");
    }

    println!("字符串表  : {} 条", ds.string_count());
    if ds.string_count() > 0 {
        println!("  语言    : {}", ds.strings.languages().join(", "));
    }

    // 本地文本表默认从 samples/game-data/lng/strings 加载
    let strings_dir = args
        .iter()
        .position(|a| a == "--strings")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from)
        .or_else(|| {
            std::env::current_dir()
                .ok()
                .map(|c| {
                    c.join("samples")
                        .join("game-data")
                        .join("lng")
                        .join("strings")
                })
                .filter(|p| p.is_dir())
        });
    if let Some(dir) = strings_dir {
        match ds.with_strings_dir(&dir) {
            Ok(n) => {
                println!("本地文本表: {n} 条（{}）", dir.display());
                match (ds.text("rvl"), ds.text("amu")) {
                    (Some(a), Some(b)) => println!("  抽查 rvl={a}  amu={b}"),
                    _ => println!("  抽查失败：未查到 rvl/amu"),
                }
            }
            Err(e) => println!("本地文本表: 加载失败 {e}"),
        }
    } else {
        println!("本地文本表: 未找到目录（用 --strings <目录> 指定）");
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
    let probes: Vec<&String> = args
        .iter()
        .skip(1)
        .filter(|a| !a.starts_with("--") && *a != &lang)
        .collect();

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
        for key in [
            "qf1", "qf2", "cap", "rvl", "hp1", "cm1", "key", "amu", "zlb",
        ] {
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

fn cmd_validate(args: &[String]) -> ExitCode {
    let lang = arg_of(args, "--lang").unwrap_or_else(|| "zhCN".to_string());
    let Some(target) = args.iter().find(|a| !a.starts_with("--")) else {
        eprintln!("用法: d2r validate <文件.d2s> [--lang zhCN] [--layout <JSON>]");
        return ExitCode::from(2);
    };
    let file = match SaveFile::open(Path::new(target)) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("打开失败：{e}");
            return ExitCode::from(1);
        }
    };
    if file.kind != SaveKind::Character {
        eprintln!("validate 目前只支持角色存档（.d2s）");
        return ExitCode::from(2);
    }

    // 物品尺寸：优先用随程序分发的数据包（用户机器上不用装游戏），loader 产物作为补充
    let mut size_map = d2r_app::bundle::item_size_map();
    if let Some(ds_root) = d2r_data::find_loader_dataset(default_game_dir()) {
        let excel = ds_root.join("data").join("global").join("excel");
        for (k, v) in d2r_data::build_size_map(&excel) {
            size_map.insert(k, v);
        }
    }
    if size_map.is_empty() {
        eprintln!("没有可用的物品尺寸数据（数据包缺失且找不到游戏目录）");
        return ExitCode::from(1);
    }
    let strings = match default_strings_dir() {
        Some(dir) => d2r_data::Strings::from_dir(dir).unwrap_or_default(),
        None => d2r_data::Strings::default(),
    };
    let mut tables = d2r_data::TableStore::empty();
    if let Some(ds_root) = d2r_data::find_loader_dataset(default_game_dir()) {
        tables.add_override_dir(ds_root.join("data").join("global").join("excel"));
    }
    let class = format!("{:?}", file.save.character.class);

    // 容器尺寸与佣兵全装备：存档里读不出来 → 先推断、再让用户确认（--layout 指定）
    {
        let mut ws = d2r_app::Workspace::new(d2r_app::default_game_dir());
        if let Err(e) = ws.open(target) {
            eprintln!("打开失败：{e}");
            return ExitCode::from(1);
        }
        if let Some(json) = arg_of(args, "--layout") {
            match serde_json::from_str::<d2r_core::LayoutSettings>(&json) {
                Ok(s) => {
                    ws.set_layout(s);
                    println!("布局      : 已按 --layout 指定");
                }
                Err(e) => {
                    eprintln!("--layout 不是合法的布局 JSON：{e}");
                    eprintln!("示例: --layout '{{\"inventoryCols\":10,\"inventoryRows\":8,\"stashCols\":16,\"stashRows\":16,\"mercFullEquipment\":true,\"confirmedByUser\":true}}'");
                    return ExitCode::from(2);
                }
            }
        }
        match ws.layout_hints() {
            Ok(h) => {
                println!(
                    "推断跨度  : 背包最大({},{})  箱子最大({})",
                    h.inventory_max.map(|m| m.0).unwrap_or(0),
                    h.inventory_max.map(|m| m.1).unwrap_or(0),
                    match h.stash_max {
                        Some((x, y)) => format!("{x},{y}"),
                        None => "无数据".into(),
                    }
                );
                if h.merc_section {
                    println!("佣兵装备  : 段存在，{} 件物品", h.merc_items);
                }
                let layout = ws.layout().clone();
                println!(
                    "当前设置  : 背包 {}×{}｜箱子 {}×{}｜佣兵全装备 {}{}",
                    layout.inventory_cols,
                    layout.inventory_rows,
                    layout.stash_cols,
                    layout.stash_rows,
                    if layout.merc_full_equipment {
                        "是"
                    } else {
                        "否"
                    },
                    if layout.confirmed_by_user {
                        "（用户确认）"
                    } else {
                        "（默认值，可用 --layout 指定）"
                    }
                );
                // 用户已经确认过就不再重复问
                if !layout.confirmed_by_user {
                    let questions = d2r_core::needs_confirmation(&h);
                    if !questions.is_empty() {
                        println!("\n需要你确认的项（这些在存档里读不出来）：");
                        for (i, q) in questions.iter().enumerate() {
                            println!("  {}. {q}", i + 1);
                        }
                    }
                }
            }
            Err(e) => println!("推断失败：{e}"),
        }
        match ws.validate_bounds() {
            Ok(issues) if issues.is_empty() => println!("越界校验  : 通过（按当前布局设置）"),
            Ok(issues) => {
                println!("越界校验  : {} 件超出容器", issues.len());
                for i in issues.iter().take(15) {
                    println!("  ! {i}");
                }
            }
            Err(e) => println!("越界校验  : 跳过（{e}）"),
        }
        println!();
    }
    // 分区：按 (模式, 存储页) 分到不同网格
    let mut regions: std::collections::BTreeMap<String, Vec<d2r_data::PlacedItem>> =
        std::collections::BTreeMap::new();
    let mut equipped: Vec<(String, String)> = Vec::new();
    {
        let mut namer = d2r_data::ItemNamer::new(&strings, &mut tables, &lang);
        for it in &file.save.items {
            let p = &it.position;
            let mode = format!("{:?}", p.mode);
            let page = format!("{:?}", p.store_page);
            let code = it.code.to_string();
            let quality = format!("{:?}", it.quality);
            let idx = it.quality_data.as_ref().and_then(|q| q.file_index());
            let name = namer.resolve(&code, &quality, idx).display();
            let (w, h) = size_map.get(&code).copied().unwrap_or((1, 1));

            match (mode.as_str(), page.as_str()) {
                ("Equipped", _) => {
                    equipped.push((format!("{:?}", p.body_location), name));
                }
                ("Stored" | "InBelt", "Inventory") => regions
                    .entry("背包".to_string())
                    .or_default()
                    .push(d2r_data::PlacedItem {
                        code,
                        name,
                        col: p.inv_x as u32,
                        row: p.inv_y as u32,
                        w,
                        h,
                        cross_panel: false,
                    }),
                // 合成箱用局部坐标：实测偏移 (12,0) 后与背包物品零冲突
                ("Stored" | "InBelt", "Cube") => regions
                    .entry("背包".to_string())
                    .or_default()
                    .push(d2r_data::PlacedItem {
                        code,
                        name,
                        col: p.inv_x as u32 + 12,
                        row: p.inv_y as u32,
                        w,
                        h,
                        cross_panel: true,
                    }),
                ("Stored" | "InBelt", "Belt") => regions
                    .entry("腰带".to_string())
                    .or_default()
                    .push(d2r_data::PlacedItem {
                        code,
                        name,
                        col: p.inv_x as u32,
                        row: p.inv_y as u32,
                        w,
                        h,
                        cross_panel: false,
                    }),
                ("Stored", "Stash") => regions.entry(format!("储物箱 页{page}")).or_default().push(
                    d2r_data::PlacedItem {
                        code,
                        name,
                        col: p.inv_x as u32,
                        row: p.inv_y as u32,
                        w,
                        h,
                        cross_panel: false,
                    },
                ),
                _ => {}
            }
        }
    }

    println!("存档      : {target}");
    println!(
        "角色      : {} Lv{} {}",
        file.save.name(),
        file.save.character.level,
        class
    );
    println!("物品总数  : {}", file.save.items.len());

    // 原版背包尺寸（只作参照，不用于判越界：mod 会扩展存储且沿用 Inventory 页）
    let vanilla = d2r_data::container_spec_from_tables(&mut tables, &class)
        .map(|s| format!("{}×{}", s.cols, s.rows))
        .unwrap_or_else(|| "未知".into());

    let mut total = 0usize;
    let mut warns = 0usize;
    for (label, items) in &regions {
        let strict = label != "背包"; // 背包区域混入 mod 扩展存储与合成箱局部坐标
        let report =
            d2r_data::validate_region(label.clone(), items, |c| size_map.get(c).copied(), strict);
        total += report
            .issues
            .iter()
            .filter(|i| i.severity() == d2r_data::Severity::Error)
            .count();
        warns += report
            .issues
            .iter()
            .filter(|i| i.severity() == d2r_data::Severity::Warning)
            .count();
        let status = if report.is_ok() {
            "通过"
        } else {
            "有冲突"
        };
        println!(
            "\n{label}: {} 件，坐标范围 {}×{}，占用 {} 格 → {status}",
            report.checked, report.extent.0, report.extent.1, report.used_cells
        );
        if label == "背包" {
            println!("    参照：原版 inventory.txt 里 {class} 是 {vanilla}；实际跨度更大说明 mod 扩展过存储（不做越界判定）");
        }
        if report.is_ok() {
            for it in items.iter().take(5) {
                println!("    {} @({},{}) {}×{}", it.name, it.col, it.row, it.w, it.h);
            }
            if items.len() > 5 {
                println!("    ...（共 {} 件）", items.len());
            }
        } else {
            for issue in report.issues.iter().take(20) {
                println!("    ! {issue}");
            }
        }
    }

    let eq_issues = d2r_data::validate_body_slots(&equipped);
    total += eq_issues.len();
    println!("\n装备位    : {} 件已装备", equipped.len());
    for issue in &eq_issues {
        println!("    ! {issue}");
    }
    if eq_issues.is_empty() {
        println!("    装备位无重复");
    }

    println!("\n合计确定问题: {total}，提示: {warns}");
    if total == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}
fn cmd_edit(args: &[String]) -> ExitCode {
    let Some(target) = args.iter().find(|a| !a.starts_with("--")) else {
        eprintln!("用法: d2r edit <文件.d2s> --set '<JSON 数组>' [--out <文件>] [--dry-run]");
        eprintln!("示例: d2r edit a.d2s --set '[{{\"kind\":\"setLevel\",\"level\":50}}]'");
        return ExitCode::from(2);
    };
    let Some(json) = arg_of(args, "--set") else {
        eprintln!("缺少 --set <JSON>");
        return ExitCode::from(2);
    };
    let edits: Vec<d2r_app::EditRequest> = match serde_json::from_str(&json) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("--set 不是合法的修改数组：{e}");
            return ExitCode::from(2);
        }
    };
    let item_edits: Vec<d2r_app::ItemEditRequest> = match arg_of(args, "--items") {
        Some(j) => match serde_json::from_str(&j) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("--items 不是合法的物品修改数组：{e}");
                return ExitCode::from(2);
            }
        },
        None => Vec::new(),
    };
    let dry_run = args.iter().any(|a| a == "--dry-run");
    let out = arg_of(args, "--out").map(PathBuf::from);

    let mut ws = d2r_app::Workspace::new(d2r_app::default_game_dir());
    if let Err(e) = ws.open(target) {
        eprintln!("打开失败：{e}");
        return ExitCode::from(1);
    }
    println!("存档      : {target}");
    let before = match ws.character() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("读取角色失败：{e}");
            return ExitCode::from(1);
        }
    };
    println!(
        "修改前    : Lv{} 力量{} 敏捷{} 体力{} 精力{} 金币{}",
        before.level,
        before.strength,
        before.dexterity,
        before.vitality,
        before.energy,
        before.gold
    );

    if let Err(e) = ws.apply(&edits) {
        eprintln!("修改被拒绝：{e}");
        return ExitCode::from(1);
    }
    if let Err(e) = ws.apply_item_edits(&item_edits) {
        eprintln!("物品修改被拒绝：{e}");
        return ExitCode::from(1);
    }
    let after = match ws.character() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("读取角色失败：{e}");
            return ExitCode::from(1);
        }
    };
    println!(
        "修改后    : Lv{} 力量{} 敏捷{} 体力{} 精力{} 金币{}",
        after.level, after.strength, after.dexterity, after.vitality, after.energy, after.gold
    );

    match ws.verify() {
        Ok(v) => println!(
            "写前校验  : {}（{} 字节）{}",
            if v.ok { "通过" } else { "未通过" },
            v.bytes,
            v.message
        ),
        Err(e) => println!("写前校验  : 失败 {e}"),
    }

    if dry_run {
        println!(
            "--dry-run：未落盘，共 {} 条角色修改 + {} 条物品修改",
            ws.pending_edits(),
            ws.pending_item_edits()
        );
        return ExitCode::SUCCESS;
    }
    match ws.save_dto(out) {
        Ok(r) => {
            println!(
                "已保存    : {}（{} 字节，{} 条角色修改 + {} 条物品修改{}）",
                r.path,
                r.bytes,
                r.edits,
                ws.pending_item_edits(),
                if r.backed_up {
                    "，已备份原文件"
                } else {
                    ""
                }
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("保存失败：{e}");
            ExitCode::from(1)
        }
    }
}
fn cmd_view(args: &[String]) -> ExitCode {
    let json = args.iter().any(|a| a == "--json");
    let Some(target) = args.iter().find(|a| !a.starts_with("--")) else {
        eprintln!("用法: d2r view <文件.d2s> [--json]");
        return ExitCode::from(2);
    };
    let mut ws = d2r_app::Workspace::new(d2r_app::default_game_dir());
    let info = match ws.open(target) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("打开失败：{e}");
            return ExitCode::from(1);
        }
    };
    let char = match ws.character() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("读取角色失败：{e}");
            return ExitCode::from(1);
        }
    };
    let skills = ws.skills().unwrap_or_default();
    let waypoints = ws.waypoints().unwrap_or_default();
    let items = ws.item_details().unwrap_or_default();

    if json {
        let out = serde_json::json!({
            "dataset": ws.dataset_info(),
            "info": info,
            "character": char,
            "skills": skills,
            "waypoints": waypoints,
            "items": items,
        });
        println!("{}", serde_json::to_string_pretty(&out).unwrap());
        return ExitCode::SUCCESS;
    }

    println!("存档      : {}", info.path);
    println!(
        "版本      : {}｜{} 字节｜往返一致 {}",
        info.version,
        info.size,
        if info.roundtrip_identical {
            "是"
        } else {
            "否"
        }
    );
    println!(
        "数据集    : {} 张表｜字符串 {} 条",
        ws.dataset_info().tables,
        ws.dataset_info().strings
    );
    println!("角色      : {} {} Lv{}", char.name, char.class, char.level);
    println!(
        "属性      : 力量{} 敏捷{} 体力{} 精力{}（剩余点 {}）",
        char.strength, char.dexterity, char.vitality, char.energy, char.stat_points
    );
    println!(
        "资源      : 生命{}/{} 法力{}/{} 体力{}｜经验{}｜金币{}+箱{}",
        char.life,
        char.max_life,
        char.mana,
        char.max_mana,
        char.stamina,
        char.experience,
        char.gold,
        char.stash_gold
    );

    let learned: Vec<_> = skills.iter().filter(|s| s.level > 0).collect();
    println!(
        "\n技能      : {} 个已加点 / 共 {} 槽位",
        learned.len(),
        skills.len()
    );
    for s in learned.iter().take(30) {
        println!(
            "  {:<16} Lv{}（槽位 {}，ID {:?}）",
            s.name.clone().unwrap_or_else(|| "未命名".into()),
            s.level,
            s.index,
            s.skill_id
        );
    }

    println!("\n传送点");
    for w in &waypoints {
        let on: Vec<String> = w
            .active
            .iter()
            .enumerate()
            .filter(|(_, a)| **a)
            .map(|(i, _)| i.to_string())
            .collect();
        println!(
            "  {:<4} 开通 {}/{}：{}",
            w.difficulty,
            on.len(),
            w.active.len(),
            on.join(",")
        );
    }

    println!("\n物品      : {} 件", items.len());
    let mut by_page: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
    for it in &items {
        *by_page.entry(it.store_page.as_str()).or_default() += 1;
    }
    for (page, n) in by_page {
        println!("  {page:<12} {n} 件");
    }
    if let Ok(st) = ws.item_stats() {
        println!(
            "  品质分布  : {}",
            st.by_quality
                .iter()
                .map(|(q, n)| format!("{q} {n}"))
                .collect::<Vec<_>>()
                .join("｜")
        );
        println!(
            "  已装备 {} 件，含镶嵌物 {} 件",
            st.equipped, st.with_sockets
        );
    }
    ExitCode::SUCCESS
}
fn cmd_export_items(args: &[String]) -> ExitCode {
    let Some(target) = args.iter().find(|a| !a.starts_with("--")) else {
        eprintln!("用法: d2r export-items <文件.d2s> [--index N] [--out 文件.json]");
        return ExitCode::from(2);
    };
    let index: Option<usize> = arg_of(args, "--index").and_then(|v| v.parse().ok());
    let mut ws = d2r_app::Workspace::new(d2r_app::default_game_dir());
    if let Err(e) = ws.open(target) {
        eprintln!("打开失败：{e}");
        return ExitCode::from(1);
    }

    let json = match index {
        Some(i) => match ws.export_blueprint(i) {
            Ok(b) => serde_json::to_string_pretty(&b).unwrap(),
            Err(e) => {
                eprintln!("导出失败：{e}");
                return ExitCode::from(1);
            }
        },
        None => match ws.export_blueprints() {
            Ok(list) => serde_json::to_string_pretty(&list).unwrap(),
            Err(e) => {
                eprintln!("导出失败：{e}");
                return ExitCode::from(1);
            }
        },
    };

    match arg_of(args, "--out") {
        Some(path) => match std::fs::write(&path, &json) {
            Ok(()) => {
                println!("已导出到 {path}");
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("写文件失败：{e}");
                ExitCode::from(1)
            }
        },
        None => {
            println!("{json}");
            ExitCode::SUCCESS
        }
    }
}

fn cmd_import_item(args: &[String]) -> ExitCode {
    let Some(target) = args.iter().find(|a| !a.starts_with("--")) else {
        eprintln!(
            "用法: d2r import-item <文件.d2s> --blueprint 文件.json --template N [--dry-run]"
        );
        return ExitCode::from(2);
    };
    let Some(bp_path) = arg_of(args, "--blueprint") else {
        eprintln!("需要 --blueprint 指定蓝图 JSON（可用 export-items 导出）");
        return ExitCode::from(2);
    };
    let Some(template) = arg_of(args, "--template").and_then(|v| v.parse::<usize>().ok()) else {
        eprintln!("需要 --template 指定同存档里的模板物品序号（用 d2r items 查）");
        return ExitCode::from(2);
    };
    let dry_run = args.iter().any(|a| a == "--dry-run");

    let text = match std::fs::read_to_string(&bp_path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("读蓝图失败：{e}");
            return ExitCode::from(1);
        }
    };
    let blueprint = match d2r_core::parse_blueprint(&text) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("蓝图不合法：{e}");
            return ExitCode::from(1);
        }
    };

    let mut ws = d2r_app::Workspace::new(d2r_app::default_game_dir());
    if let Err(e) = ws.open(target) {
        eprintln!("打开失败：{e}");
        return ExitCode::from(1);
    }
    let before = ws.item_details().map(|v| v.len()).unwrap_or(0);

    let outcome = match ws.import_blueprint(&blueprint, template) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("导入被拒绝：{e}");
            return ExitCode::from(1);
        }
    };
    println!(
        "已导入    : 模板 #{template} → 新物品 #{}（覆盖 {} 条属性{}）",
        outcome.index,
        outcome.stats_replaced,
        if outcome.durability_set {
            "，含耐久"
        } else {
            ""
        }
    );

    let vr = match ws.verify() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("写前校验失败：{e}");
            return ExitCode::from(1);
        }
    };
    if !vr.ok {
        eprintln!("写前校验不通过，已放弃：{}", vr.message);
        return ExitCode::from(1);
    }
    println!("写前校验  : 通过（{} 字节）写出并重新解析通过", vr.bytes);

    if dry_run {
        println!("--dry-run：未落盘（物品数 {before} → {}）", before + 1);
        return ExitCode::SUCCESS;
    }
    match ws.save(None) {
        Ok(r) => {
            println!(
                "已保存    : {}（{} 字节，物品 {} → {}，{}）",
                r.path.display(),
                r.bytes,
                before,
                before + 1,
                if r.backed_up {
                    "已备份原文件"
                } else {
                    "未备份"
                }
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("保存失败：{e}");
            ExitCode::from(1)
        }
    }
}
fn cmd_items(args: &[String]) -> ExitCode {
    let json = args.iter().any(|a| a == "--json");
    let filter = arg_of(args, "--filter");
    let Some(target) = args
        .iter()
        .find(|a| !a.starts_with("--") && arg_of(args, "--filter").as_deref() != Some(a.as_str()))
    else {
        eprintln!("用法: d2r items <文件.d2s> [--filter 关键字] [--json]");
        return ExitCode::from(2);
    };
    let mut ws = d2r_app::Workspace::new(d2r_app::default_game_dir());
    if let Err(e) = ws.open(target) {
        eprintln!("打开失败：{e}");
        return ExitCode::from(1);
    }
    let all = match ws.item_details() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("读取物品失败：{e}");
            return ExitCode::from(1);
        }
    };
    let items: Vec<_> = match &filter {
        Some(f) => {
            let key = f.to_lowercase();
            all.iter()
                .filter(|i| {
                    i.code.to_lowercase().contains(&key) || i.name.to_lowercase().contains(&key)
                })
                .cloned()
                .collect()
        }
        None => all.clone(),
    };

    if json {
        let out = serde_json::json!({
            "path": target,
            "stats": ws.item_stats().ok(),
            "count": items.len(),
            "items": items,
        });
        println!("{}", serde_json::to_string_pretty(&out).unwrap());
        return ExitCode::SUCCESS;
    }

    println!("存档      : {target}");
    if let Ok(st) = ws.item_stats() {
        println!(
            "物品      : {} 件｜已装备 {}｜含镶嵌物 {}",
            st.total, st.equipped, st.with_sockets
        );
        println!(
            "  存储页  : {}",
            st.by_page
                .iter()
                .map(|(k, v)| format!("{k} {v}"))
                .collect::<Vec<_>>()
                .join("｜")
        );
        println!(
            "  品质    : {}",
            st.by_quality
                .iter()
                .map(|(k, v)| format!("{k} {v}"))
                .collect::<Vec<_>>()
                .join("｜")
        );
    }
    if filter.is_some() {
        println!("过滤      : {filter:?} → {} 件", items.len());
    }
    println!();
    println!(
        "{:>3}  {:<5} {:<20} {:<8} {:>3}  {:<14} {:<5} {:>9} {:>6} {:>4}",
        "#", "代码", "名称", "品质", "等级", "位置", "占格", "耐久", "数量", "孔"
    );
    for it in &items {
        let dur = match (it.durability, it.max_durability) {
            (Some(_), Some(0)) => "-".to_string(), // 无耐久概念
            (Some(c), Some(m)) => format!("{c}/{m}"),
            (Some(c), None) => format!("{c}"),
            _ => "-".to_string(),
        };
        let qty = it
            .quantity
            .map(|q| q.to_string())
            .unwrap_or_else(|| "-".into());
        let sockets = if it.socket_count > 0 {
            it.socket_count.to_string()
        } else {
            "-".into()
        };
        let place = if it.body_location != "None" {
            format!(
                "{}·{}",
                short_page(&it.store_page),
                short_slot(&it.body_location)
            )
        } else {
            short_page(&it.store_page).to_string()
        };
        println!(
            "{:>3}  {:<5} {:<20} {:<8} {:>3}  {:<14} {}×{:<3} {:>9} {:>6} {:>4}{}",
            it.index,
            it.code,
            truncate(&it.name, 20),
            it.quality,
            it.level,
            place,
            it.width,
            it.height,
            dur,
            qty,
            sockets,
            if it.name_resolved {
                ""
            } else {
                "  (名字未解析)"
            }
        );
    }
    ExitCode::SUCCESS
}

/// 存储页缩写（表格里省地方）。
fn short_page(page: &str) -> &str {
    match page {
        "Inventory" => "背包",
        "Stash" => "箱子",
        "Cube" => "方块",
        "Belt" => "腰带",
        "Equip" => "装备",
        "Null" => "—",
        other => other,
    }
}

/// 装备位缩写。
fn short_slot(slot: &str) -> &str {
    match slot {
        "Head" => "头",
        "Neck" => "颈",
        "Torso" => "身",
        "RightArm" => "右臂",
        "LeftArm" => "左臂",
        "RightRing" => "右戒",
        "LeftRing" => "左戒",
        "Belt" => "腰",
        "Feet" => "脚",
        "Gloves" => "手",
        "RightHand" => "右手",
        "LeftHand" => "左手",
        other => other,
    }
}

/// 按字符数截断（中文按 2 宽度估算，这里简单按字符数）。
fn truncate(s: &str, max: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max {
        s.to_string()
    } else {
        format!("{}…", chars[..max - 1].iter().collect::<String>())
    }
}
fn find_ci(dir: &Path, stem: &str) -> Option<PathBuf> {
    let wanted = format!("{stem}.txt");
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .find(|p| {
            p.file_name()
                .and_then(|s| s.to_str())
                .map(|s| s.eq_ignore_ascii_case(&wanted))
                .unwrap_or(false)
        })
}

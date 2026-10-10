//! 数据表目录比对：本地导出结果 vs 加载器展开产物（base 与 merged）。
//!
//! ```text
//! compare-dirs                                   # 用下面这些默认路径
//! compare-dirs --official <目录> --loader <目录> --merged <目录>
//! ```
//!
//! 用途（对应 `docs/next-steps-materials.md` §V3）：
//! 1. 验证加载器展开的原版表是否忠实于本地导出的原表；
//! 2. 以基准表为基准，确认 mod 改了哪些表。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use d2r_data::Table;

// 默认基准：随程序分发的数据包（来自本地导出结果）
const DEFAULT_OFFICIAL: &str = concat!(env!("CARGO_MANIFEST_DIR"), r"\..\app\data-bundle\excel");
const DEFAULT_LOADER_BASE: &str = r"D:\Games\d2r92777\Diablo II Resurrected Infernal Edition\d2rloader\data\compiler\3.2.0\data\global\excel\base";
const DEFAULT_LOADER_MERGED: &str = r"D:\Games\d2r92777\Diablo II Resurrected Infernal Edition\d2rloader\data\compiler\3.2.0\data\global\excel";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let official = arg_value(&args, "--official").unwrap_or_else(|| PathBuf::from(DEFAULT_OFFICIAL));
    let loader = arg_value(&args, "--loader").unwrap_or_else(|| PathBuf::from(DEFAULT_LOADER_BASE));
    let merged = arg_value(&args, "--merged").unwrap_or_else(|| PathBuf::from(DEFAULT_LOADER_MERGED));

    for (label, dir) in [("基准导出", &official), ("loader base", &loader), ("loader merged", &merged)] {
        if !dir.is_dir() {
            eprintln!("[{label}] 目录不存在：{}", dir.display());
            return ExitCode::from(2);
        }
        println!("[{label}] {} （{} 个 .txt）", dir.display(), txt_files(dir).len());
    }

    let official_map = txt_files(&official);
    let loader_map = txt_files(&loader);
    let merged_map = txt_files(&merged);

    println!("\n===== 1. 基准表 vs loader base =====（验证 loader 展开是否忠实）");
    let mut same = 0;
    let mut differ: Vec<(String, u64, u64)> = Vec::new();
    let mut only_official: Vec<String> = Vec::new();
    let mut only_loader: Vec<String> = Vec::new();
    for (name, path) in &official_map {
        match loader_map.get(name) {
            Some(other) => {
                let a = std::fs::read(path).unwrap_or_default();
                let b = std::fs::read(other).unwrap_or_default();
                if a == b {
                    same += 1;
                } else {
                    differ.push((name.clone(), a.len() as u64, b.len() as u64));
                }
            }
            None => only_official.push(name.clone()),
        }
    }
    for name in loader_map.keys() {
        if !official_map.contains_key(name) {
            only_loader.push(name.clone());
        }
    }
    println!("  逐字节一致 : {same} 张");
    println!("  内容不同   : {} 张", differ.len());
    for (n, a, b) in differ.iter().take(40) {
        println!("      {n:<24} 基准 {a:>7} B / loader {b:>7} B");
    }
    if differ.len() > 40 {
        println!("      ...（共 {} 张）", differ.len());
    }
    println!("  仅基准有   : {} 张 {}", only_official.len(), summarize(&only_official));
    println!("  仅 loader  : {} 张 {}", only_loader.len(), summarize(&only_loader));

    println!("\n===== 2. 基准表 vs loader merged =====（= mod 实际改动范围）");
    let mut mod_same = 0;
    let mut mod_changed: Vec<String> = Vec::new();
    for (name, path) in &official_map {
        match merged_map.get(name) {
            Some(other) => {
                let a = std::fs::read(path).unwrap_or_default();
                let b = std::fs::read(other).unwrap_or_default();
                if a == b {
                    mod_same += 1;
                } else {
                    mod_changed.push(name.clone());
                }
            }
            None => mod_changed.push(format!("{name}(仅基准)")),
        }
    }
    println!("  未改动 : {mod_same} 张");
    println!("  被改动 : {} 张", mod_changed.len());
    for chunk in mod_changed.chunks(6) {
        println!("      {}", chunk.join(", "));
    }

    println!("\n===== 3. 关键表结构（行数×列数）=====");
    for name in ["inventory", "itemstatcost", "armor", "weapons", "misc", "itemtypes", "properties", "skills"] {
        println!("  {name:<14} 基准 {:<16} base {:<16} merged {}",
            describe(&official, name), describe(&loader, name), describe(&merged, name));
    }

    ExitCode::SUCCESS
}

fn arg_value(args: &[String], key: &str) -> Option<PathBuf> {
    let i = args.iter().position(|a| a == key)?;
    args.get(i + 1).map(PathBuf::from)
}

fn summarize(names: &[String]) -> String {
    if names.is_empty() {
        String::new()
    } else if names.len() <= 8 {
        format!("({})", names.join(", "))
    } else {
        format!("({}, ...)", names[..8].join(", "))
    }
}

fn txt_files(dir: &Path) -> BTreeMap<String, PathBuf> {
    let mut out = BTreeMap::new();
    let Ok(rd) = std::fs::read_dir(dir) else { return out };
    for e in rd.flatten() {
        let p = e.path();
        if p.extension().map(|x| x.eq_ignore_ascii_case("txt")).unwrap_or(false) {
            if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                out.insert(stem.to_ascii_lowercase(), p);
            }
        }
    }
    out
}

fn describe(dir: &Path, stem: &str) -> String {
    let Some(path) = txt_files(dir).get(&stem.to_ascii_lowercase()).cloned() else {
        return "缺失".into();
    };
    match Table::from_file(&path) {
        Ok(t) => format!("{}行×{}列", t.len(), t.columns.len()),
        Err(e) => format!("读取失败:{e}"),
    }
}

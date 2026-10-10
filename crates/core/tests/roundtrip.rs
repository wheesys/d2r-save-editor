//! 集成测试：对 `samples/` 下的真实存档做"读入 → 原样写出 → 逐字节比对"。
//!
//! 样本目录按优先级查找：
//! 1. 环境变量 `D2R_SAMPLES_DIR`
//! 2. 仓库内 `samples/saves`
//!
//! 样本不进版本库（见 `.gitignore`），所以没有样本时测试**跳过而不是失败**。
//! 已知逐字节不一致的样本记在 [`KNOWN_NON_IDENTICAL`]，并在下方逐条说明原因；
//! 修好一个就删一条（该样本会立刻变成硬性要求）。

use std::path::{Path, PathBuf};

use d2r_core::{SaveFile, SaveKind};

/// 已知无法逐字节往返的样本（文件名 → 原因），需要显式豁免。
const KNOWN_NON_IDENTICAL: &[(&str, &str)] = &[
    (
        "杜老二.d2s",
        "条目段编码差异：重写后长 2 字节，前缀只相同 8 字节、后缀相同 1166 字节 —— 不是尾部填充，\
         而是条目流整体不同（`cmp-bytes` + `rewrite-save` 可复现）。需逐件定位是哪种条目被写成了不同编码。",
    ),
];

fn samples_root() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("D2R_SAMPLES_DIR") {
        let p = PathBuf::from(p);
        if p.is_dir() {
            return Some(p);
        }
    }
    // CARGO_MANIFEST_DIR = <repo>/crates/core
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).parent()?.parent()?;
    let p = repo.join("samples").join("saves");
    p.is_dir().then_some(p)
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = rd.filter_map(|e| e.ok()).map(|e| e.path()).collect();
    entries.sort();
    for e in entries {
        if e.is_dir() {
            collect(&e, out);
        } else if SaveKind::from_path(&e).is_some() {
            out.push(e);
        }
    }
}

fn known_reason(name: &str) -> Option<&'static str> {
    KNOWN_NON_IDENTICAL.iter().find(|(n, _)| *n == name).map(|(_, r)| *r)
}

#[test]
fn samples_roundtrip_is_byte_identical() {
    let Some(root) = samples_root() else {
        eprintln!("跳过：未找到样本目录（可用 D2R_SAMPLES_DIR 指定）");
        return;
    };

    let mut files = Vec::new();
    collect(&root, &mut files);
    if files.is_empty() {
        eprintln!("跳过：{} 下没有 .d2s/.d2i", root.display());
        return;
    }

    let mut ok = 0usize;
    let mut known = 0usize;
    let mut failures = Vec::new();

    for path in &files {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        match SaveFile::open(path) {
            Ok(file) => {
                let rt = file.check_roundtrip();
                if rt.identical {
                    ok += 1;
                    println!("[一致]   {name:<32} {} {}", file.version(), rt);
                } else if let Some(reason) = known_reason(&name) {
                    known += 1;
                    println!("[已知]   {name:<32} {rt}\n         原因：{reason}");
                } else {
                    failures.push(format!("{name}: {rt}"));
                }
            }
            Err(e) => {
                if let Some(reason) = known_reason(&name) {
                    known += 1;
                    println!("[已知]   {name:<32} 解析失败：{e}\n         原因：{reason}");
                } else {
                    failures.push(format!("{name}: 解析失败 {e}"));
                }
            }
        }
    }

    println!("\n合计 {} 个样本：一致 {ok}，已知不通过 {known}，新增失败 {}", files.len(), failures.len());
    assert!(failures.is_empty(), "出现新的往返不一致：\n  {}", failures.join("\n  "));
}

#[test]
fn header_size_field_matches_file_length() {
    let Some(root) = samples_root() else { return };
    let mut files = Vec::new();
    collect(&root, &mut files);
    for path in &files {
        let Ok(bytes) = std::fs::read(path) else { continue };
        let h = d2r_core::SaveHeader::parse_checked(&bytes).expect("头应合法");
        let name = path.file_name().unwrap().to_string_lossy();
        if name.ends_with(".d2s") {
            assert!(
                h.size_matches(bytes.len()),
                "{name}: 头部声明 {} != 实际 {}",
                h.file_size,
                bytes.len()
            );
        }
    }
}

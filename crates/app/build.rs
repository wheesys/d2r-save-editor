//! 构建脚本：把本机数据包**预压缩**后生成嵌入用的 Rust 源码，并做缺数据的检查。
//!
//! 数据包（`crates/app/data-bundle/`）不入 git，由 `tools/make-bundle.py` 在本机生成。
//!
//! 为什么要预压缩：数据是文本（表与 JSON），deflate 能压到约 18%，
//! 直接 `include_bytes!` 原文件会让可执行文件多出十几 MB。参考实现（daidodo/diablo_edit）
//! 也是"构建期压缩成数据文件 + 运行时解压"，这里同理，只是压缩算法用 flate2。
//!
//! 产物：`$OUT_DIR/bundle_data.rs`
//! ```text
//! pub static FILES: &[(&str, &[u8], usize)] = &[ ("excel/xxx.txt", include_bytes!("..."), 8423), ... ];
//! ```
//!
//! 策略：
//! - **debug 构建**：数据包缺失只警告 —— 刚 clone 的机器也能直接 `cargo build`；
//! - **release 构建**：数据包缺失直接失败 —— 防止发出"打开就没数据"的包。

use std::io::Write;
use std::path::{Path, PathBuf};

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let bundle = manifest.join("data-bundle");
    let marker = bundle.join("BUNDLE.json");
    let has_bundle = marker.is_file() && bundle.join("excel").is_dir();

    println!("cargo:rerun-if-changed=bundle-manifest.json");
    println!("cargo:rerun-if-changed=data-bundle/BUNDLE.json");
    println!("cargo:rerun-if-changed=data-bundle/excel");
    println!("cargo:rerun-if-changed=data-bundle/strings");
    println!("cargo:rerun-if-changed=data-bundle/icons");
    println!("cargo:rerun-if-changed=data-bundle/skillicons");
    println!("cargo:rerun-if-changed=data-bundle/library");

    let profile = std::env::var("PROFILE").unwrap_or_default();
    // CI 上允许"无数据构建"：公共仓库不能带数据，CI 只为验证可编译并产出可启动的 exe。
    // 本机打包（tools/package.ps1）不设这个变量，所以仍然强制要求数据包。
    let allow_no_bundle = std::env::var("D2R_ALLOW_NO_BUNDLE").is_ok();
    if !has_bundle {
        if profile == "release" && !allow_no_bundle {
            eprintln!(
                "error: release 构建缺少数据包 {}。\n\
                 请先运行：python tools/make-bundle.py\n\
                 （数据包不入 git，打包前必须在本机生成；开发用 debug 构建可以没有它）",
                bundle.display()
            );
            std::process::exit(1);
        }
        if allow_no_bundle {
            println!("cargo:warning=D2R_ALLOW_NO_BUNDLE 已设置：按无数据方式构建（exe 可启动但缺少数据表与文本表）");
        } else {
            println!("cargo:warning=未找到数据包（{bundle:?}）：debug 构建继续，但运行时没有数据表与文本表。需要时运行 python tools/make-bundle.py");
        }
        write_empty(&out_dir);
        return;
    }

    let mut entries: Vec<(String, PathBuf, usize)> = Vec::new();
    let mut raw_total = 0usize;
    let mut comp_total = 0usize;
    // `library` 是装备库（tools/make-library.py 的产物），与其它分组一样内嵌。
    for group in ["excel", "strings", "icons", "skillicons", "library"] {
        let dir = bundle.join(group);
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut files: Vec<PathBuf> = rd
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_file())
            .collect();
        files.sort();
        for path in files {
            let rel = format!("{group}/{}", path.file_name().unwrap().to_string_lossy());
            let data = std::fs::read(&path).expect("读数据包文件");
            let compressed = deflate(&data);
            let comp_path = out_dir.join(format!("bundle_{}.bin", rel.replace(['/', '.'], "_")));
            std::fs::write(&comp_path, &compressed).expect("写压缩数据");
            raw_total += data.len();
            comp_total += compressed.len();
            entries.push((rel, comp_path, data.len()));
        }
    }

    write_source(&out_dir, &entries, has_bundle);
    println!("cargo:rustc-env=D2R_BUNDLE_FILES={}", entries.len());
    println!(
        "cargo:warning=已内嵌数据包：{} 个文件，{:.1} MB → 压缩后 {:.1} MB",
        entries.len(),
        raw_total as f64 / 1048576.0,
        comp_total as f64 / 1048576.0
    );
}

fn deflate(data: &[u8]) -> Vec<u8> {
    use flate2::write::ZlibEncoder;
    use flate2::Compression;
    let mut enc = ZlibEncoder::new(Vec::new(), Compression::best());
    enc.write_all(data).expect("压缩");
    enc.finish().expect("压缩收尾")
}

fn write_empty(out_dir: &Path) {
    let src = "// 数据包缺失：空表。\n\
               pub const AVAILABLE: bool = false;\n\
               pub static FILES: &[(&str, &[u8], usize)] = &[];\n";
    std::fs::write(out_dir.join("bundle_data.rs"), src).expect("写占位源码");
}

fn write_source(out_dir: &Path, entries: &[(String, PathBuf, usize)], has_bundle: bool) {
    let mut src = String::new();
    src.push_str("// 由 build.rs 生成：预压缩的数据包（运行时用 flate2 解压）。\n");
    src.push_str(&format!(
        "/// 数据包是否可用。\npub const AVAILABLE: bool = {has_bundle};\n"
    ));
    src.push_str("/// (相对路径, 压缩数据, 解压后长度)\n");
    src.push_str("pub static FILES: &[(&str, &[u8], usize)] = &[\n");
    for (rel, path, raw_len) in entries {
        let lit = path.to_string_lossy().replace('\\', "/");
        src.push_str(&format!(
            "    ({rel:?}, include_bytes!({lit:?}), {raw_len}),\n"
        ));
    }
    src.push_str("];\n");
    std::fs::write(out_dir.join("bundle_data.rs"), src).expect("写生成源码");
}

//! 构建脚本：检查随程序分发的数据包是否就位，并生成内嵌所需的信息。
//!
//! 数据包（`crates/app/data-bundle/`）**不入 git**，
//! 由 `tools/make-bundle.py` 在打包前生成。
//!
//! 策略：
//! - **debug 构建**：数据包缺失只警告 —— 刚 clone 的机器也能直接 `cargo build`，
//!   只是运行时会提示"数据来源：无"；
//! - **release 构建**：数据包缺失直接让构建失败 —— 防止发出一个"用户机器上
//!   没有本地数据就什么都干不了"的安装包。

use std::path::{Path, PathBuf};

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let bundle = manifest.join("data-bundle");
    let marker = bundle.join("BUNDLE.json");
    let has_bundle = marker.is_file() && bundle.join("excel").is_dir();

    // 数据包变了要重新编译（include_dir! 是编译期内嵌）
    println!("cargo:rerun-if-changed=data-bundle/BUNDLE.json");
    println!("cargo:rerun-if-changed=data-bundle/excel");
    println!("cargo:rerun-if-changed=data-bundle/strings");
    println!("cargo:rerun-if-changed=data-bundle-stub");

    let profile = std::env::var("PROFILE").unwrap_or_default();
    if has_bundle {
        let count = count_files(&bundle.join("excel")) + count_files(&bundle.join("strings"));
        println!("cargo:rustc-env=D2R_BUNDLE_FILES={count}");
        println!("cargo:warning=已内嵌数据包：{count} 个文件（{bundle:?}）");
    } else if profile == "release" {
        // 直接失败：release 产物必须带数据
        eprintln!(
            "error: release 构建缺少数据包 {}。\n\
             请先运行：python tools/make-bundle.py\n\
             （数据包不入 git，打包前必须在本机生成；开发用 debug 构建可以没有它）",
            bundle.display()
        );
        std::process::exit(1);
    } else {
        println!("cargo:rustc-env=D2R_BUNDLE_FILES=0");
        println!(
            "cargo:warning=未找到数据包（{bundle:?}）：debug 构建继续，但运行时没有数据表与字符串表。\
             需要时运行 python tools/make-bundle.py"
        );
    }
}

fn count_files(dir: &Path) -> usize {
    let Ok(rd) = std::fs::read_dir(dir) else { return 0 };
    rd.flatten().filter(|e| e.path().is_file()).count()
}

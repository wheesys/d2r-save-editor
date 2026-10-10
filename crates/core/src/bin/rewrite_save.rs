//! 把指定存档"读入再写出"的结果另存一份，供 `cmp-bytes` 做字节级对比。
//!
//! 用法: rewrite-save.exe <原文件> <输出文件>

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        eprintln!("用法: rewrite-save.exe <原文件> <输出文件>");
        return ExitCode::from(2);
    }
    match d2r_core::SaveFile::open(&args[0]) {
        Ok(file) => match file.to_bytes() {
            Ok(bytes) => {
                if let Err(e) = std::fs::write(&args[1], &bytes) {
                    eprintln!("写文件失败：{e}");
                    return ExitCode::from(1);
                }
                println!("已写出 {} 字节 → {}", bytes.len(), args[1]);
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("写出失败：{e}");
                ExitCode::from(1)
            }
        },
        Err(e) => {
            eprintln!("打开失败：{e}");
            ExitCode::from(1)
        }
    }
}

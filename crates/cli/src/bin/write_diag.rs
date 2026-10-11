//! 保存路径写盘诊断。
//!
//! 用途：用户在"自己双击启动"的环境里遇到保存失败时，用这个工具把**到底哪一步
//! 被拒**查清楚，而不是靠猜（权限、占用、安全软件、只读属性……）。
//!
//! 运行（直接双击或在任意终端执行）：
//!     d2r-writediag.exe
//!     d2r-writediag.exe "C:\Users\me\Saved Games\Diablo II Resurrected"
//!
//! 不带参数时用默认的游戏存档目录。

use std::io::Write;
use std::path::{Path, PathBuf};

/// 默认要诊断的目录（D2R 存档目录）。
fn default_dir() -> PathBuf {
    let home = std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("C:\\Users"));
    home.join("Saved Games").join("Diablo II Resurrected")
}

/// 一个待测的写操作：名称 + 具体动作。
type WriteStep = (&'static str, fn(&Path) -> std::io::Result<()>);

fn main() {
    let dir = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(default_dir);

    println!("=== 保存路径写盘诊断 ===");
    println!("目标目录: {}", dir.display());
    println!();

    // 1) 目录本身的状态
    match std::fs::metadata(&dir) {
        Ok(m) => {
            println!("[1] 目录存在");
            println!("    只读属性: {}", m.permissions().readonly());
        }
        Err(e) => {
            println!("[1] 目录不可访问: {e}");
            return;
        }
    }

    // 2) 逐项尝试各种写操作，定位到底哪一步被拒
    let steps: [WriteStep; 5] = [
        ("建新文件（write）", |d| {
            std::fs::write(d.join("d2r-diag-new.txt"), b"x")
        }),
        ("建新文件（create_new）", |d| {
            let mut f = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(d.join("d2r-diag-new2.txt"))?;
            f.write_all(b"x")
        }),
        ("建子目录并写文件", |d| {
            let sub = d.join("d2r-diag-sub");
            std::fs::create_dir_all(&sub)?;
            std::fs::write(sub.join("f.txt"), b"x")?;
            let _ = std::fs::remove_dir_all(&sub);
            Ok(())
        }),
        ("改名（临时文件 → 正式名）", |d| {
            let tmp = d.join("d2r-diag-tmp.tmp");
            let fin = d.join("d2r-diag-fin.dat");
            std::fs::write(&tmp, b"x")?;
            let r = std::fs::rename(&tmp, &fin);
            let _ = std::fs::remove_file(&tmp);
            let _ = std::fs::remove_file(&fin);
            r
        }),
        ("删除刚建的文件", |d| {
            std::fs::remove_file(d.join("d2r-diag-new.txt"))
        }),
    ];

    println!();
    println!("[2] 逐项写操作:");
    let mut blocked = Vec::new();
    for (name, f) in steps {
        match f(&dir) {
            Ok(()) => println!("    ✓ {name}"),
            Err(e) => {
                let code = e.raw_os_error();
                println!("    ✗ {name}: {e}   (os error {code:?})");
                blocked.push((name, code));
            }
        }
    }
    // 收尾：清掉可能残留的文件
    for f in [
        "d2r-diag-new.txt",
        "d2r-diag-new2.txt",
        "d2r-diag-tmp.tmp",
        "d2r-diag-fin.dat",
    ] {
        let _ = std::fs::remove_file(dir.join(f));
    }

    // 3) 现有存档文件的状态
    println!();
    println!("[3] 目录里的存档与备份:");
    if let Ok(rd) = std::fs::read_dir(&dir) {
        let mut n = 0;
        for e in rd.flatten() {
            let p = e.path();
            let ext = p.extension().and_then(|s| s.to_str()).unwrap_or("");
            if !matches!(ext, "d2s" | "d2i" | "bak") {
                continue;
            }
            n += 1;
            let size = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
            let readonly = std::fs::metadata(&p)
                .map(|m| m.permissions().readonly())
                .unwrap_or(false);
            println!(
                "    {}  {} 字节  只读={}",
                p.file_name().unwrap_or_default().to_string_lossy(),
                size,
                readonly
            );
        }
        if n == 0 {
            println!("    （没有 .d2s/.d2i/.bak）");
        }
    }

    // 4) 结论
    println!();
    println!("[4] 结论:");
    if blocked.is_empty() {
        println!("    这个目录**完全可写**，保存失败的原因不在目录权限。");
        println!("    如果游戏里保存仍失败，请检查编辑器是否在写别的路径（看错误里的完整路径）。");
    } else {
        println!("    目录**不可写**，被拒的操作: {}", blocked.len());
        println!("    常见原因（按可能性排序）:");
        println!("      1. Windows 安全中心「受控文件夹访问」（勒索软件防护）拦截了本程序");
        println!("         → Windows 安全中心 → 病毒和威胁防护 → 勒索软件防护");
        println!("           把本程序加入「允许的应用」，或暂时关闭该功能");
        println!("      2. 第三方安全软件（如各类卫士/杀软）的文件防护");
        println!("      3. 目录 ACL 不含当前用户（看「安全」选项卡）");
        println!("    注意: 备份失败时**不会改动原存档**，你的存档是安全的。");
    }
    println!();
    println!("提示: 本程序不需要管理员权限；上面这些原因都与权限提升无关。");
}

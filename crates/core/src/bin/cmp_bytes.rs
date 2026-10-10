//! 对比原始存档与"重新写出"的结果，定位差异性质：是尾部多出字节，还是整体错位。
//!
//! 用法: cmp_bytes.exe <原文件> <新文件>

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        eprintln!("用法: cmp_bytes.exe <原文件> <新文件>");
        return ExitCode::from(2);
    }
    let a = std::fs::read(&args[0]).expect("读原文件");
    let b = std::fs::read(&args[1]).expect("读新文件");
    println!("原文件 {} 字节，新文件 {} 字节，差 {}", a.len(), b.len(), b.len() as i64 - a.len() as i64);

    let n = a.len().min(b.len());
    let first = (0..n).find(|&i| a[i] != b[i]);
    match first {
        Some(i) => {
            println!("首个差异偏移: {i}");
            // 从差异处起算，后面还有多少字节不同
            let differing = (i..n).filter(|&k| a[k] != b[k]).count();
            println!("差异处之后不同字节数: {differing} / 剩余 {}", n - i);
            // 检查是否只是"插入/删除"：让 b 从 i+2 起与 a 从 i 起比较
            for shift in [1i64, 2, -1, -2] {
                let mut same = 0usize;
                let mut checked = 0usize;
                for k in i..n {
                    let j = k as i64 + shift;
                    if j < 0 || j as usize >= b.len() { break; }
                    checked += 1;
                    if a[k] == b[j as usize] { same += 1; }
                }
                if checked > 0 {
                    println!("  若把新文件偏移 {shift:+} 对齐：{same}/{checked} 字节相同");
                }
            }
            // 列出差异附近的内容
            for (label, buf) in [("原", &a), ("新", &b)] {
                let s = i.saturating_sub(16);
                let e = (i + 48).min(buf.len());
                let hex = buf[s..e].iter().map(|x| format!("{x:02x}")).collect::<Vec<_>>().join(" ");
                println!("{label} [{s}..{e}]: {hex}");
            }
        }
        None => {
            if a.len() == b.len() {
                println!("两文件完全相同");
            } else {
                println!("前 {n} 字节相同；差异全部在尾部（长度不同）");
            }
        }
    }
    if a.len() != b.len() {
        // 用"最长公共前缀 + 最长公共后缀"定位插入/删除的确切位置
        let prefix = (0..n).take_while(|&i| a[i] == b[i]).count();
        let suffix = (0..n - prefix)
            .take_while(|&k| a[a.len() - 1 - k] == b[b.len() - 1 - k])
            .count();
        println!("\n最长公共前缀: {prefix} 字节");
        println!("最长公共后缀: {suffix} 字节");
        let (longer, shorter, label) = if b.len() > a.len() { (&b, &a, "新文件多出") } else { (&a, &b, "原文件多出") };
        let ins_start = prefix;
        let ins_end = longer.len() - suffix;
        println!(
            "{label} {} 字节，位于 {}..{}（前后各留 8 字节上下文如下）",
            ins_end - ins_start,
            ins_start,
            ins_end
        );
        let ctx_start = ins_start.saturating_sub(8);
        let ctx_end = (ins_end + 8).min(longer.len());
        println!(
            "  上下文 [{ctx_start}..{ctx_end}]: {}",
            longer[ctx_start..ctx_end].iter().map(|x| format!("{x:02x}")).collect::<Vec<_>>().join(" ")
        );
        println!(
            "  插入内容: {}",
            longer[ins_start..ins_end].iter().map(|x| format!("{x:02x}")).collect::<Vec<_>>().join(" ")
        );
    }
    ExitCode::SUCCESS
}

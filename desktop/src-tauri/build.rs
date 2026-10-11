//! 构建脚本：Tauri 生成 + 声明"以管理员身份运行"的应用清单。
//!
//! 为什么要管理员权限：D2R 存档目录常被 Windows 安全中心的「受控文件夹访问」保护，
//! 普通权限进程写不进去（保存时报「拒绝访问 os error 5」）。让用户去改安全设置
//! 不现实，所以直接声明 UAC 提权。理由与各种硬性约束写在清单文件顶部。
//!
//! 走 Tauri 官方的 `WindowsAttributes::app_manifest`，**不要**自己用链接器参数或
//! winres 去嵌：Tauri 默认已嵌一份清单与 VERSION 资源，再嵌一次会撞成
//! `CVT1100: 资源重复` / `LNK1123`（实测过）。

fn main() {
    #[cfg(target_os = "windows")]
    {
        let manifest = include_str!("d2r-save-editor.manifest");
        check_manifest_is_ascii(manifest);
        let windows = tauri_build::WindowsAttributes::new().app_manifest(manifest);
        let attrs = tauri_build::Attributes::new().windows_attributes(windows);
        tauri_build::try_build(attrs).expect("Tauri 构建脚本失败");
    }

    #[cfg(not(target_os = "windows"))]
    {
        tauri_build::build();
    }
}

/// 清单必须是纯 ASCII，否则应用**根本启动不了**。
///
/// Windows 只给一句「并行配置不正确」，事件日志里是
/// `SideBySide 激活上下文生成失败 / 无效的 Xml 语法`，极难定位。
/// 真实原因：Tauri 把清单写进 `.rc` 资源时会按本地 ANSI 代码页转码，
/// 而清单自己声明 UTF-8，只要有中文就会字节不符 → 整份清单失效。
///
/// 所以这里在**构建期**直接拦下来，并指出是哪个字符、哪一行。
fn check_manifest_is_ascii(manifest: &str) {
    let mut problems = Vec::new();
    for (idx, line) in manifest.lines().enumerate() {
        for (col, ch) in line.chars().enumerate() {
            if !ch.is_ascii() {
                problems.push(format!(
                    "  第 {} 行第 {} 列: {ch:?}（U+{:04X}）",
                    idx + 1,
                    col + 1,
                    ch as u32
                ));
                break; // 每行只报第一个，避免刷屏
            }
        }
    }
    if !problems.is_empty() {
        panic!(
            "应用清单含非 ASCII 字符，会导致程序无法启动（Windows 报「并行配置不正确」）。\n\
             请把说明文字改成英文或移到代码注释里；界面上的中文不受影响。\n{}",
            problems.join("\n")
        );
    }
    if manifest.contains("--") {
        // 注释里出现连续短横也是无效 XML
        let in_comment = manifest
            .split("<!--")
            .skip(1)
            .any(|seg| seg.split("-->").next().is_some_and(|c| c.contains("--")));
        if in_comment {
            panic!("应用清单的 XML 注释里含连续短横 `--`，会导致程序无法启动。");
        }
    }
}

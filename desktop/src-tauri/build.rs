//! 构建脚本：Tauri 生成 + 声明"以管理员身份运行"的应用清单。
//!
//! 为什么要管理员权限：D2R 存档目录常被 Windows 安全中心的「受控文件夹访问」保护，
//! 普通权限进程写不进去（保存时报「拒绝访问 os error 5」）。让用户去改安全设置
//! 不现实，所以直接声明 UAC 提权，理由写在 `d2r-save-editor.manifest` 里。
//!
//! 走 Tauri 官方的 `WindowsAttributes::app_manifest`，原因是**不要**自己用链接器
//! 参数或 winres 去嵌：Tauri 默认已经嵌了一份清单与 VERSION 资源，
//! 再嵌一次会撞成 `CVT1100: 资源重复` / `LNK1123`（实测过）。

fn main() {
    #[cfg(target_os = "windows")]
    {
        let windows = tauri_build::WindowsAttributes::new()
            .app_manifest(include_str!("d2r-save-editor.manifest"));
        let attrs = tauri_build::Attributes::new().windows_attributes(windows);
        tauri_build::try_build(attrs).expect("Tauri 构建脚本失败");
    }

    #[cfg(not(target_os = "windows"))]
    {
        tauri_build::build();
    }
}

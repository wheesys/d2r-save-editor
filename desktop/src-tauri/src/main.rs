// Windows 下不弹控制台窗口。
//
// 注意这里**不带 `not(debug_assertions)` 条件**：之前只有 release 隐藏控制台，
// 结果 debug 版启动时会多出一个黑框，看起来像"开了两个窗口"（用户实际遇到过）。
// 桌面端不需要控制台；要诊断输出请用 `d2r-cli` 或 `write-diag` 那两个命令行工具。
#![windows_subsystem = "windows"]

fn main() {
    d2r_desktop_lib::run()
}

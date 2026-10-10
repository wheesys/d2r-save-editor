// Windows 下不弹控制台窗口
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    d2r_desktop_lib::run()
}

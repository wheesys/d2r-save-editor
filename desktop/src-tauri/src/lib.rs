//! 桌面端入口：注册工作区状态与全部命令。

pub mod commands;

use commands::AppState;
use d2r_app::Workspace;

/// 构建并运行 Tauri 应用。
pub fn run() {
    let workspace = Workspace::new(d2r_app::default_game_dir());

    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState(std::sync::Arc::new(std::sync::Mutex::new(workspace))))
        .setup(|app| {
            // 窗口在代码里创建（而不是写在 tauri.conf.json 的 app.windows 里），
            // 这样才能显式指定 WebView2 的数据目录。
            //
            // 背景：Tauri 在 Windows 上会强制给 webview 指定 `data_directory`
            // （默认 `app_local_data_dir()`，即 `%LOCALAPPDATA%\<identifier>`），
            // 并在建窗口前 `create_dir_all`。实测某些受限环境里该解析/创建会直接返回
            // "拒绝访问 (os error 5)"，应用在 setup 阶段就 panic。这里改用
            // exe 同目录（绿色版友好），不可写时退回系统临时目录。
            #[cfg(target_os = "windows")]
            let data_dir = webview_data_dir();

            let mut builder = tauri::WebviewWindowBuilder::new(
                app,
                "main",
                tauri::WebviewUrl::default(),
            )
            .title("D2R 存档修改器")
            .inner_size(1180.0, 800.0)
            .min_inner_size(900.0, 600.0)
            .resizable(true);

            #[cfg(target_os = "windows")]
            {
                builder = builder.data_directory(data_dir);
            }

            builder.build()?;
            Ok(())
        })
        // 开发/自测用：设置 D2R_OPEN_SAVE=<存档路径> 时，页面加载完成后自动打开该存档，
        // 免去每次手点文件对话框（对正常使用没有影响）。
        .on_page_load(|window, _payload| {
            let Ok(path) = std::env::var("D2R_OPEN_SAVE") else {
                return;
            };
            if path.is_empty() {
                return;
            }
            use tauri::Manager;
            // app_handle() 返回的是临时值，先 clone 成有所有权的 handle 再取 state
            let handle = window.app_handle().clone();
            let shared = handle.state::<AppState>().0.clone();
            if let Ok(mut workspace) = shared.lock() {
                match workspace.open(&path) {
                    Ok(info) => eprintln!("D2R_OPEN_SAVE：已打开 {}", info.path),
                    Err(e) => eprintln!("D2R_OPEN_SAVE：打开失败 {e}"),
                }
            };
        })
        .invoke_handler(tauri::generate_handler![
            commands::dataset_info,
            commands::open_save,
            commands::is_dirty,
            commands::character,
            commands::skills,
            commands::waypoints,
            commands::item_details,
            commands::item_stats,
            commands::apply_item_edits,
            commands::bundle_info,
            commands::layout,
            commands::layout_hints,
            commands::layout_questions,
            commands::set_layout,
            commands::validate_bounds,
            commands::item_detail,
            commands::stat_catalog,
            commands::import_item,
            commands::export_item,
            commands::write_blueprint,
            commands::read_blueprint,
            commands::current_save,
            commands::item_icons,
            commands::waypoint_names,
            commands::mercenary_items,
            commands::apply_edits,
            commands::verify,
            commands::save,
            commands::reload,
            commands::load_strings,
            commands::versions,
        ]);

    builder.run(tauri::generate_context!()).expect("Tauri 应用启动失败");
}

/// WebView2 用户数据目录：优先 exe 同目录的 `webview2-data`，否则退回系统临时目录。
#[cfg(target_os = "windows")]
fn webview_data_dir() -> std::path::PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let cand = dir.join("webview2-data");
            if std::fs::create_dir_all(&cand).is_ok() {
                return cand;
            }
        }
    }
    let mut fallback = std::env::temp_dir();
    fallback.push("d2r-save-editor");
    fallback.push("webview2-data");
    let _ = std::fs::create_dir_all(&fallback);
    fallback
}

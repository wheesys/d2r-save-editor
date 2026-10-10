//! Tauri 命令层：把 `d2r-app` 的工作区能力暴露给前端。
//!
//! 命令刻意保持"薄"：只做参数转换与错误包装，业务逻辑都在 `d2r-app`。

use std::path::PathBuf;
use std::sync::Mutex;

use d2r_app::{
    AppError, CharacterDto, DatasetInfo, ItemDto, SaveInfo, SaveResult, SkillDto, VerifyResult,
    WaypointsDto, Workspace,
};
use tauri::State;

/// 应用状态：一个工作区（同一时间只编辑一个存档）。
pub struct AppState(pub Mutex<Workspace>);

type R<T> = Result<T, AppError>;

/// 数据集信息（表数量、字符串表条目与语言）。
#[tauri::command]
pub fn dataset_info(state: State<'_, AppState>) -> DatasetInfo {
    state.0.lock().unwrap().dataset_info()
}

/// 打开存档。
#[tauri::command]
pub fn open_save(state: State<'_, AppState>, path: String) -> R<SaveInfo> {
    state.0.lock().unwrap().open(PathBuf::from(path))
}

/// 当前是否有未保存修改。
#[tauri::command]
pub fn is_dirty(state: State<'_, AppState>) -> bool {
    state.0.lock().unwrap().is_dirty()
}

/// 角色信息。
#[tauri::command]
pub fn character(state: State<'_, AppState>) -> R<CharacterDto> {
    state.0.lock().unwrap().character()
}

/// 技能列表。
#[tauri::command]
pub fn skills(state: State<'_, AppState>) -> R<Vec<SkillDto>> {
    state.0.lock().unwrap().skills()
}

/// 传送点状态。
#[tauri::command]
pub fn waypoints(state: State<'_, AppState>) -> R<Vec<WaypointsDto>> {
    state.0.lock().unwrap().waypoints()
}


/// 应用一批修改（不落盘）。
#[tauri::command]
pub fn apply_edits(state: State<'_, AppState>, edits: Vec<d2r_app::EditRequest>) -> R<usize> {
    state.0.lock().unwrap().apply(&edits)
}

/// 保存前自检。
#[tauri::command]
pub fn verify(state: State<'_, AppState>) -> R<VerifyResult> {
    state.0.lock().unwrap().verify()
}

/// 保存（校验通过才落盘，已有文件先备份）。
#[tauri::command]
pub fn save(state: State<'_, AppState>, path: Option<String>) -> R<SaveResult> {
    state.0.lock().unwrap().save_dto(path.map(PathBuf::from))
}

/// 重新加载（丢弃未保存修改）。
#[tauri::command]
pub fn reload(state: State<'_, AppState>) -> R<SaveInfo> {
    state.0.lock().unwrap().reload()
}

/// 挂载额外的字符串表目录。
#[tauri::command]
pub fn load_strings(state: State<'_, AppState>, dir: String) -> R<usize> {
    state.0.lock().unwrap().set_strings_dir(PathBuf::from(dir))
}

/// 物品明细（含中文名与占格）。
#[tauri::command]
pub fn item_details(state: State<'_, AppState>) -> R<Vec<ItemDto>> {
    state.0.lock().unwrap().item_details()
}

/// 物品汇总统计。
#[tauri::command]
pub fn item_stats(state: State<'_, AppState>) -> R<d2r_core::ItemStats> {
    state.0.lock().unwrap().item_stats()
}

/// 应用一批物品修改（不落盘）。
#[tauri::command]
pub fn apply_item_edits(state: State<'_, AppState>, edits: Vec<d2r_app::ItemEditRequest>) -> R<usize> {
    state.0.lock().unwrap().apply_item_edits(&edits)
}

/// 内置数据包信息（界面显示"数据来源"）。
#[tauri::command]
pub fn bundle_info(state: State<'_, AppState>) -> d2r_app::BundleInfo {
    state.0.lock().unwrap().bundle_info()
}
/// 应用版本信息。
#[tauri::command]
pub fn versions() -> serde_json::Value {
    serde_json::json!({
        "core": d2r_core::VERSION,
        "tauri": tauri::VERSION,
    })
}

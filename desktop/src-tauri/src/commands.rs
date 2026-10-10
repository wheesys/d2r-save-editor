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
pub struct AppState(pub std::sync::Arc<Mutex<Workspace>>);

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
/// 当前布局设置（背包/箱子尺寸、佣兵全装备）。
#[tauri::command]
pub fn layout(state: State<'_, AppState>) -> d2r_core::LayoutSettings {
    state.0.lock().unwrap().layout().clone()
}

/// 从存档推断布局（给出需要用户确认的项）。
#[tauri::command]
pub fn layout_hints(state: State<'_, AppState>) -> R<d2r_core::LayoutHints> {
    state.0.lock().unwrap().layout_hints()
}

/// 需要用户确认的项（空数组 = 不用问）。
#[tauri::command]
pub fn layout_questions(state: State<'_, AppState>) -> R<Vec<String>> {
    state.0.lock().unwrap().layout_questions()
}

/// 用户确认/修改布局设置。
#[tauri::command]
pub fn set_layout(state: State<'_, AppState>, layout: d2r_core::LayoutSettings) {
    state.0.lock().unwrap().set_layout(layout);
}

/// 按当前布局做越界校验。
#[tauri::command]
pub fn validate_bounds(state: State<'_, AppState>) -> R<Vec<d2r_core::BoundsIssue>> {
    state.0.lock().unwrap().validate_bounds()
}
/// 一件物品的完整明细（含带名字的属性列表）。
#[tauri::command]
pub fn item_detail(state: State<'_, AppState>, index: usize) -> R<d2r_app::ItemDetailDto> {
    state.0.lock().unwrap().item_detail_dto(index)
}

/// 属性目录（界面做"添加属性"下拉用）。`all = true` 时含内部字段。
#[tauri::command]
pub fn stat_catalog(state: State<'_, AppState>, all: Option<bool>) -> Vec<d2r_app::StatCatalogEntry> {
    state.0.lock().unwrap().stat_catalog(all.unwrap_or(false))
}

/// 导入一件物品：从 JSON 蓝图克隆模板并覆盖属性。
#[tauri::command]
pub fn import_item(
    state: State<'_, AppState>,
    blueprint_json: String,
    template: usize,
) -> R<d2r_core::ImportOutcome> {
    let blueprint = d2r_core::parse_blueprint(&blueprint_json)
        .map_err(|e| d2r_app::AppError::invalid(format!("蓝图不合法：{e}")))?;
    state.0.lock().unwrap().import_blueprint(&blueprint, template)
}

/// 导出某件物品为蓝图 JSON。
#[tauri::command]
pub fn export_item(state: State<'_, AppState>, index: usize) -> R<d2r_core::ItemBlueprint> {
    state.0.lock().unwrap().export_blueprint(index)
}
/// 把物品蓝图写到文件。
#[tauri::command]
pub fn write_blueprint(path: String, json: String) -> R<String> {
    std::fs::write(&path, json).map_err(|e| d2r_app::AppError::new("write", e.to_string()))?;
    Ok(path)
}

/// 从文件读物品蓝图文本。
#[tauri::command]
pub fn read_blueprint(path: String) -> R<String> {
    std::fs::read_to_string(&path).map_err(|e| d2r_app::AppError::new("read", e.to_string()))
}
/// 当前已打开的存档（未打开时返回 null）。
#[tauri::command]
pub fn current_save(state: State<'_, AppState>) -> Option<SaveInfo> {
    state.0.lock().unwrap().current_save()
}
/// 应用版本信息。
#[tauri::command]
pub fn versions() -> serde_json::Value {
    serde_json::json!({
        "core": d2r_core::VERSION,
        "tauri": tauri::VERSION,
    })
}

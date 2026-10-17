//! Tauri 命令层：把 `d2r-app` 的工作区能力暴露给前端。
//!
//! 命令刻意保持"薄"：只做参数转换与错误包装，业务逻辑都在 `d2r-app`。

use std::path::PathBuf;
use std::sync::Mutex;

use d2r_app::{
    AppError, CharacterDto, DatasetInfo, ItemDto, QuestDifficultyDto, SaveInfo, SaveResult,
    SkillDto, SkillsViewDto, VerifyResult, WaypointsDto, Workspace,
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

/// 技能列表（平铺，不带技能树分组）。
#[tauri::command]
pub fn skills(state: State<'_, AppState>) -> R<Vec<SkillDto>> {
    state.0.lock().unwrap().skills()
}

/// 技能页视图：按游戏的**三系技能树**分组（含未归类的技能）。
#[tauri::command]
pub fn skills_view(state: State<'_, AppState>) -> R<SkillsViewDto> {
    state.0.lock().unwrap().skills_view()
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
///
/// `target` 不传时作用于主物品列表；佣兵段与主列表**序号体系不同**，
/// 所以界面编辑佣兵物品时必须显式传 `Mercenary`，否则会改错物品。
#[tauri::command]
pub fn apply_item_edits(
    state: State<'_, AppState>,
    edits: Vec<d2r_app::ItemEditRequest>,
    target: Option<d2r_core::ItemTarget>,
) -> R<usize> {
    let target = target.unwrap_or_default();
    state.0.lock().unwrap().apply_item_edits_on(target, &edits)
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

/// 用户确认/修改布局设置（会写进设置文件，下次启动直接带上）。
///
/// 写盘失败会返回错误——界面必须如实告诉用户"这次生效但下次会丢"，
/// 不能像以前那样吞掉。
#[tauri::command]
pub fn set_layout(state: State<'_, AppState>, layout: d2r_core::LayoutSettings) -> R<()> {
    state.0.lock().unwrap().set_layout(layout)
}

/// 设置文件路径 + 上一次保存的结果（界面用来排查"设置没保存"）。
#[tauri::command]
pub fn settings_status(state: State<'_, AppState>) -> (String, Option<String>) {
    let ws = state.0.lock().unwrap();
    (ws.config_path(), ws.settings_note().map(|s| s.to_string()))
}

/// 按当前布局做越界校验。
#[tauri::command]
pub fn validate_bounds(state: State<'_, AppState>) -> R<Vec<d2r_core::BoundsIssue>> {
    state.0.lock().unwrap().validate_bounds()
}
/// 一件物品的完整明细（含带名字的属性列表）。
#[tauri::command]
pub fn item_detail(
    state: State<'_, AppState>,
    index: usize,
    target: Option<d2r_core::ItemTarget>,
) -> R<d2r_app::ItemDetailDto> {
    let target = target.unwrap_or_default();
    state.0.lock().unwrap().item_detail_dto_on(target, index)
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

/// 导出当前存档的**全部物品**（数组，每项一个蓝图）。
///
/// 与 `export_item` 的区别：一次导出整栏，便于备份/迁移整套装备。
#[tauri::command]
pub fn export_items(state: State<'_, AppState>) -> R<Vec<d2r_core::ItemBlueprint>> {
    state.0.lock().unwrap().export_blueprints()
}

/// 删除一件物品（含其镶嵌物）。
#[tauri::command]
pub fn delete_item(state: State<'_, AppState>, index: usize) -> R<usize> {
    state.0.lock().unwrap().delete_item(index)
}

/// 给物品起自定义名字（空字符串清除命名）。
#[tauri::command]
pub fn rename_item(state: State<'_, AppState>, index: usize, name: String) -> R<()> {
    state.0.lock().unwrap().rename_item(index, &name)
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
/// 物品代码 → 图标的 data URL（物品网格用）。数据包缺失时返回空表。
#[tauri::command]
pub fn item_icons() -> Vec<(String, String)> {
    d2r_app::bundle::item_icons()
}
/// 小站名字表（下标 = 存档里的位下标）。
#[tauri::command]
pub fn waypoint_names(state: State<'_, AppState>) -> Vec<String> {
    state.0.lock().unwrap().waypoint_names()
}
/// 佣兵装备段里的物品（含完整明细，界面直接复用物品属性弹窗）。
#[tauri::command]
pub fn mercenary_items(state: State<'_, AppState>) -> R<Vec<d2r_app::ItemDetailDto>> {
    state.0.lock().unwrap().mercenary_item_details()
}
/// 任务视图（每个难度 → 各幕 → 各槽）。
#[tauri::command]
pub fn quests(state: State<'_, AppState>) -> R<Vec<QuestDifficultyDto>> {
    state.0.lock().unwrap().quests()
}

/// 应用任务修改。
#[tauri::command]
pub fn apply_quest_edits(state: State<'_, AppState>, edits: Vec<d2r_core::Edit>) -> R<usize> {
    state.0.lock().unwrap().apply_quest_edits(&edits)
}
/// 记下刚打开的存档路径（跨重启保留），供下次定对话框起始目录。
///
/// 前端在 `open_save` 成功后调用。写盘失败不影响结果，所以不返回错误。
#[tauri::command]
pub fn remember_open_path(state: State<'_, AppState>, path: String) {
    state.0.lock().unwrap().remember_path(&path);
}

/// 最近打开的存档路径（最新的在最前）。前端用它给文件对话框定起始目录。
#[tauri::command]
pub fn recent_paths(state: State<'_, AppState>) -> Vec<String> {
    state.0.lock().unwrap().recent_paths().to_vec()
}

/// 下次"打开存档"的起始目录（目录不存在时返回 `None`）。
#[tauri::command]
pub fn default_open_dir(state: State<'_, AppState>) -> Option<String> {
    state.0.lock().unwrap().default_open_dir()
}
/// 应用版本信息。
#[tauri::command]
pub fn versions() -> serde_json::Value {
    serde_json::json!({
        "core": d2r_core::VERSION,
        "tauri": tauri::VERSION,
    })
}


/// 内置装备库（暗金 / 套装）。第一次调用后走后端缓存，不会重复解析。
#[tauri::command]
pub fn item_library(state: State<'_, AppState>) -> Vec<d2r_app::LibraryItem> {
    state.0.lock().unwrap().item_library()
}

/// 从内置装备库导入一件物品到指定容器（自动找空位）。
#[tauri::command]
pub fn import_library_item(
    state: State<'_, AppState>,
    id: String,
    store_page: String,
) -> R<d2r_app::library::ImportResult> {
    state.0.lock().unwrap().import_library_item(&id, &store_page)
}

/// 复制一件物品到同容器的空位（序号会变，返回新序号）。
#[tauri::command]
pub fn duplicate_item(
    state: State<'_, AppState>,
    index: usize,
    store_page: Option<String>,
) -> R<d2r_app::library::ImportResult> {
    state
        .0
        .lock()
        .unwrap()
        .duplicate_item(index, store_page.as_deref())
}

/// 项目的 Releases 页面（"检查更新"按钮打开的就是它）。
pub const RELEASES_URL: &str = "https://github.com/wheesys/d2r-save-editor/releases";

/// 当前版本号（取自 `Cargo.toml`，界面右上角显示）。
#[tauri::command]
pub fn app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// 用系统默认浏览器打开 Releases 页面。
///
/// 不在程序里做联网检查：那要额外拉网络权限、还要处理失败分支，
/// 而用户真正需要的是"能拿到新版本"，直接打开页面最省事也最可靠。
#[tauri::command]
pub fn open_releases_page() -> R<String> {
    #[cfg(target_os = "windows")]
    let status = std::process::Command::new("cmd")
        .args(["/C", "start", "", RELEASES_URL])
        .status();
    #[cfg(target_os = "macos")]
    let status = std::process::Command::new("open").arg(RELEASES_URL).status();
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    let status = std::process::Command::new("xdg-open").arg(RELEASES_URL).status();

    match status {
        Ok(s) if s.success() => Ok(RELEASES_URL.to_string()),
        Ok(_) => Err(d2r_app::AppError::new("open", "浏览器没有成功打开".to_string())),
        Err(e) => Err(d2r_app::AppError::new("open", format!("打不开浏览器：{e}"))),
    }
}

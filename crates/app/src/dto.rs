//! 界面与 CLI 共用的数据传输对象（DTO）。
//!
//! 全部 `serde` 可序列化：Tauri 命令直接把这些结构返回给前端，
//! 前端不需要知道存档格式，也不需要知道数据集细节。

use serde::{Deserialize, Serialize};

/// 当前挂载的数据集信息。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DatasetInfo {
    /// 数据集根目录（loader 编译产物）。
    pub root: String,
    /// excel 表数量。
    pub tables: usize,
    /// 字符串表条目数。
    pub strings: usize,
    /// 字符串表语言列表。
    pub languages: Vec<String>,
}

/// 存档概览。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SaveInfo {
    /// 文件路径。
    pub path: String,
    /// 类型（`character` / `stash`）。
    pub kind: String,
    /// 存档版本号（`.d2s` 取头部；`.d2i` 取偏移 8）。
    pub version: u32,
    /// 文件字节数。
    pub size: usize,
    /// 校验和（`.d2i` 没有）。
    pub checksum: Option<u32>,
    /// 往返是否逐字节一致。
    pub roundtrip_identical: bool,
}

/// 角色信息（可编辑项）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CharacterDto {
    /// 角色名。
    pub name: String,
    /// 职业。
    pub class: String,
    /// 等级。
    pub level: u8,
    /// 力量。
    pub strength: i64,
    /// 敏捷。
    pub dexterity: i64,
    /// 体力。
    pub vitality: i64,
    /// 精力。
    pub energy: i64,
    /// 未分配属性点。
    pub stat_points: i64,
    /// 未分配技能点。
    pub skill_points: i64,
    /// 生命 / 上限。
    pub life: i64,
    /// 生命上限。
    pub max_life: i64,
    /// 法力 / 上限。
    pub mana: i64,
    /// 法力上限。
    pub max_mana: i64,
    /// 体力值。
    pub stamina: i64,
    /// 经验值。
    pub experience: i64,
    /// 身上金币。
    pub gold: i64,
    /// 储物箱金币。
    pub stash_gold: i64,
}

/// 一条技能。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SkillDto {
    /// 槽位下标。
    pub index: usize,
    /// 技能 ID。
    pub skill_id: Option<u16>,
    /// 技能显示名。
    pub name: Option<String>,
    /// 已加点数。
    pub level: u8,
}

/// 传送点视图。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WaypointsDto {
    /// 难度名。
    pub difficulty: String,
    /// 每个传送点是否开通。
    pub active: Vec<bool>,
}

/// 一条物品（供物品页显示）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ItemDto {
    /// 物品代码。
    pub code: String,
    /// 显示名。
    pub name: String,
    /// 名字是否解析成功。
    pub name_resolved: bool,
    /// 品质。
    pub quality: String,
    /// 物品等级。
    pub level: u8,
    /// 位置模式。
    pub mode: String,
    /// 装备位。
    pub body_location: String,
    /// 所在存储页。
    pub store_page: String,
    /// 列。
    pub inv_x: u8,
    /// 行。
    pub inv_y: u8,
    /// 属性条数。
    pub stats_count: usize,
}

/// 修改请求（前端 → 后端）。
///
/// 与 `d2r_core::Edit` 的 JSON 形态一致（`tag = "kind"`）。
pub type EditRequest = d2r_core::Edit;

/// 保存结果。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SaveResult {
    /// 写入路径。
    pub path: String,
    /// 写入字节数。
    pub bytes: usize,
    /// 是否创建了备份。
    pub backed_up: bool,
    /// 应用了多少条修改。
    pub edits: usize,
}

/// 校验结果（保存前的自检）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VerifyResult {
    /// 写出并重新解析是否通过。
    pub ok: bool,
    /// 写出后的字节数。
    pub bytes: usize,
    /// 说明信息（失败时是原因）。
    pub message: String,
}

/// 统一错误（给前端的字符串形态）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppError {
    /// 错误类别。
    pub kind: String,
    /// 错误消息。
    pub message: String,
}

impl AppError {
    /// 构造。
    pub fn new(kind: impl Into<String>, message: impl Into<String>) -> Self {
        Self { kind: kind.into(), message: message.into() }
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.kind, self.message)
    }
}

impl std::error::Error for AppError {}

impl From<d2r_core::EditError> for AppError {
    fn from(e: d2r_core::EditError) -> Self {
        let kind = match e {
            d2r_core::EditError::Load(_) => "load",
            d2r_core::EditError::Invalid(_) => "invalid",
            d2r_core::EditError::Write(_) => "write",
            d2r_core::EditError::Verification(_) => "verification",
        };
        Self::new(kind, e.to_string())
    }
}

impl From<d2r_core::SaveError> for AppError {
    fn from(e: d2r_core::SaveError) -> Self {
        Self::new("save", e.to_string())
    }
}

impl From<d2r_data::DataError> for AppError {
    fn from(e: d2r_data::DataError) -> Self {
        Self::new("data", e.to_string())
    }
}

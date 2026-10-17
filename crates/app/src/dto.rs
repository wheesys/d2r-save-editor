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
    /// 存档里的角色名（`.d2i` 或读不到时为空）。
    ///
    /// 用途是保存前给用户确认"要覆盖的是哪个角色"——同目录下多个存档容易改错。
    pub character_name: String,
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

/// 一棵技能树（含树内技能），供技能页按游戏样式显示。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SkillTreeDto {
    /// 树名（游戏内中文名）。
    pub name: String,
    /// 该树的技能，顺序即游戏里的排列顺序（6 行 × 3 列行优先）。
    pub skills: Vec<SkillDto>,
    /// 与 [`Self::skills`] 一一对应的等级门槛（0 表示空格）。
    pub levels: Vec<u8>,
    /// 与 [`Self::skills`] 一一对应的图标编号（1..=30，0 表示空格）。
    pub icons: Vec<u8>,
}

/// 技能页视图：按游戏的"三系技能树"分组。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SkillsViewDto {
    /// 职业代码（`PlayerClass.txt` 的 `Code`，如 `sor`；读不到时为空）。
    pub class_code: String,
    /// 职业中文名（界面显示用）。
    pub class_name: String,
    /// 三棵树。职业未知时为**空表**（此时界面退回平铺列表，不要瞎猜归属）。
    pub trees: Vec<SkillTreeDto>,
    /// 未被归类的技能（正常情况下应为空；不为空时界面会照样列出来，
    /// 免得某个技能因为表写漏了而完全无法编辑）。
    pub unclassified: Vec<SkillDto>,
    /// 未分配技能点。
    pub skill_points: i64,
    /// 技能 ID → 英文技能名（`skills.txt` 的 `skill` 列）。
    ///
    /// 用途是按名字找图标：界面上的技能名是本地化的（"火焰弹"），
    /// 而图标是按英文名命名的。
    pub english_names: std::collections::HashMap<String, String>,
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

/// 一个任务槽（供任务页显示）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct QuestDto {
    /// 幕下标（0–4）。
    pub act: usize,
    /// 槽下标（0–15）。
    pub slot: usize,
    /// 任务名（来自文本表；非任务槽会是占位名）。
    pub name: String,
    /// 该槽的原始位。界面显示它便于核对，不参与判定。
    pub flags: u16,
    /// 状态。
    pub state: d2r_data::QuestState,
    /// 是不是可编辑的真任务槽。
    ///
    /// 序章槽、收尾槽、Act IV 的 NPC 对话槽、Act V 的未使用槽都是 `false`，
    /// 界面据此禁用勾选——这些槽改了没有意义甚至有害。
    pub editable: bool,
}

/// 一个难度的任务。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct QuestDifficultyDto {
    /// 难度名。
    pub difficulty: String,
    /// 难度下标（0=普通 1=噩梦 2=地狱）。
    pub difficulty_index: u8,
    /// 各幕的任务，`[幕][槽]`。
    pub acts: Vec<Vec<QuestDto>>,
}

/// 一条物品（供物品页显示）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ItemDto {
    /// 序号（编辑时用它定位物品）。
    pub index: usize,
    /// 物品代码。
    pub code: String,
    /// 显示名。
    pub name: String,
    /// 名字是否解析成功。
    pub name_resolved: bool,
    /// 自定义名字（`personalized_name`，空表示没有）。
    pub personalized_name: String,
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
    /// 防御值。
    pub defense: Option<i32>,
    /// 当前耐久。
    pub durability: Option<i32>,
    /// 最大耐久。
    pub max_durability: Option<i32>,
    /// 堆叠数量。
    pub quantity: Option<i32>,
    /// 镶嵌孔数。
    pub socket_count: usize,
    /// 已镶嵌数量。
    pub socketed_count: usize,
    /// 是否带 SOCKETED 标志（孔数字段只在带标志时写入）。
    pub socketed_flag: bool,
    /// 占用宽（格）。
    pub width: u32,
    /// 占用高（格）。
    pub height: u32,
}

/// 物品上的一条属性（含中文名与步长，界面直接用）。
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemStatDto {
    /// 属性 ID。
    pub id: u16,
    /// 层（多数为 0）。
    pub layer: i32,
    /// 当前数值。
    pub value: i64,
    /// 中文名（解析不出时为 `属性#id`）。
    pub name: String,
    /// 内部名（数据表里的 `Stat`）。
    pub internal: String,
    /// 值的最小小步长（>1 时值必须是它的倍数）。
    pub step: i64,
    /// 是否带参数位（值里含参数分量，界面应提示）。
    pub has_param_bits: bool,
    /// 游戏是否会为它生成描述（`false` = 内部字段）。
    pub user_visible: bool,
    /// 可表示的**最小值（用户可见值）**。查不到属性规格时为 `None`。
    pub min_value: Option<f64>,
    /// 可表示的**最大值（用户可见值）**。
    pub max_value: Option<f64>,
    /// 范围的可读文本（用户可见值单位），例如 `0–127`、`0–7.875`。
    pub range_text: Option<String>,
    /// 是否"按等级加成"（`*perlevel` 属性）。
    ///
    /// 这类属性的**数值部分是系数**，界面显示的是"每级加多少"
    /// （系数 `63`、`op param=3` → 每级 `7.875`）。
    /// 用户明确要求："只需要展示每级加多少就行，完全不用管一共加了多少"。
    pub per_level: bool,
    /// 给用户看的**值**。
    ///
    /// 按等级加成的是"每级加多少"（可能是小数，`7.875`）；
    /// 生命/法力这类移位属性是换算后的整数（16128 → 63）。
    pub display_value: f64,
    /// 界面输入框的最小变化量。
    ///
    /// 按等级加成的是 `1/2^op_param`（生命是 `0.125`：填 7.875 会存成 63）。
    pub display_step: f64,
    /// 带参数的属性的**参数含义与已解析的名字**（如光环名）。
    ///
    /// 例如 `item_aura` 的参数是光环，界面可显示成"获得 N 级 XXX 光环"。
    pub param_label: Option<String>,
    /// 该属性是否带参数位（`Save Param Bits > 0`）。
    ///
    /// 带参数位的属性有**两个可改的数**，界面上叫"值1/值2"
    /// （`Save Param Bits > 0` 就是"有值2"）。两个都要展示，
    /// 否则同一件物品上会出现两行看起来一模一样的属性，用户无法区分。
    ///
    /// 序列化成 `layer`：这是存档里的真实字段名（`d2s` 的 `Stat.layer`），
    /// 保持与数据一致，界面把它显示成"值2"。
    pub has_layer: bool,
    /// 层（参数）的**可选值**：层号 + 该层含义的名字。
    ///
    /// 界面把它做成下拉框，用户就能像游戏里那样"改成法师的技能等级"
    /// 而不是去猜层号。空表示这个属性的层没有已知的可选集合，
    /// 此时界面只显示层号（不猜）。
    pub param_options: Vec<ParamOptionDto>,
}

/// 参数位（层）的一个可选值。
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParamOptionDto {
    /// 层号（写回存档的值）。
    pub layer: i32,
    /// 该层的含义（如"法师"、"狂热"、"火焰弹"）。
    pub label: String,
    /// 所属分组（如技能所属职业）。界面用它做**两级选单**：
    /// 第一级选分组（职业），第二级选具体项（技能）。`None` 表示不分级。
    #[serde(default)]
    pub group: Option<String>,
}

/// 一件物品的完整明细（界面的物品编辑面板用）。
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemDetailDto {
    /// 基础信息（与列表里同一件）。
    pub item: ItemDto,
    /// 属性列表（含名字）。
    pub stats: Vec<ItemStatDto>,
    /// 符文之语 ID。
    pub runeword_id: Option<u16>,
}
/// 修改请求（前端 → 后端）。
///
/// 与 `d2r_core::Edit` 的 JSON 形态一致（`tag = "kind"`）。
pub type EditRequest = d2r_core::Edit;

/// 物品修改请求（前端 → 后端）。
pub type ItemEditRequest = d2r_core::ItemEdit;

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
        Self {
            kind: kind.into(),
            message: message.into(),
        }
    }
}

impl AppError {
    /// 取值不合法（越界、超位宽等）。
    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new("invalid", message)
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

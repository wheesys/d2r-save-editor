//! `d2r-core`：D2R 存档（`.d2s` / `.d2i`）读写与校验内核。
//!
//! 分层（对应调研文档 §4）：
//!
//! ```text
//! d2r-core   位流/结构读写、往返校验、版本识别   ← 本 crate
//! d2r-data   数据集加载（excel txt/json、字符串表）、容器规格、属性位宽
//! d2r-cli    命令行验证工具（round-trip / 数据表比对）
//! （后续）Tauri 界面
//! ```
//!
//! 硬性约束：
//! - **不写死游戏常量**：属性位宽、物品宽高、容器行列数一律来自数据集；
//! - **失败安全**：解析不了就报错，绝不"猜着写"；
//! - **未知数据透传**：读到时原样保留，写回时不丢弃。

pub mod blueprint;
pub mod d2i;
pub mod edit;
pub mod header;
pub mod items;
pub mod layout;
pub mod save;

pub use blueprint::{
    export_blueprint, parse_blueprint, BlueprintStat, ImportOutcome, ItemBlueprint,
};
pub use d2i::{D2iError, SharedStash};
pub use edit::{CharacterView, Edit, EditError, EditSession, SkillView};
pub use header::{HeaderError, SaveHeader, SaveVersion, MAGIC};
pub use items::{BoundsIssue, ItemDetail, ItemEdit, ItemStats, MAX_SOCKETS};
pub use layout::{needs_confirmation, LayoutHints, LayoutSettings};
pub use save::{RoundTrip, SaveError, SaveFile, SaveKind};

/// 编译期版本信息。
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

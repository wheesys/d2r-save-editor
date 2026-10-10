//! `d2r-app`：编辑器服务层。
//!
//! CLI 与 Tauri 界面共用这一层，界面只负责展示与收集输入。
//! 提供的都是**纯数据**接口（`serde` 可序列化），所以前端不需要了解存档格式。
//!
//! ```text
//! Workspace::open_save(path)       加载存档 + 自动挂载数据集与字符串表
//! Workspace::character()           角色信息（含中文名）
//! Workspace::skills()              技能列表（含中文名）
//! Workspace::waypoints()           三难度传送点状态
//! Workspace::apply(edits)          应用修改（不落盘，原子）
//! Workspace::save(path)            校验后落盘（带备份）
//! ```

pub mod bundle;
pub mod dto;
pub mod workspace;

pub use bundle::BundleInfo;
pub use dto::*;
pub use workspace::StatCatalogEntry;
pub use workspace::{default_game_dir, find_strings_dir, SaveOutcome, Workspace};

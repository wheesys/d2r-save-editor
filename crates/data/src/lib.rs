//! `d2r-data`：数据集加载（游戏表 + 字符串表 + mod 描述）。
//!
//! 核心原则（调研文档 §4.1）：**不写死任何游戏常量**。
//! 容器尺寸、物品宽高、属性位宽、词缀等全部来自数据集，mod 改表即自动适配。
//!
//! 目前支持的数据来源：D2RLoader 的编译产物
//!
//! ```text
//! <游戏>\d2rloader\data\compiler\<游戏版本>\data\global\excel\       合并了 mod 的表
//! <游戏>\d2rloader\data\compiler\<游戏版本>\data\global\excel\base\  原版基准表
//! ```
//!
//! 后续扩展：官方 CASC 导出的 `excel`（官方基准）、`local/lng/strings`（字符串表）、
//! mod 自带 txt、以及 D2R 的 json 表格式。

pub mod dataset;
pub mod strings;
pub mod table;

pub use dataset::{Dataset, ModInfo, TableStore};
pub use strings::Strings;
pub use table::{DataError, Table};

/// 在游戏目录下定位 loader 编译产物（数据集根），取版本号最大的一个。
///
/// 例如 `<游戏>\d2rloader\data\compiler\3.2.0`。
pub fn find_loader_dataset(game_root: impl AsRef<std::path::Path>) -> Option<std::path::PathBuf> {
    let compiler = game_root
        .as_ref()
        .join("d2rloader")
        .join("data")
        .join("compiler");
    let mut candidates: Vec<std::path::PathBuf> = std::fs::read_dir(&compiler)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.join("data").join("global").join("excel").is_dir())
        .collect();
    // 按版本号数值排序，越大越新
    candidates.sort_by_key(|p| {
        p.file_name()
            .and_then(|s| s.to_str())
            .and_then(|s| s.parse::<f32>().ok())
            .map(|v| (v * 1000.0) as i64)
            .unwrap_or(0)
    });
    candidates.pop()
}

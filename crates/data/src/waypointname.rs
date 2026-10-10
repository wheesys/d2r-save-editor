//! 传送点（小站）名称：存档里的位下标 → 小站中文名。
//!
//! 映射链（已实测确认）：
//!
//! ```text
//! 存档里的小站位下标（每难度 39 位）
//!        │
//!        └──→ Levels.txt 的 Waypoint 列（取值 0..=38，连续无缺失）
//!                 │
//!                 └─ LevelName → 文本表（levels.json）→ 中文名
//! ```
//!
//! 关键点：`Waypoint` 列的取值**就是存档里的位下标**，所以按它排序后
//! 直接用下标取名字即可；`Waypoint = 255` 表示该关卡没有小站。

use crate::strings::Strings;
use crate::table::Table;

/// 小站总数（每难度）。
pub const WAYPOINT_COUNT: usize = 39;

/// 传送点名称表。
#[derive(Debug, Clone, Default)]
pub struct WaypointNamer {
    /// 按下标排列的小站名（解析不出时用关卡内部名兜底）。
    names: Vec<String>,
}

impl WaypointNamer {
    /// 从 `Levels` 表与文本表构建。
    pub fn new(levels: &Table, strings: &Strings) -> Self {
        let mut indexed: Vec<(usize, String)> = Vec::new();
        for row in 0..levels.len() {
            let Some(wp) = levels.int(row, "Waypoint") else {
                continue;
            };
            // 只有 0..=38 是有小站的关卡；255 是"没有小站"
            if !(0..WAYPOINT_COUNT as i64).contains(&wp) {
                continue;
            }
            let Some(level_name) = levels.cell_nonempty(row, "LevelName") else {
                continue;
            };
            let display = strings
                .text(level_name)
                .filter(|s| !s.is_empty())
                .unwrap_or(level_name)
                .to_string();
            indexed.push((wp as usize, display));
        }
        indexed.sort_by_key(|(i, _)| *i);

        // 按下标落位；缺的补成占位，避免整体错位
        let mut names = vec![String::new(); indexed.iter().map(|(i, _)| *i + 1).max().unwrap_or(0)];
        for (i, name) in indexed {
            if i < names.len() {
                names[i] = name;
            }
        }
        Self { names }
    }

    /// 解析出几个小站名。
    pub fn len(&self) -> usize {
        self.names.iter().filter(|n| !n.is_empty()).count()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// 按下标取名字。
    ///
    /// 存档里每个难度是 **40 位**，而真实小站只有 39 个（0..=38），
    /// 第 39 位是填充位——它没有名字，返回空串，界面应跳过而不是显示"小站#39"。
    pub fn name_of(&self, index: usize) -> String {
        self.names.get(index).cloned().unwrap_or_default()
    }

    /// 全部名字（下标顺序）。
    pub fn all(&self) -> &[String] {
        &self.names
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_namer_has_no_names() {
        let n = WaypointNamer::default();
        assert!(n.is_empty());
        // 没有映射的位置返回空串：调用方（界面）据此跳过，不显示"小站#N"
        assert_eq!(n.name_of(5), "");
    }

    #[test]
    fn real_names_by_index() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("app")
            .join("data-bundle");
        if !dir.join("excel").is_dir() {
            eprintln!("跳过：没有数据包");
            return;
        }
        let mut store = crate::dataset::TableStore::empty();
        store.add_override_dir(dir.join("excel"));
        let mut strings = Strings::default();
        for e in std::fs::read_dir(dir.join("strings")).unwrap().flatten() {
            if let Ok(text) = std::fs::read_to_string(e.path()) {
                strings.merge(Strings::from_json_bytes(text.as_bytes()));
            }
        }
        let levels = store.get("Levels").expect("Levels 表");
        let namer = WaypointNamer::new(levels, &strings);

        assert_eq!(namer.len(), WAYPOINT_COUNT, "应解析出 39 个小站名");
        assert!(namer.name_of(0).contains("流亡者营地"));
        assert!(namer.name_of(38).contains("世界之石"));
        // 名字里不应出现编号兜底
        for i in 0..WAYPOINT_COUNT {
            assert!(!namer.name_of(i).starts_with("小站#"), "{i} 号没解析出名字");
        }
    }
}

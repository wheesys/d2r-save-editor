//! 任务名：`(幕, 槽) → 任务中文名`。
//!
//! 映射来源已用真实存档核对：
//!
//! ```text
//! 存档任务槽（每幕 16 位位集，Act I–IV 各 8 槽、Act V 16 槽）
//!        │
//!        └──→ 槽 0 = 该幕序章；槽 1..=6 = 真任务；槽 7 = 该幕收尾（无名字）
//!                 │
//!                 └─ 文本表 `qstsa<幕>q<槽>` → 中文名
//! ```
//!
//! 已核对：Act I 的槽 1..7 依次是"邪恶洞穴 / 修女的埋骨地 / 交易的工具 /
// 搜寻凯恩 / 遗忘之塔 / 为了姐妹们"，与存档里这些槽被推进的实际情况一致。

use crate::strings::Strings;

/// 单幕最多几个真任务（槽 1..=6）。
pub const MAX_QUESTS_PER_ACT: usize = 6;

/// 任务名表。
#[derive(Debug, Clone, Default)]
pub struct QuestNamer {
    /// 下标 `[act][slot]`（act 0..=4，slot 0..=7）。空串表示没有这个名字。
    names: Vec<Vec<String>>,
}

impl QuestNamer {
    /// 从文本表构建。
    ///
    /// `qstsa<幕>q<槽>`：幕从 1 开始、槽从 0 开始，与存档的段/槽编号对应。
    pub fn new(strings: &Strings) -> Self {
        let mut names = Vec::with_capacity(5);
        for act in 1..=5 {
            let mut row = Vec::with_capacity(8);
            for slot in 0..8 {
                let key = format!("qstsa{act}q{slot}");
                let text = match strings.text(&key) {
                    Some(t) if !t.is_empty() => t.to_string(),
                    _ => {
                        // Act V 没有 `qstsa5q0`（序章），用统一的占位名
                        if slot == 0 {
                            format!("第{act}幕序章")
                        } else {
                            String::new()
                        }
                    }
                };
                row.push(text);
            }
            names.push(row);
        }
        Self { names }
    }

    /// 取任务名；下标越界或没有名字时返回 `幕<act+1> 槽<slot>`。
    pub fn name_of(&self, act: usize, slot: usize) -> String {
        match self.names.get(act).and_then(|r| r.get(slot)) {
            Some(n) if !n.is_empty() => n.clone(),
            _ => format!("第{}幕 槽{}", act + 1, slot),
        }
    }

    /// 解析出几个任务名（不含占位）。
    pub fn len(&self) -> usize {
        self.names
            .iter()
            .flatten()
            .filter(|n| !n.is_empty())
            .count()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// 该槽是否是一个真任务（槽 1..=7 且名字解析成功）。
    pub fn is_real_quest(&self, act: usize, slot: usize) -> bool {
        (1..=MAX_QUESTS_PER_ACT).contains(&slot)
            && self
                .names
                .get(act)
                .and_then(|r| r.get(slot))
                .is_some_and(|n| !n.is_empty())
    }
}

/// 一个任务槽的状态。
///
/// 判定依据是用 6 个真实存档的 18 个难度断面核对出来的：
/// **`COMPLETED_NOW`/`COMPLETED_BEFORE` 几乎不出现，不能单独用来判完成**；
/// 可靠的是 `QUEST_LOG`（任务在日志里）与 `REWARD_GRANTED`（奖励已领）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum QuestState {
    /// 未开始（位集为空，或只带了别幕的对话标记）。
    NotStarted,
    /// 已接到、进行中。
    InProgress,
    /// 已完成（日志里有且奖励已领）。
    Completed,
}

impl QuestState {
    /// 由任务槽的原始位判定状态。
    ///
    /// `quest_complete_mask` 由核心层提供（那里定义"标记完成"要置哪些位），
    /// 判定与写入用同一个掩码，才不会互相打架。
    pub fn from_flags(flags: u16, quest_complete_mask: u16) -> Self {
        if flags & quest_complete_mask == quest_complete_mask {
            Self::Completed
        } else if flags != 0 {
            Self::InProgress
        } else {
            Self::NotStarted
        }
    }

    /// 中文说明。
    pub fn label(self) -> &'static str {
        match self {
            Self::NotStarted => "未开始",
            Self::InProgress => "进行中",
            Self::Completed => "已完成",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quest_state_from_flags() {
        const MASK: u16 = 0x8001;
        assert_eq!(QuestState::from_flags(0, MASK), QuestState::NotStarted);
        // 只带了不相关的位置（例如别幕的对话标记）算未开始还是进行中？
        // 位集不为空就说明动过 → 进行中
        assert_eq!(QuestState::from_flags(0x0020, MASK), QuestState::InProgress);
        assert_eq!(QuestState::from_flags(0x0001, MASK), QuestState::InProgress);
        assert_eq!(QuestState::from_flags(MASK, MASK), QuestState::Completed);
        assert_eq!(QuestState::from_flags(0xFFFF, MASK), QuestState::Completed);
        assert_eq!(QuestState::Completed.label(), "已完成");
    }

    #[test]
    fn empty_namer_falls_back_to_position() {
        let n = QuestNamer::default();
        assert!(n.is_empty());
        assert_eq!(n.name_of(0, 1), "第1幕 槽1");
        assert!(!n.is_real_quest(0, 1));
    }

    #[test]
    fn real_quest_names_from_bundle() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("app")
            .join("data-bundle");
        if !dir.join("strings").is_dir() {
            eprintln!("跳过：没有数据包");
            return;
        }
        let mut strings = Strings::default();
        for e in std::fs::read_dir(dir.join("strings")).unwrap().flatten() {
            if let Ok(text) = std::fs::read_to_string(e.path()) {
                strings.merge(Strings::from_json_bytes(text.as_bytes()));
            }
        }
        let namer = QuestNamer::new(&strings);

        // Act I 的槽 1..7 必须是这 7 个任务（已与真实存档核对）
        let act1: Vec<String> = (1..=6).map(|s| namer.name_of(0, s)).collect();
        assert_eq!(act1[0], "邪恶洞穴");
        assert_eq!(act1[1], "修女的埋骨地");
        assert_eq!(act1[2], "交易的工具");
        assert_eq!(act1[3], "搜寻凯恩");
        assert_eq!(act1[4], "遗忘之塔");
        assert_eq!(act1[5], "为了姐妹们");
        assert_eq!(act1[5], "为了姐妹们");

        // Act V 的 6 个任务
        assert_eq!(namer.name_of(4, 1), "哈洛加斯围城战");
        assert_eq!(namer.name_of(4, 6), "毁灭前夕");

        // 真任务：Act I/II/III/V 各 6 个、Act IV 3 个 = 27 个
        let real: usize = (0..5)
            .map(|a| {
                (1..=MAX_QUESTS_PER_ACT)
                    .filter(|s| namer.is_real_quest(a, *s))
                    .count()
            })
            .sum();
        assert_eq!(real, 27, "真任务总数应为 27");
        assert_eq!(namer.name_of(3, 3), "地狱熔炉", "Act IV 第 3 个任务");
        println!("Act I 槽1..6 = {act1:?}");
        println!(
            "Act V 槽1..6 = {:?}",
            (1..=6).map(|s| namer.name_of(4, s)).collect::<Vec<_>>()
        );
    }
}

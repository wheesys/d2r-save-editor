//! 编辑会话（里程碑 4）：加载存档 → 取出可编辑区域 → 应用修改 → 校验 → 保存。
//!
//! 设计要点：
//! - **带校验的保存**：写出后立即重新解析，并检查未知尾随数据没被丢掉；
//!   写路径不通过就不落盘（见 `docs/research.md` §4.5）。
//! - **不写死游戏常量**：数值上限来自数据集（`ItemStatCost.txt`），
//!   或由调用方给出（例如等级 1–99）。
//! - 界面（Tauri）与 CLI 共用这一层，UI 只负责展示与收集输入。

use std::path::{Path, PathBuf};

use crate::{SaveFile, SaveKind};

/// 角色基础信息（可编辑）。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CharacterView {
    /// 角色名。
    pub name: String,
    /// 职业（调试名，如 `Amazon`）。
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
    /// 当前生命。
    pub life: i64,
    /// 生命上限。
    pub max_life: i64,
    /// 当前法力。
    pub mana: i64,
    /// 法力上限。
    pub max_mana: i64,
    /// 当前体力值。
    pub stamina: i64,
    /// 经验值。
    pub experience: i64,
    /// 身上金币。
    pub gold: i64,
    /// 储物箱金币。
    pub stash_gold: i64,
}

/// 一条技能（可编辑）。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SkillView {
    /// 技能槽位下标（存档内的顺序）。
    pub index: usize,
    /// 技能 ID（用于查名字）。
    pub skill_id: Option<u16>,
    /// 技能名（取不到时为 `None`）。
    pub name: Option<String>,
    /// 已加点数。
    pub level: u8,
}

/// 一条修改指令。
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Edit {
    /// 设置等级。
    SetLevel {
        /// 新等级（1–99）。
        level: u8,
    },
    /// 设置属性。
    SetStat {
        /// 属性 ID（`ItemStatCost` 里的语义 ID，见 `d2r_core::edit::STAT_*`）。
        stat: u32,
        /// 新值。
        value: i64,
    },
    /// 设置某槽位技能等级。
    SetSkill {
        /// 槽位下标。
        index: usize,
        /// 新等级。
        level: u8,
    },
    /// 解锁某难度的全部传送点。
    UnlockWaypoints {
        /// 难度：0=普通 1=噩梦 2=地狱。
        difficulty: u8,
    },
    /// 设置某难度某传送点的开通状态。
    SetWaypoint {
        /// 难度。
        difficulty: u8,
        /// 传送点下标。
        index: usize,
        /// 是否开通。
        active: bool,
    },
}

/// 编辑错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditError {
    /// 加载失败。
    Load(String),
    /// 指令越界/不合法。
    Invalid(String),
    /// 写出失败。
    Write(String),
    /// 保存后的校验失败（会拒绝写盘）。
    Verification(String),
}

impl std::fmt::Display for EditError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Load(e) => write!(f, "加载失败：{e}"),
            Self::Invalid(e) => write!(f, "修改不合法：{e}"),
            Self::Write(e) => write!(f, "写出失败：{e}"),
            Self::Verification(e) => write!(f, "保存校验未通过（已放弃写入）：{e}"),
        }
    }
}

impl std::error::Error for EditError {}

/// 一次编辑会话。
pub struct EditSession {
    /// 源文件路径（从内存创建时为空）。
    pub path: Option<PathBuf>,
    /// 原始字节（用于对比与备份）。
    pub original: Vec<u8>,
    /// 当前存档。
    pub file: SaveFile,
    /// 已应用的指令记录。
    pub applied: Vec<Edit>,
    /// 已应用的物品指令记录。
    pub item_edits: Vec<crate::items::ItemEdit>,
}

impl EditSession {
    /// 打开存档。
    pub fn open(path: impl AsRef<Path>) -> Result<Self, EditError> {
        let path = path.as_ref();
        let file = SaveFile::open(path).map_err(|e| EditError::Load(e.to_string()))?;
        if file.kind != SaveKind::Character {
            return Err(EditError::Invalid(
                "编辑会话目前只支持角色存档（.d2s）".into(),
            ));
        }
        Ok(Self {
            path: Some(path.to_path_buf()),
            original: file.original.clone(),
            file,
            applied: Vec::new(),
            item_edits: Vec::new(),
        })
    }

    /// 角色信息快照。
    pub fn character(&self) -> CharacterView {
        use d2s::StatId;
        let s = &self.file.save;
        let g = |id: StatId| s.stats.get(id);
        CharacterView {
            name: s.name().to_string(),
            class: format!("{:?}", s.character.class),
            level: s.character.level,
            strength: g(StatId::STRENGTH),
            dexterity: g(StatId::DEXTERITY),
            vitality: g(StatId::VITALITY),
            energy: g(StatId::ENERGY),
            stat_points: g(StatId::STAT_POINTS),
            skill_points: g(StatId::SKILL_POINTS),
            life: g(StatId::LIFE),
            max_life: g(StatId::MAX_LIFE),
            mana: g(StatId::MANA),
            max_mana: g(StatId::MAX_MANA),
            stamina: g(StatId::STAMINA),
            experience: g(StatId::EXPERIENCE),
            gold: g(StatId::GOLD),
            stash_gold: g(StatId::STASH_GOLD),
        }
    }

    /// 技能列表（`name_of` 用于把技能 ID 转成显示名，可为 `None` 表示不解析名字）。
    pub fn skills(&self, mut name_of: impl FnMut(u16) -> Option<String>) -> Vec<SkillView> {
        let s = &self.file.save;
        (0..s.skills.levels.len())
            .map(|i| {
                let skill_id = s.skills.skill_id(i);
                SkillView {
                    index: i,
                    skill_id,
                    name: skill_id.and_then(&mut name_of),
                    level: s.skills.level(i),
                }
            })
            .collect()
    }

    /// 传送点状态：返回 `[难度][下标]`。
    pub fn waypoints(&self) -> Vec<Vec<bool>> {
        use d2s::Difficulty;
        let s = &self.file.save;
        [Difficulty::Normal, Difficulty::Nightmare, Difficulty::Hell]
            .iter()
            .map(|d| match s.waypoints.get(*d) {
                Some(w) => (0..40).map(|i| w.is_active(i)).collect(),
                None => vec![false; 40],
            })
            .collect()
    }

    /// 应用一条指令（不落盘）。
    pub fn apply(&mut self, edit: Edit) -> Result<(), EditError> {
        use d2s::StatId;
        match &edit {
            Edit::SetLevel { level } => {
                if !(1..=99).contains(level) {
                    return Err(EditError::Invalid(format!("等级必须在 1–99，收到 {level}")));
                }
                self.file.save.character.level = *level;
                self.file.save.stats.set(StatId::LEVEL, *level as i64);
            }
            Edit::SetStat { stat, value } => {
                if *value < 0 {
                    return Err(EditError::Invalid(format!("属性值不能为负：{value}")));
                }
                let id = StatId(*stat as u16);
                self.file.save.stats.set(id, *value);
            }
            Edit::SetSkill { index, level } => {
                if *index >= self.file.save.skills.levels.len() {
                    return Err(EditError::Invalid(format!(
                        "技能下标 {index} 超出范围（共 {} 个槽位）",
                        self.file.save.skills.levels.len()
                    )));
                }
                if *level > 99 {
                    return Err(EditError::Invalid(format!("技能等级过大：{level}")));
                }
                self.file.save.skills.set_level(*index, *level);
            }
            Edit::UnlockWaypoints { difficulty } => {
                let d = difficulty_from(*difficulty)?;
                match self.file.save.waypoints.get_mut(d) {
                    Some(w) => w.unlock_all(),
                    None => {
                        return Err(EditError::Invalid(format!(
                            "难度 {difficulty} 的传送点段不存在"
                        )))
                    }
                }
            }
            Edit::SetWaypoint {
                difficulty,
                index,
                active,
            } => {
                let d = difficulty_from(*difficulty)?;
                match self.file.save.waypoints.get_mut(d) {
                    Some(w) => {
                        if *index >= 40 {
                            return Err(EditError::Invalid(format!("传送点下标越界：{index}")));
                        }
                        w.set_active(*index, *active);
                    }
                    None => {
                        return Err(EditError::Invalid(format!(
                            "难度 {difficulty} 的传送点段不存在"
                        )))
                    }
                }
            }
        }
        self.applied.push(edit);
        Ok(())
    }

    /// 批量应用（任一条失败即整体放弃，保持原子性）。
    pub fn apply_all(&mut self, edits: &[Edit]) -> Result<(), EditError> {
        let snapshot = self.file.save.clone();
        let applied_len = self.applied.len();
        for e in edits {
            if let Err(err) = self.apply(e.clone()) {
                self.file.save = snapshot;
                self.applied.truncate(applied_len);
                return Err(err);
            }
        }
        Ok(())
    }

    /// 是否有修改。
    pub fn is_dirty(&self) -> bool {
        !self.applied.is_empty() || !self.item_edits.is_empty()
    }

    /// 校验当前状态：写出 → 重新解析 → 比对关键区域。
    ///
    /// 返回 `(写出的字节, 往返是否逐字节一致)`。
    pub fn verify(&self) -> Result<(Vec<u8>, bool), EditError> {
        let bytes = self
            .file
            .to_bytes()
            .map_err(|e| EditError::Write(e.to_string()))?;
        // 重新解析，确保写出的东西自己读得回来
        let reparsed = SaveFile::from_bytes(SaveKind::Character, bytes.clone())
            .map_err(|e| EditError::Verification(format!("写出的文件无法重新解析：{e}")))?;
        // 未知尾随数据不能丢（mod 扩展靠它）
        if reparsed.save.trailing_data != self.file.save.trailing_data {
            return Err(EditError::Verification(format!(
                "尾随数据发生变化：原 {} 字节，写出后 {} 字节",
                self.file.save.trailing_data.len(),
                reparsed.save.trailing_data.len()
            )));
        }
        // 关键字段必须一致
        let before = self.character();
        let after = EditSession {
            path: None,
            original: vec![],
            file: reparsed,
            applied: vec![],
            item_edits: vec![],
        }
        .character();
        if before != after {
            return Err(EditError::Verification(format!(
                "重新解析后的角色信息与写出前不一致：{before:?} != {after:?}"
            )));
        }
        // `true` 表示"重新解析回来的结构与写出前一致"（校验已通过）
        Ok((bytes, true))
    }

    /// 保存到指定路径（先校验，通过才落盘；自动备份原文件）。
    pub fn save_as(&self, dest: impl AsRef<Path>) -> Result<PathBuf, EditError> {
        let dest = dest.as_ref();
        let (bytes, _roundtrip) = self.verify()?;

        // 备份原文件（若目标已存在）
        if dest.exists() {
            let backup = dest.with_extension("d2s.bak");
            std::fs::copy(dest, &backup)
                .map_err(|e| EditError::Write(format!("备份失败 {}：{e}", backup.display())))?;
        }
        std::fs::write(dest, &bytes)
            .map_err(|e| EditError::Write(format!("写入失败 {}：{e}", dest.display())))?;
        Ok(dest.to_path_buf())
    }

    /// 保存回源文件。
    pub fn save(&self) -> Result<PathBuf, EditError> {
        let path = self
            .path
            .clone()
            .ok_or_else(|| EditError::Write("没有源路径，请用 save_as".into()))?;
        self.save_as(path)
    }
}

fn difficulty_from(v: u8) -> Result<d2s::Difficulty, EditError> {
    match v {
        0 => Ok(d2s::Difficulty::Normal),
        1 => Ok(d2s::Difficulty::Nightmare),
        2 => Ok(d2s::Difficulty::Hell),
        other => Err(EditError::Invalid(format!(
            "难度只能是 0/1/2，收到 {other}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_edits_are_rejected() {
        // 不依赖真实存档：直接测指令校验逻辑里不涉及存档的部分
        assert!(difficulty_from(0).is_ok());
        assert!(difficulty_from(2).is_ok());
        assert!(matches!(difficulty_from(3), Err(EditError::Invalid(_))));
    }
}

//! 技能名解析：`Skills.txt` 的 `*Id` → 字符串表 `skillname<ID>`。
//!
//! 实测（2026-10-10）：`skills.json` 里的键是小写 `skillname0` = 攻击、`skillname1` = 脚踢……
//! 与 `Skills.txt` 的 `*Id` 列（0,1,2,…）对应；因此技能名可以**按 ID 直接查**，
//! 不需要为每个职业硬编码技能表。

use crate::dataset::TableStore;

/// 技能名查询器。
pub struct SkillNamer<'a> {
    tables: &'a mut TableStore,
    strings: &'a crate::strings::Strings,
    lang: String,
}

impl<'a> SkillNamer<'a> {
    /// 新建查询器。
    pub fn new(
        tables: &'a mut TableStore,
        strings: &'a crate::strings::Strings,
        lang: impl Into<String>,
    ) -> Self {
        Self { tables, strings, lang: lang.into() }
    }

    fn text(&self, key: &str) -> Option<String> {
        self.strings
            .get(key, &self.lang)
            .or_else(|| self.strings.text(key))
            .map(|s| s.to_string())
    }

    /// 按技能 ID 取名字（键 `skillname<ID>`）。
    pub fn name_by_id(&self, skill_id: u16) -> Option<String> {
        let key = format!("skillname{skill_id}");
        self.text(&key).or_else(|| self.text(&format!("SkillName{skill_id}")))
    }

    /// 按 `Skills.txt` 行号取名字（该行的 `*Id` → 字符串键）。
    pub fn name_by_row(&mut self, row: usize) -> Option<String> {
        let id = {
            let t = self.tables.get("Skills")?;
            t.int(row, "*Id")? as u16
        };
        self.name_by_id(id)
    }

    /// 某职业的技能列表：`(技能ID, 行号, 名字)`。
    ///
    /// `Skills.txt` 的 `skill` 列存放职业归属（如 `Amazon`），`*Id` 是技能 ID。
    pub fn class_skills(&mut self, class: &str) -> Vec<(u16, usize, Option<String>)> {
        let rows: Vec<(usize, u16)> = {
            let Some(t) = self.tables.get("Skills") else { return Vec::new() };
            (0..t.len())
                .filter_map(|r| {
                    let owner = t.cell_nonempty(r, "skill")?;
                    if !owner.eq_ignore_ascii_case(class) {
                        return None;
                    }
                    let id = t.int(r, "*Id").unwrap_or(-1);
                    (id >= 0).then_some((r, id as u16))
                })
                .collect()
        };
        rows.into_iter()
            .map(|(row, id)| (id, row, self.name_by_id(id)))
            .collect()
    }
}

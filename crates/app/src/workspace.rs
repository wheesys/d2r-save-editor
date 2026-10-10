//! 工作区：把"数据集 + 字符串表 + 存档 + 编辑会话"绑在一起，供界面/CLI 调用。

use std::path::{Path, PathBuf};

use d2r_core::{
    Edit, EditSession, ItemDetail, ItemEdit, ItemStats, LayoutHints, LayoutSettings, SaveKind,
};
use d2r_data::{ItemNamer, SkillNamer, Strings, TableStore};

use crate::dto::{
    AppError, CharacterDto, DatasetInfo, EditRequest, ItemDto, SaveInfo, SaveResult, SkillDto,
    VerifyResult, WaypointsDto,
};

/// 保存结果（内部形态，含是否备份）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveOutcome {
    /// 写入路径。
    pub path: PathBuf,
    /// 字节数。
    pub bytes: usize,
    /// 是否创建了备份。
    pub backed_up: bool,
    /// 应用了多少条修改。
    pub edits: usize,
}

/// 编辑器工作区。
pub struct Workspace {
    /// 游戏目录（用于定位数据集）。
    pub game_dir: PathBuf,
    /// mod 数据表目录（用户装了 mod 时用于覆盖同名表）。
    mod_tables_dir: Option<PathBuf>,
    /// 数据来源说明（展示给用户看）。
    data_source: String,
    /// excel 表集合。
    tables: TableStore,
    /// 字符串表。
    strings: Strings,
    /// 显示语言。
    pub lang: String,
    /// 容器/佣兵布局设置（读不出来时由用户勾选）。
    layout: LayoutSettings,
    /// 当前编辑会话。
    session: Option<EditSession>,
}

/// 默认游戏目录。
pub fn default_game_dir() -> PathBuf {
    if let Ok(p) = std::env::var("D2R_GAME_DIR") {
        return PathBuf::from(p);
    }
    PathBuf::from(r"D:\Games\d2r92777\Diablo II Resurrected Infernal Edition")
}

/// 在仓库/当前目录附近找本地文本表（`samples/game-data/lng/strings`）。
pub fn find_strings_dir() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("D2R_STRINGS_DIR") {
        let p = PathBuf::from(p);
        if p.is_dir() {
            return Some(p);
        }
    }
    let mut dir = std::env::current_dir().ok()?;
    loop {
        let cand = dir
            .join("samples")
            .join("game-data")
            .join("lng")
            .join("strings");
        if cand.is_dir() {
            return Some(cand);
        }
        if !dir.pop() {
            return None;
        }
    }
}

impl Workspace {
    /// 新建工作区。
    ///
    /// **数据来源优先级**（目的是"用户机器上什么都不用找"）：
    ///
    /// 1. **随程序分发的数据包**（`crates/app/data-bundle`，构建期内嵌）——
    ///    装了本程序就有数据，不需要游戏目录、不需要任何解包工具、不需要 mod 加载器；
    /// 2. 本机游戏目录里的 loader 编译产物 —— 只有用户自己装了 mod 才需要，
    ///    作为**补充**合并（mod 改了表时优先用游戏目录的版本）；
    /// 3. 开发期仓库里的 `samples/game-data`（CLI 开发者方便）。
    pub fn new(game_dir: impl Into<PathBuf>) -> Self {
        let game_dir = game_dir.into();

        // 1) 内置数据包
        let mut source = "（无数据）".to_string();
        let mut tables = TableStore::empty();
        let mut strings = Strings::default();

        if crate::bundle::available() {
            let n = crate::bundle::load_tables_into(&mut tables);
            strings = crate::bundle::load_strings();
            let info = crate::bundle::info();
            source = format!(
                "内置数据包（{} 张表 / {} 个字符串文件 / {:.1} MB）",
                n,
                info.string_files,
                info.total_bytes as f64 / 1048576.0
            );
        }

        // 2) 用户自己装 mod 时的 loader 产物（覆盖同名表）
        let mut mod_tables = None;
        if let Some(root) = d2r_data::find_loader_dataset(&game_dir) {
            let dir = root.join("data").join("global").join("excel");
            mod_tables = Some(dir.clone());
            tables.add_override_dir(dir);
            source.push_str(&format!("；外加 mod 数据表（{}）", root.display()));
        }

        // 3) 开发期本机导出（仓库内）
        if strings.is_empty() {
            if let Some(d) = find_strings_dir() {
                if let Ok(s) = Strings::from_dir(&d) {
                    let n = s.len();
                    strings.merge(s);
                    source.push_str(&format!("；字符串来自 {d:?}（{n} 条）"));
                }
            }
        }

        Self {
            game_dir,
            mod_tables_dir: mod_tables,
            data_source: source,
            tables,
            strings,
            lang: "zhCN".to_string(),
            layout: LayoutSettings::default(),
            session: None,
        }
    }

    /// 挂载本地文本表目录（界面里可让用户选；合并进已有字符串）。
    pub fn set_strings_dir(&mut self, dir: impl AsRef<Path>) -> Result<usize, AppError> {
        let s = Strings::from_dir(dir).map_err(|e| AppError::new("strings", e.to_string()))?;
        let n = s.len();
        self.strings.merge(s);
        Ok(n)
    }

    /// 数据集信息。
    pub fn dataset_info(&self) -> DatasetInfo {
        DatasetInfo {
            root: self.data_source.clone(),
            tables: self.tables.available().len(),
            strings: self.strings.len(),
            languages: self.strings.languages().to_vec(),
        }
    }

    /// 用户自己装的 mod 数据表目录（没有则为空）。
    pub fn mod_tables_dir(&self) -> Option<&Path> {
        self.mod_tables_dir.as_deref()
    }

    /// 数据包概览（界面可显示"内置数据版本"）。
    pub fn bundle_info(&self) -> crate::bundle::BundleInfo {
        crate::bundle::info()
    }

    /// 打开存档。
    pub fn open(&mut self, path: impl AsRef<Path>) -> Result<SaveInfo, AppError> {
        let path = path.as_ref().to_path_buf();
        let session = EditSession::open(&path)?;
        let info = SaveInfo {
            path: path.display().to_string(),
            kind: match session.file.kind {
                SaveKind::Character => "character".into(),
                SaveKind::Stash => "stash".into(),
            },
            version: match &session.file.stash {
                Some(s) => s.save_version,
                None => session.file.header.version,
            },
            size: session.file.original.len(),
            checksum: session
                .file
                .stash
                .as_ref()
                .map(|_| 0)
                .or(Some(session.file.header.checksum)),
            roundtrip_identical: session.file.check_roundtrip().identical,
        };
        self.session = Some(session);
        Ok(info)
    }

    /// 会话是否存在。
    pub fn is_open(&self) -> bool {
        self.session.is_some()
    }

    /// 是否有未保存的修改。
    pub fn is_dirty(&self) -> bool {
        self.session.as_ref().map(|s| s.is_dirty()).unwrap_or(false)
    }

    fn session(&self) -> Result<&EditSession, AppError> {
        self.session
            .as_ref()
            .ok_or_else(|| AppError::new("state", "还没有打开存档"))
    }

    fn session_mut(&mut self) -> Result<&mut EditSession, AppError> {
        self.session
            .as_mut()
            .ok_or_else(|| AppError::new("state", "还没有打开存档"))
    }

    /// 角色信息。
    pub fn character(&self) -> Result<CharacterDto, AppError> {
        let v = self.session()?.character();
        Ok(CharacterDto {
            name: v.name,
            class: v.class,
            level: v.level,
            strength: v.strength,
            dexterity: v.dexterity,
            vitality: v.vitality,
            energy: v.energy,
            stat_points: v.stat_points,
            skill_points: v.skill_points,
            life: v.life,
            max_life: v.max_life,
            mana: v.mana,
            max_mana: v.max_mana,
            stamina: v.stamina,
            experience: v.experience,
            gold: v.gold,
            stash_gold: v.stash_gold,
        })
    }

    /// 技能列表（含名字）。
    pub fn skills(&mut self) -> Result<Vec<SkillDto>, AppError> {
        // 先把 (槽位, 技能ID, 等级) 取出来，避免同时借用 session 与 tables
        let rows: Vec<(usize, Option<u16>, u8)> = {
            let session = self.session()?;
            let skills = &session.file.save.skills;
            (0..skills.levels.len())
                .map(|i| (i, skills.skill_id(i), skills.levels[i]))
                .collect()
        };
        let namer = SkillNamer::new(&mut self.tables, &self.strings, self.lang.clone());
        Ok(rows
            .into_iter()
            .map(|(index, skill_id, level)| SkillDto {
                index,
                skill_id,
                name: skill_id.and_then(|i| namer.name_by_id(i)),
                level,
            })
            .collect())
    }

    /// 传送点状态。
    pub fn waypoints(&self) -> Result<Vec<WaypointsDto>, AppError> {
        let raw = self.session()?.waypoints();
        const NAMES: [&str; 3] = ["普通", "噩梦", "地狱"];
        Ok(raw
            .into_iter()
            .enumerate()
            .map(|(i, active)| WaypointsDto {
                difficulty: NAMES.get(i).copied().unwrap_or("?").to_string(),
                active,
            })
            .collect())
    }

    /// 当前布局设置。
    pub fn layout(&self) -> &LayoutSettings {
        &self.layout
    }

    /// 用户确认/修改布局设置。
    pub fn set_layout(&mut self, mut s: LayoutSettings) {
        s.confirmed_by_user = true;
        self.layout = s;
    }

    /// 从存档推断布局（读不出来的项列在 `unknowns` 里，交给界面提示用户）。
    pub fn layout_hints(&self) -> Result<LayoutHints, AppError> {
        Ok(self.session()?.layout_hints())
    }

    /// 需要用户确认的项（空数组表示没啥要问的）。
    pub fn layout_questions(&self) -> Result<Vec<String>, AppError> {
        Ok(d2r_core::needs_confirmation(
            &self.session()?.layout_hints(),
        ))
    }

    /// 物品明细（含中文名）。
    pub fn item_details(&mut self) -> Result<Vec<ItemDto>, AppError> {
        let details: Vec<ItemDetail> = {
            let session = self.session()?;
            session.item_details()
        };
        let size_map = crate::bundle::item_size_map();
        let mut namer = ItemNamer::new(&self.strings, &mut self.tables, self.lang.clone());
        Ok(details
            .into_iter()
            .map(|d| {
                let n = namer.resolve(&d.code, &d.quality, d.quality_index);
                let (w, h) = size_map.get(&d.code).copied().unwrap_or((1, 1));
                ItemDto {
                    index: d.index,
                    code: d.code,
                    name: n.display(),
                    name_resolved: n.resolved,
                    quality: d.quality,
                    level: d.level,
                    mode: d.mode,
                    body_location: d.body_location,
                    store_page: d.store_page,
                    inv_x: d.inv_x,
                    inv_y: d.inv_y,
                    stats_count: d.stats.len(),
                    defense: d.defense,
                    durability: d.durability,
                    max_durability: d.max_durability,
                    quantity: d.quantity,
                    socket_count: d.socket_count,
                    width: w,
                    height: h,
                }
            })
            .collect())
    }

    /// 按当前布局设置做越界校验（尺寸来自内置数据包 + mod 覆盖）。
    pub fn validate_bounds(&self) -> Result<Vec<d2r_core::BoundsIssue>, AppError> {
        let session = self.session()?;
        let size_map = crate::bundle::item_size_map();
        let (issues, _checked) =
            session.validate_bounds(&self.layout, |code| size_map.get(code).copied());
        Ok(issues)
    }

    /// 物品汇总统计。
    pub fn item_stats(&self) -> Result<ItemStats, AppError> {
        Ok(self.session()?.item_stats())
    }

    /// 应用一批物品改动（原子）。
    pub fn apply_item_edits(&mut self, edits: &[ItemEdit]) -> Result<usize, AppError> {
        self.session_mut()?.apply_items(edits)?;
        Ok(edits.len())
    }

    /// 挂起的物品修改条数。
    pub fn pending_item_edits(&self) -> usize {
        self.session
            .as_ref()
            .map(|s| s.item_edits.len())
            .unwrap_or(0)
    }

    /// 应用修改（原子：任一条失败全部回滚）。
    pub fn apply(&mut self, edits: &[EditRequest]) -> Result<usize, AppError> {
        let session = self.session_mut()?;
        session.apply_all(edits)?;
        Ok(edits.len())
    }

    /// 保存前自检（不落盘）。
    pub fn verify(&self) -> Result<VerifyResult, AppError> {
        let session = self.session()?;
        match session.verify() {
            Ok((bytes, ok)) => Ok(VerifyResult {
                ok,
                bytes: bytes.len(),
                message: if ok {
                    "写出并重新解析通过".into()
                } else {
                    "往返不一致".into()
                },
            }),
            Err(e) => Ok(VerifyResult {
                ok: false,
                bytes: 0,
                message: e.to_string(),
            }),
        }
    }

    /// 保存（校验通过才落盘，目标已存在时先备份）。
    pub fn save(&self, dest: Option<PathBuf>) -> Result<SaveOutcome, AppError> {
        let session = self.session()?;
        let dest = match dest {
            Some(d) => d,
            None => session
                .path
                .clone()
                .ok_or_else(|| AppError::new("state", "没有源路径"))?,
        };
        let backed_up = dest.exists();
        let edits = session.applied.len();
        let written = session.save_as(&dest)?;
        let bytes = std::fs::metadata(&written)
            .map(|m| m.len() as usize)
            .unwrap_or(0);
        Ok(SaveOutcome {
            path: written,
            bytes,
            backed_up,
            edits,
        })
    }

    /// 把保存结果转成 DTO。
    pub fn save_dto(&self, dest: Option<PathBuf>) -> Result<SaveResult, AppError> {
        let o = self.save(dest)?;
        Ok(SaveResult {
            path: o.path.display().to_string(),
            bytes: o.bytes,
            backed_up: o.backed_up,
            edits: o.edits,
        })
    }

    /// 当前挂起的修改条数。
    pub fn pending_edits(&self) -> usize {
        self.session.as_ref().map(|s| s.applied.len()).unwrap_or(0)
    }

    /// 已应用的修改明细（供界面展示/撤销提示）。
    pub fn applied_edits(&self) -> Vec<Edit> {
        self.session
            .as_ref()
            .map(|s| s.applied.clone())
            .unwrap_or_default()
    }

    /// 重新加载（丢弃未保存修改）。
    pub fn reload(&mut self) -> Result<SaveInfo, AppError> {
        let path = self
            .session()?
            .path
            .clone()
            .ok_or_else(|| AppError::new("state", "没有源路径"))?;
        self.open(path)
    }
}

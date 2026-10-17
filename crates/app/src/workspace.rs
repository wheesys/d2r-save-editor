//! 工作区：把"数据集 + 字符串表 + 存档 + 编辑会话"绑在一起，供界面/CLI 调用。

use std::path::{Path, PathBuf};

use d2r_core::{
    Edit, EditSession, ItemDetail, ItemEdit, ItemStats, LayoutHints, LayoutSettings, SaveKind,
};
use d2r_data::{ItemNamer, SkillNamer, Strings, TableStore};

use crate::dto::{
    AppError, CharacterDto, DatasetInfo, EditRequest, ItemDetailDto, ItemDto, ItemStatDto,
    ParamOptionDto, QuestDifficultyDto, QuestDto, SaveInfo, SaveResult, SkillDto, SkillTreeDto,
    SkillsViewDto, VerifyResult, WaypointsDto,
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

/// 属性目录条目（界面做"添加属性"下拉用）。
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatCatalogEntry {
    /// 属性 ID。
    pub id: u16,
    /// 显示名（中文）。
    pub name: String,
    /// 内部名（数据表里的 `Stat`）。
    pub internal: String,
    /// 值的最小小步长。
    pub step: i64,
    /// 是否带参数位。
    pub has_param_bits: bool,
    /// 游戏是否会为它生成描述（`false` = 引擎内部字段，改了可能不生效）。
    pub user_visible: bool,
    /// 描述串模板（洗格式串前的原文，仅供参考）。
    pub desc_template: Option<String>,
    /// 可表示的**最小值（用户可见值）**。
    pub min_value: Option<f64>,
    /// 可表示的**最大值（用户可见值）**。
    pub max_value: Option<f64>,
    /// 范围的可读文本（用户可见值单位）。
    pub range_text: Option<String>,
    /// 是否按等级加成（数值部分是系数，界面显示"每级加多少"）。
    pub per_level: bool,
    /// 界面输入框的最小变化量（按等级的是 `1/2^op_param`）。
    pub display_step: f64,
    /// 层（参数）的可选值；界面用它做"值2"的下拉框。
    ///
    /// 空表示没有已知的可选集合（界面退化成数字输入框）。
    pub param_options: Vec<ParamOptionDto>,
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
    /// 属性取值上限（来自数据表的位宽列；空表表示不限制）。
    stat_limits: d2r_data::StatLimits,
    /// 属性名解析器（属性 ID → 中文名/内部名/步长）。
    stat_namer: d2r_data::StatNamer,
    /// 小站名解析器（位下标 → 小站中文名）。
    waypoint_namer: d2r_data::WaypointNamer,
    /// 任务名解析器（幕/槽 → 任务中文名）。
    quest_namer: d2r_data::QuestNamer,
    /// 用户设置的持久化（整机一份）。
    config: crate::config::ConfigStore,
    /// 上一次存设置的提示（存成功/失败各一句）。
    config_note: Option<String>,
    /// 最近打开的存档路径（最新的在最前）。
    recent: Vec<String>,
    /// 当前编辑会话。
    session: Option<EditSession>,
    /// 当前已打开的存档信息（未打开时为 `None`）。
    current: Option<SaveInfo>,
}

/// 最近打开的存档最多记几条。
///
/// 只需要"上次打开的那个"来定起始目录，多留几条是为了万一前一个被删/改名
/// 还能退回上一个可用的。
const MAX_RECENT: usize = 8;

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
        Self::with_config(game_dir, crate::config::ConfigStore::new())
    }

    /// 用**指定**的设置文件构建工作区（测试用）。
    ///
    /// 存在的意义是让"设置读回来当默认布局"这条链路可测——
    /// 曾经 Tauri 命令忽略了 `set_layout` 的返回值，界面上改了尺寸却写不进磁盘，
    /// 重开就丢；当时没有任何测试盯着这条链路。
    pub fn with_config(game_dir: impl Into<PathBuf>, config: crate::config::ConfigStore) -> Self {
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
                "内置数据包（{} 张表 / {} 个字符串文件 / {} 个图标 / {:.1} MB）",
                n,
                info.string_files,
                info.icons,
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

        // 属性位宽上限、属性名、小站名：装配完表之后立刻取一次（避免 apply 时再借表）
        let stat_limits = d2r_data::StatLimits::from_store(&mut tables);
        let stat_namer = match tables.get("ItemStatCost") {
            Some(t) => d2r_data::StatNamer::new(t, &strings),
            None => d2r_data::StatNamer::empty(),
        };
        let waypoint_namer = match tables.get("Levels") {
            Some(t) => d2r_data::WaypointNamer::new(t, &strings),
            None => d2r_data::WaypointNamer::default(),
        };
        let quest_layout: Vec<(usize, usize)> =
            (0..5).map(d2r_core::quest_slot_range_of_act).collect();
        let quest_namer = d2r_data::QuestNamer::new(&strings, &quest_layout);

        // 读取用户设置：布局是 mod 级、整机一份。读出来直接当默认值，
        // 这样"设置过一次"之后每次打开任何角色都不用再填；
        // 最近打开的路径也一起读回来，用于给"打开存档"对话框定起始目录。
        // 读失败（没有文件/内容坏了）就退回默认值，不让配置问题挡住主流程。
        let saved = config.load();
        let layout = saved.as_ref().map(|s| s.layout.clone()).unwrap_or_default();
        let recent = saved.map(|s| s.recent_paths).unwrap_or_default();

        Self {
            game_dir,
            mod_tables_dir: mod_tables,
            data_source: source,
            tables,
            stat_limits,
            stat_namer,
            waypoint_namer,
            quest_namer,
            config,
            config_note: None,
            recent,
            strings,
            lang: "zhCN".to_string(),
            layout,
            session: None,
            current: None,
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
            // 角色名用于保存前确认"要覆盖的是谁"；.d2i 没有角色名
            character_name: match session.file.kind {
                SaveKind::Character => session.file.save.character.name.clone(),
                SaveKind::Stash => String::new(),
            },
        };
        self.session = Some(session);
        self.current = Some(info.clone());
        // 记下这次打开的路径：下次"打开存档"直接从这里开始，不用重新找目录
        self.remember_path(&info.path);
        Ok(info)
    }

    /// 记录最近打开的存档路径（最多保留 `MAX_RECENT` 条，最新的在最前）。
    ///
    /// 存到设置文件里，所以**跨重启有效**。写盘失败不影响打开操作本身。
    pub fn remember_path(&mut self, path: &str) {
        if path.is_empty() {
            return;
        }
        self.recent.retain(|p| p != path);
        self.recent.insert(0, path.to_string());
        self.recent.truncate(MAX_RECENT);
        // 记路径是"顺手存一下"，写盘失败不该影响打开存档这个动作本身；
        // 失败原因已经在 save_settings 里记进 config_note，界面能查到。
        let _ = self.save_settings();
    }

    /// 最近打开的存档路径（最新的在最前）。
    pub fn recent_paths(&self) -> &[String] {
        &self.recent
    }

    /// 下次打开对话框的起始目录：最近一次打开的那个文件所在目录。
    ///
    /// 用目录而不是文件本身，是因为对话框插件对 `defaultPath` 的处理
    /// 在不同平台上不一致（有的当目录、有的当文件名）；给目录最稳。
    pub fn default_open_dir(&self) -> Option<String> {
        let last = self.recent.first()?;
        let parent = Path::new(last).parent()?;
        // 目录还在才给（存档可能被删/移走）
        parent.is_dir().then(|| parent.display().to_string())
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
        // 生命/法力/体力在存档里按 1/256 精度存（`ValShift = 8`），
        // 直接给原始值会比游戏里大 256 倍。这里按属性表的精度换算回游戏值，
        // 力量/敏捷等 `ValShift = 0` 的属性不受影响。
        let shown = |id: d2s::StatId, raw: i64| -> i64 {
            self.stat_limits
                .get(id.0)
                .map(|l| l.display_value(raw) as i64)
                .unwrap_or(raw)
        };
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
            life: shown(d2s::StatId::LIFE, v.life),
            max_life: shown(d2s::StatId::MAX_LIFE, v.max_life),
            mana: shown(d2s::StatId::MANA, v.mana),
            max_mana: shown(d2s::StatId::MAX_MANA, v.max_mana),
            stamina: shown(d2s::StatId::STAMINA, v.stamina),
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

    /// 技能页视图：按游戏的**三系技能树**分组。
    ///
    /// 技能树的归属不在数据文件里，用 [`d2r_data::skilltree`] 里写死的名单。
    /// 职业名来自存档（调试名如 `Sorceress`），要用 `PlayerClass.txt` 的
    /// `Player Class → Code` 映射成 `sor` 这类代码才能查表。
    ///
    /// 未知职业（例如某 mod 的新职业）时 `trees` 为空，由界面退回平铺列表——
    /// **不要瞎猜归属**，猜错会让人改错技能。
    pub fn skills_view(&mut self) -> Result<SkillsViewDto, AppError> {
        let all = self.skills()?;
        let skill_points = self.character()?.skill_points;
        let class_name = {
            let session = self.session()?;
            format!("{:?}", session.file.save.character.class)
        };
        let class_code = self.class_code_of(&class_name);
        let raw_code = class_code.clone().unwrap_or_default();
        let class_name_zh = class_short_zh(&raw_code);

        // 没有对应的树（未知职业）就把全部技能当"未归类"
        let Some(trees) = class_code
            .as_deref()
            .and_then(d2r_data::skilltree::trees_for_class)
        else {
            return Ok(SkillsViewDto {
                class_code: raw_code,
                class_name: class_name_zh,
                trees: Vec::new(),
                unclassified: all,
                skill_points,
                english_names: self.skill_id_to_name(),
            });
        };

        // 建立"技能 ID → (树, 树内序号)"。
        //
        // **必须按 ID 匹配**：技能树名单里是英文技能名（`skills.txt` 的 `skill` 列），
        // 而 `self.skills()` 给出的 `name` 是**已本地化的**名字（"火焰弹"），
        // 直接按名字比会全部对不上（踩过：30 个技能全被判为"未归类"）。
        let ids = self.skill_name_to_id();
        let mut by_id: std::collections::HashMap<u16, (usize, usize)> =
            std::collections::HashMap::new();
        for (ti, tree) in trees.iter().enumerate() {
            for (si, name) in tree.skills.iter().enumerate() {
                if let Some(&id) = ids.get(*name) {
                    by_id.insert(id, (ti, si));
                }
            }
        }

        // 按树内序号摆放，空位补 `None`，这样界面能按游戏那样留出空格
        let mut grouped: Vec<Vec<Option<SkillDto>>> =
            trees.iter().map(|t| vec![None; t.skills.len()]).collect();
        let mut unclassified = Vec::new();
        for mut dto in all {
            match dto.skill_id.and_then(|id| by_id.get(&id).copied()) {
                Some((ti, si)) => {
                    // 名字没解析出来时用技能树名单里的英文名兜底：
                    // 至少让用户看出这是哪个技能，而不是"（未解析）"。
                    if dto.name.is_none() {
                        dto.name = Some(trees[ti].skills[si].to_string());
                    }
                    grouped[ti][si] = Some(dto);
                }
                None => unclassified.push(dto),
            }
        }

        Ok(SkillsViewDto {
            class_code: raw_code,
            class_name: class_name_zh,
            trees: trees
                .iter()
                .zip(grouped)
                .map(|(tree, skills)| SkillTreeDto {
                    name: tree.name.to_string(),
                    // **空格也要保留**：技能树是 6 行 × 3 列的行优先网格，
                    // 缺格用占位项填上，界面才能还原游戏里的左中右三条线。
                    skills: skills
                        .into_iter()
                        .map(|slot| {
                            slot.unwrap_or(SkillDto {
                                index: 0,
                                skill_id: None,
                                name: None,
                                level: 0,
                            })
                        })
                        .collect(),
                    levels: tree.levels.to_vec(),
                    icons: tree.icons.to_vec(),
                })
                .collect(),
            unclassified,
            skill_points,
            english_names: self.skill_id_to_name(),
        })
    }

    /// 某个技能 ID 的图标（PNG data URL）。
    ///
    /// 图标按「职业 + 编号」存，编号来自技能树表；查不到时返回 `None`，
    /// 界面会退化成用技能名前两字占位。
    pub fn skill_icon(&mut self, skill_id: u16) -> Option<String> {
        let class_name = {
            let session = self.session().ok()?;
            format!("{:?}", session.file.save.character.class)
        };
        let class_code = self.class_code_of(&class_name)?;
        let trees = d2r_data::skilltree::trees_for_class(&class_code)?;
        let english = self.skill_id_to_name().remove(&skill_id.to_string())?;
        let (tree_index, slot) = d2r_data::skilltree::locate_skill(&english, trees)?;
        let icon = *trees[tree_index].icons.get(slot)?;
        if icon == 0 {
            return None;
        }
        crate::bundle::skill_icon(&class_code, icon)
    }

    /// 技能 ID → 英文技能名（供界面按名字找图标）。
    fn skill_id_to_name(&mut self) -> std::collections::HashMap<String, String> {
        self.skill_name_to_id()
            .into_iter()
            .map(|(name, id)| (id.to_string(), name))
            .collect()
    }

    /// 技能 ID → **本地化名字**（供参数解析用）。
    ///
    /// 抽出来是因为属性列表的渲染闭包里已有 `&self` 借用，
    /// 不能再调 `&mut self` 的方法；所以先算好映射再进闭包。
    fn skill_local_names(&mut self) -> std::collections::HashMap<u16, String> {
        use std::collections::HashMap;
        let ids: Vec<u16> = {
            let Some(table) = self.tables.get("skills") else {
                return HashMap::new();
            };
            (0..table.len())
                .filter_map(|row| table.int(row, "*Id").filter(|v| *v >= 0).map(|v| v as u16))
                .collect()
        };
        let namer = SkillNamer::new(&mut self.tables, &self.strings, self.lang.clone());
        ids.into_iter()
            .filter_map(|id| namer.name_by_id(id).map(|n| (id, n)))
            .collect()
    }

    /// `PlayerClass.txt` 行号 → **中文职业名**。
    fn class_local_names(&mut self) -> std::collections::HashMap<u16, String> {
        use std::collections::HashMap;
        let Some(table) = self.tables.get("PlayerClass") else {
            return HashMap::new();
        };
        // 层 = **职业序号**，不是表行号。
        //
        // `PlayerClass.txt` 第 5 行是 `Expansion` 占位行（`Code` 为空），
        // 游戏里 `item_addclassskills` 的层会跳过它：
        // 0=亚马逊 … 5=德鲁伊、6=刺客。早先直接拿表行号当层，从德鲁伊开始
        // 就整体错一位（用户实测：界面显示德鲁伊、游戏里是刺客）。
        let mut out = HashMap::new();
        let mut index: u16 = 0;
        for row in 0..table.len() {
            let has_code = table
                .cell_nonempty(row, "Code")
                .map(|c| !c.is_empty())
                .unwrap_or(false);
            if !has_code {
                continue;
            }
            let Some(english) = table.cell_nonempty(row, "Player Class") else {
                continue;
            };
            out.insert(index, class_name_zh(english).to_string());
            index += 1;
        }
        out
    }

    /// 某个属性的**层（参数）可选值**，供界面做下拉框。
    ///
    /// 只有含义已经查实的属性才给选项，其余返回空（界面就只显示层号）。
    /// 这样用户可以像游戏里那样选"法师"，而不是去猜层号。
    fn param_options_for(
        &mut self,
        id: u16,
        skills: &std::collections::HashMap<u16, String>,
        classes: &std::collections::HashMap<u16, String>,
    ) -> Vec<ParamOptionDto> {
        if !self
            .stat_limits
            .get(id)
            .map(|l| l.has_param_bits)
            .unwrap_or(false)
        {
            return Vec::new();
        }
        match id {
            // item_addclassskills：层 = 职业序号（见 `class_local_names` 的说明）
            83 => {
                let mut rows: Vec<(u16, String)> = classes
                    .iter()
                    .map(|(index, name)| (*index, name.clone()))
                    .collect();
                rows.sort_by_key(|(k, _)| *k);
                rows.into_iter()
                    .map(|(layer, label)| ParamOptionDto {
                        layer: layer as i32,
                        label,
                        group: None,
                    })
                    .collect()
            }
            // item_aura：层 = 光环 ID
            151 => {
                // 可选值就是**圣骑士的 21 个光环**。
                //
                // 这里是**写死的常量**，不在每次运行时扫 `skills.txt`：
                // 这些 ID 是游戏的固定事实（和装备位布局一样），每次开程序
                // 都扫一遍纯属浪费；而且判据一改就容易出错——之前查了一个
                // 不存在的 `aura` 列，过滤形同虚设，把野蛮人的几种"精通"
                // 混了进来（用户反馈过）。
                //
                // 常量的来源（写死前实测过）：`skills.txt` 里
                // `charclass == "pal"` 且 `aurastate` 非空的技能，正好这 21 条。
                // 名字仍按当前语言从技能表取（**ID 才是固定事实，名字是本地化的**）。
                const ITEM_AURA_SKILL_IDS: [u16; 21] = [
                    98,  // Might 力量
                    99,  // Prayer 祈祷
                    100, // Resist Fire 抗火
                    102, // Holy Fire 神圣之火
                    103, // Thorns 荆棘
                    104, // Defiance 反抗
                    105, // Resist Cold 抗冰
                    108, // Blessed Aim 祝福瞄准
                    109, // Cleansing 净化
                    110, // Resist Lightning 抗雷
                    113, // Concentration 专注
                    114, // Holy Freeze 神圣之冰
                    115, // Vigor 精力
                    117, // Holy Shield 圣盾术
                    118, // Holy Shock 神圣之雷
                    119, // Sanctuary 庇护之光
                    120, // Meditation 冥思
                    122, // Fanaticism 狂热
                    123, // Conviction 定罪
                    124, // Redemption 救赎
                    125, // Salvation 拯救
                ];
                let mut rows: Vec<(u16, String)> = ITEM_AURA_SKILL_IDS
                    .iter()
                    .filter_map(|id| skills.get(id).map(|name| (*id, name.clone())))
                    .collect();
                rows.sort_by_key(|(k, _)| *k);
                rows.into_iter()
                    .map(|(layer, label)| ParamOptionDto {
                        layer: layer as i32,
                        label,
                        group: None,
                    })
                    .collect()
            }
            // 技能类属性：层 = 技能 ID
            //
            //   107 item_singleskill    **限定职业**的技能等级（"+3 火焰弹"）
            //    97 item_nonclassskill  **不限定职业**的技能等级（`oskill` 那类）
            //   204 item_charged_skill  **聚气**（自带 N 次充能的技能）
            //
            // 三者层的含义相同（技能 ID），区别只在"能不能跨职业"。界面统一给
            // **两级选单**：第一级选职业，第二级选该职业的技能；97 额外多一个
            // "不限职业"分组，用来选非职业技能。
            //
            // 只列有名字的技能；表里 `charclass` 为空的（攻击/脚踢/投掷这类
            // 基础动作）只在 97 的"不限职业"分组里出现。
            97 | 107 | 204 => {
                // 用户要求：不列"不限职业"那一组，只按职业选
                let allow_non_class = false;
                let mut rows: Vec<(String, u16, String)> = {
                    let Some(table) = self.tables.get("skills") else {
                        return Vec::new();
                    };
                    (0..table.len())
                        .filter_map(|row| {
                            let class = table
                                .cell_nonempty(row, "charclass")
                                .unwrap_or("")
                                .to_string();
                            if class.is_empty() && !allow_non_class {
                                return None;
                            }
                            let skill_id = table.int(row, "*Id")?;
                            let skill_id = u16::try_from(skill_id).ok()?;
                            let name = skills.get(&skill_id)?.clone();
                            // `charclass` 是 `ama`/`sor` 这种短码，
                            // 换成中文职业名做分组标题
                            let group = class_short_zh(&class);
                            Some((group, skill_id, name))
                        })
                        .collect()
                };
                rows.sort_by(|a, b| (a.0.as_str(), a.1).cmp(&(b.0.as_str(), b.1)));
                rows.dedup_by_key(|(_, id, _)| *id);
                rows.into_iter()
                    .map(|(group, layer, label)| ParamOptionDto {
                        layer: layer as i32,
                        label,
                        group: Some(group),
                    })
                    .collect()
            }
            _ => Vec::new(),
        }
    }

    /// 技能名（`skills.txt` 的 `skill` 列英文值）→ 技能 ID。
    ///
    /// **直接用表里的 `skill` 列**，不要用解析出来的显示名：显示名是本地化的
    /// （`Fire Bolt` → "火焰弹"），而技能树名单里是英文名，两者永远匹配不上
    /// ——踩过这个坑，表现为 30 个技能全部被判为"未归类"。
    /// 用 `skill` 列还有个好处：与界面语言无关。
    fn skill_name_to_id(&mut self) -> std::collections::HashMap<String, u16> {
        let Some(table) = self.tables.get("skills") else {
            return std::collections::HashMap::new();
        };
        let mut out = std::collections::HashMap::new();
        for row in 0..table.len() {
            let (Some(name), Some(id)) = (table.cell_nonempty(row, "skill"), table.int(row, "*Id"))
            else {
                continue;
            };
            if id >= 0 {
                out.insert(name.to_string(), id as u16);
            }
        }
        out
    }

    /// 把职业调试名（`Sorceress`）换成 `PlayerClass.txt` 里的代码（`sor`）。
    fn class_code_of(&mut self, class_name: &str) -> Option<String> {
        let table = self.tables.get("PlayerClass")?;
        let row = table.find_row_by("Player Class", class_name)?;
        let code = table.cell_nonempty(row, "Code")?;
        Some(code.to_string())
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

    /// 任务视图：每个难度 → 各幕 → 各槽。
    ///
    /// 只有真任务槽会标 `editable`，界面据此禁用其它槽的勾选
    /// （序章、收尾槽、Act IV 的 NPC 对话槽、Act V 的未使用槽都不是任务）。
    pub fn quests(&self) -> Result<Vec<QuestDifficultyDto>, AppError> {
        let raw = self.session()?.quests();
        const NAMES: [&str; 3] = ["普通", "噩梦", "地狱"];
        Ok(raw
            .into_iter()
            .enumerate()
            .map(|(index, acts)| QuestDifficultyDto {
                difficulty: NAMES.get(index).copied().unwrap_or("?").to_string(),
                difficulty_index: index as u8,
                acts: acts
                    .into_iter()
                    .enumerate()
                    .map(|(act, slots)| {
                        slots
                            .into_iter()
                            .enumerate()
                            .map(|(slot, flags)| QuestDto {
                                act,
                                slot,
                                name: self.quest_namer.name_of(act, slot),
                                flags,
                                state: d2r_data::QuestState::from_flags(
                                    flags,
                                    d2r_core::QUEST_COMPLETE_MASK,
                                ),
                                editable: self.quest_namer.is_real_quest(act, slot),
                            })
                            .collect()
                    })
                    .collect(),
            })
            .collect())
    }

    /// 应用任务修改，返回生效条数。
    pub fn apply_quest_edits(&mut self, edits: &[d2r_core::Edit]) -> Result<usize, AppError> {
        let count = edits.len();
        self.session_mut()?
            .apply_all(edits)
            .map_err(|e| AppError::invalid(e.to_string()))?;
        Ok(count)
    }

    /// 当前已打开的存档信息（未打开时为 `None`）。
    pub fn current_save(&self) -> Option<SaveInfo> {
        self.current.clone()
    }

    /// 当前布局设置。
    pub fn layout(&self) -> &LayoutSettings {
        &self.layout
    }

    /// 用户确认/修改布局设置。
    ///
    /// 内存里一定生效；**写盘失败会把错误返回给调用方**，让界面能如实提示——
    /// 之前这里吞掉了失败、永远返回 `Ok(())`，于是界面显示"布局已确认"，
    /// 磁盘上却什么都没写，重开就丢（用户实际遇到的"容器布局没保存"）。
    pub fn set_layout(&mut self, mut s: LayoutSettings) -> Result<(), AppError> {
        s.confirmed_by_user = true;
        self.layout = s;
        self.save_settings()
    }

    /// 配置文件路径（界面上显示出来，方便用户知道设置存哪了）。
    pub fn config_path(&self) -> String {
        self.config.path().display().to_string()
    }

    /// 把当前设置写盘。
    ///
    /// 写失败时把原因同时记进 `config_note`（界面会显示）并**作为错误返回**。
    fn save_settings(&mut self) -> Result<(), AppError> {
        let settings = crate::config::Settings {
            layout: self.layout.clone(),
            layout_confirmed: self.layout.confirmed_by_user,
            recent_paths: self.recent.clone(),
        };
        match self.config.save(&settings) {
            Ok(p) => {
                self.config_note = Some(format!("设置已保存到 {}", p.display()));
                Ok(())
            }
            Err(e) => {
                let note = format!(
                    "设置没能存盘（{e}）。设置这次仍然生效，但**下次启动会丢**。\
                     配置文件打算写到 {}，请检查该目录是否可写。",
                    self.config.path().display()
                );
                self.config_note = Some(note.clone());
                Err(AppError::invalid(note))
            }
        }
    }

    /// 上一次存设置的提示（没存过时为 `None`）。
    ///
    /// 方法名不叫 `config_note`——那会和同名字段冲突。
    pub fn settings_note(&self) -> Option<&str> {
        self.config_note.as_deref()
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
                // 自造的空白暗金护符：暗金名字由表行决定，这里改回我们自己的名字
                let name = if !d.personalized_name.is_empty() {
                    // 自定义名字优先：游戏里也显示成"名字's 底材名"，这里只显示名字
                    d.personalized_name.clone()
                } else if crate::library::is_blank_charm(
                    &d.code,
                    &d.quality,
                    d.quality_index,
                    d.stats.len(),
                ) {
                    crate::library::blank_charm_name(&self.lang).to_string()
                } else {
                    n.display()
                };
                // "书只占一格"的设置在这里生效：渲染与拖拽都按这个尺寸走
                let (w, h) = self
                    .layout
                    .item_size(&d.code, size_map.get(&d.code).copied().unwrap_or((1, 1)));
                ItemDto {
                    index: d.index,
                    code: d.code,
                    name,
                    name_resolved: n.resolved,
                    personalized_name: d.personalized_name.clone(),
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
                    socketed_count: d.socketed_count,
                    socketed_flag: d.socketed_flag,
                    width: w,
                    height: h,
                }
            })
            .collect())
    }

    /// 导出全部物品为蓝图（JSON 形态）。
    pub fn export_blueprints(&self) -> Result<Vec<d2r_core::ItemBlueprint>, AppError> {
        Ok(self.session()?.export_blueprints())
    }

    /// 导出单件物品为蓝图。
    pub fn export_blueprint(&self, index: usize) -> Result<d2r_core::ItemBlueprint, AppError> {
        Ok(d2r_core::export_blueprint(self.session()?, index)?)
    }

    /// 导入一件物品（模板克隆 + 覆盖属性）。
    pub fn import_blueprint(
        &mut self,
        blueprint: &d2r_core::ItemBlueprint,
        template: usize,
    ) -> Result<d2r_core::ImportOutcome, AppError> {
        // 蓝图里的属性也要按位宽校验（指令在 core 内部构造，这里先行拦一道）
        if let Some(stats) = &blueprint.stats {
            for s in stats {
                self.stat_limits
                    .check(s.stat, s.value)
                    .map_err(AppError::invalid)?;
            }
        }
        Ok(self.session_mut()?.import_blueprint(blueprint, template)?)
    }

    /// 把某件物品的属性整体替换。
    pub fn replace_item_stats(
        &mut self,
        index: usize,
        stats: Vec<(u16, i64, i32)>,
    ) -> Result<(), AppError> {
        self.session_mut()?.replace_stats(index, stats)?;
        Ok(())
    }

    /// 内置装备库（暗金 / 套装）。**第一次调用后走缓存**，不会重复解析。
    ///
    /// 显示名在生成期就本地化进数据了（`tools/make-library.py`），
    /// 所以这里不需要再查名字表——数据由 `tools/make-library.py` 产出，
    /// 随数据包内嵌，与语言设置无关。
    pub fn item_library(&mut self) -> Vec<crate::library::LibraryItem> {
        let lang = self.lang.clone();
        crate::library::items(&lang, &mut |key| Some(key.to_string()))
    }

    /// 从装备库导入一件物品到指定容器，自动找空位。
    ///
    /// 与蓝图导入的区别：不要求存档里已有同类物品当模板——这是从零构造，
    /// 所以能导入存档里原本没有的暗金/套装。
    pub fn import_library_item(
        &mut self,
        id: &str,
        store_page: &str,
    ) -> Result<crate::library::ImportResult, AppError> {
        let items = self.item_library();
        let entry = items
            .iter()
            .find(|i| i.id == id)
            .ok_or_else(|| AppError::invalid(format!("装备库里没有这一项：{id}")))?;
        // 属性也要按位宽先行拦一道，避免写进一个会被截断的值
        for s in &entry.stats {
            self.stat_limits
                .check(s.stat, s.value)
                .map_err(AppError::invalid)?;
        }
        let stats: Vec<(u16, i64, i32)> = entry
            .stats
            .iter()
            .map(|s| (s.stat, s.value, s.layer))
            .collect();
        let code = entry.code.clone();
        let quality = entry.quality.clone();
        let file_index = entry.file_index;
        let name = entry.name.clone();
        let level = entry.required_level.clamp(1, 99) as u8;

        let (col, row) = self.find_free_slot(store_page, entry.width, entry.height)?;
        let before = self.session()?.item_count();
        self.session_mut()?
            .apply_item(d2r_core::ItemEdit::CreateItem {
                code,
                quality,
                file_index: Some(file_index),
                stats: stats.clone(),
                store_page: store_page.to_string(),
                col,
                row,
                level,
            })
            .map_err(|e| AppError::invalid(e.to_string()))?;
        Ok(crate::library::ImportResult {
            index: before,
            name,
            stats: stats.len(),
            col,
            row,
        })
    }
    /// 给物品起自定义名字（空字符串表示清除）。
    pub fn rename_item(&mut self, index: usize, name: &str) -> Result<(), AppError> {
        self.session_mut()?
            .apply_item(d2r_core::ItemEdit::RenameItem {
                index,
                name: name.to_string(),
            })
            .map_err(|e| AppError::invalid(e.to_string()))?;
        Ok(())
    }

    /// 删除一件物品，返回剩余件数。
    ///
    /// 删除会**同时带走镶嵌在它里面的物品**（宝石/符文），这是游戏语义：
    /// 镶孔物不是独立物品。删除后序号会前移，所以界面要整表重读。
    pub fn delete_item(&mut self, index: usize) -> Result<usize, AppError> {
        let before = self.session()?.item_count();
        self.session_mut()?
            .apply_item(d2r_core::ItemEdit::Remove { index })
            .map_err(|e| AppError::invalid(e.to_string()))?;
        let after = self.session()?.item_count();
        if after >= before {
            return Err(AppError::invalid("删除没有生效".to_string()));
        }
        Ok(after)
    }

    /// 复制一件物品到同容器的空位。
    ///
    /// 用 `CloneItem`（复制整条记录）而不是重新构造：复制要**连品质数据、
    /// 镶孔、耐久一起带过来**，重建反而容易丢东西。
    /// 位置由 [`Self::find_free_slot`] 自动挑，用户不用自己算坐标。
    pub fn duplicate_item(
        &mut self,
        index: usize,
        store_page: Option<&str>,
    ) -> Result<crate::library::ImportResult, AppError> {
        let size_map = crate::bundle::item_size_map();
        let (page, w, h, name) = {
            let details = self.session()?.item_details();
            let src = details
                .get(index)
                .ok_or_else(|| AppError::invalid(format!("物品序号 {index} 越界")))?;
            let (iw, ih) = self.layout.item_size(
                &src.code,
                size_map.get(&src.code).copied().unwrap_or((1, 1)),
            );
            (
                store_page
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| src.store_page.clone()),
                iw,
                ih,
                src.code.clone(),
            )
        };
        let (col, row) = self.find_free_slot(&page, w, h)?;
        let before = self.session()?.item_count();
        self.session_mut()?
            .apply_item(d2r_core::ItemEdit::CloneItem { template: index })
            .map_err(|e| AppError::invalid(e.to_string()))?;
        // 克隆出来的记录带的是原物品的位置，要挪到新空位
        self.session_mut()?
            .apply_item(d2r_core::ItemEdit::MoveTo {
                index: before,
                col,
                row,
            })
            .map_err(|e| AppError::invalid(e.to_string()))?;
        Ok(crate::library::ImportResult {
            index: before,
            name,
            stats: 0,
            col,
            row,
        })
    }

    /// 在指定容器里找一块能放下 `w × h` 的空位。
    ///
    /// 用于"从装备库导入"：用户不该自己算坐标，程序挑第一个能放下的位置。
    /// 算法是**按行扫描、逐格检查**：把已占格标记出来，再找连续空位。
    /// 返回第一个可放位置；放不下则报错（而不是塞到重叠的位置上）。
    fn find_free_slot(&mut self, store_page: &str, w: u32, h: u32) -> Result<(u8, u8), AppError> {
        // 容器尺寸：背包/方块/箱子各自的列数、行数
        let (cols, rows) = match store_page {
            "Cube" => (self.layout.cube_cols, self.layout.cube_rows),
            "Stash" => (self.layout.stash_cols, self.layout.stash_rows),
            // 腰带按物品数量算，不是格子；装备页由固定槽位决定，都不做自动放置
            "Belt" | "Equip" => {
                return Err(AppError::invalid(
                    "腰带与装备页不支持自动放置，请导入到背包后手动移动".to_string(),
                ));
            }
            _ => (self.layout.inventory_cols, self.layout.inventory_rows),
        };
        let cols = cols.max(1);
        let rows = rows.max(1);
        if w > cols || h > rows {
            return Err(AppError::invalid(format!(
                "这件物品占 {w}×{h}，容器只有 {cols}×{rows}，放不下"
            )));
        }

        // 标记已占格：同容器里每件物品按它的占格涂一遍
        let size_map = crate::bundle::item_size_map();
        let occupied: Vec<Vec<bool>> = {
            let details = self.session()?.item_details();
            let mut grid = vec![vec![false; cols as usize]; rows as usize];
            for d in &details {
                if d.store_page != store_page {
                    continue;
                }
                let (iw, ih) = self
                    .layout
                    .item_size(&d.code, size_map.get(&d.code).copied().unwrap_or((1, 1)));
                for y in d.inv_y..d.inv_y.saturating_add(ih as u8) {
                    for x in d.inv_x..d.inv_x.saturating_add(iw as u8) {
                        if let Some(row) = grid.get_mut(y as usize) {
                            if let Some(cell) = row.get_mut(x as usize) {
                                *cell = true;
                            }
                        }
                    }
                }
            }
            grid
        };

        for y in 0..=(rows - h) {
            for x in 0..=(cols - w) {
                let fits = (0..h)
                    .all(|dy| (0..w).all(|dx| !occupied[(y + dy) as usize][(x + dx) as usize]));
                if fits {
                    return Ok((x as u8, y as u8));
                }
            }
        }
        Err(AppError::invalid(format!(
            "{store_page} 里没有 {w}×{h} 的空位，先挪开一些物品再试"
        )))
    }

    /// 按当前布局设置做越界校验（尺寸来自内置数据包 + mod 覆盖）。
    pub fn validate_bounds(&self) -> Result<Vec<d2r_core::BoundsIssue>, AppError> {
        let session = self.session()?;
        let size_map = crate::bundle::item_size_map();
        let (issues, _checked) =
            session.validate_bounds(&self.layout, |code| size_map.get(code).copied());
        Ok(issues)
    }

    /// 一件物品的完整明细（基础信息 + 带名字的属性列表）。
    pub fn item_detail_dto(&mut self, index: usize) -> Result<ItemDetailDto, AppError> {
        self.item_detail_dto_on(d2r_core::ItemTarget::Inventory, index)
    }

    /// 一件物品的完整明细（可指定列表）。
    ///
    /// 佣兵段与主列表序号体系不同，界面按佣兵序号取明细时要显式给目标。
    pub fn item_detail_dto_on(
        &mut self,
        target: d2r_core::ItemTarget,
        index: usize,
    ) -> Result<ItemDetailDto, AppError> {
        let detail = { self.session()?.item_detail_on(target, index)? };
        let size_map = crate::bundle::item_size_map();
        let mut namer = ItemNamer::new(&self.strings, &mut self.tables, self.lang.clone());
        let mut n = namer.resolve(&detail.code, &detail.quality, detail.quality_index);
        // 自造的空白暗金护符：名字改回我们自己的（见表里 `is_blank_charm` 的说明）
        if crate::library::is_blank_charm(
            &detail.code,
            &detail.quality,
            detail.quality_index,
            detail.stats.len(),
        ) {
            n = d2r_data::ItemName {
                base: crate::library::blank_charm_name(&self.lang).to_string(),
                resolved: true,
                quality_prefix: None,
                note: None,
            };
        }
        // 同样考虑"书只占一格"的设置，否则弹窗里的占格会和网格里显示的不一致
        let (w, h) = self.layout.item_size(
            &detail.code,
            size_map.get(&detail.code).copied().unwrap_or((1, 1)),
        );
        // 参数解析需要的两张映射先算好：闭包里已有 `&self` 借用，
        // 不能再调 `&mut self` 的方法（会借用冲突）。
        let skill_names = self.skill_local_names();
        let class_names = self.class_local_names();
        // 参数下拉的可选值：闭包里有 `&self` 借用，所以在这里先算好。
        // 只有带参数位的属性（ItemStatCost 里 `Save Param Bits > 0`，共 17 条）
        // 才需要，其余跳过以免白算。
        let param_ids: Vec<u16> = detail
            .stats
            .iter()
            .map(|(id, _, _)| *id)
            .filter(|id| {
                self.stat_namer
                    .get(*id)
                    .map(|s| s.has_param_bits)
                    .unwrap_or(false)
            })
            .collect();
        let param_options: std::collections::HashMap<u16, Vec<ParamOptionDto>> = param_ids
            .into_iter()
            .map(|id| {
                let opts = self.param_options_for(id, &skill_names, &class_names);
                (id, opts)
            })
            .collect();
        let mut stats: Vec<ItemStatDto> = detail
            .stats
            .iter()
            .map(|(id, value, layer)| {
                let info = self.stat_namer.get(*id);
                // 逐条属性各有自己的有效范围（ItemStatCost 的位宽/偏移/移位），
                // 界面要把它显示出来，用户才知道能填多少。
                let limit = self.stat_limits.get(*id);
                // 按等级加成的属性（*perlevel）存的是**系数**，不是直接加成：
                // 界面显示"每级加多少" = 字段值 / 2^op_param（生命 63/8 = 7.875）。
                let per_level = limit.map(|l| l.per_level).unwrap_or(false);
                // 用户可见值：按等级的是每级加成（可能是小数），否则就是字段值
                let display = limit
                    .map(|l| l.display_value(*value))
                    .unwrap_or(*value as f64);
                // 范围也用同一口径（按等级的是"每级"区间）
                let (min_value, max_value) = match limit {
                    Some(l) => {
                        let (lo, hi) = l.display_range();
                        (Some(lo), Some(hi))
                    }
                    None => (None, None),
                };
                ItemStatDto {
                    id: *id,
                    layer: *layer,
                    value: *value,
                    // 带参数位的属性：同一族用**不同字符串键**区分层
                    // （item_addclassskills 的层是职业，键是 ModStr3a/b/c…）。
                    // 直接用表里的 descstrpos 会让所有职业都显示成第一个
                    // （"亚马逊技能等级"），所以按层改写键。
                    name: resolve_stat_name(&self.stat_namer, info, *id, *layer, &class_names),
                    internal: info.map(|s| s.internal.clone()).unwrap_or_default(),
                    step: info.map(|s| s.step).unwrap_or(1),
                    has_param_bits: info.map(|s| s.has_param_bits).unwrap_or(false),
                    user_visible: info.map(|s| s.user_visible).unwrap_or(false),
                    min_value,
                    max_value,
                    range_text: limit.map(|l| l.range_text()),
                    per_level,
                    display_value: display,
                    display_step: limit.map(|l| l.user_step()).unwrap_or(1.0),
                    param_label: param_label_from(
                        *id,
                        *value,
                        (*layer).max(0) as u16,
                        &skill_names,
                        &class_names,
                    ),
                    has_layer: info.map(|s| s.has_param_bits).unwrap_or(false),
                    param_options: param_options.get(id).cloned().unwrap_or_default(),
                }
            })
            .collect();
        stats.sort_by_key(|s| (s.id, s.layer));
        Ok(ItemDetailDto {
            item: ItemDto {
                index: detail.index,
                code: detail.code,
                name: if detail.personalized_name.is_empty() {
                    n.display()
                } else {
                    detail.personalized_name.clone()
                },
                name_resolved: n.resolved,
                personalized_name: detail.personalized_name.clone(),
                quality: detail.quality,
                level: detail.level,
                mode: detail.mode,
                body_location: detail.body_location,
                store_page: detail.store_page,
                inv_x: detail.inv_x,
                inv_y: detail.inv_y,
                stats_count: detail.stats.len(),
                defense: detail.defense,
                durability: detail.durability,
                max_durability: detail.max_durability,
                quantity: detail.quantity,
                socket_count: detail.socket_count,
                socketed_count: detail.socketed_count,
                socketed_flag: detail.socketed_flag,
                width: w,
                height: h,
            },
            stats,
            runeword_id: detail.runeword_id,
        })
    }

    /// 物品汇总统计。
    pub fn item_stats(&self) -> Result<ItemStats, AppError> {
        Ok(self.session()?.item_stats())
    }

    /// 佣兵物品的完整明细（序号是佣兵段内的序号）。
    pub fn mercenary_item_details(&mut self) -> Result<Vec<ItemDetailDto>, AppError> {
        let total = self.session()?.mercenary_items().len();
        (0..total)
            .map(|i| self.item_detail_dto_on(d2r_core::ItemTarget::Mercenary, i))
            .collect()
    }

    /// 按目标应用物品改动。
    ///
    /// 界面上填的是**游戏显示值**，所以这里先换算回**存档原始值**，
    /// 再逐条按自己的规格校验（`ItemStatCost` 的位宽/偏移/移位）：
    /// 不同属性的有效值完全不同，不校验的话会被 d2s 静默截断、
    /// 甚至让整条属性写盘后消失。
    ///
    /// 校验分两步，与不带目标的 [`Self::apply_item_edits`] 完全一致：
    /// 1. `check_item_edits` 预检范围（能提前拦住明显超范围的值）；
    /// 2. 应用后 `verify_item_values` 读回来比对——属性按位存储，
    ///    超出位宽会**静默截断**，写进去的和读出来的不一致就回滚报错。
    pub fn apply_item_edits_on(
        &mut self,
        target: d2r_core::ItemTarget,
        edits: &[d2r_core::ItemEdit],
    ) -> Result<usize, AppError> {
        // 显示值 → 原始值（内部换算）
        let raw_edits = self.convert_stat_edits(edits);
        self.check_item_edits(&raw_edits)?;
        self.session_mut()?
            .apply_items_on(target, &raw_edits)
            .map_err(|e| AppError::invalid(e.to_string()))?;
        if let Err(e) = self.verify_item_values(&raw_edits) {
            self.discard_item_edits();
            return Err(e);
        }
        Ok(raw_edits.len())
    }

    /// 佣兵装备段里的物品。
    pub fn mercenary_items(&self) -> Result<Vec<d2r_core::MercenaryItem>, AppError> {
        Ok(self.session()?.mercenary_items())
    }

    /// 小站名字表（下标 = 存档里的位下标）。界面用它把 1 2 3 4 换成站名。
    pub fn waypoint_names(&self) -> Vec<String> {
        self.waypoint_namer.all().to_vec()
    }

    /// 属性名解析器（只读）。
    pub fn stat_names(&self) -> &d2r_data::StatNamer {
        &self.stat_namer
    }

    /// 属性目录：界面做"添加属性"下拉用。
    ///
    /// **默认只返回游戏会显示、且解析出名字的属性**（`all = false`）；
    /// 那批引擎内部记账字段（改了不生效）默认不列。
    /// 传 `all = true` 可以看到全部（界面应标注"可能无效"）。
    pub fn stat_catalog(&mut self, all: bool) -> Vec<StatCatalogEntry> {
        // 先克隆成拥有所有权的列表，释放对 `self.stat_namer` 的借用；
        // 否则后面没法再借 `self` 去查技能表/职业表（算参数选项要用）。
        let source: Vec<d2r_data::StatName> = if all {
            self.stat_namer.named().into_iter().cloned().collect()
        } else {
            self.stat_namer.editable().into_iter().cloned().collect()
        };
        // 参数选项要用到技能表/职业表，先算好（闭包里无法再借 self）
        let skill_names = self.skill_local_names();
        let class_names = self.class_local_names();
        let mut entries: Vec<StatCatalogEntry> = source
            .into_iter()
            .map(|s| {
                // 范围与精度也要给出来：界面要在**用户填值之前**就显示
                // "有效范围 0–127"，否则只能盲填再被拒。
                let limit = self.stat_limits.get(s.id);
                StatCatalogEntry {
                    id: s.id,
                    // 带参数位的属性在**目录**里要用通用名，不能用某一个具体值。
                    // 数据里 `item_addclassskills` 的 `descstrpos` 指向
                    // `ModStr3a`（亚马逊），直接用就会显示成"亚马逊技能等级"，
                    // 让用户以为只能加亚马逊（用户反馈过）。
                    // 具体加哪个由"值2"决定，所以这里显示通用名。
                    name: catalog_name(s.id, &s.display),
                    internal: s.internal.clone(),
                    step: s.step,
                    has_param_bits: s.has_param_bits,
                    user_visible: s.user_visible,
                    desc_template: s.desc_template.clone(),
                    min_value: limit.map(|l| l.to_user(l.min())),
                    max_value: limit.map(|l| l.to_user(l.max())),
                    range_text: limit.map(|l| l.range_text()),
                    per_level: limit.map(|l| l.per_level).unwrap_or(false),
                    display_step: limit.map(|l| l.user_step()).unwrap_or(1.0),
                    param_options: Vec::new(),
                }
            })
            .collect();
        // 参数选项要逐条查表（`param_options_for` 需要 `&mut self`），
        // 所以先收集完再统一补。只有带参数位的属性才有内容。
        let ids: Vec<u16> = entries
            .iter()
            .filter(|e| e.has_param_bits)
            .map(|e| e.id)
            .collect();
        for id in ids {
            let options = self.param_options_for(id, &skill_names, &class_names);
            if let Some(entry) = entries.iter_mut().find(|e| e.id == id) {
                entry.param_options = options;
            }
        }
        entries
    }

    /// 某个属性的名字（查不到时返回 `属性#<id>`）。
    pub fn stat_display_name(&self, id: u16) -> String {
        self.stat_namer.display_or_id(id)
    }

    /// 解析出的属性名数量（0 表示没有属性名数据）。
    pub fn stat_name_count(&self) -> usize {
        self.stat_namer.len()
    }

    /// 属性取值上限表的条数（0 表示没有上限数据）。
    pub fn stat_limit_count(&self) -> usize {
        self.stat_limits.len()
    }

    /// 校验一批物品改动里的属性值是否可表示（超了返回拒绝原因）。
    ///
    /// 单独成函数而不是塞进 apply：这样界面可以在用户输入时就提示，
    /// 而不是等到点"应用"才报错。
    ///
    /// 报错里带**属性名**而不是 ID：用户看到"属性 96"根本不知道是哪个。
    pub fn check_item_edits(&self, edits: &[d2r_core::ItemEdit]) -> Result<(), AppError> {
        if self.stat_limits.is_empty() {
            return Ok(());
        }
        for e in edits {
            match e {
                d2r_core::ItemEdit::SetStat { stat, value, .. }
                | d2r_core::ItemEdit::AddStat { stat, value, .. } => {
                    self.check_one_stat(*stat, *value)?;
                }
                d2r_core::ItemEdit::ReplaceStats { stats, .. } => {
                    for (stat, value, _) in stats {
                        self.check_one_stat(*stat, *value)?;
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// 校验单个属性值，报错里带属性名。
    ///
    /// `value` 是**界面上的游戏显示值**。
    fn check_one_stat(&self, stat: u16, value: i64) -> Result<(), AppError> {
        let label = self.stat_namer.display_or_id(stat);
        self.stat_limits
            .check_named(stat, value, Some(&label))
            .map_err(AppError::invalid)
    }

    /// 把一批改动里的属性值从**界面显示值**换算成**存档原始值**。
    ///
    /// 界面只显示/输入游戏值（生命 63、跑速 30），存档里存的是原始数
    /// （生命 16128、跑速 50）。这层换算由后端做掉，用户不需要感知。
    /// 非属性类改动原样保留。
    fn convert_stat_edits(&self, edits: &[ItemEdit]) -> Vec<ItemEdit> {
        edits
            .iter()
            .map(|e| match e {
                ItemEdit::SetStat {
                    index,
                    stat,
                    value,
                    layer,
                } => ItemEdit::SetStat {
                    index: *index,
                    stat: *stat,
                    value: self.stat_limits.display_to_raw(*stat, *value as f64),
                    layer: *layer,
                },
                ItemEdit::AddStat {
                    index,
                    stat,
                    value,
                    layer,
                } => ItemEdit::AddStat {
                    index: *index,
                    stat: *stat,
                    value: self.stat_limits.display_to_raw(*stat, *value as f64),
                    layer: *layer,
                },
                ItemEdit::ReplaceStats { index, stats } => ItemEdit::ReplaceStats {
                    index: *index,
                    stats: stats
                        .iter()
                        .map(|(s, v, l)| (*s, self.stat_limits.display_to_raw(*s, *v as f64), *l))
                        .collect(),
                },
                other => other.clone(),
            })
            .collect()
    }

    /// 应用一批物品改动（原子）。
    pub fn apply_item_edits(&mut self, edits: &[ItemEdit]) -> Result<usize, AppError> {
        // 先按位宽做一次预检（能提前拦住明显超范围的值）
        self.check_item_edits(edits)?;
        self.session_mut()?.apply_items(edits)?;
        // 再读回来比对：属性是按位存储的，超范围会被**静默截断**。
        // 预检无法覆盖带参数/编码的属性（值不是直接存的），所以这里做事实校验：
        // 写进去的和读出来的不一致就回滚并报错。
        if let Err(e) = self.verify_item_values(edits) {
            self.discard_item_edits();
            return Err(e);
        }
        Ok(edits.len())
    }

    /// 校验属性值能否被**按位精确存储**。
    ///
    /// 存档里属性是按位存的，读路径是 `(raw - SaveAdd) << ValShift`
    /// （见 `d2s` 的 `data/mod.rs` 与 `model/item.rs`）。所以：
    /// - 值必须是 `2^ValShift` 的倍数，否则写盘时会右移丢掉低位；
    /// - `raw` 必须落在 `Save Bits` 位宽内。
    ///
    /// 判定标准是"**写盘再读回**的值等于用户想要的值"——这才是用户真正关心的。
    fn verify_item_values(&self, edits: &[ItemEdit]) -> Result<(), AppError> {
        let mut problems: Vec<String> = Vec::new();
        for e in edits {
            let (index, wanted): (usize, Vec<(u16, i64)>) = match e {
                ItemEdit::SetStat {
                    index,
                    stat,
                    value,
                    layer: _,
                } => (*index, vec![(*stat, *value)]),
                ItemEdit::ReplaceStats { index, stats } => {
                    (*index, stats.iter().map(|(s, v, _)| (*s, *v)).collect())
                }
                _ => continue,
            };
            for (stat, value) in wanted {
                use d2s::data::ExternalData;
                // 用库自带的表读该属性的位宽与移位（与存档版本无关的默认表）
                let Ok(info) = d2s::data::default_data().stat_info(d2s::StatId(stat), 105) else {
                    continue; // 表里没有：交给 core 的通用校验
                };
                let shift = info.val_shift as u32;
                let unit = 1i64 << shift;
                // 模拟"写盘 → 读回"：
                //   写路径 `raw = (value >> shift) + SaveAdd`（SaveAdd 在写盘时加）
                //   读路径 `value = (raw - SaveAdd) << shift`
                // 两边的 SaveAdd 相互抵消，所以读回来就是 `(value >> shift) << shift`。
                let raw = value >> shift;
                let read_back = raw << shift;
                let bits_ok = raw >= -(info.save_add as i64)
                    && raw <= (1i64 << info.save_bits) - 1 - info.save_add as i64;
                if read_back == value && bits_ok {
                    continue;
                }
                let hint = if shift > 0 && value % unit != 0 {
                    format!(
                        "该属性按 2^{shift} = {unit} 的步长存储，请取 {unit} 的倍数（例如 {}）",
                        value / unit * unit
                    )
                } else {
                    format!(
                        "位宽为 {} 位（Save Add {}），值超出可精确存储的范围",
                        info.save_bits, info.save_add
                    )
                };
                problems.push(format!(
                    "物品 #{index} 属性 ID {stat}：想写 {value}，写盘后会变成 {read_back}。{hint}"
                ));
            }
        }
        if problems.is_empty() {
            return Ok(());
        }
        Err(AppError::invalid(format!(
            "属性值校验不通过（{} 处）：{}",
            problems.len(),
            problems.join("；")
        )))
    }
    /// 放弃当前挂起的物品修改（回到打开时的状态）。
    fn discard_item_edits(&mut self) {
        if let Some(s) = self.session.as_mut() {
            s.revert_item_edits();
        }
    }

    /// 挂起的物品修改条数。
    pub fn pending_item_edits(&self) -> usize {
        self.session
            .as_ref()
            .map(|s| s.item_edits.len())
            .unwrap_or(0)
    }

    /// 某等级对应的**总经验值**（按当前职业查 `experience.txt`）。
    ///
    /// 表里的 `Level` 列是 0 基：等级 L 对应 `Level = L-1`
    /// （L1 → 0、L2 → 500、L99 → 3520485254）。改等级时一起写经验，
    /// 否则游戏会按旧经验重新判定等级，出现"改完又弹回去"。
    fn experience_for_level(&mut self, level: u8) -> Result<i64, AppError> {
        let class_name = {
            let session = self.session()?;
            format!("{:?}", session.file.save.character.class)
        };
        let table = self
            .tables
            .get("experience")
            .ok_or_else(|| AppError::new("data", "缺少 experience 表，无法换算等级经验"))?;
        let row = table
            .find_row_by("Level", &level.saturating_sub(1).to_string())
            .ok_or_else(|| {
                AppError::new("data", format!("experience 表里没有等级 {level} 对应行"))
            })?;
        table.int(row, &class_name).ok_or_else(|| {
            AppError::new(
                "data",
                format!("experience 表里没有职业 {class_name} 的经验列"),
            )
        })
    }

    /// 应用修改（原子：任一条失败全部回滚）。
    ///
    /// `SetLevel` 会在应用层补齐经验值：等级与经验必须同步，
    /// 前端只传等级，经验表由数据包提供。
    pub fn apply(&mut self, edits: &[EditRequest]) -> Result<usize, AppError> {
        let mut expanded: Vec<Edit> = Vec::with_capacity(edits.len() + 1);
        for edit in edits {
            match edit {
                Edit::SetLevel { level, .. } => {
                    let experience = self.experience_for_level(*level)?;
                    expanded.push(Edit::SetLevel {
                        level: *level,
                        experience: Some(experience),
                    });
                }
                other => expanded.push(other.clone()),
            }
        }
        let session = self.session_mut()?;
        session.apply_all(&expanded)?;
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

/// 属性显示名的解析（带参数位的属性要按层给出对应职业的名字）。
///
/// ## 为什么不用数据里的字符串键
///
/// `item_addclassskills` 的 `descstrpos` 是 `ModStr3a`，而数据里
/// 按职业分了 `ModStr3a/b/c/d/e`（亚马逊/圣骑士/死灵法师/法师/野蛮人）。
/// 直接读表里那一个键，所有职业都会显示成"亚马逊技能等级"（用户反馈过）。
///
/// 但**只有 5 个职业有键**：德鲁伊/刺客/术士缺失，他们的物品会退回成
/// "亚马逊技能等级"（更错）。而且键里法师写作"巫师"，与界面用词不一致。
///
/// 所以这里**直接用职业名拼**：`<职业>技能等级`。对所有职业都对，
/// 也不依赖数据里有没有那个键（拼法与字符串表里的既有文本一致）。
fn resolve_stat_name(
    _namer: &d2r_data::StatNamer,
    _info: Option<&d2r_data::StatName>,
    id: u16,
    layer: i32,
    classes: &std::collections::HashMap<u16, String>,
) -> String {
    let fallback = || _namer.display_or_id(id);
    // 只有 item_addclassskills 这一族确认过"层 = 职业行号"，
    // 别的属性不猜（猜错会让用户看不懂这行在说什么）。
    if id != 83 {
        return fallback();
    }
    match classes.get(&(layer.max(0) as u16)) {
        Some(class_zh) => format!("{class_zh}技能等级"),
        None => fallback(),
    }
}

/// 解析带参数属性的**参数含义**（纯函数，便于在借用受限处调用）。
///
/// `id` 是属性 ID，`value` 是数值，`param` 是层（参数位）的值。
/// 逐条确认过的规则见属性列表渲染处与 `Workspace::param_label` 的说明。
/// 认不出来返回 `None`，由界面原样显示层号，**不猜**。
fn param_label_from(
    id: u16,
    value: i64,
    param: u16,
    skills: &std::collections::HashMap<u16, String>,
    classes: &std::collections::HashMap<u16, String>,
) -> Option<String> {
    match id {
        // item_aura：层 = 光环 ID，值 = 等级。模板 `ModitemAura = "获得%d级%s光环"`
        151 => {
            let aura = skills.get(&param)?;
            Some(format!("获得 {value} 级「{aura}」光环"))
        }
        // item_addclassskills：层 = PlayerClass 行号
        83 => {
            let class = classes.get(&param)?;
            Some(format!("{class} 技能等级 +{value}"))
        }
        // item_singleskill / item_nonclassskill：层 = 技能 ID
        107 | 97 => {
            let skill = skills.get(&param)?;
            Some(format!("技能「{skill}」+{value}"))
        }
        _ => None,
    }
}

/// 属性目录里用的名字（带参数位的属性要换成**通用名**）。
///
/// 数据里同一族属性用不同字符串键区分参数值，而 `descstrpos` 只指向**某一个**
/// 具体值：`item_addclassskills` 指向 `ModStr3a`（亚马逊）。目录里直接用它，
/// 用户会以为这条属性只能加亚马逊技能（实际由"值2"决定加哪个职业）。
///
/// 所以这里把"具体值"替换成通用说法。只有确知含义的属性才换，其余原样返回。
fn catalog_name(id: u16, display: &str) -> String {
    match id {
        // 层 = 职业行号 → 通用名
        83 => "职业技能等级".to_string(),
        // item_aura：值 = 光环等级，层 = 光环；通用名去掉"级"与后缀占位
        151 => "装备时获得光环".to_string(),
        // 下面几条的模板里含**两个**占位符（技能 + 职业），洗掉占位符后
        // 只剩下连接词（`'%+d 至%s %s'` → "至"），当名字毫无意义，
        // 所以给它们固定的可读名。
        107 => "指定技能等级（限定职业）".to_string(),
        97 => "指定技能等级（不限职业）".to_string(),
        188 => "指定技能页等级".to_string(),
        195 => "攻击时施放技能".to_string(),
        196 => "杀死敌人时施放技能".to_string(),
        197 => "死亡时施放技能".to_string(),
        198 => "击中时施放技能".to_string(),
        199 => "升级时施放技能".to_string(),
        201 => "被击中时施放技能".to_string(),
        204 => "聚气技能".to_string(),
        126 => "火焰/冰霜/闪电技能".to_string(),
        _ => display.to_string(),
    }
}

/// 职业名的中文（`PlayerClass.txt` 的 `Player Class` 列 → 中文）。
///
/// 参数位里存的是**职业行号**，展示时必须让用户看懂是哪个职业；
/// 数据表里只有英文名。认不出来时原样返回英文，不猜。
/// `skills.txt` 的 `charclass` 短码 → 中文职业名。
///
/// 技能表用 `ama`/`sor` 这类三字母短码，界面要显示"亚马逊"。
/// 与 [`class_name_zh`] 分开：那个吃的是 `PlayerClass` 的英文全名。
fn class_short_zh(code: &str) -> String {
    match code {
        "ama" => "亚马逊",
        "sor" => "法师",
        "nec" => "死灵法师",
        "pal" => "圣骑士",
        "bar" => "野蛮人",
        "dru" => "德鲁伊",
        "ass" => "刺客",
        "war" => "术士",
        // 未知职业原样显示，便于排查
        other => other,
    }
    .to_string()
}

fn class_name_zh(english: &str) -> &str {
    match english {
        "Amazon" => "亚马逊",
        "Sorceress" => "法师",
        "Necromancer" => "死灵法师",
        "Paladin" => "圣骑士",
        "Barbarian" => "野蛮人",
        "Druid" => "德鲁伊",
        "Assassin" => "刺客",
        "Warlock" => "术士",
        other => other,
    }
}

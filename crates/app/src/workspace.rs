//! 工作区：把"数据集 + 字符串表 + 存档 + 编辑会话"绑在一起，供界面/CLI 调用。

use std::path::{Path, PathBuf};

use d2r_core::{
    Edit, EditSession, ItemDetail, ItemEdit, ItemStats, LayoutHints, LayoutSettings, SaveKind,
};
use d2r_data::{ItemNamer, SkillNamer, Strings, TableStore};

use crate::dto::{
    AppError, CharacterDto, DatasetInfo, EditRequest, ItemDetailDto, ItemDto, ItemStatDto,
    QuestDifficultyDto, QuestDto, SaveInfo, SaveResult, SkillDto, SkillTreeDto, SkillsViewDto,
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

/// 属性目录条目（界面做"添加属性"下拉用）。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
    /// 可表示的**最小值（游戏显示值）**。
    pub min_value: Option<i64>,
    /// 可表示的**最大值（游戏显示值）**。
    pub max_value: Option<i64>,
    /// 范围的可读文本（显示值单位）。
    pub range_text: Option<String>,
    /// 存储精度（`2^ValShift`），1 表示无移位。
    pub display_step: i64,
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
        let quest_namer = d2r_data::QuestNamer::new(&strings);

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

        // 没有对应的树（未知职业）就把全部技能当"未归类"
        let Some(trees) = class_code
            .as_deref()
            .and_then(d2r_data::skilltree::trees_for_class)
        else {
            return Ok(SkillsViewDto {
                class_code: class_code.unwrap_or_default(),
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
            class_code: class_code.unwrap_or_default(),
            trees: trees
                .iter()
                .zip(grouped)
                .map(|(tree, skills)| SkillTreeDto {
                    name: tree_name_zh(tree.name).to_string(),
                    skills: skills.into_iter().flatten().collect(),
                })
                .collect(),
            unclassified,
            skill_points,
            english_names: self.skill_id_to_name(),
        })
    }

    /// 技能 ID → 英文技能名（供界面按名字找图标）。
    fn skill_id_to_name(&mut self) -> std::collections::HashMap<String, String> {
        self.skill_name_to_id()
            .into_iter()
            .map(|(name, id)| (id.to_string(), name))
            .collect()
    }

    /// 带参数属性的参数含义（把 `layer` 翻译成人看得懂的名字）。
    ///
    /// 参数位存的是**另一个命名空间里的 ID**，和技能 ID 不是一回事，
    /// 所以不能一股脑按技能表查。目前只解最常用的一类：
    /// `item_aura`（装备时获得光环）——参数是光环 ID，值是该光环的等级。
    /// 游戏里的模板是 `ModitemAura = "装备时获得%d级%s光环"`。
    ///
    /// 认不出来的参数一律返回 `None`，由界面原样显示层号，
    /// **不猜**（猜错会让人看不懂这行到底在说什么）。
    fn param_label(&mut self, id: u16, value: i64, layer: i32) -> Option<String> {
        let limit = self.stat_limits.get(id)?;
        if !limit.has_param_bits {
            return None;
        }
        // 参数位的值是**另一个命名空间里的 ID**：`item_aura` 的层是光环 ID
        // （样本里的 122/123 正好对上 Fanaticism(*Id 122) / Conviction(*Id 123)），
        // 而数值部分是光环等级（31 级合理，122 级不合理，所以两者不能互换）。
        let param = layer.max(0) as u16;
        match id {
            // item_aura：层 = 光环 ID，值 = 等级。模板 ModitemAura = "获得%d级%s光环"
            151 => {
                let aura = self.skill_name_by_id(param)?;
                Some(format!("获得 {value} 级「{aura}」光环"))
            }
            // item_addclassskills：层是职业行号
            83 => {
                let class = self.class_name_by_code(param)?;
                Some(format!("{class} 技能等级 +{value}"))
            }
            // item_singleskill / item_nonclassskill：层是技能 ID
            107 | 97 => {
                let skill = self.skill_name_by_id(param)?;
                Some(format!("技能「{skill}」+{value}"))
            }
            _ => None,
        }
    }

    /// 按技能 ID 取**显示名**（本地化）。
    fn skill_name_by_id(&mut self, skill_id: u16) -> Option<String> {
        let namer = SkillNamer::new(&mut self.tables, &self.strings, self.lang.clone());
        namer.name_by_id(skill_id)
    }

    /// 按 `PlayerClass.txt` 的行号取职业显示名（参数位存的是行号）。
    fn class_name_by_code(&mut self, row_index: u16) -> Option<String> {
        let table = self.tables.get("PlayerClass")?;
        table
            .cell_nonempty(row_index as usize, "Player Class")
            .map(str::to_string)
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
                        let real_act_count = d2r_core::quest_count_of_act(act);
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
                                editable: self.quest_namer.is_real_quest(act, slot)
                                    && slot <= real_act_count,
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
                // "书只占一格"的设置在这里生效：渲染与拖拽都按这个尺寸走
                let (w, h) = self
                    .layout
                    .item_size(&d.code, size_map.get(&d.code).copied().unwrap_or((1, 1)));
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
        let n = namer.resolve(&detail.code, &detail.quality, detail.quality_index);
        // 同样考虑"书只占一格"的设置，否则弹窗里的占格会和网格里显示的不一致
        let (w, h) = self.layout.item_size(
            &detail.code,
            size_map.get(&detail.code).copied().unwrap_or((1, 1)),
        );
        let mut stats: Vec<ItemStatDto> = detail
            .stats
            .iter()
            .map(|(id, value, layer)| {
                let info = self.stat_namer.get(*id);
                // 逐条属性各有自己的有效范围（ItemStatCost 的位宽/偏移/移位），
                // 界面要把它显示出来，用户才知道能填多少。
                let limit = self.stat_limits.get(*id);
                // 按等级加成的属性（*perlevel）存的是**每级的值**，不是总值；
                // 界面要标明，并给出"等级 N → 合计 M"，否则会被当成总值。
                let per_level = limit.map(|l| l.per_level).unwrap_or(false);
                // 换算成**游戏里显示的值**（内部做掉，用户不需要知道）：
                //   生命/法力 按 1/256 精度存；攻速/跑速这类带 SaveAdd 偏移。
                let display = limit.map(|l| l.to_display(*value)).unwrap_or(*value);
                // 范围也要用显示值，否则会显示成 -27–107 这种内部数
                let (min_value, max_value) = match limit {
                    Some(l) => (Some(l.to_display(l.min())), Some(l.to_display(l.max()))),
                    None => (None, None),
                };
                ItemStatDto {
                    id: *id,
                    layer: *layer,
                    value: *value,
                    name: self.stat_namer.display_or_id(*id),
                    internal: info.map(|s| s.internal.clone()).unwrap_or_default(),
                    step: info.map(|s| s.step).unwrap_or(1),
                    has_param_bits: info.map(|s| s.has_param_bits).unwrap_or(false),
                    user_visible: info.map(|s| s.user_visible).unwrap_or(false),
                    min_value,
                    max_value,
                    range_text: limit.map(|l| l.range_text()),
                    per_level,
                    // 合计与显示值同单位：显示值 × 物品等级
                    effective_value: per_level.then(|| display * i64::from(detail.level)),
                    display_value: display,
                    display_step: limit.map(|l| l.step()).unwrap_or(1),
                    param_label: self.param_label(*id, *value, *layer),
                }
            })
            .collect();
        stats.sort_by_key(|s| (s.id, s.layer));
        Ok(ItemDetailDto {
            item: ItemDto {
                index: detail.index,
                code: detail.code,
                name: n.display(),
                name_resolved: n.resolved,
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
    pub fn stat_catalog(&self, all: bool) -> Vec<StatCatalogEntry> {
        let source = if all {
            self.stat_namer.named()
        } else {
            self.stat_namer.editable()
        };
        source
            .into_iter()
            .map(|s| {
                // 范围与精度也要给出来：界面要在**用户填值之前**就显示
                // "有效范围 0–127"，否则只能盲填再被拒。
                let limit = self.stat_limits.get(s.id);
                StatCatalogEntry {
                    id: s.id,
                    name: s.display.clone(),
                    internal: s.internal.clone(),
                    step: s.step,
                    has_param_bits: s.has_param_bits,
                    user_visible: s.user_visible,
                    desc_template: s.desc_template.clone(),
                    min_value: limit.map(|l| l.to_display(l.min())),
                    max_value: limit.map(|l| l.to_display(l.max())),
                    range_text: limit.map(|l| l.range_text()),
                    display_step: limit.map(|l| l.step()).unwrap_or(1),
                }
            })
            .collect()
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
                d2r_core::ItemEdit::SetStat { stat, value, .. } => {
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
                ItemEdit::SetStat { index, stat, value } => ItemEdit::SetStat {
                    index: *index,
                    stat: *stat,
                    value: self.stat_limits.display_to_raw(*stat, *value),
                },
                ItemEdit::ReplaceStats { index, stats } => ItemEdit::ReplaceStats {
                    index: *index,
                    stats: stats
                        .iter()
                        .map(|(s, v, l)| (*s, self.stat_limits.display_to_raw(*s, *v), *l))
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
                ItemEdit::SetStat { index, stat, value } => (*index, vec![(*stat, *value)]),
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

/// 技能树名的中文（按游戏界面用词）。
///
/// 这些名字**不在字符串表里**——游戏界面上确实显示它们，但字符串存在界面代码
/// 里而不是数据表里（查过 `ui.json`/`skills.json` 都没有）。所以只能写死。
/// 认不出来时原样返回英文，至少不会显示成空白。
fn tree_name_zh(english: &str) -> &str {
    match english {
        // 法师
        "Fire Spells" => "火系",
        "Cold Spells" => "冰系",
        "Lightning Spells" => "电系",
        // 亚马逊
        "Bow and Crossbow" => "弓与弩",
        "Passive and Magic" => "被动与魔法",
        "Javelin and Spear" => "标枪与长矛",
        // 死灵法师
        "Summoning Skills" => "召唤",
        "Poison and Bone" => "毒素与白骨",
        "Curses" => "诅咒",
        // 圣骑士
        "Combat Skills" => "作战技能",
        "Offensive Auras" => "攻击灵气",
        "Defensive Auras" => "防御灵气",
        // 野蛮人
        "Combat Masteries" => "战斗专精",
        "Warcries" => "战吼",
        // 德鲁伊
        "Elemental" => "元素",
        "Shape Shifting" => "变形",
        "Summoning" => "召唤",
        // 刺客
        "Martial Arts" => "武艺",
        "Traps" => "陷阱",
        "Shadow Disciplines" => "影子训练",
        // 某个 mod 自创职业（术士）；原版没有对应资料，按数据顺序切的三组
        "Summoning and Blood" => "召唤与血祭",
        "Hexes and Combat" => "咒术与战技",
        "Sigils and Elements" => "印记与元素",
        other => other,
    }
}

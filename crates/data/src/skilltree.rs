//! 技能树结构：职业 → 三系 → 技能名单、等级门槛与图标编号。
//!
//! ## 为什么要把这张表写死在代码里
//!
//! 游戏里技能树是 3 系 × 10 技能，但**树的归属与行列位置不在任何数据文件里**
//! （在游戏界面代码里）。`skills.txt` 只给了 `charclass`、`reqlevel`、
//! `reqskill1..3`；靠依赖关系也推不出归属——**每棵树都有 2 个根技能**
//! （例如法师的 Fire Bolt 与 Warmth 都没有前置），所以必须显式给出名单。
//!
//! ## 名单里的名字是"技能代码名"
//!
//! 这些是 `skills.txt` 的 `skill` 列值，也就是技能的英文标识。用它去查
//! 名称表就能拿到当前 mod 的显示名（例如某些 mod 把 Wake of Fire 改叫
//! Fire Trauma，把 Werewolf 改叫 Wearwolf），所以这里**要保持英文原名**，
//! 不要跟着 mod 改。
//!
//! ## 树名、等级与图标
//!
//! 树名用游戏内中文名；`levels` 是该格的等级门槛（0 表示空格）；
//! `icons` 是图标编号（1..=30，0 表示空格），与 `skilldesc.txt` 的
//! `IconCel` 对应（编号 = `IconCel / 2 + 1`）。
//!
//! 正确性由 `crates/data/tests/skilltrees.rs` 兜底：逐个断言名单里的技能
//! 在数据里真实存在，且每个职业的 30 个技能**全部被归类**。

/// 一棵技能树：树名 + 该树的 10 个技能（`skills.txt` 的 `skill` 列值）。
pub struct SkillTree {
    /// 树名（游戏内中文名）。
    pub name: &'static str,
    /// 该树的技能，**6 行 × 3 列行优先**铺满，缺格用 `""` 占位。
    pub skills: &'static [&'static str],
    /// 与 [`Self::skills`] 一一对应的等级门槛（0 表示该格为空）。
    pub levels: &'static [u8],
    /// 与 [`Self::skills`] 一一对应的图标编号（1..=30，0 表示该格为空）。
    pub icons: &'static [u8],
}

/// 一个职业的三系技能树。
pub struct ClassSkillTrees {
    /// `PlayerClass.txt` 的 `Code` 列值（`ama`/`sor`/…）。
    pub class_code: &'static str,
    pub trees: &'static [SkillTree],
}

/// 全部职业的技能树（按 `PlayerClass.txt` 顺序）。
pub const CLASS_SKILL_TREES: &[ClassSkillTrees] = &[
    ClassSkillTrees {
        class_code: "ama",
        trees: &[
            SkillTree {
                name: "标枪和长矛",
                skills: &[
                    "Jab",
                    "",
                    "",
                    "",
                    "Power Strike",
                    "Poison Javelin",
                    "Impale",
                    "",
                    "Lightning Bolt",
                    "",
                    "Charged Strike",
                    "Plague Javelin",
                    "Fend",
                    "",
                    "",
                    "",
                    "Lightning Strike",
                    "Lightning Fury",
                ],
                levels: &[1, 0, 0, 0, 6, 6, 12, 0, 12, 0, 18, 18, 24, 0, 0, 0, 30, 30],
                icons: &[5, 0, 0, 0, 9, 10, 14, 0, 15, 0, 19, 20, 25, 0, 0, 0, 29, 30],
            },
            SkillTree {
                name: "被动和魔法",
                skills: &[
                    "Inner Sight",
                    "",
                    "Critical Strike",
                    "",
                    "Dodge",
                    "",
                    "Slow Missiles",
                    "Avoid",
                    "",
                    "",
                    "",
                    "Penetrate",
                    "Dopplezon",
                    "Evade",
                    "",
                    "Valkyrie",
                    "",
                    "Pierce",
                ],
                levels: &[1, 0, 1, 0, 6, 0, 12, 12, 0, 0, 0, 18, 24, 24, 0, 30, 0, 30],
                icons: &[3, 0, 4, 0, 8, 0, 12, 13, 0, 0, 0, 18, 23, 24, 0, 27, 0, 28],
            },
            SkillTree {
                name: "弓和十字弩",
                skills: &[
                    "",
                    "Magic Arrow",
                    "Fire Arrow",
                    "Cold Arrow",
                    "Multiple Shot",
                    "",
                    "",
                    "",
                    "Exploding Arrow",
                    "Ice Arrow",
                    "Guided Arrow",
                    "",
                    "",
                    "Strafe",
                    "Immolation Arrow",
                    "Freezing Arrow",
                    "",
                    "",
                ],
                levels: &[0, 1, 1, 6, 6, 0, 0, 0, 12, 18, 18, 0, 0, 24, 24, 30, 0, 0],
                icons: &[0, 1, 2, 6, 7, 0, 0, 0, 11, 16, 17, 0, 0, 21, 22, 26, 0, 0],
            },
        ],
    },
    ClassSkillTrees {
        class_code: "sor",
        trees: &[
            SkillTree {
                name: "冰系法术",
                skills: &[
                    "",
                    "Ice Bolt",
                    "Frozen Armor",
                    "Frost Nova",
                    "Ice Blast",
                    "",
                    "",
                    "",
                    "Shiver Armor",
                    "",
                    "Glacial Spike",
                    "",
                    "Blizzard",
                    "",
                    "Chilling Armor",
                    "Frozen Orb",
                    "Cold Mastery",
                    "",
                ],
                levels: &[0, 1, 1, 6, 6, 0, 0, 0, 12, 0, 18, 0, 24, 0, 24, 30, 30, 0],
                icons: &[0, 4, 5, 9, 10, 0, 0, 0, 15, 0, 20, 0, 24, 0, 25, 29, 30, 0],
            },
            SkillTree {
                name: "雷系法术",
                skills: &[
                    "",
                    "Charged Bolt",
                    "",
                    "Static Field",
                    "",
                    "Telekinesis",
                    "Nova",
                    "Lightning",
                    "",
                    "",
                    "Chain Lightning",
                    "Teleport",
                    "Thunder Storm",
                    "",
                    "Energy Shield",
                    "",
                    "Lightning Mastery",
                    "",
                ],
                levels: &[0, 1, 0, 6, 0, 6, 12, 12, 0, 0, 18, 18, 24, 0, 24, 0, 30, 0],
                icons: &[0, 3, 0, 7, 0, 8, 13, 14, 0, 0, 18, 19, 22, 0, 23, 0, 28, 0],
            },
            SkillTree {
                name: "火系法术",
                skills: &[
                    "",
                    "Fire Bolt",
                    "Warmth",
                    "Inferno",
                    "",
                    "",
                    "Blaze",
                    "Fire Ball",
                    "",
                    "Fire Wall",
                    "",
                    "Enchant",
                    "",
                    "Meteor",
                    "",
                    "",
                    "Fire Mastery",
                    "Hydra",
                ],
                levels: &[0, 1, 1, 6, 0, 0, 12, 12, 0, 18, 0, 18, 0, 24, 0, 0, 30, 30],
                icons: &[0, 1, 2, 6, 0, 0, 11, 12, 0, 16, 0, 17, 0, 21, 0, 0, 26, 27],
            },
        ],
    },
    ClassSkillTrees {
        class_code: "nec",
        trees: &[
            SkillTree {
                name: "召唤术",
                skills: &[
                    "Skeleton Mastery",
                    "",
                    "Raise Skeleton",
                    "",
                    "Clay Golem",
                    "",
                    "Golem Mastery",
                    "",
                    "Raise Skeletal Mage",
                    "",
                    "BloodGolem",
                    "",
                    "Summon Resist",
                    "IronGolem",
                    "",
                    "",
                    "FireGolem",
                    "Revive",
                ],
                levels: &[1, 0, 1, 0, 6, 0, 12, 0, 12, 0, 18, 0, 24, 24, 0, 0, 30, 30],
                icons: &[4, 0, 5, 0, 10, 0, 14, 0, 15, 0, 20, 0, 24, 25, 0, 0, 29, 30],
            },
            SkillTree {
                name: "毒素和白骨术",
                skills: &[
                    "",
                    "Teeth",
                    "Bone Armor",
                    "Poison Dagger",
                    "Corpse Explosion",
                    "",
                    "",
                    "",
                    "Bone Wall",
                    "Poison Explosion",
                    "Bone Spear",
                    "",
                    "",
                    "",
                    "Bone Prison",
                    "Poison Nova",
                    "Bone Spirit",
                    "",
                ],
                levels: &[0, 1, 1, 6, 6, 0, 0, 0, 12, 18, 18, 0, 0, 0, 24, 30, 30, 0],
                icons: &[0, 2, 3, 8, 9, 0, 0, 0, 13, 18, 19, 0, 0, 0, 23, 27, 28, 0],
            },
            SkillTree {
                name: "诅咒术",
                skills: &[
                    "",
                    "Amplify Damage",
                    "",
                    "Dim Vision",
                    "",
                    "Weaken",
                    "",
                    "Iron Maiden",
                    "Terror",
                    "Confuse",
                    "Life Tap",
                    "",
                    "Attract",
                    "",
                    "Decrepify",
                    "",
                    "Lower Resist",
                    "",
                ],
                levels: &[0, 1, 0, 6, 0, 6, 0, 12, 12, 18, 18, 0, 24, 0, 24, 0, 30, 0],
                icons: &[0, 1, 0, 6, 0, 7, 0, 11, 12, 16, 17, 0, 21, 0, 22, 0, 26, 0],
            },
        ],
    },
    ClassSkillTrees {
        class_code: "pal",
        trees: &[
            SkillTree {
                name: "防御光环",
                skills: &[
                    "Prayer",
                    "",
                    "Resist Fire",
                    "",
                    "Defiance",
                    "Resist Cold",
                    "Cleansing",
                    "",
                    "Resist Lightning",
                    "",
                    "Vigor",
                    "",
                    "Meditation",
                    "",
                    "",
                    "",
                    "Redemption",
                    "Salvation",
                ],
                levels: &[1, 0, 1, 0, 6, 6, 12, 0, 12, 0, 18, 0, 24, 0, 0, 0, 30, 30],
                icons: &[4, 0, 5, 0, 9, 10, 20, 0, 15, 0, 14, 0, 25, 0, 0, 0, 28, 30],
            },
            SkillTree {
                name: "攻击光环",
                skills: &[
                    "Might",
                    "",
                    "",
                    "",
                    "Holy Fire",
                    "Thorns",
                    "Blessed Aim",
                    "",
                    "",
                    "Concentration",
                    "Holy Freeze",
                    "",
                    "",
                    "Holy Shock",
                    "Sanctuary",
                    "Fanaticism",
                    "",
                    "Conviction",
                ],
                levels: &[1, 0, 0, 0, 6, 6, 12, 0, 0, 18, 18, 0, 0, 24, 24, 30, 0, 30],
                icons: &[3, 0, 0, 0, 7, 8, 13, 0, 0, 18, 19, 0, 0, 23, 24, 27, 0, 29],
            },
            SkillTree {
                name: "战斗技能",
                skills: &[
                    "Sacrifice",
                    "",
                    "Smite",
                    "",
                    "Holy Bolt",
                    "",
                    "Zeal",
                    "",
                    "Charge",
                    "Vengeance",
                    "Blessed Hammer",
                    "",
                    "Conversion",
                    "",
                    "Holy Shield",
                    "",
                    "Fist of the Heavens",
                    "",
                ],
                levels: &[1, 0, 1, 0, 6, 0, 12, 0, 12, 18, 18, 0, 24, 0, 24, 0, 30, 0],
                icons: &[1, 0, 2, 0, 6, 0, 11, 0, 12, 21, 17, 0, 16, 0, 22, 0, 26, 0],
            },
        ],
    },
    ClassSkillTrees {
        class_code: "bar",
        trees: &[
            SkillTree {
                name: "呐喊技能",
                skills: &[
                    "Howl",
                    "",
                    "Find Potion",
                    "Taunt",
                    "Shout",
                    "",
                    "",
                    "",
                    "Find Item",
                    "Battle Cry",
                    "",
                    "",
                    "",
                    "Battle Orders",
                    "Grim Ward",
                    "War Cry",
                    "Battle Command",
                    "",
                ],
                levels: &[1, 0, 1, 6, 6, 0, 0, 0, 12, 18, 0, 0, 0, 24, 24, 30, 30, 0],
                icons: &[5, 0, 6, 12, 13, 0, 0, 0, 17, 21, 0, 0, 0, 24, 25, 29, 30, 0],
            },
            SkillTree {
                name: "战斗精通",
                skills: &[
                    "Blade Mastery",
                    "Axe Mastery",
                    "Mace Mastery",
                    "Pole Arm Mastery",
                    "Throwing Mastery",
                    "Spear Mastery",
                    "Increased Stamina",
                    "",
                    "",
                    "",
                    "",
                    "Iron Skin",
                    "Increased Speed",
                    "",
                    "",
                    "",
                    "",
                    "Natural Resistance",
                ],
                levels: &[1, 1, 1, 6, 6, 6, 12, 0, 0, 0, 0, 18, 24, 0, 0, 0, 0, 30],
                icons: &[2, 3, 4, 9, 10, 11, 16, 0, 0, 0, 0, 20, 23, 0, 0, 0, 0, 28],
            },
            SkillTree {
                name: "战斗技能",
                skills: &[
                    "",
                    "Bash",
                    "",
                    "Leap",
                    "",
                    "Double Swing",
                    "",
                    "Stun",
                    "Double Throw",
                    "Leap Attack",
                    "Concentrate",
                    "",
                    "",
                    "",
                    "Frenzy",
                    "Whirlwind",
                    "Berserk",
                    "",
                ],
                levels: &[0, 1, 0, 6, 0, 6, 0, 12, 12, 18, 18, 0, 0, 0, 24, 30, 30, 0],
                icons: &[0, 1, 0, 7, 0, 8, 0, 14, 15, 18, 19, 0, 0, 0, 22, 26, 27, 0],
            },
        ],
    },
    ClassSkillTrees {
        class_code: "dru",
        trees: &[
            SkillTree {
                name: "元素",
                skills: &[
                    "Firestorm",
                    "",
                    "",
                    "Molten Boulder",
                    "",
                    "Arctic Blast",
                    "Eruption",
                    "",
                    "Cyclone Armor",
                    "",
                    "Twister",
                    "",
                    "Volcano",
                    "Tornado",
                    "",
                    "Armageddon",
                    "Hurricane",
                    "",
                ],
                levels: &[1, 0, 0, 6, 0, 6, 12, 0, 12, 0, 18, 0, 24, 24, 0, 30, 30, 0],
                icons: &[
                    17, 0, 0, 18, 0, 19, 20, 0, 21, 0, 22, 0, 23, 24, 0, 26, 25, 0,
                ],
            },
            SkillTree {
                name: "变形",
                skills: &[
                    "Wearwolf",
                    "Shape Shifting",
                    "",
                    "",
                    "",
                    "Wearbear",
                    "Feral Rage",
                    "",
                    "Maul",
                    "Rabies",
                    "Fire Claws",
                    "",
                    "",
                    "Hunger",
                    "Shock Wave",
                    "Fury",
                    "",
                    "",
                ],
                levels: &[1, 1, 0, 0, 0, 6, 12, 0, 12, 18, 18, 0, 0, 24, 24, 30, 0, 0],
                icons: &[
                    11, 27, 0, 0, 0, 12, 28, 0, 29, 13, 14, 0, 0, 30, 15, 16, 0, 0,
                ],
            },
            SkillTree {
                name: "召唤",
                skills: &[
                    "",
                    "Raven",
                    "Plague Poppy",
                    "Oak Sage",
                    "Summon Spirit Wolf",
                    "",
                    "",
                    "",
                    "Cycle of Life",
                    "Heart of Wolverine",
                    "Summon Fenris",
                    "",
                    "",
                    "",
                    "Vines",
                    "Spirit of Barbs",
                    "Summon Grizzly",
                    "",
                ],
                levels: &[0, 1, 1, 6, 6, 0, 0, 0, 12, 18, 18, 0, 0, 0, 24, 30, 30, 0],
                icons: &[0, 1, 2, 9, 4, 0, 0, 0, 5, 6, 7, 0, 0, 0, 8, 3, 10, 0],
            },
        ],
    },
    ClassSkillTrees {
        class_code: "ass",
        trees: &[
            SkillTree {
                name: "武学技艺",
                skills: &[
                    "",
                    "Tiger Strike",
                    "Dragon Talon",
                    "Fists of Fire",
                    "",
                    "Dragon Claw",
                    "",
                    "Cobra Strike",
                    "",
                    "Claws of Thunder",
                    "",
                    "Dragon Tail",
                    "Blades of Ice",
                    "",
                    "Dragon Flight",
                    "",
                    "Royal Strike",
                    "",
                ],
                levels: &[0, 1, 1, 6, 0, 6, 0, 12, 0, 18, 0, 18, 24, 0, 24, 0, 30, 0],
                icons: &[
                    0, 21, 22, 23, 0, 24, 0, 26, 0, 30, 0, 25, 27, 0, 28, 0, 29, 0,
                ],
            },
            SkillTree {
                name: "影身训练",
                skills: &[
                    "",
                    "Claw Mastery",
                    "Psychic Hammer",
                    "Quickness",
                    "",
                    "",
                    "",
                    "Weapon Block",
                    "Cloak of Shadows",
                    "Fade",
                    "Shadow Warrior",
                    "",
                    "",
                    "",
                    "Mind Blast",
                    "Venom",
                    "Shadow Master",
                    "",
                ],
                levels: &[0, 1, 1, 6, 0, 0, 0, 12, 12, 18, 18, 0, 0, 0, 24, 30, 30, 0],
                icons: &[
                    0, 11, 12, 19, 0, 0, 0, 14, 13, 16, 20, 0, 0, 0, 18, 15, 17, 0,
                ],
            },
            SkillTree {
                name: "陷阱",
                skills: &[
                    "",
                    "Fire Trauma",
                    "",
                    "Shock Field",
                    "",
                    "Blade Sentinel",
                    "Charged Bolt Sentry",
                    "Wake of Fire Sentry",
                    "",
                    "",
                    "",
                    "Blade Fury",
                    "Lightning Sentry",
                    "Inferno Sentry",
                    "",
                    "Death Sentry",
                    "",
                    "Blade Shield",
                ],
                levels: &[0, 1, 0, 6, 0, 6, 12, 12, 0, 0, 0, 18, 24, 24, 0, 30, 0, 30],
                icons: &[0, 1, 0, 2, 0, 3, 4, 5, 0, 0, 0, 6, 7, 8, 0, 9, 0, 10],
            },
        ],
    },
    ClassSkillTrees {
        class_code: "war",
        trees: &[
            SkillTree {
                name: "混沌",
                skills: &[
                    "",
                    "",
                    "Miasma Bolt",
                    "Ring of Fire",
                    "Sigil Lethargy",
                    "",
                    "",
                    "Sigil Rancor",
                    "Miasma Chains",
                    "Flame Wave",
                    "",
                    "",
                    "",
                    "Sigil Death",
                    "Enhanced Entropy",
                    "Apocalypse",
                    "",
                    "Abyss",
                ],
                levels: &[0, 0, 1, 6, 6, 0, 0, 12, 12, 18, 0, 0, 0, 24, 24, 30, 0, 30],
                icons: &[
                    0, 0, 24, 22, 21, 0, 0, 23, 27, 26, 0, 0, 0, 25, 28, 29, 0, 30,
                ],
            },
            SkillTree {
                name: "邪术",
                skills: &[
                    "Levitate",
                    "",
                    "Hex Bane",
                    "",
                    "Cleave",
                    "",
                    "Echoing Strike",
                    "",
                    "Hex Purge",
                    "Blade Warp",
                    "Psychic Ward",
                    "",
                    "",
                    "Eldritch Blast",
                    "Hex Siphon",
                    "",
                    "Mirrored Blades",
                    "",
                ],
                levels: &[1, 0, 1, 0, 6, 0, 12, 0, 12, 18, 18, 0, 0, 24, 24, 0, 30, 0],
                icons: &[
                    11, 0, 13, 0, 19, 0, 16, 0, 17, 18, 12, 0, 0, 15, 14, 0, 20, 0,
                ],
            },
            SkillTree {
                name: "恶魔",
                skills: &[
                    "Demonic Mastery",
                    "",
                    "Summon Goatman",
                    "Blood Oath",
                    "Death Mark",
                    "",
                    "",
                    "",
                    "Summon Tainted",
                    "",
                    "Blood Boil",
                    "Summon Defiler",
                    "",
                    "Engorge",
                    "",
                    "Consume",
                    "",
                    "Bind Demon",
                ],
                levels: &[1, 0, 1, 6, 6, 0, 0, 0, 12, 0, 18, 18, 0, 24, 0, 30, 0, 30],
                icons: &[1, 0, 2, 3, 4, 0, 0, 0, 5, 0, 8, 6, 0, 7, 0, 9, 0, 10],
            },
        ],
    },
];

/// 取某职业的技能树；职业未知时返回 `None`。
///
/// `class_code` 用 `PlayerClass.txt` 的 `Code` 列（`ama`/`sor`/…），
/// 大小写不敏感，前后空格会被忽略。
pub fn trees_for_class(class_code: &str) -> Option<&'static [SkillTree]> {
    let want = class_code.trim();
    CLASS_SKILL_TREES
        .iter()
        .find(|c| c.class_code.eq_ignore_ascii_case(want))
        .map(|c| c.trees)
}

/// 某技能属于哪棵树（返回 `(树下标, 树内下标)`）。
///
/// 找不到时返回 `None`——调用方应把它当作"未归类"处理，而不是猜。
pub fn locate_skill(skill: &str, trees: &[SkillTree]) -> Option<(usize, usize)> {
    trees.iter().enumerate().find_map(|(ti, tree)| {
        tree.skills
            .iter()
            .position(|s| *s == skill)
            .map(|si| (ti, si))
    })
}

/// 技能名（`skills.txt` 的 `skill` 列英文值）→ 技能 ID。
///
/// **为什么要这层映射**：本模块的名单存的是**英文技能名**，而存档里是**技能 ID**，
/// 界面上拿到的又是**已本地化的名字**（例如"火焰弹"而不是 `Fire Bolt`）。
/// 直接拿本地化名比对会全部对不上（踩过：30 个技能全被判为"未归类"）。
/// 所以由调用方给出这层映射，之后统一按 ID 匹配。
pub type SkillIdMap = std::collections::HashMap<String, u16>;

/// 按技能 ID 定位它属于哪棵树。
pub fn locate_by_id(id: u16, trees: &[SkillTree], ids: &SkillIdMap) -> Option<(usize, usize)> {
    trees.iter().enumerate().find_map(|(ti, tree)| {
        tree.skills
            .iter()
            .position(|name| ids.get(*name) == Some(&id))
            .map(|si| (ti, si))
    })
}

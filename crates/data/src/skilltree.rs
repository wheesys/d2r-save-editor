//! 技能树结构：职业 → 三系 → 技能名单。
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
//! 正确性由 `crates/data/tests/skilltrees.rs` 兜底：逐个断言名单里的技能
//! 在数据里真实存在，且每个职业的 30 个技能**全部被归类**。

/// 一棵技能树：树名 + 该树的 10 个技能（`skills.txt` 的 `skill` 列值）。
pub struct SkillTree {
    /// 树名（英文，界面上按需翻译）。
    pub name: &'static str,
    /// 该树的技能，顺序即游戏里的排列顺序。
    pub skills: &'static [&'static str],
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
                name: "Bow and Crossbow",
                skills: &[
                    "Magic Arrow",
                    "Fire Arrow",
                    "Cold Arrow",
                    "Multiple Shot",
                    "Exploding Arrow",
                    "Ice Arrow",
                    "Guided Arrow",
                    "Strafe",
                    "Immolation Arrow",
                    "Freezing Arrow",
                ],
            },
            SkillTree {
                name: "Passive and Magic",
                skills: &[
                    "Inner Sight",
                    "Critical Strike",
                    "Dodge",
                    "Slow Missiles",
                    "Avoid",
                    "Penetrate",
                    "Dopplezon",
                    "Evade",
                    "Valkyrie",
                    "Pierce",
                ],
            },
            SkillTree {
                name: "Javelin and Spear",
                skills: &[
                    "Jab",
                    "Power Strike",
                    "Poison Javelin",
                    "Lightning Bolt",
                    "Charged Strike",
                    "Plague Javelin",
                    "Lightning Strike",
                    "Lightning Fury",
                    "Impale",
                    "Fend",
                ],
            },
        ],
    },
    ClassSkillTrees {
        class_code: "sor",
        trees: &[
            SkillTree {
                name: "Fire Spells",
                skills: &[
                    "Fire Bolt",
                    "Warmth",
                    "Inferno",
                    "Blaze",
                    "Fire Ball",
                    "Fire Wall",
                    "Enchant",
                    "Meteor",
                    "Fire Mastery",
                    "Hydra",
                ],
            },
            SkillTree {
                name: "Cold Spells",
                skills: &[
                    "Ice Bolt",
                    "Frozen Armor",
                    "Frost Nova",
                    "Ice Blast",
                    "Shiver Armor",
                    "Glacial Spike",
                    "Blizzard",
                    "Chilling Armor",
                    "Frozen Orb",
                    "Cold Mastery",
                ],
            },
            SkillTree {
                name: "Lightning Spells",
                skills: &[
                    "Charged Bolt",
                    "Static Field",
                    "Nova",
                    "Lightning",
                    "Chain Lightning",
                    "Telekinesis",
                    "Teleport",
                    "Thunder Storm",
                    "Energy Shield",
                    "Lightning Mastery",
                ],
            },
        ],
    },
    ClassSkillTrees {
        class_code: "nec",
        trees: &[
            SkillTree {
                name: "Summoning Skills",
                skills: &[
                    "Skeleton Mastery",
                    "Raise Skeleton",
                    "Clay Golem",
                    "Golem Mastery",
                    "Raise Skeletal Mage",
                    "BloodGolem",
                    "Summon Resist",
                    "IronGolem",
                    "FireGolem",
                    "Revive",
                ],
            },
            SkillTree {
                name: "Poison and Bone",
                skills: &[
                    "Teeth",
                    "Bone Armor",
                    "Corpse Explosion",
                    "Bone Wall",
                    "Poison Dagger",
                    "Poison Explosion",
                    "Bone Spear",
                    "Bone Prison",
                    "Poison Nova",
                    "Bone Spirit",
                ],
            },
            SkillTree {
                name: "Curses",
                skills: &[
                    "Amplify Damage",
                    "Dim Vision",
                    "Weaken",
                    "Iron Maiden",
                    "Terror",
                    "Confuse",
                    "Life Tap",
                    "Attract",
                    "Decrepify",
                    "Lower Resist",
                ],
            },
        ],
    },
    ClassSkillTrees {
        class_code: "pal",
        trees: &[
            SkillTree {
                name: "Combat Skills",
                skills: &[
                    "Sacrifice",
                    "Smite",
                    "Holy Bolt",
                    "Zeal",
                    "Charge",
                    "Vengeance",
                    "Blessed Hammer",
                    "Conversion",
                    "Holy Shield",
                    "Fist of the Heavens",
                ],
            },
            SkillTree {
                name: "Offensive Auras",
                skills: &[
                    "Might",
                    "Holy Fire",
                    "Thorns",
                    "Blessed Aim",
                    "Concentration",
                    "Holy Freeze",
                    "Holy Shock",
                    "Sanctuary",
                    "Fanaticism",
                    "Conviction",
                ],
            },
            SkillTree {
                name: "Defensive Auras",
                skills: &[
                    "Prayer",
                    "Resist Fire",
                    "Defiance",
                    "Resist Cold",
                    "Cleansing",
                    "Resist Lightning",
                    "Vigor",
                    "Meditation",
                    "Redemption",
                    "Salvation",
                ],
            },
        ],
    },
    ClassSkillTrees {
        class_code: "bar",
        trees: &[
            SkillTree {
                name: "Combat Skills",
                skills: &[
                    "Bash",
                    "Leap",
                    "Double Swing",
                    "Stun",
                    "Double Throw",
                    "Leap Attack",
                    "Concentrate",
                    "Frenzy",
                    "Whirlwind",
                    "Berserk",
                ],
            },
            SkillTree {
                name: "Combat Masteries",
                skills: &[
                    "Blade Mastery",
                    "Axe Mastery",
                    "Mace Mastery",
                    "Pole Arm Mastery",
                    "Throwing Mastery",
                    "Spear Mastery",
                    "Increased Stamina",
                    "Iron Skin",
                    "Increased Speed",
                    "Natural Resistance",
                ],
            },
            SkillTree {
                name: "Warcries",
                skills: &[
                    "Howl",
                    "Find Potion",
                    "Taunt",
                    "Shout",
                    "Find Item",
                    "Battle Cry",
                    "Grim Ward",
                    "Battle Orders",
                    "War Cry",
                    "Battle Command",
                ],
            },
        ],
    },
    ClassSkillTrees {
        class_code: "dru",
        trees: &[
            SkillTree {
                name: "Elemental",
                skills: &[
                    "Firestorm",
                    "Molten Boulder",
                    "Arctic Blast",
                    "Eruption",
                    "Cyclone Armor",
                    "Twister",
                    "Volcano",
                    "Tornado",
                    "Armageddon",
                    "Hurricane",
                ],
            },
            SkillTree {
                name: "Shape Shifting",
                skills: &[
                    "Wearwolf",
                    "Shape Shifting",
                    "Wearbear",
                    "Feral Rage",
                    "Maul",
                    "Rabies",
                    "Fire Claws",
                    "Hunger",
                    "Shock Wave",
                    "Fury",
                ],
            },
            SkillTree {
                name: "Summoning",
                skills: &[
                    "Raven",
                    "Plague Poppy",
                    "Oak Sage",
                    "Summon Spirit Wolf",
                    "Cycle of Life",
                    "Heart of Wolverine",
                    "Summon Fenris",
                    "Vines",
                    "Spirit of Barbs",
                    "Summon Grizzly",
                ],
            },
        ],
    },
    ClassSkillTrees {
        class_code: "ass",
        trees: &[
            SkillTree {
                name: "Martial Arts",
                skills: &[
                    "Tiger Strike",
                    "Dragon Talon",
                    "Fists of Fire",
                    "Dragon Claw",
                    "Cobra Strike",
                    "Claws of Thunder",
                    "Dragon Tail",
                    "Blades of Ice",
                    "Dragon Flight",
                    "Royal Strike",
                ],
            },
            SkillTree {
                name: "Traps",
                skills: &[
                    "Fire Trauma",
                    "Shock Field",
                    "Blade Sentinel",
                    "Wake of Fire Sentry",
                    "Charged Bolt Sentry",
                    "Blade Fury",
                    "Lightning Sentry",
                    "Inferno Sentry",
                    "Death Sentry",
                    "Blade Shield",
                ],
            },
            SkillTree {
                name: "Shadow Disciplines",
                skills: &[
                    "Claw Mastery",
                    "Psychic Hammer",
                    "Quickness",
                    "Weapon Block",
                    "Cloak of Shadows",
                    "Fade",
                    "Shadow Warrior",
                    "Mind Blast",
                    "Venom",
                    "Shadow Master",
                ],
            },
        ],
    },
    // 下面是**某个 mod 自创的职业**（`charclass = war`），不是原版职业。
    //
    // 它有 30 个技能，但原版没有对应资料，所以按数据表里的顺序每 10 个切一棵，
    // 树名取自该组技能的语义。名字是英文，中文由界面层映射。
    ClassSkillTrees {
        class_code: "war",
        trees: &[
            SkillTree {
                name: "Summoning and Blood",
                skills: &[
                    "Summon Goatman",
                    "Demonic Mastery",
                    "Death Mark",
                    "Summon Tainted",
                    "Summon Defiler",
                    "Blood Oath",
                    "Engorge",
                    "Blood Boil",
                    "Consume",
                    "Bind Demon",
                ],
            },
            SkillTree {
                name: "Hexes and Combat",
                skills: &[
                    "Levitate",
                    "Eldritch Blast",
                    "Hex Bane",
                    "Hex Siphon",
                    "Psychic Ward",
                    "Echoing Strike",
                    "Hex Purge",
                    "Blade Warp",
                    "Cleave",
                    "Mirrored Blades",
                ],
            },
            SkillTree {
                name: "Sigils and Elements",
                skills: &[
                    "Sigil Lethargy",
                    "Ring of Fire",
                    "Miasma Bolt",
                    "Sigil Rancor",
                    "Enhanced Entropy",
                    "Flame Wave",
                    "Miasma Chains",
                    "Sigil Death",
                    "Apocalypse",
                    "Abyss",
                ],
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

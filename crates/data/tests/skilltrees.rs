//! 技能树名单的正确性。
//!
//! 技能树的归属**不在任何数据文件里**（在游戏界面代码里），所以那张表是写死的。
//! 写死就会写错，因此这里对着 `skills.txt` 逐个核对：
//!
//! 1. 名单里的每个技能在数据里**真实存在**（挡住拼写错误、挡住我凭记忆写错名字）；
//! 2. 每个职业的 30 个技能**全部被归类**（挡住漏掉技能）；
//! 3. 每棵树正好 10 个技能、每职业正好 3 棵树（挡住结构写错）。

use d2r_data::skilltree::{locate_skill, trees_for_class, CLASS_SKILL_TREES};

/// 读数据包里的 `skills.txt`，返回 `(职业代码, 技能名)` 列表。
fn load_skills() -> Option<Vec<(String, String)>> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()?
        .join("app")
        .join("data-bundle")
        .join("excel")
        .join("skills.txt");
    let text = std::fs::read_to_string(&path).ok()?;

    let mut lines = text.lines();
    let header: Vec<&str> = lines.next()?.split('\t').collect();
    let col = |name: &str| header.iter().position(|h| *h == name);
    let (skill_col, class_col) = (col("skill")?, col("charclass")?);

    let mut out = Vec::new();
    for line in lines {
        let fields: Vec<&str> = line.split('\t').collect();
        let (Some(skill), Some(class)) = (fields.get(skill_col), fields.get(class_col)) else {
            continue;
        };
        let (skill, class) = (skill.trim(), class.trim());
        if !skill.is_empty() && !class.is_empty() {
            out.push((class.to_string(), skill.to_string()));
        }
    }
    Some(out)
}

#[test]
fn every_listed_skill_exists_in_data() {
    let Some(rows) = load_skills() else {
        eprintln!("跳过：没有数据包");
        return;
    };

    let mut problems = Vec::new();
    for class in CLASS_SKILL_TREES {
        for tree in class.trees {
            for skill in tree.skills.iter().filter(|s| !s.is_empty()) {
                let found = rows
                    .iter()
                    .any(|(c, s)| c == class.class_code && s == skill);
                if !found {
                    problems.push(format!(
                        "{}/{}: 数据里没有技能 {skill:?}",
                        class.class_code, tree.name
                    ));
                }
            }
        }
    }
    assert!(
        problems.is_empty(),
        "技能树名单与数据不符（{0} 处）：\n{1}",
        problems.len(),
        problems.join("\n")
    );
}

#[test]
fn every_class_skill_is_classified() {
    let Some(rows) = load_skills() else {
        eprintln!("跳过：没有数据包");
        return;
    };

    let mut problems = Vec::new();
    for class in CLASS_SKILL_TREES {
        let Some(trees) = trees_for_class(class.class_code) else {
            problems.push(format!("{}: 取不到技能树", class.class_code));
            continue;
        };
        let in_data: Vec<&String> = rows
            .iter()
            .filter(|(c, _)| c == class.class_code)
            .map(|(_, s)| s)
            .collect();
        for skill in &in_data {
            if locate_skill(skill, trees).is_none() {
                problems.push(format!(
                    "{}: 数据里的技能 {skill:?} 没有被归类",
                    class.class_code
                ));
            }
        }
    }
    assert!(
        problems.is_empty(),
        "有技能没被归类（{0} 处）：\n{1}",
        problems.len(),
        problems.join("\n")
    );
}

#[test]
fn trees_are_three_by_ten() {
    let mut problems = Vec::new();
    for class in CLASS_SKILL_TREES {
        if class.trees.len() != 3 {
            problems.push(format!(
                "{}: 有 {} 棵树（应为 3）",
                class.class_code,
                class.trees.len()
            ));
        }
        for tree in class.trees {
            if tree.skills.iter().filter(|s| !s.is_empty()).count() != 10 {
                problems.push(format!(
                    "{}/{}: 有 {} 个技能（应为 10）",
                    class.class_code,
                    tree.name,
                    tree.skills.iter().filter(|s| !s.is_empty()).count()
                ));
            }
            // 同一棵树里不能有重复
            let real: Vec<&str> = tree
                .skills
                .iter()
                .copied()
                .filter(|s| !s.is_empty())
                .collect();
            for (i, a) in real.iter().enumerate() {
                if real[i + 1..].contains(a) {
                    problems.push(format!(
                        "{}/{}: 技能 {a:?} 重复",
                        class.class_code, tree.name
                    ));
                }
            }
        }
    }
    assert!(
        problems.is_empty(),
        "技能树结构不对：\n{}",
        problems.join("\n")
    );
}

/// 同一个技能不能同时出现在两棵树里。
#[test]
fn skills_belong_to_exactly_one_tree() {
    let mut seen: Vec<(&str, &str)> = Vec::new();
    let mut problems = Vec::new();
    for class in CLASS_SKILL_TREES {
        for tree in class.trees {
            for skill in tree.skills.iter().filter(|s| !s.is_empty()) {
                if let Some((c, t)) = seen
                    .iter()
                    .find(|(c, s)| *c == class.class_code && s == skill)
                {
                    problems.push(format!(
                        "{}/{skill} 同时在 {t} 和 {} 里",
                        class.class_code, tree.name
                    ));
                    let _ = c;
                } else {
                    seen.push((class.class_code, skill));
                }
            }
        }
    }
    assert!(
        problems.is_empty(),
        "技能重复归类：\n{}",
        problems.join("\n")
    );
}

/// 未知职业要返回 `None`，不能瞎猜一棵树给它。
#[test]
fn unknown_class_has_no_trees() {
    assert!(trees_for_class("xyz").is_none());
    assert!(trees_for_class("").is_none());
    // 大小写与空格要能容错
    assert!(trees_for_class(" SOR ").is_some());
    assert!(trees_for_class("Sor").is_some());
}

/// 图标编号与等级门槛必须和技能名单一一对应，空格用 0 占位。
#[test]
fn icons_and_levels_align_with_skills() {
    let mut problems = Vec::new();
    for class in CLASS_SKILL_TREES {
        for tree in class.trees {
            if tree.icons.len() != tree.skills.len() || tree.levels.len() != tree.skills.len() {
                problems.push(format!(
                    "{}/{}: 三个数组长度不一致（skills={}, levels={}, icons={}）",
                    class.class_code,
                    tree.name,
                    tree.skills.len(),
                    tree.levels.len(),
                    tree.icons.len()
                ));
                continue;
            }
            for (i, skill) in tree.skills.iter().enumerate() {
                let (level, icon) = (tree.levels[i], tree.icons[i]);
                if skill.is_empty() {
                    if level != 0 || icon != 0 {
                        problems.push(format!(
                            "{}/{} 第 {i} 格是空格，但 levels={level} icons={icon}",
                            class.class_code, tree.name
                        ));
                    }
                } else if !(1..=30).contains(&icon) {
                    problems.push(format!(
                        "{}/{} 第 {i} 格 {skill:?} 的图标编号 {icon} 不在 1..=30",
                        class.class_code, tree.name
                    ));
                } else if level == 0 {
                    problems.push(format!(
                        "{}/{} 第 {i} 格 {skill:?} 没有等级门槛",
                        class.class_code, tree.name
                    ));
                }
            }
        }
    }
    assert!(
        problems.is_empty(),
        "技能树数据不一致：\n{}",
        problems.join("\n")
    );
}

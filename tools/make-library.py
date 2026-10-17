#!/usr/bin/env python3
"""把装备库（暗金 / 套装）整理成一份紧凑数据，随数据包内嵌。

## 为什么要生成而不是运行时解析

运行时要展示装备库、导入时还要再用。如果每次都去解析
`uniqueitems.txt` / `properties.txt` / `itemstatcost.txt`，
每次打开导入框都会卡一下。这里在**构建期**解析一次，产出一份紧凑 TSV；
程序把它随数据包内嵌，运行时**只解析一次并缓存**。

## 产物

`crates/app/data-bundle/library/items.tsv`（**不入库**：数据包整个目录都在
`.gitignore` 里，只随二进制内嵌）。一行一件物品，最后一列是属性列表
（`属性ID:值:层`，逗号分隔）。物品**显示名**在运行时按当前语言解析，
产物里只放名字键，不固化界面文案。

用法：
    python tools/make-library.py
"""

from __future__ import annotations

import pathlib
import sys

REPO = pathlib.Path(__file__).resolve().parent.parent
EXCEL = REPO / "samples" / "game-data" / "excel"
BUNDLE = REPO / "crates" / "app" / "data-bundle" / "excel"
OUT = REPO / "crates" / "app" / "data-bundle" / "library" / "items.tsv"


def excel_dir() -> pathlib.Path:
    """优先用本机解包目录，没有就用已生成的数据包。"""
    if (EXCEL / "uniqueitems.txt").is_file():
        return EXCEL
    return BUNDLE


def load(directory: pathlib.Path, name: str) -> list[dict[str, str]]:
    path = directory / name
    if not path.is_file():
        return []
    lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
    if not lines:
        return []
    header = lines[0].split("\t")
    rows = []
    for line in lines[1:]:
        cells = line.split("\t")
        rows.append(
            {h: (cells[i] if i < len(cells) else "").strip() for i, h in enumerate(header)}
        )
    return rows


def string_tables() -> list[pathlib.Path]:
    """字符串表目录：本机解包目录优先，其次数据包。"""
    candidates = [
        REPO / "samples" / "game-data" / "lng" / "strings",
        REPO / "crates" / "app" / "data-bundle" / "strings",
    ]
    for c in candidates:
        if c.is_dir():
            return sorted(c.glob("*.json"))
    return []


def load_names(lang: str = "zhCN") -> dict[str, str]:
    """名字键 → 指定语言的显示名。

    物品名分散在 `item-names.json` 与 `item-nameaffixes.json` 里，
    两处都读，后者优先（它的键更具体）。
    """
    import json

    names: dict[str, str] = {}
    for path in string_tables():
        if "item" not in path.name:
            continue
        try:
            data = json.loads(path.read_text(encoding="utf-8-sig"))
        except Exception:
            continue
        if not isinstance(data, list):
            continue
        for entry in data:
            if not isinstance(entry, dict):
                continue
            key = str(entry.get("Key", ""))
            value = entry.get(lang) or entry.get("enUS")
            if key and value:
                names[key] = value
    return names


def as_int(raw: str) -> int:
    try:
        return int(raw)
    except (TypeError, ValueError):
        return 0


# 基型代码 → 分组（界面按它做类型筛选）。
# 不做完整类型表推导：这里只需要够用户分辨的粗分类。
# 物品类型码（数据里 `armor/weapons/misc` 的 `type` 列）→ 分组名。
# 用数据自带的类型码，而不是按代码前缀猜：`ful`（全身铠甲）这类
# 4 字符代码按前缀猜会全部落到"其他"。
GROUP_BY_TYPE = {
    # 防具
    "tors": "身体护甲",
    "helm": "头盔",
    "phlm": "头盔",
    "pelt": "头盔",
    "circ": "头盔",
    "shie": "盾牌",
    "ashd": "圣骑士盾",
    "glov": "手套",
    "boot": "鞋子",
    "belt": "腰带",
    # 武器
    "axe": "武器·斧",
    "swor": "武器·剑",
    "mace": "武器·锤",
    "pole": "武器·长柄",
    "spea": "武器·长矛",
    "jave": "武器·标枪",
    "bow": "武器·弓",
    "xbow": "武器·弩",
    "staf": "武器·法杖",
    "wand": "武器·法杖",
    "orb": "武器·法球",
    "knif": "武器·匕首",
    "h2h": "武器·爪",
    "club": "武器·棍棒",
    "scep": "武器·权杖",
    "hamm": "武器·锤",
    # 首饰与其它
    "ring": "戒指",
    "amul": "项链",
    "jewl": "珠宝",
    "cham": "护身符",
    "rune": "符文",
    "gemz": "宝石",
    "gema": "宝石",
    "gems": "宝石",
    "gemd": "宝石",
    "geme": "宝石",
    "gemr": "宝石",
    "gemt": "宝石",
    # 补全：护身符按大小、职业专用武器、爪/Ama 专用等
    "lcha": "护身符·大",
    "mcha": "护身符·中",
    "scha": "护身符·小",
    "grim": "图腾/专属",
    "pelt": "头盔",
    "head": "专属头饰",
    "h2h2": "武器·爪",
    "abow": "武器·弓",
    "aspe": "武器·长矛",
    "ajav": "武器·标枪",
    "taxe": "武器·斧",
    "tkni": "武器·匕首",
    "cjwl": "珠宝",
    "csch": "卷轴/书",
}


def load_type_map(directory: pathlib.Path) -> dict[str, str]:
    """基型代码 → 类型码（来自 armor/weapons/misc 的 `type` 列）。"""
    out: dict[str, str] = {}
    for name in ("armor.txt", "weapons.txt", "misc.txt"):
        for row in load(directory, name):
            code = row.get("code", "")
            kind = row.get("type", "")
            if code and kind:
                out[code] = kind
    return out


def group_of(code: str, type_map: dict[str, str]) -> str:
    kind = type_map.get(code, "")
    return GROUP_BY_TYPE.get(kind, "其他")


def main() -> int:
    directory = excel_dir()
    print(f"数据目录: {directory}")

    item_stat_cost = load(directory, "itemstatcost.txt")
    if not item_stat_cost:
        print("缺少 itemstatcost.txt，无法换算属性 ID", file=sys.stderr)
        return 1
    stat_id = {r["Stat"]: r["*ID"] for r in item_stat_cost if r.get("Stat")}
    # 按等级属性的"每级多少"要用 op param 换算（值 ÷ 2^op_param），
    # 与属性面板显示同一口径。
    op_param = {
        r["Stat"]: as_int(r.get("op param", "")) for r in item_stat_cost if r.get("Stat")
    }
    # 带 ValShift 的属性（如 item_hp_perlevel 的 8）在存档里按 2^ValShift 对齐，
    # 每级量要再乘回去，否则写进去的值会被当成 0 丢弃（右移后为 0）。
    val_shift = {
        r["Stat"]: as_int(r.get("ValShift", "")) for r in item_stat_cost if r.get("Stat")
    }

    # properties.txt：属性代码 → [(func, stat, param)]
    rules: dict[str, list[tuple[str, str, str]]] = {}
    for row in load(directory, "properties.txt"):
        code = row.get("code", "")
        if not code:
            continue
        legs = []
        for k in range(1, 7):
            func = row.get(f"func{k}", "")
            if func:
                legs.append((func, row.get(f"stat{k}", ""), row.get(f"val{k}", "")))
        if legs:
            rules[code] = legs

    # 基型尺寸
    type_map = load_type_map(directory)

    sizes: dict[str, tuple[int, int]] = {}
    for name in ("weapons.txt", "armor.txt", "misc.txt"):
        for row in load(directory, name):
            code = row.get("code", "")
            if not code:
                continue
            sizes[code] = (
                as_int(row.get("invwidth", "")) or 1,
                as_int(row.get("invheight", "")) or 1,
            )

    def translate(
        prop: str, value: int, par: int = 0
    ) -> list[tuple[int, int, int]] | None:
        """属性代码 → [(属性ID, 值, 层)]；无法翻译返回 None。

        `value` 是取满后的固定值；`par` 是"按等级"属性的每级量
        （表里 `hp/lvl` 这类 `min/max` 为空、`par` 有值）。
        按等级的属性把 `par` 换成存档里的**每级系数**，不去算总值。
        """
        rule = rules.get(prop)
        if not rule:
            return None
        # 按等级：物品只记录"每级加多少"，总量由游戏按角色等级算。
        #
        # 存档里这一项是 **6 位整数系数 + ValShift=8 的存储精度**：
        # 字段只能放 0–63 的整数，写盘时再左移 8 位（× 256）。
        # 所以每级量只能是整数（游戏里 1.5 这类值由 `par ÷ 8` 得出，
        # 在存档里无法精确表达，这里按该属性的步长取整）。
        #
        # 换算：每级量 = par / 2^op_param → 系数 = round(每级量)
        #      存档值   = 系数 × 2^ValShift
        if value == 0 and par != 0:
            per_level = rule[0][1]
            divisor = 1 << op_param.get(per_level, 0)
            shift = 1 << val_shift.get(per_level, 0)
            coefficient = max(round(par / divisor), 1)
            value = coefficient * shift
        out: list[tuple[int, int, int]] = []
        inherited = ""
        for func, stat_name, param in rule:
            sid = stat_id.get(stat_name)
            if func in ("1", "2"):  # 直接写该属性
                if not sid:
                    return None
                out.append((int(sid), value, as_int(param)))
                inherited = param
            elif func == "3":  # 写该属性，参数沿用上一条
                if not sid:
                    return None
                out.append((int(sid), value, as_int(param or inherited)))
            elif func == "7":  # 增强伤害
                sid = stat_id.get("item_maxdamage_percent")
                if not sid:
                    return None
                out.append((int(sid), value, 0))
            elif func in ("8", "15", "16", "17"):  # 速度 / 伤害 min-max / 持续时间
                if not sid:
                    return None
                out.append((int(sid), value, 0))
            elif func in ("10", "11", "19", "22"):  # 技能页 / 施法 / 聚气 / 单技能
                if not sid:
                    return None
                out.append((int(sid), value, as_int(param)))
            else:
                return None
        return out or None

    names = load_names()
    print(f"名字表: {len(names)} 条（{excel_dir()}）")

    items: list[tuple] = []
    skipped: dict[str, int] = {}
    stat_ok = 0

    sources = [
        ("uniqueitems.txt", "暗金", "u", "code"),
        ("setitems.txt", "套装", "s", "item"),
    ]
    for filename, quality, prefix, code_col in sources:
        for row in load(directory, filename):
            if row.get("disabled") == "1":
                continue
            code = row.get(code_col, "")
            # `*ID` 是写进 `quality_data.file_index` 的值（不是行号）
            index = row.get("*ID", "") or row.get("index", "")
            if not code or not index.isdigit():
                continue
            name_key = row.get("*ItemName", "") or row.get("index", "")
            # 生成期就把名字本地化：运行时不必再读名字表，也不会因缺语言而退化
            display_name = names.get(name_key) or names.get(row.get("index", "")) or name_key
            stats: list[tuple[int, int, int]] = []
            for k in range(1, 13):
                prop = row.get(f"prop{k}", "")
                if not prop:
                    continue
                hi = as_int(row.get(f"max{k}", ""))
                lo = as_int(row.get(f"min{k}", ""))
                value = hi if hi else lo  # 取满值
                # 表里 min/max 为空表示"按等级/按公式计算"，没有固定值可取；
                # 这类属性**跳过**，否则会写进 0，而 0 在部分属性上
                # （带 ValShift 的，如每级生命）会让 `d2s` 直接丢弃这条属性。
                par = as_int(row.get(f"par{k}", ""))
                # 固定值与每级量都没有 → 没法复现（随机/特殊公式），跳过
                if value == 0 and par == 0:
                    skipped[prop] = skipped.get(prop, 0) + 1
                    continue
                got = translate(prop, value, par)
                if got:
                    stats.extend(got)
                    stat_ok += 1
                else:
                    skipped[prop] = skipped.get(prop, 0) + 1
            stats.sort()
            w, h = sizes.get(code, (1, 1))
            items.append(
                (
                    f"{prefix}{index}",
                    int(index),
                    display_name,
                    code,
                    quality,
                    group_of(code, type_map),
                    as_int(row.get("lvl req", "")),
                    w,
                    h,
                    stats,
                )
            )

    # 排序：品质 → 分组 → 需求等级 → 名字
    items.sort(key=lambda r: (r[4], r[5], r[6], r[2]))

    OUT.parent.mkdir(parents=True, exist_ok=True)
    header = ["id", "file_index", "name_key", "code", "quality", "group",
              "required_level", "width", "height", "stats"]
    out_lines = ["\t".join(header)]
    for rid, file_index, name_key, code, quality, group, req, w, h, stats in items:
        stat_src = ",".join(f"{s}:{v}:{l}" for s, v, l in stats)
        out_lines.append(
            "\t".join([rid, str(file_index), name_key, code, quality, group,
                       str(req), str(w), str(h), stat_src])
        )
    OUT.write_text("\n".join(out_lines) + "\n", encoding="utf-8")

    total_stats = sum(len(r[9]) for r in items)
    print(f"条目 {len(items)} 条（暗金 {sum(1 for r in items if r[4] == '暗金')}、"
          f"套装 {sum(1 for r in items if r[4] == '套装')}）")
    print(f"属性 {total_stats} 条；成功翻译 {stat_ok} 处属性代码")
    if skipped:
        top = sorted(skipped.items(), key=lambda x: -x[1])[:8]
        print("未能翻译（已跳过）:", ", ".join(f"{k}x{v}" for k, v in top))
    print(f"已写出 {OUT.relative_to(REPO)}（{OUT.stat().st_size // 1024} KB）")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

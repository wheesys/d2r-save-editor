#!/usr/bin/env python3
"""把物品图标整理进数据包（`crates/app/data-bundle/icons/`）。

来源：本机旧编辑器项目（`D:\\code\\diablo_edit`）里已从游戏资源解出并处理好的
物品位图，以及它的 `data/image_order.txt`（**物品代码 → 图片相对路径**的映射）。

产物（都在数据包内，不入 git，随后被内嵌进可执行文件）：
    icons/<ASCII 文件名>.png      网页能直接显示的 PNG
    icons/IMAGE-MAP.tsv           物品代码 → 图标文件名（制表符分隔）

为什么要转 PNG：原图是 24bpp 未压缩 BMP，网页端不能用；
顺便也让体积小很多。**只做格式转换，不缩放**——原图尺寸就是
"格子数×30 像素"，缩了就没法按格子对齐。

用法：
    python tools/make-icons.py --source D:\\code\\diablo_edit
    python tools/make-icons.py --source ... --check
"""

from __future__ import annotations

import argparse
import pathlib
import sys

# 目录名里有中文，映射成 ASCII 以便网页引用稳定
FOLDER_ALIAS = {
    "Misc": "misc",
    "武器": "weapon",
    "帽子": "helm",
    "盾牌": "shield",
    "衣服": "armor",
    "手套": "glove",
    "鞋子": "boot",
    "腰带": "belt",
}


def ascii_name(relative: str) -> str:
    """把 `武器/Sor球/4 Clasped Orb.bmp` 变成稳定的 ASCII 文件名。"""
    parts = pathlib.PurePath(relative).parts
    stem = pathlib.PurePath(relative).stem
    folder = FOLDER_ALIAS.get(parts[0], "other")
    sub = ""
    if len(parts) > 2:
        # 二级目录（如 武器/Sor球）压成 ASCII 前缀
        sub = "_" + "".join(ch for ch in parts[1] if ch.isascii() and ch.isalnum())
    safe = "".join(ch if (ch.isascii() and (ch.isalnum() or ch in " -_")) else "_" for ch in stem)
    safe = safe.strip().replace(" ", "_")
    return f"{folder}{sub}_{safe}.png"


def load_mapping(source: pathlib.Path) -> list[tuple[str, str]]:
    path = source / "data" / "image_order.txt"
    if not path.is_file():
        raise SystemExit(f"[x] 找不到映射文件：{path}")
    out: list[tuple[str, str]] = []
    for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
        if "\t" not in line:
            continue
        code, rel = line.split("\t", 1)
        code, rel = code.strip(), rel.strip()
        if code and rel:
            out.append((code, rel))
    return out


def build(source: pathlib.Path, dest: pathlib.Path, quiet: bool = False) -> int:
    from PIL import Image  # 延迟导入，便于 --help 在没有 Pillow 时也能用

    images_root = source / "resources" / "images"
    if not images_root.is_dir():
        raise SystemExit(f"[x] 找不到图片目录：{images_root}")

    icons = dest / "icons"
    if icons.exists():
        for p in icons.iterdir():
            p.unlink()
    icons.mkdir(parents=True, exist_ok=True)

    mapping = load_mapping(source)
    rows: list[str] = []
    total_bytes = 0
    missing: list[str] = []

    for code, rel in mapping:
        src = images_root / rel
        if not src.is_file():
            missing.append(rel)
            continue
        out_name = ascii_name(rel)
        out_path = icons / out_name
        with Image.open(src) as image:
            image.convert("RGBA").save(out_path, "PNG", optimize=True)
        total_bytes += out_path.stat().st_size
        rows.append(f"{code}\t{out_name}")

    (icons / "IMAGE-MAP.tsv").write_text("\n".join(rows) + "\n", encoding="utf-8")

    if not quiet:
        print(f"图标已生成：{len(rows)} 张 → {icons.relative_to(dest.parent.parent)}")
        print(f"  合计 {total_bytes / 1024:.0f} KB（含 IMAGE-MAP.tsv）")
        if missing:
            print(f"  [!] 源里缺 {len(missing)} 张：{missing[:3]}")
    return len(rows)


def check(dest: pathlib.Path, source: pathlib.Path) -> int:
    icons = dest / "icons"
    mapping_file = icons / "IMAGE-MAP.tsv"
    if not mapping_file.is_file():
        print("[x] 图标还没生成", file=sys.stderr)
        return 2
    rows = [l for l in mapping_file.read_text(encoding="utf-8").splitlines() if l.strip()]
    expected = {l.split("\t")[1] for l in rows}
    actual = {p.name for p in icons.glob("*.png")}
    extra = actual - expected
    missing = expected - actual
    print(f"图标校验：映射 {len(rows)} 条，PNG {len(actual)} 张")
    if missing:
        print(f"  [缺失] {sorted(missing)[:5]}")
    if extra:
        print(f"  [多余] {sorted(extra)[:5]}")
    del source
    return 1 if (missing or extra) else 0


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--source", default=r"D:\code\diablo_edit", help="旧编辑器项目目录（图标来源）")
    ap.add_argument("--dest", default=None, help="数据包目录（默认 crates/app/data-bundle）")
    ap.add_argument("--check", action="store_true")
    args = ap.parse_args()

    repo = pathlib.Path(__file__).resolve().parent.parent
    dest = pathlib.Path(args.dest) if args.dest else repo / "crates" / "app" / "data-bundle"
    source = pathlib.Path(args.source)

    if args.check:
        return check(dest, source)
    count = build(source, dest)
    return 0 if count else 1


if __name__ == "__main__":
    sys.exit(main())

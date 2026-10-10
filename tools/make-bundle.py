#!/usr/bin/env python3
"""把编辑器真正需要的游戏数据整理成一个可随程序分发的数据包。

产物：`crates/app/data-bundle/`

    crates/app/data-bundle/
    ├── excel/            官方 3.2 原版数据表（只有顶层 *.txt，不含 .bin / base\\）
    ├── strings/          字符串表（剔除语音等用不上的大文件）
    └── BUNDLE.json       清单（来源、包含项、逐文件 sha256、总大小）

为什么要筛选：CASC 导出的一整份是 35.53 MB，其中

    excel\\*.bin        15.94 MB  编辑器不读（游戏运行时用的二进制表）
    excel\\base\\       12.50 MB  loader 自己的基准层，不是原版表
    strings\\vo.json     3.09 MB  语音元数据，界面用不到

筛完约 14 MB，随安装包分发是合理的。

用法：
    python tools/make-bundle.py             # 生成数据包
    python tools/make-bundle.py --check     # 校验数据包与来源是否一致
"""

from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import shutil
import sys

REPO = pathlib.Path(__file__).resolve().parent.parent
SRC = REPO / "samples" / "game-data"
DST = REPO / "crates" / "app" / "data-bundle"

# 字符串表里剔除的文件（体积大且编辑器用不到）
STRING_SKIP = {
    "vo.json",  # 3.1 MB 语音元数据
    "chinese-overlay.json",  # 中文覆盖层（改字体用）
}


def sha256_of(path: pathlib.Path, chunk: int = 1 << 20) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        while True:
            b = f.read(chunk)
            if not b:
                break
            h.update(b)
    return h.hexdigest()


def collect() -> list[tuple[pathlib.Path, pathlib.Path]]:
    """返回 (源文件, 目标相对路径) 列表。"""
    out: list[tuple[pathlib.Path, pathlib.Path]] = []

    excel_src = SRC / "excel"
    if excel_src.is_dir():
        # 只要顶层 .txt（跳过 base\ 与 .bin）
        for p in sorted(excel_src.glob("*.txt")):
            out.append((p, pathlib.Path("excel") / p.name))

    strings_src = SRC / "lng" / "strings"
    if strings_src.is_dir():
        for p in sorted(strings_src.glob("*.json")):
            if p.name in STRING_SKIP:
                continue
            out.append((p, pathlib.Path("strings") / p.name))

    return out


def build(quiet: bool = False) -> dict:
    items = collect()
    if not items:
        print("[x] 没有找到来源数据。先运行：pwsh -File tools/fetch-data.ps1", file=sys.stderr)
        return {}

    if DST.exists():
        shutil.rmtree(DST)
    files = []
    for src, rel in items:
        dst = DST / rel
        dst.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(src, dst)
        files.append(
            {
                "path": rel.as_posix(),
                "size": dst.stat().st_size,
                "sha256": sha256_of(dst),
                "source": str(src.relative_to(REPO)).replace("\\", "/"),
            }
        )

    manifest = {
        "schema": 1,
        "note": "随程序分发的游戏数据包；由 tools/make-bundle.py 生成。"
        "内容为暴雪版权数据（官方 3.2 原表与字符串表），仅用于本机存档编辑。",
        "excel_files": sum(1 for f in files if f["path"].startswith("excel/")),
        "string_files": sum(1 for f in files if f["path"].startswith("strings/")),
        "total_bytes": sum(f["size"] for f in files),
        "files": files,
    }
    (DST / "BUNDLE.json").write_text(json.dumps(manifest, ensure_ascii=False, indent=1), encoding="utf-8")

    if not quiet:
        mb = manifest["total_bytes"] / 1048576
        print(f"数据包已生成：{len(files)} 个文件 / {mb:.2f} MB → {DST.relative_to(REPO)}")
        print(f"  excel   {manifest['excel_files']} 个")
        print(f"  strings {manifest['string_files']} 个")
    return manifest


def check() -> int:
    manifest_path = DST / "BUNDLE.json"
    if not manifest_path.is_file():
        print("[x] 数据包不存在，先运行 python tools/make-bundle.py", file=sys.stderr)
        return 2
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    bad = 0
    for f in manifest["files"]:
        p = DST / f["path"]
        if not p.is_file():
            print(f"[缺失] {f['path']}")
            bad += 1
            continue
        if p.stat().st_size != f["size"] or sha256_of(p) != f["sha256"]:
            print(f"[不一致] {f['path']}")
            bad += 1
    # 反向检查：有没有多余文件
    expected = {f["path"] for f in manifest["files"]} | {"BUNDLE.json"}
    for p in DST.rglob("*"):
        if p.is_file():
            rel = p.relative_to(DST).as_posix()
            if rel not in expected:
                print(f"[多余] {rel}")
                bad += 1
    print(f"数据包校验：{len(manifest['files'])} 个文件，问题 {bad}")
    return 1 if bad else 0


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true")
    args = ap.parse_args()
    if args.check:
        return check()
    m = build()
    return 0 if m else 1


if __name__ == "__main__":
    sys.exit(main())

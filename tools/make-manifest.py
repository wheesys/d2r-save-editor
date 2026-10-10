#!/usr/bin/env python3
"""生成 / 校验 samples 的数据清单（manifest.json + DATA-MANIFEST.md）。

清单只记录**元数据**（文件数、字节数、sha256），不含任何游戏内容，
所以可以安全入库；真正的数据留在本机、不进 git（见 .gitignore）。

用法：
    python tools/make-manifest.py                 # 生成清单（写入 samples/）
    python tools/make-manifest.py --check         # 用清单校验本机数据是否一致
"""

from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import sys
from datetime import datetime, timezone

REPO = pathlib.Path(__file__).resolve().parent.parent
SAMPLES = REPO / "samples"

# 清单覆盖的组（相对 samples/ 的路径 → 说明）
GROUPS: dict[str, str] = {
    "game-data/excel": "官方 CASC 导出的 3.2 原版 excel 表（判定 mod 改动的基准）",
    "game-data/lng/strings": "官方字符串表（物品名/技能名/界面文案，13 种语言）",
    "saves": "存档样本（角色 .d2s、共享箱 .d2i）",
    "sidecar": "D2RLoader 与 mod 的旁车文件（.d2rl/.abs/.mercs）",
    "archives": "原始备份压缩包",
    "work": "临时工作副本（改动前后对照等，可随时删）",
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


def scan_group(rel: str) -> dict:
    root = SAMPLES / rel
    files: list[dict] = []
    if root.is_dir():
        for p in sorted(root.rglob("*")):
            if p.is_file():
                files.append(
                    {
                        "path": p.relative_to(SAMPLES).as_posix(),
                        "size": p.stat().st_size,
                        "sha256": sha256_of(p),
                    }
                )
    return {
        "group": rel,
        "description": GROUPS[rel],
        "exists": root.is_dir(),
        "file_count": len(files),
        "total_bytes": sum(f["size"] for f in files),
        "files": files,
    }


def build() -> dict:
    groups = [scan_group(g) for g in GROUPS]
    return {
        "schema": 1,
        "note": "只记录元数据（文件数/字节数/sha256），不含游戏内容；数据由本机导出，详见 samples/README.md",
        "generated_at": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "groups": groups,
    }


def write_markdown(manifest: dict) -> str:
    lines = [
        "# 数据清单（自动生成，请勿手改）",
        "",
        f"> 生成时间：{manifest['generated_at']}　│　由 `tools/make-manifest.py` 生成",
        ">",
        "> 本文件只记录**元数据**（文件数、字节数、sha256），不含任何游戏内容。",
        "> 数据留在本机、不入 git；用 `python tools/make-manifest.py --check` 校验完整性，",
        "> 用 `tools/fetch-data.ps1` 从游戏目录重新导出。",
        "",
        "| 组 | 文件数 | 大小 | 说明 |",
        "|---|---:|---:|---|",
    ]
    for g in manifest["groups"]:
        mb = g["total_bytes"] / 1048576
        state = "" if g["exists"] else "（本机缺失）"
        lines.append(
            f"| `{g['group']}` | {g['file_count']} | {mb:.2f} MB | {g['description']}{state} |"
        )
    lines += ["", "## 逐文件 sha256", ""]
    for g in manifest["groups"]:
        if not g["files"]:
            continue
        lines += [f"### `{g['group']}`（{g['file_count']} 个文件）", "", "| 文件 | 字节 | sha256 |", "|---|---:|---|"]
        for f in g["files"]:
            lines.append(f"| `{f['path']}` | {f['size']} | `{f['sha256']}` |")
        lines.append("")
    return "\n".join(lines)


def check(manifest: dict) -> int:
    bad = 0
    missing = 0
    for g in manifest["groups"]:
        root = SAMPLES / g["group"]
        if not root.is_dir():
            if g["exists"]:
                print(f"[缺失] 组 {g['group']} 不在本机")
                missing += 1
            continue
        for f in g["files"]:
            p = SAMPLES / f["path"]
            if not p.is_file():
                print(f"[缺失] {f['path']}")
                missing += 1
                continue
            if p.stat().st_size != f["size"] or sha256_of(p) != f["sha256"]:
                print(f"[不一致] {f['path']}")
                bad += 1
    total = sum(g["file_count"] for g in manifest["groups"])
    print(f"校验完成：清单 {total} 个文件，缺失 {missing}，内容不一致 {bad}")
    return 1 if (missing or bad) else 0


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true", help="用现有清单校验本机数据")
    args = ap.parse_args()

    manifest_path = SAMPLES / "manifest.json"
    if args.check:
        if not manifest_path.is_file():
            print("没有找到 samples/manifest.json，先生成一次清单")
            return 2
        return check(json.loads(manifest_path.read_text(encoding="utf-8")))

    manifest = build()
    manifest_path.write_text(json.dumps(manifest, ensure_ascii=False, indent=1), encoding="utf-8")
    (SAMPLES / "DATA-MANIFEST.md").write_text(write_markdown(manifest), encoding="utf-8")
    total = sum(g["file_count"] for g in manifest["groups"])
    size = sum(g["total_bytes"] for g in manifest["groups"]) / 1048576
    print(f"已生成清单：{total} 个文件 / {size:.2f} MB")
    for g in manifest["groups"]:
        flag = "" if g["exists"] else "  ← 本机缺失"
        print(f"  {g['group']:<24} {g['file_count']:>4} 个  {g['total_bytes']/1048576:>7.2f} MB{flag}")
    return 0


if __name__ == "__main__":
    sys.exit(main())

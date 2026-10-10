#!/usr/bin/env python3
"""统计本机样本目录的**数量与体积**（不记录来源/清单，产物不入库）。

用途：开发时确认"本机数据齐不齐"，以及打包前核对数据包规模。
产物 `samples/local-stats.json` 只含聚合数字（每组文件数、总字节数），
不含任何文件名与哈希，且已被 `.gitignore` 排除。

用法：
    python tools/local-stats.py
"""

from __future__ import annotations

import json
import pathlib
import sys
from datetime import datetime, timezone

REPO = pathlib.Path(__file__).resolve().parent.parent
SAMPLES = REPO / "samples"
OUT = SAMPLES / "local-stats.json"

# 只看这几组（相对 samples/），只统计数量与体积
GROUPS = [
    "game-data",
    "saves",
    "sidecar",
    "archives",
    "work",
]


def measure(rel: str) -> dict:
    root = SAMPLES / rel
    files = [p for p in root.rglob("*") if p.is_file()] if root.is_dir() else []
    return {
        "group": rel,
        "present": root.is_dir(),
        "file_count": len(files),
        "total_bytes": sum(p.stat().st_size for p in files),
    }


def main() -> int:
    stats = [measure(g) for g in GROUPS]
    doc = {
        "note": "只含聚合数量与体积，不含来源、文件名或哈希；本文件不入库。",
        "generated_at": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "groups": stats,
        "total_files": sum(s["file_count"] for s in stats),
        "total_bytes": sum(s["total_bytes"] for s in stats),
    }
    SAMPLES.mkdir(parents=True, exist_ok=True)
    OUT.write_text(json.dumps(doc, ensure_ascii=False, indent=1), encoding="utf-8")
    print(f"已统计：{doc['total_files']} 个文件 / {doc['total_bytes'] / 1048576:.2f} MB → {OUT.relative_to(REPO)}")
    for s in stats:
        flag = "" if s["present"] else "  ← 本机缺失"
        print(f"  {s['group']:<14} {s['file_count']:>4} 个  {s['total_bytes'] / 1048576:>7.2f} MB{flag}")
    return 0


if __name__ == "__main__":
    sys.exit(main())

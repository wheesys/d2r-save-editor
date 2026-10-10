# D2R 存档修改器（d2r-save-editor）

暗黑破坏神2：重制版（Diablo II: Resurrected）单机存档修改器。Windows 桌面客户端，Rust + Tauri v2 实现。

> 状态：调研完成，Rust 内核骨架已可跑（`.d2s` 读写 + 往返校验 + 数据集加载），界面未开始。

## 仓库结构

```text
crates/
├── core/   d2r-core   存档读写、头部/版本识别、往返一致性校验
├── data/   d2r-data   数据集加载（excel 表、字符串表、mod 描述），数据驱动不写死常量
└── cli/    d2r        命令行工具：roundtrip / inspect / dataset
samples/               本地样本（存档、旁车文件、导出的游戏数据；已 gitignore）
.tools/                开发辅助脚本（CASC 提取器源码、UI 驱动脚本等）
docs/                  调研与资料清单
```

## 开发

```powershell
# 构建 + 测试
cargo build
cargo test --workspace

# 往返一致性校验（写路径的门槛：读入 → 原样写出 → 逐字节比对）
cargo run -p d2r-cli -- roundtrip

# 查看存档内容（--json 供界面调用）
cargo run -p d2r-cli -- inspect "samples/saves/d2r-105/杜仲.d2s" --json

# 检查数据集：表数量、与原版基准的差异表、字符串表
cargo run -p d2r-cli -- dataset
```

环境变量：`D2R_GAME_DIR`（游戏目录）、`D2R_SAMPLES_DIR`（样本目录）。

## 当前进度

| 里程碑 | 状态 |
|---|---|
| 1 数据集加载器 | 🟢 excel 表（loader 展开的 88 张 + 官方 CASC 导出 91 张）、字符串表（24,179 条 / 13 种语言）、mod 描述；官方表与本机数据已交叉比对 |
| 2 `.d2s` 读入→写出逐字节一致 | 🟢 8 个样本中 **7 个逐字节一致**（含 `.d2i`）；剩 1 个已知条目段编码差异，见 `docs/next-steps-materials.md` §3.10 |
| 3 容器与物品摆放校验 | 🟢 区域级重叠 / 装备位重复 / 未知物品代码；发现"不能用原版表判越界"与"合成箱局部坐标"两条坑（§3.9） |
| 4 属性/技能/任务/传送点编辑 | ⬜ 待开始 |
| 5 物品编辑与导入 | ⬜ 待开始 |
| 6 佣兵装备 | ⬜ 待开始 |
| 7 Tauri 界面整合 | ⬜ 待开始 |

已能用的命令：

```powershell
cargo run -p d2r-cli -- roundtrip                       # 往返一致性（写路径门槛）
cargo run -p d2r-cli -- inspect <存档> [--json]          # 头部/角色/物品（中文名）
cargo run -p d2r-cli -- validate <文件.d2s>              # 容器与摆放校验
cargo run -p d2r-cli -- dataset                          # 数据集与字符串表
cargo run -p d2r-cli -- strings [rvl amu]                # 字符串表抽查
cargo run -p d2r-data --bin compare-dirs                 # 官方表 vs loader base vs merged
cargo run -p d2r-core --bin rewrite-save -- <原> <新>     # 读入再写出（配合 cmp-bytes 定位差异）
cargo run -p d2r-core --bin cmp-bytes -- <原> <新>        # 字节级差异定位
```

## 功能目标

- **角色编辑**：属性、属性点、技能等级、技能点、任务完成情况、传送点（小站）开通情况
- **物品编辑**：
  - 导入本项目自定义格式的物品文件
  - 导入基础物品并修改全部属性（词缀、孔数、耐久、无形、镶嵌物等）
  - 编辑过程全程校验，不允许生成破坏存档的物品
- **容器支持**：背包、个人储物箱、共享储物箱（`.d2i`）、合成箱（方块）均支持不同尺寸
- **Mod 首批支持**（不是后续支持）：
  - Mod 修改背包/箱子/合成箱尺寸
  - 物品占格改动（传送书、辨识书占格等）
  - 佣兵全装备
- 仅本地操作存档文件，不联网、不接触在线服务

## 技术栈

| 层 | 选型 |
|---|---|
| 核心 | Rust 独立库：解析、编码、校验（与界面解耦，可单测、可先做 CLI） |
| 界面 | Tauri v2（WebView2，单 exe 约 5-10MB） |
| 构建 | GitHub Actions（公开项目），仅推送 tag 触发 release 构建 |

## 文档

- [调研文档](docs/research.md)
- [继续开发所需资料清单](docs/next-steps-materials.md)
- [样本目录说明](samples/README.md)

## 许可证

MIT © 2026 wheesys

# D2R 存档修改器（d2r-save-editor）

暗黑破坏神2：重制版（Diablo II: Resurrected）单机存档修改器。Windows 桌面客户端，Rust + Tauri v2 实现。

> 状态：调研完成，Rust 内核骨架已可跑（`.d2s` 读写 + 往返校验 + 数据集加载），界面未开始。

## 仓库结构

```text
crates/
├── core/   d2r-core   存档读写、头部/版本识别、往返校验、编辑会话
├── data/   d2r-data   数据集加载（excel 表、字符串表、物品/技能名、摆放校验）
├── app/    d2r-app    服务层：数据集 + 字符串表 + 编辑会话（CLI 与界面共用）
└── cli/    d2r        命令行工具
desktop/  Tauri v2 桌面端（src-tauri = Rust 后端，dist = 前端构建产物）
web/      前端源码（Vite + TypeScript，无框架）
tools/    数据导出与清单工具（fetch-data.ps1、make-manifest.py）
samples/  本地样本与数据清单（数据不入库，只入清单；见 samples/README.md）
.tools/   开发辅助脚本（CASC 提取器源码、UI 驱动脚本等）
docs/     调研与资料清单
```

## 数据从哪来（不进 git，但可复现）

游戏数据（官方 excel 表、字符串表）是暴雪版权内容，存档是个人数据——**都不入 git**。
仓库里只保留元数据清单（`samples/DATA-MANIFEST.md`、`samples/manifest.json`，含逐文件 sha256）。

```powershell
# 在一台装有游戏 + CascView 的机器上重建数据
pwsh -File tools/fetch-data.ps1            # 自动定位游戏目录，导出 excel 与字符串表
python tools/make-manifest.py --check      # 用清单校验（418 个文件 / 35.53 MB）
```

详见 [samples/README.md](samples/README.md)。

**发布打包**（数据包不进 git，打包时在本地生成并压缩进产物）：

```powershell
pwsh -File tools/package.ps1        # → dist/d2r-save-editor-<版本>-win-x64.zip
```

流程：生成数据包 → 构建前端 → release 构建（数据内嵌进 exe）→ 组装 → 压缩。
release 构建在缺少数据包时会**直接失败**，避免发出打开就没数据的包。

## 开发

```powershell
# 核心（工作区）
cargo build
cargo test --workspace

# 命令行
cargo run -p d2r-cli -- roundtrip                       # 往返一致性（写路径门槛）
cargo run -p d2r-cli -- inspect <存档> [--json]          # 头部/角色/物品（中文名）
cargo run -p d2r-cli -- view   <存档> [--json]           # 角色+技能+传送点+物品总览
cargo run -p d2r-cli -- edit   <存档> --set '<JSON>'     # 编辑并保存（先校验后落盘）
cargo run -p d2r-cli -- items  <存档> [--filter 关键字]  # 物品清单/明细（--json）
cargo run -p d2r-cli -- validate <文件.d2s> [--layout JSON]  # 容器与摆放校验（尺寸可确认）
cargo run -p d2r-cli -- dataset                          # 数据集与字符串表
cargo run -p d2r-cli -- strings [rvl amu]                # 字符串表抽查
cargo run -p d2r-data --bin compare-dirs                 # 官方表 vs loader base vs merged
cargo run -p d2r-core --bin rewrite-save -- <原> <新>     # 读入再写出（配合 cmp-bytes）
cargo run -p d2r-core --bin cmp-bytes -- <原> <新>        # 字节级差异定位

# 桌面端（独立于工作区，避免 Tauri 依赖拖慢核心 crate 构建）
cd web; npm install; npm run build      # 前端 → desktop/dist
cd desktop/src-tauri; cargo build       # 后端 + 壳（产物 target/debug/d2r-desktop.exe）
```

编辑示例：

```powershell
d2r edit 角色.d2s --set '[{"kind":"setLevel","level":90},
                          {"kind":"setStat","stat":0,"value":300},
                          {"kind":"setSkill","index":0,"level":20},
                          {"kind":"unlockWaypoints","difficulty":0}]' --dry-run
```

保存前会自动"写出 → 重新解析 → 比对角色信息与尾随数据"，通过才落盘，并给原文件留 `.bak` 备份。

环境变量：`D2R_GAME_DIR`（游戏目录）、`D2R_SAMPLES_DIR`（样本目录）、`D2R_STRINGS_DIR`（字符串表目录）。

## 当前进度

| 里程碑 | 状态 |
|---|---|
| 1 数据集加载器 | 🟢 excel 表（loader 展开 88 张 + 官方 CASC 导出 91 张）、字符串表（24,179 条 / 13 种语言）、物品名与技能名解析 |
| 2 `.d2s` 读入→写出逐字节一致 | 🟢 8 个样本中 **7 个逐字节一致**（含 `.d2i`）；剩 1 个已知条目段编码差异（见 `docs/next-steps-materials.md` §3.10） |
| 3 容器与物品摆放校验 | 🟢 区域级重叠 / 装备位重复 / 未知物品代码；记录了"不能用原版表判越界"与"合成箱局部坐标"两条坑（§3.9） |
| 4 属性/技能/任务/传送点编辑 | 🟢 等级、8 项属性、技能等级、三难度传送点；带校验与备份的保存 |
| 5 物品编辑 | 🟢 查看明细（属性/耐久/数量/孔）、改耐久·数量·防御、增删属性、移动格子、删除物品（含自包含数据包） |
| 5b 物品导入（自定义 JSON） | ⬜ 待开始 |
| 6 佣兵装备 | ⬜ 待开始 |
| 7 Tauri 界面整合 | 🟢 桌面端可运行：打开存档 → 显示角色/技能/传送点/物品 → 编辑 → 校验 → 保存（界面中文） |

> 桌面端的已知环境适配：Tauri 默认把 WebView2 数据目录设为 `%LOCALAPPDATA%\<identifier>`，
> 某些受限环境下该目录创建会失败（`os error 5`）。因此窗口改为在代码里创建并显式指定
> 数据目录（exe 同级 `webview2-data`，不可写则退回系统临时目录），见 `desktop/src-tauri/src/lib.rs`。

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
- [样本目录说明与数据清单](samples/README.md)
- [逐文件 sha256 清单](samples/DATA-MANIFEST.md)

## 许可证

MIT © 2026 wheesys

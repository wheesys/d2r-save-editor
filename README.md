# D2R 存档修改器（d2r-save-editor）

暗黑破坏神2：重制版（Diablo II: Resurrected）单机存档修改器。Windows 桌面客户端，Rust + Tauri v2 实现。

> 状态：调研完成，核心与桌面端均可运行（存档读写 + 往返校验 + 角色/物品编辑 + 界面）。

## 仓库结构

```text
crates/
├── core/   d2r-core   存档读写、头部/版本识别、往返校验、编辑会话、布局设置
├── data/   d2r-data   数据集加载（表解析、字符串表、物品/技能名、摆放校验）
├── app/    d2r-app    服务层：数据集 + 字符串表 + 编辑会话（CLI 与界面共用）
└── cli/    d2r        命令行工具
desktop/  Tauri v2 桌面端（src-tauri = Rust 后端，dist = 前端构建产物）
web/      前端源码（Vite + TypeScript，无框架）
tools/    本地数据准备与打包脚本（不入库的数据由它们在本机生成）
samples/  本地测试样本（不入库；见 samples/README.md）
.tools/   GUI 自动化辅助脚本
docs/     调研与开发记录
```

## 关于运行所需的本地数据

程序运行需要的**数据表与字符串表不放在仓库里**，也不随仓库分发：

- 程序构建时会把这些数据**内嵌进可执行文件**（见 `tools/make-bundle.py`），
  因此终端用户只需要本程序 + 要修改的存档文件；
- 这些数据由**每个使用者在本机自行准备**（`tools/fetch-data.ps1`），
  生成物统一落在 `samples/game-data/`，该目录已被 `.gitignore` 排除；
- 仓库与文档**不记录**这些数据的来源、清单、体积或文件名。

> 贡献者请注意：**不要**在 issue、PR、文档或注释里提交本地数据的来源说明、
> 文件清单、哈希或提取方法。涉及数据准备的内容请只留在本机。

## 发布打包

```powershell
pwsh -File tools/package.ps1                  # → dist/D2RSaveEditor-<版本>-win-x64.exe
pwsh -File tools/package.ps1 -AlsoZip         # 额外再打一个 zip（含说明文件）
```

产物是**一个自足的 exe**：不需要安装、不需要额外 DLL、不需要游戏或任何工具。

流程：准备本地数据 → 构建前端 → release 构建 → 拷出单文件。

数据在构建期**先压缩再内嵌**（文本数据能压到约 18%），所以 exe 体积远小于"数据原样内嵌"。
release 构建在缺少本地数据时会**直接失败**，避免发出打开就没数据的包。

### 发布到 GitHub Release

```powershell
git tag v0.1.0
git push origin v0.1.0      # 触发 .github/workflows/release.yml
```

CI 构建并上传一个 exe 到 Release。**CI 上没有游戏数据**（仓库不存放），
所以 CI 产出的 exe、以及源码构建出来的 exe，都只有程序本体：可以启动、打开存档，
但物品名/技能名会退化成代码。

要发布带完整数据的版本，用本机打包的产物：

```powershell
pwsh -File tools/package.ps1        # dist/D2RSaveEditor-<版本>-win-x64.exe
```

两种 exe 都以"公开 Release 资产"的形式分发，或私下传递。

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
cargo run -p d2r-cli -- stats  [--filter 关键字]        # 属性 ID ↔ 中文名对照（步长/参数位）
cargo run -p d2r-cli -- export-items <存档> [--index N] --out bp.json   # 导出物品蓝图
cargo run -p d2r-cli -- import-item  <存档> --blueprint bp.json --template N  # 导入物品
cargo run -p d2r-cli -- validate <文件.d2s> [--layout JSON]  # 容器与摆放校验（尺寸可确认）
cargo run -p d2r-cli -- dataset                          # 数据集与字符串表
cargo run -p d2r-cli -- strings [rvl amu]                # 字符串表抽查
cargo run -p d2r-data --bin compare-dirs                 # 三方数据表比对
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

环境变量：`D2R_GAME_DIR`（游戏目录，可选）、`D2R_SAMPLES_DIR`（测试样本目录）、
`D2R_STRINGS_DIR`（字符串表目录，可选）。

## 当前进度

| 里程碑 | 状态 |
|---|---|
| 1 数据集加载器 | 🟢 表解析与缓存、字符串表、物品名/技能名解析（含大小写不敏感查找与 BOM 容错） |
| 2 `.d2s` 读入→写出逐字节一致 | 🟢 8 个样本中 **7 个逐字节一致**（含 `.d2i`）；剩 1 个已知条目段编码差异（见 `docs/next-steps-materials.md` §3.10） |
| 3 容器与物品摆放校验 | 🟢 区域级重叠 / 装备位重复 / 未知物品代码；**容器尺寸与佣兵全装备可让用户确认**，确认后越界判定为硬校验（§3.9） |
| 4 属性/技能/任务/传送点编辑 | 🟢 等级、8 项属性、技能等级、三难度传送点；带校验与备份的保存 |
| 5 物品编辑 | 🟢 查看明细（属性/耐久/数量/孔）、改耐久·数量·防御、增删属性、移动格子、删除物品 |
| 5b 物品导入（自定义 JSON） | 🟢 物品导出/导入（模板克隆 + 属性覆盖）、属性位宽与移位校验（写盘后会被截断的值直接拒绝）、属性 ID ↔ 中文名对照 |
| 6 佣兵装备 | ⬜ 待开始 |
| 7 Tauri 界面整合 | 🟢 打开存档 → 角色/技能/传送点/物品 → 编辑 → 校验 → 保存（界面中文） |

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
  - Mod 修改背包/箱子/合成箱尺寸（由用户在界面上确认尺寸）
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

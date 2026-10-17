# 技术文档

面向**开发者与贡献者**。普通使用请看 [README.md](../README.md)。

---

## 仓库结构

```text
crates/
├── core/   d2r-core   存档读写、头部/版本识别、往返校验、编辑会话、布局设置
├── data/   d2r-data   数据集加载（表解析、字符串表、物品/技能名、摆放校验）
├── app/    d2r-app    服务层：数据集 + 字符串表 + 编辑会话（CLI 与界面共用）
└── cli/    d2r        命令行工具
desktop/  Tauri v2 桌面端（src-tauri = Rust 后端，dist = 前端构建产物）
web/      前端源码（Vite + TypeScript，无框架）
tools/    本地数据准备与打包脚本
samples/  本地测试样本（不入库）
docs/     调研与开发记录
```

## 技术栈

| 层 | 选型 | 说明 |
|---|---|---|
| 核心 | Rust 独立库 | 解析、编码、校验与界面解耦，可单测、可先做 CLI |
| 界面 | Tauri v2 + WebView2 | 前端 Vite + TypeScript，不引入框架 |
| 构建 | GitHub Actions | 仅推送 tag 触发 release 构建 |

**桌面端刻意不纳入 Cargo workspace**（见根 `Cargo.toml` 的 `exclude`）：
Tauri 依赖较重，纳进来会拖慢核心 crate 的每次构建。

## 构建与打包

构建分两步：先准备本机资源包，再编译。

```powershell
# 核心（workspace）
cargo build
cargo test --workspace

# 桌面端（独立于 workspace）
cd web; npm install; npm run build      # 前端 → desktop/dist
cd desktop/src-tauri; cargo build       # 后端 + 壳

# 发布包
pwsh -File tools/package.ps1            # → dist/D2RSaveEditor-<版本>-win-x64.exe
pwsh -File tools/package.ps1 -AlsoZip   # 额外打一个 zip（含说明文件）
```

流程：准备本机资源包 → 构建前端 → release 构建 → 拷出单文件。
资源包在构建期先压缩，所以 exe 体积远小于原样打包。

### WebView2 数据目录

Tauri 默认把 WebView2 数据目录设为 `%LOCALAPPDATA%\<identifier>`，
某些受限环境下该目录创建会失败（`os error 5`）。因此窗口改为在代码里创建
并显式指定数据目录（exe 级 `webview2-data`，不可写则退回系统临时目录），
见 `desktop/src-tauri/src/lib.rs`。

### 应用清单（管理员权限）

程序需要写入游戏存档（通常在受保护目录），因此清单声明 `requireAdministrator`。
清单文件是**纯 ASCII**：Tauri 会把 `.rc` 转成 ANSI，而清单声明 UTF-8，
含非 ASCII 字符会让清单变成非法 XML，程序启动即报"并行配置不正确"。
`desktop/src-tauri/build.rs` 里有构建期检查，非 ASCII 会直接编译失败。

> 不要用链接器 `/MANIFESTINPUT` 或 `winres` 内嵌清单，会产生
> 资源重复（`CVT1100`）或 `LNK1123`。

### 发布到 GitHub Release

```powershell
git tag v0.0.1
git push origin v0.0.1      # 触发 .github/workflows/release.yml
```

CI 上没有本地数据（仓库不存放），所以 **CI 产出的 exe 只有程序本体**：
可以启动、打开存档，但物品名/技能名会退化成代码。
要发布带完整数据的版本，用本机 `tools/package.ps1` 的产物。

## 开发命令

```powershell
# 校验与探查
cargo run -p d2r-cli -- roundtrip                        # 往返一致性（写路径门槛）
cargo run -p d2r-cli -- inspect <存档> [--json]           # 头部/角色/物品
cargo run -p d2r-cli -- view   <存档> [--json]            # 角色+技能+传送点+物品总览
cargo run -p d2r-cli -- validate <文件.d2s> [--layout JSON]  # 容器与摆放校验
cargo run -p d2r-cli -- dataset                          # 数据集与字符串表
cargo run -p d2r-cli -- strings [rvl amu]                # 字符串表抽查

# 编辑
cargo run -p d2r-cli -- edit   <存档> --set '<JSON>'      # 编辑并保存（先校验后落盘）
cargo run -p d2r-cli -- items  <存档> [--filter 关键字]    # 物品清单（--json）
cargo run -p d2r-cli -- item   <存档> --index N           # 单件物品明细
cargo run -p d2r-cli -- quests <存档>                      # 任务进度
cargo run -p d2r-cli -- items  <存档> --merc              # 佣兵装备段
cargo run -p d2r-cli -- stats  [--filter 关键字] [--all]   # 属性 ID ↔ 中文名对照
cargo run -p d2r-cli -- export-items <存档> [--index N] --out bp.json
cargo run -p d2r-cli -- import-item  <存档> --blueprint bp.json --template N

# 底层工具
cargo run -p d2r-cli --bin grid-probe -- <存档> [--inv 16x8]   # 网格布局校验
cargo run -p d2r-data --bin compare-dirs                       # 三方数据表比对
cargo run -p d2r-core --bin rewrite-save -- <原> <新>           # 读入再写出
cargo run -p d2r-core --bin cmp-bytes -- <原> <新>              # 字节级差异定位
node tools/render-check.mjs                                    # 界面渲染逻辑校验（不开窗口）
```

编辑示例：

```powershell
d2r edit 角色.d2s --set '[{"kind":"setLevel","level":90},
                          {"kind":"setStat","stat":0,"value":300},
                          {"kind":"setSkill","index":0,"level":20},
                          {"kind":"unlockWaypoints","difficulty":0}]' --dry-run
```

保存前会自动"写出 → 重新解析 → 比对角色信息与尾随数据"，通过才落盘，
并给原文件留 `.bak` 备份。

环境变量：`D2R_GAME_DIR`（游戏目录，可选）、`D2R_SAMPLES_DIR`（测试样本目录）、
`D2R_STRINGS_DIR`（字符串表目录，可选）、`D2R_CONFIG_DIR`（配置目录覆盖）。

## 关键实现约定

### 存档写入必须往返一致

非结构性编辑（改值、增删属性）以**写出后逐字节等于原文件**为门槛；
长度会变的编辑改为校验"头部长度 + 校验和 + 重新解析"。

### 属性值就是存档里的值

界面编辑与显示的都是**存档里的原始值**，不做"游戏显示值"换算。
原因是各属性的显示换算规则不统一（`ItemStatCost.txt` 里除
`Save Bits`/`Save Add`/`ValShift` 外还有 `Encode`/`Add`/`Multiply`
参与显示计算，没有可靠公开定义），猜错会直接改坏数据。
界面改为把每一条属性的**有效范围**说明清楚：

```text
最小值 = -(Save Add)
最大值 = (2^Save Bits - 1 - Save Add) × 2^ValShift
步长   = 2^ValShift        （值必须是它的倍数，否则写盘被截断）
```

### 带参数位的属性有两个可改的数

`Save Param Bits > 0` 的属性（共 17 条）同一 ID 会有多条记录，靠"层"区分：
`item_addclassskills` 的层是职业行号、`item_aura` 的层是光环 ID。
界面把它们显示成**值1 / 值2**。

编辑与删除都**必须按 `stat + layer` 精确定位**：

- 只按 `stat` 定位会让带参数属性永远匹配不上 → 每次改动都**新增一条**，
  存档越改越多；
- 删除时不带层会**连带删掉同 ID 的其他层**。

### 固定事实写成常量，不每次扫表

职业行号、光环 ID 这类是游戏的固定事实，直接写成常量
（`crates/app/src/workspace.rs` 的 `ITEM_AURA_SKILL_IDS` 等），
不在每次打开物品时扫 `skills.txt` 去推断——既浪费又容易因判据写错而出错
（曾查了一个不存在的 `aura` 列，过滤形同虚设）。

### 界面渲染要有后台断言

网格渲染与属性行的错误（占格算错、值整列空白）在截图里很难发现，
因此渲染逻辑抽成**无 DOM 依赖的纯函数**，由 `tools/render-check.mjs`
在不开窗口的情况下断言。

## 装备库导入

「从装备库导入」是把数据表里的**暗金与套装**整理成可选列表，选中后从零构造
一件物品写进容器。三处关键设计：

### 只做暗金 / 套装，属性取满值

白装、蓝装、黄装的属性是随机生成的，没有"标准值"；暗金与套装的每条属性
都写在表里，所以能按**满值**复现。

属性不是直接可用的：`uniqueitems.txt` / `setitems.txt` 的 `prop1..N` 是
`properties.txt` 里的**属性代码**（`str`、`dmg%`…），要经 `func / stat / val`
翻译成 `ItemStatCost` 的属性 ID，`val` 作为层（技能页、单一技能等参数位）。
支持 func `1/2/3/7/8/10/11/15/16/17/19/22`；其余（按等级、随机、特殊公式）
跳过——它们没有固定值可取，硬塞一个会写出游戏不认的属性。

`min/max` 为空表示"按等级计算"（如 `hp/lvl` `mana/lvl`），
这类属性用表里的 **`par` 列**换算成存档里的**每级系数**，不去算总值
（总量由游戏按角色等级自己算）：

```
每级量     = par / 2^op_param         该属性 op=2、opParam=3 → ÷8
存档里的值 = 系数 × 2^ValShift          该属性 ValShift=8 → ×256
```

字段只有 6 位（0–63 的整数系数），所以每级量只能是整数；
游戏里 1.5 这类值在存档里无法精确表达，按步长取整。

`min/max` 与 `par` 都为空的（随机或特殊公式）才整条跳过。

### 构建期固化，运行时只解析一次

条目由 `tools/make-library.py` 在构建期整理成 `library/items.tsv`，
随数据包内嵌（与 `excel/` `strings/` `icons/` 同一套机制）。
运行时 `crate::library` 在第一次用到时解析并**按语言缓存**，之后导入多少次
都不会再解析。

物品**显示名在生成期就已本地化**（取字符串表的 `zhCN`），所以运行时不依赖
名字表；产物里不含界面文案。

分组用数据自带的 `type` 列（`armor/weapons/misc` 里是 `tors`/`helm`/`shie`…）
映射成中文类别，**不按物品代码前缀猜**——`ful`（全身铠甲）这类 4 字符代码
按前缀猜会全部落到"其他"。

### 从零构造物品

`ItemEdit::CreateItem` 直接 `d2s::Item::new(code)` 再填品质与属性，
因此能导入存档里原本没有的物品（`CloneItem` 只能复制已有物品）。

两个必须注意的点：

- **`version` 要对齐存档版本**：写盘时按物品自身的 version 决定位宽编码，
  用默认值会写出别的格式，读回来直接报错；
- **暗金/套装的 `file_index`** 是表里的 `*ID`（不是行号），
  指错就会变成别的物品。

放位置由 `Workspace::find_free_slot` 自动挑：按容器尺寸建立占用表，
逐格找第一块能放下的连续空位；放不下时明确报错，不会塞到重叠位置。

## 当前进度

| 里程碑 | 状态 |
|---|---|
| 1 数据集加载器 | 🟢 表解析与缓存、字符串表、物品名/技能名解析 |
| 2 `.d2s` 读入→写出逐字节一致 | 🟢 8 个样本中 7 个逐字节一致（含 `.d2i`）；剩 1 个已知条目段编码差异 |
| 3 容器与物品摆放校验 | 🟢 区域级重叠 / 装备位重复 / 未知物品代码；容器尺寸与佣兵全装备可由用户确认 |
| 4 属性/技能/任务/传送点编辑 | 🟢 等级、8 项属性、技能等级、三难度传送点 |
| 5 物品编辑 | 🟢 明细、改耐久·数量·防御、增删属性、移动格子、删除物品 |
| 5b 物品导入（自定义 JSON） | 🟢 导出/导入（模板克隆 + 属性覆盖）、位宽与移位校验 |
| 6 佣兵装备 | ⬜ 待开始 |
| 7 Tauri 界面整合 | 🟢 容器网格视图 → 物品明细 → 校验 → 保存 |

## 代码约定

- 单文件 ≤ 1000 行、单函数 ≤ 80 行；重复逻辑出现 ≥2 次即抽公共方法
- 状态/类型值用枚举，DAO/API 层不用魔法数字
- 提交信息 `<type>(<scope>): <简述>`，type 限 feat/fix/refactor/docs/test/chore
- DTO 使用 `#[serde(rename_all = "camelCase")]`

## 相关文档

- [待办清单](TODO.md) ← **接着做什么看这份**
- [开发交接（断点续作）](HANDOFF.md) ← **环境、验证办法、已知坑**
- [调研文档](research.md)
- [继续开发所需资料清单](next-steps-materials.md)

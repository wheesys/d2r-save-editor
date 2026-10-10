# 样本目录说明

本目录存放开发/测试用的存档与数据样本。**目录本身已被 `.gitignore` 忽略**（`samples/`），只有本说明文件入库，样本不进版本库。

## 来源

| 来源 | 路径 | 说明 |
|---|---|---|
| 疯兔2 整合包存档备份 | `D:\Games\d2r92777\Diablo II Resurrected Infernal Edition\存档备份\` | 用户本机，D2R 3.2.92777 + fengtu2 mod |
| `Saved Games` 备份压缩包 | `C:\Users\walle\Saved Games\Diablo II Resurrected.rar` | 含角色存档、共享箱、D2RLoader/mod 旁车文件 |
| 旧编辑器项目样本 | `D:\code\diablo_edit\doc\` | 旧项目（wheesys/diablo_edit）的回归测试存档，**副本**，原件仍在原处 |

## 目录结构

```
samples/
├── saves/
│   ├── d2r-105/         # 存档版本 105（D2R 3.1/3.2，含 RotW 结构）
│   └── d2r-legacy/      # 旧版本存档（版本 99，D2R 2.5）
├── sidecar/             # D2RLoader 与 mod 的旁车文件（正常游戏不读，mod 读）
│   └── fengtu2/         # fengtu2 mod 的 .abs（额外储物箱）/ .mercs（佣兵全装备）
├── game-data/           # 2026-10-10 用 CascView 从游戏 CASC 导出的官方数据
│   ├── excel/           #   官方 3.2 原表（91 张 .txt）← 判定 mod 改动的基准
│   └── lng/strings/     #   官方字符串表（21 个 json，13 种语言，9.14 MB）
└── archives/            # 原始备份压缩包（未解开的完整备份）
```

### `game-data/` 说明

从游戏 CASC 导出（CascView，见 `docs/next-steps-materials.md` §3.6），是本项目的**官方基准数据**：

- `excel/`：官方 3.2 原版数据表。**loader 展开的 `base\` 不是原版表**（与官方差 36 张），
  判定 mod 改动必须用这里的表；已实测 loader 的 merged 输出与它逐字节一致（88 张）。
- `lng/strings/`：物品名、词缀名、技能名、界面文案等，`d2r-data` 已能解析（24,179 条）。
  注意官方导出的 JSON **带 UTF-8 BOM**，解析前必须剥掉。

## 样本清单

### `saves/d2r-105/`（存档版本 105）

| 文件 | 大小 | SHA256(前16) | 说明 |
|---|---|---|---|
| `杜老二.d2s` | 2475 | `7FE1AB40918454C0` | 角色存档，有 `.d2rl` + `.abs` 旁车 |
| `杜老五.d2s` | 3080 | `1CD16E6951F64E95` | 角色存档，`.abs` 932 字节（额外储物箱内容多） |
| `杜仲.d2s` | 2686 | `DA82B240C3116E53` | 角色存档，无旁车；旧项目回归样本 |
| `小五.d2s` | 4552 | `9D886AA9D6200D42` | 角色存档，含佣兵旁车 |
| `aruisi.d2s` | 2393 | `E9CD5E26FF47258E` | 角色存档 |
| `blank-testnew.d2s` | 1037 | `51C67F7759B3DED0` | 3.1 新建空白角色（模板候选，旧项目已知 D2Error(6)） |
| `ModernSharedStashSoftCoreV2.d2i` | 6833 | `56EE8B719C593C4C` | 共享储物箱（软核）；头部第 2 个 DWORD=2，**不是** .d2s 的版本字段，需单独确认格式 |

### `saves/d2r-legacy/`

| 文件 | 大小 | SHA256(前16) | 说明 |
|---|---|---|---|
| `saves-v099-2.5.d2s` | 3963 | `F4AD1F1E7311F3E6` | 存档版本 99（D2R 2.5）；旧项目解析报 D2Error(6)（原始代码既有问题） |

### `sidecar/`

| 文件 | 大小 | 说明 |
|---|---|---|
| `杜老二.d2rl` / `杜老五.d2rl` / `小五.d2rl` / `薇薇安.d2rl` | 1230 各 | D2RLoader 1.3.1-beta 旁车，记录游戏版本、存档路径、已启用插件清单 |
| `fengtu2/小五.mercs` | 1418 | 头部 `DMMS`，佣兵全装备 mod 的佣兵数据 |
| `fengtu2/薇薇安.mercs` | 677 | 头部 `DMMS` |
| `fengtu2/*.abs` | 296–7928 | 头部 `ABS1`，额外储物箱（无限箱V2）数据 |

> `.d2rl` / `.mercs` / `.abs` 都是 D2RLoader + mod 的**私有格式**，原版游戏不读，编辑器要支持对应功能必须解析它们（当前无公开格式文档，需逆向或找 loader 作者资料）。

### `archives/`

| 文件 | 大小 | 说明 |
|---|---|---|
| `Diablo II Resurrected.rar` | 228771 | 完整 `Saved Games` 备份（含已解出的样本 + `.ctl/.key/.ma0/.map` 等游戏自身文件） |
| `fengtu2-backup-2026-10-07.rar` | 16461 | 疯兔2 整合包自带的存档备份，含 `.d2s` 与共享箱 |

## 使用约定

- 样本仅本地使用，**不要提交进 git**（`samples/` 已忽略）；如需入库请先脱敏并确认版权。
- 任何写路径测试都先复制样本到临时目录，禁止直接改样本原件。
- 新增样本请在本文件登记大小与哈希。

## 待补充样本（见 `docs/next-steps-materials.md`）

- 共享箱 `ModernSharedStashSoftCoreV2.d2i` 的**改动前后对照**
- 含佣兵装备 / 镶嵌物 / 无形 / 神符之语 / 魔像的**完整状态**角色
- 2.4 / 2.6 / RotW 早期 / 经典 D2（1.13/1.14）存档
- 术士（Warlock）职业的满级角色

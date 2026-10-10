# .tools —— 开发辅助工具

不是产品代码，只用于开发/验证。构建产物（`*.exe`/`*.obj`/`*.dll`/`*.png`）已被 `.gitignore` 忽略。

| 文件 | 用途 |
|---|---|
| `casc_tool.c` | D2R CASC 提取/列举工具（依赖 `CascLib.dll`，同目录）。**当前卡住**：CascLib 未公开的打开标志导致 `CascFindFirstFile`/`CascOpenFile` 返回 `ERROR_FILE_NOT_FOUND`，见 `docs/next-steps-materials.md` §V3 |
| `casc_diag.c` | 逐条探测 CascLib 各导出，定位上面那个问题的诊断程序 |
| `build_casc_tool.cmd` | 用 `D:\VS2022` 的 cl.exe + C 盘 Windows SDK 编译上面两个 C 文件 |
| `ui-drive.ps1` | 驱动 Windows GUI（点击/输入/截图），用于操作 CascView 等没有命令行接口的工具 |
| `screenshot-window.ps1` | 截取指定进程主窗口，便于核对 GUI 状态 |

## 编译 CASC 工具

```powershell
& cmd /c 'D:\code\d2r-save-editor\.tools\build_casc_tool.cmd'
# CascLib.dll 从 D:\diablo2 mod util\D2RExtractor\CascLib.dll 复制到本目录
```

## 结论：走 GUI

CascView（`D:\diablo2 mod util\Cascview\x64\CascView.exe`）已实测能打开本机游戏存储，
导出官方数据的一次性步骤见 `docs/next-steps-materials.md` §3.6。

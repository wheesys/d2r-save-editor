<#
.SYNOPSIS
    打 Windows 发布包：把内嵌了数据包的桌面端 + 使用说明压缩成一个 zip 放到 dist/。

.DESCRIPTION
    数据包（crates/app/data-bundle/，约 11 MB）**不进 git**，而是在打包这一步
    本地生成并**内嵌进 exe**，然后连同文档一起压缩成 zip。
    这样用户拿到 zip 解压即用：除了要改的存档文件，不需要在自己机器上找任何东西
    （不需要装游戏、不需要任何解包工具、不需要 mod 加载器）。

    流程：
      1) 生成/刷新数据包     tools/make-bundle.py
      2) 构建前端            web: npm run build
      3) release 构建        desktop/src-tauri: cargo build --release
      4) 组装并压缩          dist/d2r-save-editor-<版本>-win-x64.zip

    注意：release 构建在缺少数据包时会**直接失败**（见 crates/app/build.rs），
    避免发出一个打开就没数据的包。

.PARAMETER SkipBundle
    跳过数据包生成（数据包已就绪时用）。会明确警告"可能打包了旧数据"。

.PARAMETER KeepBundle
    源数据不在时允许复用现有数据包。默认行为是：源数据在就**重新生成**，
    保证打进 exe 的永远是最新数据；源数据不在则报错（而不是静默用旧包）。

.PARAMETER SkipFrontend
    跳过前端构建（前端未改动时用）。

.PARAMETER Version
    产物名里的版本号；默认取根 Cargo.toml 的 version。

.EXAMPLE
    pwsh -File tools/package.ps1
#>
[CmdletBinding()]
param(
    [switch]$SkipBundle,
    [switch]$KeepBundle,
    [switch]$SkipFrontend,
    [string]$Version,
    [switch]$AlsoZip
)

$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$env:RUSTUP_HOME = if ($env:RUSTUP_HOME) { $env:RUSTUP_HOME } else { 'D:\rust\rustup' }
$env:CARGO_HOME = if ($env:CARGO_HOME) { $env:CARGO_HOME } else { 'D:\rust\cargo' }
$cargo = Join-Path $env:CARGO_HOME 'bin\cargo.exe'
if (-not (Test-Path $cargo)) { $cargo = 'cargo' }

function Step($msg) { Write-Host "`n=== $msg ===" -ForegroundColor Cyan }
function Ok($msg) { Write-Host "  ✓ $msg" -ForegroundColor Green }
function Warn($msg) { Write-Host "  ! $msg" -ForegroundColor Yellow }

# 版本号
if (-not $Version) {
    $toml = Get-Content (Join-Path $repo 'Cargo.toml') -Raw
    if ($toml -match '(?m)^version\s*=\s*"([^"]+)"') { $Version = $Matches[1] } else { $Version = '0.0.0' }
}

$dist = Join-Path $repo 'dist'
New-Item -ItemType Directory -Force -Path $dist | Out-Null

# 1) 本机资源包
#
# 默认策略：源数据在就重新生成，保证打进 exe 的是最新表；
# 源数据不在才考虑复用现有数据包（-KeepBundle），且明确警告。
$bundleMarker = Join-Path $repo 'crates\app\data-bundle\BUNDLE.json'
$srcData = Join-Path $repo 'samples\game-data\excel'
$haveSrc = (Test-Path $srcData) -and ((Get-ChildItem $srcData -Filter *.txt -ErrorAction SilentlyContinue | Measure-Object).Count -gt 0)

if ($haveSrc) {
    Step '生成本机资源包'
    $py = (Get-Command python -ErrorAction SilentlyContinue).Source
    if (-not $py) {
        $cand = 'C:\Users\walle\.dsh\dsh-runtimes\dsh-primary-runtime\dependencies\python\python.exe'
        if (Test-Path $cand) { $py = $cand }
    }
    if (-not $py) {
        if ((Test-Path $bundleMarker) -and -not $KeepBundle) {
            throw "源数据在但找不到 python：无法刷新数据包。请装 python，或显式传 -KeepBundle 复用现有数据包"
        }
        Warn '找不到 python，跳过数据包生成'
    } else {
        & $py (Join-Path $PSScriptRoot 'make-bundle.py')
        if ($LASTEXITCODE -ne 0) { throw '数据包生成失败' }
        Ok '数据包已按本机源数据重新生成'
    }
} elseif (Test-Path $bundleMarker) {
    if ($SkipBundle -or $KeepBundle) {
        Warn '源数据不在，复用现有数据包 —— **可能打包了旧数据**，请确认它的来源与时间'
        $age = (Get-Date) - (Get-Item $bundleMarker).LastWriteTime
        Warn ("现有数据包生成于 {0:yyyy-MM-dd HH:mm}（{1:N1} 小时前）" -f (Get-Item $bundleMarker).LastWriteTime, $age.TotalHours)
    } else {
        throw "源数据不在（$srcData），但现有数据包存在。`n默认不静默复用它（避免发货时用了旧表）。`n确认要复用请显式传 -KeepBundle。"
    }
} else {
    throw "既没有源数据（$srcData），也没有数据包（$bundleMarker）。`n请先运行 tools/fetch-data.ps1 导出数据，再运行 tools/make-bundle.py。"
}

if (-not (Test-Path $bundleMarker)) {
    throw "数据包不存在：$bundleMarker —— release 构建会失败"
}
$bundleMb = [math]::Round(((Get-ChildItem (Split-Path $bundleMarker) -Recurse -File | Measure-Object -Sum Length).Sum / 1MB), 2)
# 装备库：构建期把它整理成一份数据随包内嵌，运行时不解析原始表
    & $py (Join-Path $repo 'tools/make-library.py')
    if ($LASTEXITCODE -ne 0) { throw '装备库生成失败' }
    Ok "装备库就绪（library/items.tsv）"

    Ok "资源包就绪（$bundleMb MB）"

# 2) 前端
if (-not $SkipFrontend) {
    Step '构建前端（Vite → desktop/dist）'
    Push-Location (Join-Path $repo 'web')
    try {
        if (-not (Test-Path 'node_modules')) { npm install --no-audit --no-fund | Out-Null }
        npm run build
        if ($LASTEXITCODE -ne 0) { throw '前端构建失败' }
    } finally { Pop-Location }
    Ok '前端产物已生成'
} else {
    Warn '按参数跳过前端构建'
}

# 3) release 构建
Step 'release 构建桌面端'
Push-Location (Join-Path $repo 'desktop\src-tauri')
try {
    & $cargo build --release
    if ($LASTEXITCODE -ne 0) { throw 'release 构建失败' }
} finally { Pop-Location }
$exe = Join-Path $repo 'desktop\src-tauri\target\release\d2r-desktop.exe'
if (-not (Test-Path $exe)) { throw "找不到产物：$exe" }
$exeMb = [math]::Round((Get-Item $exe).Length / 1MB, 1)
Ok "d2r-desktop.exe（$exeMb MB）"

# 4) 产出单文件 exe（数据与界面资源都在里面，配套一份说明）
Step '产出单文件 exe'
$outName = "D2RSaveEditor-win-x64.exe"
$outExe = Join-Path $dist $outName
Copy-Item $exe $outExe -Force
$outMb = [math]::Round((Get-Item $outExe).Length / 1MB, 1)
Ok "$outName（$outMb MB，单文件）"

$usage = @"
D2R 存档修改器 v$Version（Windows x64 单文件版）
==============================================

怎么用
------
1. 双击 $outName（就这一个文件，不需要安装，也不需要额外的 DLL）
   程序会请求**管理员权限**（存档在受保护目录，没权限改不了），系统弹 UAC 时点「是」
2. 点「打开存档…」，选你的角色存档（.d2s）或共享储物箱（.d2i）
   - 默认位置：%USERPROFILE%\Saved Games\Diablo II Resurrected\
   - 程序会记住上次打开的路径；**不会自动打开存档**（避免手抖改错文件）
3. 在「角色 / 技能 / 任务 / 物品栏 / 佣兵」页签里改
   - 物品栏里**左键拖拽**物品块可以换格子或换容器
   - **右键**点物品块打开属性窗（左键留给拖拽）
   - 属性值改完点别处就生效；带参数的属性有两个值（值1 / 值2）
4. 改完点「保存」。保存前会自动校验，通过才落盘，并给原文件留一份 .bak 备份
   （已有 .bak 时依次用 .2.bak、.3.bak…，不会覆盖你之前的备份）
   不想保存就点「放弃修改」，重新从磁盘读取

不需要装什么
------------
程序已经把运行所需的数据表与文本表内嵌在 exe 里：
- 不需要装《暗黑破坏神2：重制版》
- 不需要装任何解包工具
- 不需要装 mod 加载器
除你要修改的存档文件外，不需要在本机找任何东西。

如果你自己装了 mod（数据表被改过）
----------------------------------
程序启动时会自动扫描游戏目录里的 mod 数据表并覆盖同名表；
找不到也不影响，用的就是内置的原版数据。
背包/箱子/合成箱的尺寸与「佣兵全装备」这类 mod 改动存档里读不出来，
界面会给出推断结果并让你勾选确认。

兼容性
------
- 只支持**当前版本**的存档（105）。旧版本存档（D2R 早期版本、原版 D2）没有测试过，
  不保证能打开，更不保证改了之后游戏能读
- 共享储物箱（.d2i）目前支持查看与保真保存，编辑功能待后续版本
- 已知问题：个别存档重写后与原件差 2 字节（条目段编码差异），程序会在保存前
  的校验里拦住并提示，不会写出损坏的文件

反馈
----
https://github.com/wheesys/d2r-save-editor/issues

提交问题时请附上：版本号、复现步骤、界面上的报错原文、截图；
如果能说明"游戏里实际显示的是什么"会更快定位。

许可与免责
----------
MIT License (c) 2026 wheesys，源码见上方链接。

本程序按"现状"提供，不附带任何担保。改存档有风险，可能导致角色/物品损坏
或无法进入游戏，请务必自己保留备份，后果由使用者自行承担。
"@
$usagePath = Join-Path $dist "使用说明.txt"
Set-Content -Path $usagePath -Value $usage -Encoding UTF8
Ok "使用说明.txt"

if ($AlsoZip) {
    Step '另外打一个 zip（含 exe 与说明）'
    $stage = Join-Path $dist "d2r-save-editor-win-x64"
    if (Test-Path $stage) { Remove-Item $stage -Recurse -Force }
    New-Item -ItemType Directory -Force -Path $stage | Out-Null
    Copy-Item $outExe (Join-Path $stage 'd2r-save-editor.exe')
    Copy-Item $usagePath $stage
    foreach ($doc in 'README.md', 'LICENSE') {
        $src = Join-Path $repo $doc
        if (Test-Path $src) { Copy-Item $src $stage }
    }
    $zip = Join-Path $dist "d2r-save-editor-win-x64.zip"
    if (Test-Path $zip) { Remove-Item $zip -Force }
    Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $zip -CompressionLevel Optimal
    Ok "$([System.IO.Path]::GetFileName($zip))（$([math]::Round((Get-Item $zip).Length/1MB,1)) MB）"
}

Write-Host ''
Write-Host "完成：$outExe" -ForegroundColor Green
Write-Host "单文件 exe（$outMb MB）；配套说明：$usagePath" -ForegroundColor DarkGray
Write-Host ''

<#
.SYNOPSIS
    从本机《暗黑破坏神2：重制版》安装目录导出编辑器需要的数据到 samples/。

.DESCRIPTION
    数据（官方 excel 表、字符串表）是暴雪版权内容，**不入 git**；
    本脚本负责在任何一台装有游戏的机器上把它们重新导出来，配合
    samples/manifest.json 的 sha256 校验，保证换机后环境可复现。

    导出内容：
      samples/game-data/excel/          官方 3.2 原版数据表（判定 mod 改动的基准）
      samples/game-data/lng/strings/    字符串表（物品名/技能名/界面文案，含中文）

    导出方式：调用本机已有的 CascView（Ladik's CASC Viewer）打开游戏目录。
    CascView 没有可靠的命令行导出接口，所以脚本会打开它并打印需要点选的路径；
    也可以传 -Interactive:$false 让它只打印指引。

.PARAMETER GameDir
    游戏安装目录。默认取 $env:D2R_GAME_DIR，再退回常见的本机路径。

.PARAMETER CascView
    CascView.exe 路径。默认在常见的工具目录里搜索。

.PARAMETER Interactive
    是否打开 CascView 图形界面（默认打开）。

.PARAMETER UpdateManifest
    导出完成后是否刷新 samples/manifest.json（默认刷新）。

.EXAMPLE
    pwsh -File tools/fetch-data.ps1

.EXAMPLE
    pwsh -File tools/fetch-data.ps1 -GameDir 'D:\Games\d2r92777\Diablo II Resurrected Infernal Edition'
#>
[CmdletBinding()]
param(
    [string]$GameDir,
    [string]$CascView,
    [bool]$Interactive = $true,
    [bool]$UpdateManifest = $true
)

$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$samples = Join-Path $repo 'samples'
$excelDst = Join-Path $samples 'game-data\excel'
$lngDst = Join-Path $samples 'game-data\lng\strings'

function Find-GameDir {
    param([string]$Explicit)
    if ($Explicit -and (Test-Path $Explicit)) { return (Resolve-Path $Explicit).Path }
    if ($env:D2R_GAME_DIR -and (Test-Path $env:D2R_GAME_DIR)) { return (Resolve-Path $env:D2R_GAME_DIR).Path }
    $candidates = @(
        'D:\Games\d2r92777\Diablo II Resurrected Infernal Edition',
        'D:\online battle\Diablo II Resurrected',
        'C:\Program Files (x86)\Diablo II Resurrected',
        'C:\Program Files\Diablo II Resurrected'
    )
    foreach ($c in $candidates) { if (Test-Path $c) { return (Resolve-Path $c).Path } }
    # 再扫一层常见根目录
    foreach ($root in 'C:\', 'D:\', 'E:\') {
        $hit = Get-ChildItem -Path $root -Directory -Depth 2 -ErrorAction SilentlyContinue -Filter '*Diablo*' |
            Select-Object -First 1
        if ($hit) { return $hit.FullName }
    }
    return $null
}

function Find-CascView {
    param([string]$Explicit)
    if ($Explicit -and (Test-Path $Explicit)) { return (Resolve-Path $Explicit).Path }
    $candidates = @(
        'D:\diablo2 mod util\Cascview\x64\CascView.exe',
        'D:\diablo2 mod util\Cascview\Win32\CascView.exe',
        'C:\Program Files\CascView\CascView.exe'
    )
    foreach ($c in $candidates) { if (Test-Path $c) { return $c } }
    $hit = Get-ChildItem -Path 'C:\', 'D:\' -Recurse -Depth 4 -Filter 'CascView*.exe' -ErrorAction SilentlyContinue |
        Select-Object -First 1
    if ($hit) { return $hit.FullName }
    return $null
}

Write-Host '=== D2R 编辑器 · 数据导出 ===' -ForegroundColor Cyan

$game = Find-GameDir -Explicit $GameDir
if (-not $game) {
    Write-Host '[x] 没找到游戏安装目录。请用 -GameDir 指定，或设置环境变量 D2R_GAME_DIR。' -ForegroundColor Red
    exit 2
}
Write-Host "游戏目录 : $game"
if (-not (Test-Path (Join-Path $game 'Data\data'))) {
    Write-Host '[!] 该目录下没有 Data\data（CASC 存储），确认这是 D2R 安装目录。' -ForegroundColor Yellow
}

$cv = Find-CascView -Explicit $CascView
if (-not $cv) {
    Write-Host '[x] 没找到 CascView.exe。请用 -CascView 指定（可从 Ladik''s CASC Viewer 获取）。' -ForegroundColor Red
    exit 2
}
Write-Host "CascView : $cv"

New-Item -ItemType Directory -Force -Path $excelDst, $lngDst | Out-Null

# CascView 的工作目录会生成 <Work>\data\data\... 的嵌套结构，导出后需要搬平
$work = Join-Path (Split-Path $cv) 'Work'
$workExcel = Join-Path $work 'data\data\global\excel'
$workLng = Join-Path $work 'data\data\local\lng\strings'

function Install-FromWork {
    param([string]$Src, [string]$Dst, [string]$Label)
    if (-not (Test-Path $Src)) {
        Write-Host "  [$Label] 还没看到导出结果：$Src" -ForegroundColor Yellow
        return $false
    }
    $files = Get-ChildItem $Src -Recurse -File -ErrorAction SilentlyContinue
    if ($files.Count -eq 0) {
        Write-Host "  [$Label] 导出目录为空" -ForegroundColor Yellow
        return $false
    }
    robocopy $Src $Dst /E /NFL /NDL /NJH /NJS /NP | Out-Null
    $n = (Get-ChildItem $Dst -Recurse -File).Count
    Write-Host "  [$Label] 已归档 $n 个文件 → $Dst" -ForegroundColor Green
    return $true
}

if ($Interactive) {
    Write-Host ''
    Write-Host '即将打开 CascView。请在界面里做两步导出：' -ForegroundColor Cyan
    Write-Host '  1) 展开 data → data → local → lng → 选中 strings → Ctrl+A → 点工具栏 Extract'
    Write-Host "     目标目录： $lngDst"
    Write-Host '  2) 展开 data → data → global → 选中 excel → Ctrl+A → 点工具栏 Extract'
    Write-Host "     目标目录： $excelDst"
    Write-Host ''
    Write-Host '（如果它把文件放进了自己的 Work 目录，脚本会自动搬平过来）' -ForegroundColor DarkGray
    Start-Process -FilePath $cv -ArgumentList "`"$game`""
    Write-Host ''
    Write-Host '导出完成后按回车继续…' -ForegroundColor Cyan
    [void](Read-Host)
    # 先看目标目录；没有再从 Work 搬
    $okExcel = (Get-ChildItem $excelDst -Recurse -File -ErrorAction SilentlyContinue | Measure-Object).Count -gt 0
    $okLng = (Get-ChildItem $lngDst -Recurse -File -ErrorAction SilentlyContinue | Measure-Object).Count -gt 0
    if (-not $okExcel) { [void](Install-FromWork -Src $workExcel -Dst $excelDst -Label 'excel') }
    if (-not $okLng) { [void](Install-FromWork -Src $workLng -Dst $lngDst -Label 'strings') }
} else {
    Write-Host ''
    Write-Host '（-Interactive:$false）请在 CascView 里按下列路径手工导出：' -ForegroundColor Cyan
    Write-Host "  游戏目录   : $game"
    Write-Host "  excel      : data\data\global\excel        → $excelDst"
    Write-Host "  strings    : data\data\local\lng\strings   → $lngDst"
    if (Test-Path $workExcel) { [void](Install-FromWork -Src $workExcel -Dst $excelDst -Label 'excel(Work)') }
    if (Test-Path $workLng) { [void](Install-FromWork -Src $workLng -Dst $lngDst -Label 'strings(Work)') }
}

# 汇总 + 校验
Write-Host ''
foreach ($pair in @(@('excel', $excelDst), @('strings', $lngDst))) {
    $f = Get-ChildItem $pair[1] -Recurse -File -ErrorAction SilentlyContinue
    $mb = [math]::Round((($f | Measure-Object -Sum Length).Sum) / 1MB, 2)
    Write-Host ("{0,-9} {1,4} 个文件  {2,7} MB" -f $pair[0], $f.Count, $mb)
}

if ($UpdateManifest) {
    $py = $null
    foreach ($cand in @('python', 'python3')) {
        $cmd = Get-Command $cand -ErrorAction SilentlyContinue
        if ($cmd) { $py = $cmd.Source; break }
    }
    if ($py) {
        Write-Host ''
        Write-Host '刷新数据清单（samples/manifest.json）…' -ForegroundColor Cyan
        & $py (Join-Path $PSScriptRoot 'make-manifest.py')
    } else {
        Write-Host '[!] 没找到 python，跳过清单刷新。可稍后手工运行：python tools/make-manifest.py' -ForegroundColor Yellow
    }
}

Write-Host ''
Write-Host '完成。数据在 samples/ 下（.gitignore 已排除，不会进 git）。' -ForegroundColor Green
Write-Host '校验命令：python tools/make-manifest.py --check' -ForegroundColor DarkGray

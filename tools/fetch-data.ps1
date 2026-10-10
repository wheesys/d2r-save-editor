<#
.SYNOPSIS
    从本机《暗黑破坏神2：重制版》安装目录导出编辑器需要的数据到 samples/。

.DESCRIPTION
    本脚本把程序运行需要的本地数据准备到 samples/game-data/ 下。
    该目录**不入 git**，也不需要随仓库分发：数据只在本机存在，
    构建时会被内嵌进可执行文件（见 tools/make-bundle.py）。

    导出内容（都落在 samples/ 下，已被 .gitignore 排除）：
      samples/game-data/excel/          数据表
      samples/game-data/lng/strings/    文本表

    准备方式：用本机已有的资源查看工具打开游戏目录并选中需要的那两层导出。
    这类工具没有可靠的命令行导出接口，所以脚本会打开它并打印需要点选的路径；
    也可以传 -Interactive:$false 让它只打印指引。

    请注意：不要在文档、注释或提交信息里记录这些数据的来源与清单。

.PARAMETER GameDir
    游戏安装目录。默认取 $env:D2R_GAME_DIR，再退回常见的本机路径。

.PARAMETER Viewer
    资源查看工具的可执行文件路径。默认在常见位置搜索。

.PARAMETER Interactive
    是否打开资源查看工具的图形界面（默认打开）。

.PARAMETER Stats
    导出完成后是否刷新本机统计（samples/local-stats.json，默认刷新）。

.EXAMPLE
    pwsh -File tools/fetch-data.ps1

.EXAMPLE
    pwsh -File tools/fetch-data.ps1 -GameDir 'D:\Games\d2r92777\Diablo II Resurrected Infernal Edition'
#>
[CmdletBinding()]
param(
    [string]$GameDir,
    [string]$Viewer,
    [bool]$Interactive = $true,
    [bool]$Stats = $true
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

function Find-Viewer {
    param([string]$Explicit)
    if ($Explicit -and (Test-Path $Explicit)) { return (Resolve-Path $Explicit).Path }
    foreach ($name in 'CascView.exe', 'D2RExtractor.exe') {  # 常见工具名，可按需增补
        $hit = Get-ChildItem -Path 'C:\', 'D:\' -Recurse -Depth 4 -Filter $name -ErrorAction SilentlyContinue |
            Select-Object -First 1
        if ($hit) { return $hit.FullName }
    }
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
    Write-Host '[!] 该目录下没有 Data\data（游戏数据存储），确认这是 D2R 安装目录。' -ForegroundColor Yellow
}

$cv = Find-Viewer -Explicit $Viewer
if (-not $cv) {
    Write-Host '[x] 没找到资源查看工具。请用 -Viewer 指定可执行文件路径。' -ForegroundColor Red
    exit 2
}
Write-Host "查看工具 : $cv"

New-Item -ItemType Directory -Force -Path $excelDst, $lngDst | Out-Null

# 这类工具的工作目录会生成嵌套结构，导出后需要搬平
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
    Write-Host '即将打开查看工具。请在界面里做两步导出：' -ForegroundColor Cyan
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
    Write-Host '（-Interactive:$false）请在查看工具里按下列路径手工导出：' -ForegroundColor Cyan
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

if ($Stats) {
    $py = $null
    foreach ($cand in @('python', 'python3')) {
        $cmd = Get-Command $cand -ErrorAction SilentlyContinue
        if ($cmd) { $py = $cmd.Source; break }
    }
    if ($py) {
        Write-Host ''
        Write-Host '刷新本机统计（samples/local-stats.json，不入库）…' -ForegroundColor Cyan
        & $py (Join-Path $PSScriptRoot 'local-stats.py')
    } else {
        Write-Host '[!] 没找到 python，跳过统计。可稍后手工运行：python tools/local-stats.py' -ForegroundColor Yellow
    }
}

Write-Host ''
Write-Host '完成。数据在 samples/ 下（.gitignore 已排除，不会进 git，也不会随仓库分发）。' -ForegroundColor Green
Write-Host '统计命令：python tools/local-stats.py' -ForegroundColor DarkGray

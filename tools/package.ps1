<#
.SYNOPSIS
    打 Windows 发布包：把内嵌了数据包的桌面端 + 使用说明压缩成一个 zip 放到 dist/。

.DESCRIPTION
    数据包（crates/app/data-bundle/，约 11 MB）**不进 git**，而是在打包这一步
    本地生成并**内嵌进 exe**，然后连同文档一起压缩成 zip。
    这样用户拿到 zip 解压即用：除了要改的存档文件，不需要在自己机器上找任何东西
    （不需要 D2R、不需要 CascView、不需要 D2RLoader）。

    流程：
      1) 生成/刷新数据包     tools/make-bundle.py
      2) 构建前端            web: npm run build
      3) release 构建        desktop/src-tauri: cargo build --release
      4) 组装并压缩          dist/d2r-save-editor-<版本>-win-x64.zip

    注意：release 构建在缺少数据包时会**直接失败**（见 crates/app/build.rs），
    避免发出一个打开就没数据的包。

.PARAMETER SkipBundle
    跳过数据包生成（数据包已就绪时用）。

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
    [switch]$SkipFrontend,
    [string]$Version
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

# 1) 数据包
if (-not $SkipBundle) {
    Step '生成数据包（内嵌进 exe）'
    $py = (Get-Command python -ErrorAction SilentlyContinue).Source
    if (-not $py) {
        # 退回 bundled python
        $cand = 'C:\Users\walle\.dsh\dsh-runtimes\dsh-primary-runtime\dependencies\python\python.exe'
        if (Test-Path $cand) { $py = $cand }
    }
    if (-not $py) { throw '找不到 python，无法生成数据包（或用 -SkipBundle）' }
    & $py (Join-Path $PSScriptRoot 'make-bundle.py')
    if ($LASTEXITCODE -ne 0) { throw '数据包生成失败' }
    Ok '数据包就绪'
} else {
    Warn '按参数跳过数据包生成'
}
$bundleMarker = Join-Path $repo 'crates\app\data-bundle\BUNDLE.json'
if (-not (Test-Path $bundleMarker)) {
    throw "数据包不存在：$bundleMarker —— release 构建会失败，请先运行 tools/make-bundle.py"
}

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
Ok "d2r-desktop.exe（$exeMb MB，数据已内嵌）"

# 4) 组装
Step '组装发布目录'
$name = "d2r-save-editor-$Version-win-x64"
$stage = Join-Path $dist $name
if (Test-Path $stage) { Remove-Item $stage -Recurse -Force }
New-Item -ItemType Directory -Force -Path $stage | Out-Null
Copy-Item $exe (Join-Path $stage 'd2r-save-editor.exe')
foreach ($doc in 'README.md', 'LICENSE') {
    $src = Join-Path $repo $doc
    if (Test-Path $src) { Copy-Item $src $stage }
}
$usage = @"
D2R 存档修改器 v$Version（Windows x64）
=====================================

怎么用
------
1. 双击 d2r-save-editor.exe
2. 点「打开存档…」，选你的角色存档（.d2s）或共享储物箱（.d2i）
   - 默认位置：%USERPROFILE%\Saved Games\Diablo II Resurrected\
3. 改完点「保存」。保存前会自动校验，通过才落盘，并给原文件留一份 .bak 备份

不需要装什么
------------
本程序已内嵌官方数据表与字符串表（约 11 MB）：
- 不需要装《暗黑破坏神2：重制版》
- 不需要装 CascView 或其他解包工具
- 不需要装 mod 加载器（D2RLoader）
除你要修改的存档文件外，不需要在本机找任何东西。

如果你自己装了 mod（数据表被改过）
----------------------------------
程序启动时会自动扫描游戏目录里的 mod 数据表并覆盖同名表；
找不到也不影响，用的就是内置的原版数据。

兼容性
------
- 存档版本 105（D2R 3.x / 术士之治）与 99（D2R 2.4–2.8）已验证逐字节往返一致
- 共享储物箱（.d2i）目前支持查看与保真保存，编辑功能待后续版本
- 已知问题：个别存档重写后与原件差 2 字节（条目段编码差异），程序会在保存前
  的校验里拦住并提示，不会写出损坏的文件

反馈
----
https://github.com/wheesys/d2r-save-editor
"@
Set-Content -Path (Join-Path $stage '使用说明.txt') -Value $usage -Encoding UTF8
Ok "已组装 $name"

# 5) 压缩
Step '压缩'
$zip = Join-Path $dist "$name.zip"
if (Test-Path $zip) { Remove-Item $zip -Force }
Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $zip -CompressionLevel Optimal
$zipMb = [math]::Round((Get-Item $zip).Length / 1MB, 1)
Ok "$zip（$zipMb MB）"

Write-Host ''
Write-Host "完成：$zip" -ForegroundColor Green
Write-Host "内含：d2r-save-editor.exe（$exeMb MB，数据已内嵌）、使用说明.txt、README.md、LICENSE" -ForegroundColor DarkGray

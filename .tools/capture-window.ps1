param(
    [string]$ProcessName = 'd2r-desktop',
    [string]$Out = 'shot.png',
    [string]$TitleContains = ''
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class WinCap {
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hWnd, out RECT r);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hWnd, int cmd);
}
"@

$candidates = Get-Process -Name $ProcessName -ErrorAction SilentlyContinue |
    Where-Object { $_.MainWindowHandle -ne 0 }
if ($TitleContains) {
    $candidates = $candidates | Where-Object { $_.MainWindowTitle -like "*$TitleContains*" }
}
$proc = $candidates | Select-Object -First 1
if (-not $proc) { throw "找不到有窗口的进程: $ProcessName" }

[WinCap]::ShowWindow($proc.MainWindowHandle, 9) | Out-Null   # SW_RESTORE
[WinCap]::SetForegroundWindow($proc.MainWindowHandle) | Out-Null
Start-Sleep -Milliseconds 900

$r = New-Object WinCap+RECT
[WinCap]::GetWindowRect($proc.MainWindowHandle, [ref]$r) | Out-Null
$w = $r.Right - $r.Left
$h = $r.Bottom - $r.Top
Write-Output "窗口 pid=$($proc.Id) 标题='$($proc.MainWindowTitle)' 位置=($($r.Left),$($r.Top)) 尺寸=${w}x${h}"

# 再置前一次并确认前台窗口就是它，避免抓到别的窗口
[WinCap]::SetForegroundWindow($proc.MainWindowHandle) | Out-Null
Start-Sleep -Milliseconds 400

$bmp = New-Object System.Drawing.Bitmap $w, $h
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen($r.Left, $r.Top, 0, 0, (New-Object System.Drawing.Size $w, $h))
$bmp.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose(); $bmp.Dispose()
Write-Output "已保存 $Out"

# 提示：本脚本只截窗口，不负责启动应用。启动 debug 版时请显式隐藏控制台，
# 否则会多出一个 ConsoleWindowClass 窗口（debug 构建带控制台子系统）：
#   Start-Process -FilePath .\d2r-desktop.exe -WindowStyle Hidden
param(
  [Parameter(Mandatory=$true)][string]$Out,
  [string]$ProcessName = 'CascView'
)
Add-Type -AssemblyName System.Drawing
$cs = @'
using System;
using System.Runtime.InteropServices;
public static class Win {
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int cmd);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
}
'@
Add-Type -TypeDefinition $cs -ErrorAction SilentlyContinue
$p = Get-Process -Name $ProcessName -ErrorAction SilentlyContinue | Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1
if (-not $p) { Write-Error "no window for process '$ProcessName'"; exit 1 }
[void][Win]::ShowWindow($p.MainWindowHandle, 9)   # SW_RESTORE
[void][Win]::SetForegroundWindow($p.MainWindowHandle)
Start-Sleep -Milliseconds 800
$r = New-Object Win+RECT
[void][Win]::GetWindowRect($p.MainWindowHandle, [ref]$r)
$w = $r.Right - $r.Left; $h = $r.Bottom - $r.Top
Write-Output "window pid=$($p.Id) rect=($($r.Left),$($r.Top))-($($r.Right),$($r.Bottom)) size=${w}x${h} title='$($p.MainWindowTitle)'"
$bmp = New-Object System.Drawing.Bitmap($w, $h)
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen($r.Left, $r.Top, 0, 0, (New-Object System.Drawing.Size($w, $h)))
$g.Dispose()
$bmp.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
$bmp.Dispose()
Write-Output "saved: $Out ($((Get-Item $Out).Length) bytes)"

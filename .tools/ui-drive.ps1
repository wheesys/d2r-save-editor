param(
  [Parameter(Mandatory=$true)][string]$Action,   # click | dblclick | type | key | shot
  [int]$X = 0,
  [int]$Y = 0,
  [string]$Text = '',
  [string]$Out = '',
  [string]$ProcessName = 'CascView',
  [int]$Wait = 800
)
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms
$cs = @'
using System;
using System.Runtime.InteropServices;
public static class Ui {
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint dx, uint dy, uint d, IntPtr e);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int cmd);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
  public const uint LEFTDOWN = 0x0002, LEFTUP = 0x0004;
  public static void Click(int x, int y, bool dbl) {
    SetCursorPos(x, y);
    System.Threading.Thread.Sleep(150);
    mouse_event(LEFTDOWN, 0, 0, 0, IntPtr.Zero);
    mouse_event(LEFTUP, 0, 0, 0, IntPtr.Zero);
    if (dbl) { System.Threading.Thread.Sleep(120); mouse_event(LEFTDOWN,0,0,0,IntPtr.Zero); mouse_event(LEFTUP,0,0,0,IntPtr.Zero); }
  }
}
'@
Add-Type -TypeDefinition $cs -ErrorAction SilentlyContinue

function Get-Win {
  $p = Get-Process -Name $ProcessName -ErrorAction SilentlyContinue | Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1
  if (-not $p) { throw "no window for $ProcessName" }
  [void][Ui]::ShowWindow($p.MainWindowHandle, 9)
  [void][Ui]::SetForegroundWindow($p.MainWindowHandle)
  Start-Sleep -Milliseconds 400
  return $p
}
function Save-Shot([string]$path) {
  $p = Get-Win
  $r = New-Object Ui+RECT
  [void][Ui]::GetWindowRect($p.MainWindowHandle, [ref]$r)
  $w = $r.Right - $r.Left; $h = $r.Bottom - $r.Top
  $bmp = New-Object System.Drawing.Bitmap($w, $h)
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.CopyFromScreen($r.Left, $r.Top, 0, 0, (New-Object System.Drawing.Size($w, $h)))
  $g.Dispose()
  $bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
  $bmp.Dispose()
  Write-Output "shot: $path"
}

$p = Get-Win
$rect = New-Object Ui+RECT
[void][Ui]::GetWindowRect($p.MainWindowHandle, [ref]$rect)
Write-Output "window origin=($($rect.Left),$($rect.Top))"

switch ($Action) {
  'click'    { [Ui]::Click($rect.Left + $X, $rect.Top + $Y, $false); Write-Output "clicked rel=($X,$Y) abs=($($rect.Left+$X),$($rect.Top+$Y))" }
  'dblclick' { [Ui]::Click($rect.Left + $X, $rect.Top + $Y, $true);  Write-Output "dblclicked rel=($X,$Y)" }
  'type'     { [System.Windows.Forms.SendKeys]::SendWait($Text); Write-Output "typed: $Text" }
  'key'      { [System.Windows.Forms.SendKeys]::SendWait($Text); Write-Output "key: $Text" }
  'shot'     { }
  default    { throw "unknown action $Action" }
}
Start-Sleep -Milliseconds $Wait
if ($Out) { Save-Shot $Out }

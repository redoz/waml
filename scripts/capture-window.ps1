# Capture a running window at its native pixel size via PrintWindow.
#
# Unlike a full-desktop screenshot, PrintWindow(hwnd, hdc, 2) grabs the window's
# own client buffer -- correct on HiDPI (no downscale that hides hairlines/faint
# tints) and immune to occluding windows / unreliable SetForegroundWindow.
#
# The C# below is P/Invoke ONLY (no System.Drawing types) so it compiles under
# PowerShell 7+ (pwsh), where System.Drawing.Common is not referenced by the
# Add-Type compiler. The bitmap work runs in PowerShell against the assembly
# loaded by Add-Type -AssemblyName below -- works under pwsh 7 and Windows PS 5.1.
#
# Usage:
#   pwsh -File scripts/capture-window.ps1 -Out shot.png [-Process waml-editor]
#   pwsh -File scripts/capture-window.ps1 -Out shot.png -ProcessId 1234
#
# Pass -ProcessId when several editors are open (a dev session of your own
# alongside the one under test) -- by-name picks whichever comes back first.
param(
    [Parameter(Mandatory = $true)][string]$Out,
    [string]$Process = "waml-editor",
    [int]$ProcessId = 0
)

Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class WinCap {
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr hdc, uint f);
  [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out RECT r);
  public struct RECT { public int L, T, R, B; }
}
"@

$p = if ($ProcessId) { Get-Process -Id $ProcessId -ErrorAction SilentlyContinue }
     else { Get-Process $Process -ErrorAction SilentlyContinue }
$p = $p | Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1
if (-not $p) { Write-Error "no window found for process '$Process'"; exit 1 }

$hwnd = $p.MainWindowHandle
$r = New-Object WinCap+RECT
[WinCap]::GetClientRect($hwnd, [ref]$r) | Out-Null
$w = $r.R - $r.L; $h = $r.B - $r.T
if ($w -le 0 -or $h -le 0) { Write-Error "window has zero client area (minimized?)"; exit 1 }

$bmp = New-Object System.Drawing.Bitmap $w, $h, ([System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
$g = [System.Drawing.Graphics]::FromImage($bmp)
$hdc = $g.GetHdc()
[WinCap]::PrintWindow($hwnd, $hdc, 2) | Out-Null
$g.ReleaseHdc($hdc)
$bmp.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
Write-Output ("captured $Process pid=$($p.Id) -> $Out")

# Capture a running window at its native pixel size via PrintWindow.
#
# Unlike a full-desktop screenshot, PrintWindow(hwnd, hdc, 2) grabs the window's
# own client buffer -- correct on HiDPI (no downscale that hides hairlines/faint
# tints) and immune to occluding windows / unreliable SetForegroundWindow.
#
# Usage:
#   pwsh -File scripts/capture-window.ps1 -Out shot.png [-Process waml-editor]
param(
    [Parameter(Mandatory = $true)][string]$Out,
    [string]$Process = "waml-editor"
)

Add-Type @"
using System;
using System.Runtime.InteropServices;
using System.Drawing;
using System.Drawing.Imaging;
public class WinCap {
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr hdc, uint f);
  [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out RECT r);
  public struct RECT { public int L, T, R, B; }
  public static bool Grab(IntPtr hwnd, string path) {
    RECT r; GetClientRect(hwnd, out r);
    int w = r.R - r.L, h = r.B - r.T;
    if (w <= 0 || h <= 0) { return false; }
    var bmp = new Bitmap(w, h, PixelFormat.Format32bppArgb);
    var g = Graphics.FromImage(bmp);
    IntPtr hdc = g.GetHdc();
    PrintWindow(hwnd, hdc, 2);
    g.ReleaseHdc(hdc);
    bmp.Save(path, ImageFormat.Png);
    return true;
  }
}
"@ -ReferencedAssemblies System.Drawing

$p = Get-Process $Process -ErrorAction SilentlyContinue |
    Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1
if (-not $p) { Write-Error "no window found for process '$Process'"; exit 1 }
if (-not [WinCap]::Grab($p.MainWindowHandle, $Out)) {
    Write-Error "window has zero client area (minimized?)"; exit 1
}
Write-Output ("captured $Process pid=$($p.Id) -> $Out")

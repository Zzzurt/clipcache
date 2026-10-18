# Captures the ClipCache window for README screenshots.
#
# Two facts learned the hard way:
#   1. WebView2 ignores posted (PostMessage) mouse/key messages, so interaction
#      has to go through a real foreground click.
#   2. Screen-copying a window that merely looks on top still catches whatever
#      is actually above it, so the final grab uses PrintWindow(PW_RENDERFULLCONTENT),
#      which renders the window's own surface regardless of z-order.
#
# Usage: powershell -File grab.ps1 [-Row N] [-Modal] [-Wait ms] [-Out name.png]
param(
  [int]$Row = 0,
  [switch]$Modal,
  [int]$Wait = 0,
  [string]$Out = "shots\real-app.png"
)
$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class W {
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr hdc, uint flags);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("dwmapi.dll")] public static extern int DwmGetWindowAttribute(IntPtr h, int a, out RECT r, int s);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int c);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint dx, uint dy, uint d, int e);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
}
"@
[W]::SetProcessDPIAware() | Out-Null

$proc = Get-Process clipcache -ErrorAction SilentlyContinue | Select-Object -First 1
if (-not $proc) { throw "clipcache is not running" }
$h = $proc.MainWindowHandle

function Get-Frame {
  $r = New-Object W+RECT
  # EXTENDED_FRAME_BOUNDS excludes the invisible resize/shadow margin that
  # GetWindowRect includes, which would add a black ring to the PNG.
  if ([W]::DwmGetWindowAttribute($h, 9, [ref]$r, 16) -ne 0) {
    [W]::GetWindowRect($h, [ref]$r) | Out-Null
  }
  return $r
}

function Click-At([int]$cx, [int]$cy) {
  [W]::SetCursorPos($cx, $cy) | Out-Null
  Start-Sleep -Milliseconds 200
  [W]::mouse_event(0x0002, 0, 0, 0, 0) | Out-Null   # LEFTDOWN
  [W]::mouse_event(0x0004, 0, 0, 0, 0) | Out-Null   # LEFTUP
  Start-Sleep -Milliseconds 700
}

[W]::ShowWindow($h, 9) | Out-Null    # SW_RESTORE
[W]::SetForegroundWindow($h) | Out-Null
Start-Sleep -Milliseconds 800

$r0 = Get-Frame

# Rows are 57px tall under a 56px toolbar and 4px list padding; x=150 lands in
# the text column, clear of the hover-only delete button at the row's right edge.
if ($Row -gt 0) {
  Click-At ($r0.L + 150) ($r0.T + 56 + 4 + ($Row - 1) * 57 + 28)
}
if ($Modal) {
  Click-At ($r0.R - 112) ($r0.T + 27)   # settings gear, left of the window controls
}
if ($Wait -gt 0) { Start-Sleep -Milliseconds $Wait }

$r = Get-Frame
$w = $r.R - $r.L
$hgt = $r.B - $r.T
$bmp = New-Object System.Drawing.Bitmap($w, $hgt)
$g = [System.Drawing.Graphics]::FromImage($bmp)
$hdc = $g.GetHdc()
$ok = [W]::PrintWindow($h, $hdc, 2)      # 0x2 = PW_RENDERFULLCONTENT
$g.ReleaseHdc($hdc)
$g.Dispose()
if (-not $ok) { throw "PrintWindow failed" }

$path = Join-Path $PSScriptRoot $Out
$bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
$bmp.Dispose()
Write-Host "saved $path (${w}x${hgt})"

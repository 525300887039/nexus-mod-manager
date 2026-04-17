param(
    [string]$ExePath = (Join-Path $PSScriptRoot '..\src-tauri\target\release\nexus-mod-manager.exe'),
    [string]$OutputDir = (Join-Path $PSScriptRoot '..\docs'),
    [int]$StartupDelayMs = 7000,
    [switch]$LeaveAppOpen,
    [switch]$BuildIfMissing
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms

Add-Type @"
using System;
using System.Runtime.InteropServices;

public static class WindowCaptureNative
{
    [StructLayout(LayoutKind.Sequential)]
    public struct RECT
    {
        public int Left;
        public int Top;
        public int Right;
        public int Bottom;
    }

    [DllImport("user32.dll")]
    public static extern bool GetWindowRect(IntPtr hWnd, out RECT rect);

    [DllImport("user32.dll")]
    public static extern bool SetForegroundWindow(IntPtr hWnd);

    [DllImport("user32.dll")]
    public static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);

    [DllImport("user32.dll")]
    public static extern bool SetCursorPos(int x, int y);

    [DllImport("user32.dll")]
    public static extern void mouse_event(uint dwFlags, uint dx, uint dy, uint dwData, UIntPtr dwExtraInfo);
}
"@

$MouseLeftDown = 0x0002
$MouseLeftUp = 0x0004
$ShowWindowRestore = 9

function Resolve-FullPath {
    param([Parameter(Mandatory = $true)][string]$Path)

    $resolved = Resolve-Path -LiteralPath $Path -ErrorAction SilentlyContinue
    if ($resolved) {
        return $resolved.Path
    }

    return [System.IO.Path]::GetFullPath($Path)
}

function Wait-AppWindow {
    param(
        [Parameter(Mandatory = $true)][int]$ProcessId,
        [int]$TimeoutSeconds = 45
    )

    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    do {
        $process = Get-Process -Id $ProcessId -ErrorAction SilentlyContinue
        if ($process -and $process.MainWindowHandle -ne 0) {
            return $process
        }
        Start-Sleep -Milliseconds 500
    } while ((Get-Date) -lt $deadline)

    throw "Timed out waiting for application window from process $ProcessId."
}

function Get-WindowRect {
    param([Parameter(Mandatory = $true)][System.IntPtr]$Handle)

    $rect = New-Object WindowCaptureNative+RECT
    if (-not [WindowCaptureNative]::GetWindowRect($Handle, [ref]$rect)) {
        throw "GetWindowRect failed for handle $Handle."
    }

    return [pscustomobject]@{
        Left = $rect.Left
        Top = $rect.Top
        Right = $rect.Right
        Bottom = $rect.Bottom
        Width = $rect.Right - $rect.Left
        Height = $rect.Bottom - $rect.Top
    }
}

function Activate-Window {
    param([Parameter(Mandatory = $true)][System.IntPtr]$Handle)

    [WindowCaptureNative]::ShowWindow($Handle, $ShowWindowRestore) | Out-Null
    Start-Sleep -Milliseconds 200
    [WindowCaptureNative]::SetForegroundWindow($Handle) | Out-Null
    Start-Sleep -Milliseconds 350
}

function Move-CursorOutsideWindow {
    param([Parameter(Mandatory = $true)]$Rect)

    $screen = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
    $x = if ($Rect.Right + 10 -lt $screen.Right) { $Rect.Right + 10 } else { [Math]::Max(0, $Rect.Left - 10) }
    $y = if ($Rect.Bottom + 10 -lt $screen.Bottom) { $Rect.Bottom + 10 } else { [Math]::Max(0, $Rect.Top - 10) }
    [WindowCaptureNative]::SetCursorPos($x, $y) | Out-Null
    Start-Sleep -Milliseconds 150
}

function Get-ContentBoundsFromBitmap {
    param([Parameter(Mandatory = $true)][System.Drawing.Bitmap]$Bitmap)

    $background = $Bitmap.GetPixel(0, 0)
    $tolerance = 18
    $minimumHits = 40
    $left = 0
    $right = $Bitmap.Width - 1
    $top = 0
    $bottom = $Bitmap.Height - 1

    for ($x = 0; $x -lt $Bitmap.Width; $x++) {
        $hits = 0
        for ($y = 0; $y -lt $Bitmap.Height; $y++) {
            $pixel = $Bitmap.GetPixel($x, $y)
            if (([Math]::Abs($pixel.R - $background.R) + [Math]::Abs($pixel.G - $background.G) + [Math]::Abs($pixel.B - $background.B)) -gt $tolerance) {
                $hits++
            }
        }
        if ($hits -ge $minimumHits) {
            $left = $x
            break
        }
    }

    for ($x = $Bitmap.Width - 1; $x -ge 0; $x--) {
        $hits = 0
        for ($y = 0; $y -lt $Bitmap.Height; $y++) {
            $pixel = $Bitmap.GetPixel($x, $y)
            if (([Math]::Abs($pixel.R - $background.R) + [Math]::Abs($pixel.G - $background.G) + [Math]::Abs($pixel.B - $background.B)) -gt $tolerance) {
                $hits++
            }
        }
        if ($hits -ge $minimumHits) {
            $right = $x
            break
        }
    }

    for ($y = 0; $y -lt $Bitmap.Height; $y++) {
        $hits = 0
        for ($x = 0; $x -lt $Bitmap.Width; $x++) {
            $pixel = $Bitmap.GetPixel($x, $y)
            if (([Math]::Abs($pixel.R - $background.R) + [Math]::Abs($pixel.G - $background.G) + [Math]::Abs($pixel.B - $background.B)) -gt $tolerance) {
                $hits++
            }
        }
        if ($hits -ge $minimumHits) {
            $top = $y
            break
        }
    }

    for ($y = $Bitmap.Height - 1; $y -ge 0; $y--) {
        $hits = 0
        for ($x = 0; $x -lt $Bitmap.Width; $x++) {
            $pixel = $Bitmap.GetPixel($x, $y)
            if (([Math]::Abs($pixel.R - $background.R) + [Math]::Abs($pixel.G - $background.G) + [Math]::Abs($pixel.B - $background.B)) -gt $tolerance) {
                $hits++
            }
        }
        if ($hits -ge $minimumHits) {
            $bottom = $y
            break
        }
    }

    return New-Object System.Drawing.Rectangle(
        $left,
        $top,
        ([Math]::Max(1, $right - $left + 1)),
        ([Math]::Max(1, $bottom - $top + 1))
    )
}

function Click-WindowRelative {
    param(
        [Parameter(Mandatory = $true)][System.IntPtr]$Handle,
        [Parameter(Mandatory = $true)][double]$XPercent,
        [Parameter(Mandatory = $true)][double]$YPercent,
        [int]$DelayMs = 1600
    )

    Activate-Window -Handle $Handle
    $rect = Get-WindowRect -Handle $Handle
    $bitmap = New-Object System.Drawing.Bitmap $rect.Width, $rect.Height
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)

    try {
        $graphics.CopyFromScreen(
            (New-Object System.Drawing.Point($rect.Left, $rect.Top)),
            [System.Drawing.Point]::Empty,
            $bitmap.Size
        )
        $contentRect = Get-ContentBoundsFromBitmap -Bitmap $bitmap
    }
    finally {
        $graphics.Dispose()
        $bitmap.Dispose()
    }

    $targetX = $rect.Left + $contentRect.Left + [Math]::Round($contentRect.Width * $XPercent)
    $targetY = $rect.Top + $contentRect.Top + [Math]::Round($contentRect.Height * $YPercent)

    [WindowCaptureNative]::SetCursorPos($targetX, $targetY) | Out-Null
    Start-Sleep -Milliseconds 150
    [WindowCaptureNative]::mouse_event($MouseLeftDown, 0, 0, 0, [UIntPtr]::Zero)
    Start-Sleep -Milliseconds 60
    [WindowCaptureNative]::mouse_event($MouseLeftUp, 0, 0, 0, [UIntPtr]::Zero)
    Start-Sleep -Milliseconds $DelayMs
}

function Send-WindowKeys {
    param(
        [Parameter(Mandatory = $true)][System.IntPtr]$Handle,
        [Parameter(Mandatory = $true)][string]$Keys,
        [int]$DelayMs = 1800
    )

    Activate-Window -Handle $Handle
    [System.Windows.Forms.SendKeys]::SendWait($Keys)
    Start-Sleep -Milliseconds $DelayMs
}

function Save-WindowScreenshot {
    param(
        [Parameter(Mandatory = $true)][System.IntPtr]$Handle,
        [Parameter(Mandatory = $true)][string]$Path
    )

    Activate-Window -Handle $Handle
    $rect = Get-WindowRect -Handle $Handle
    Move-CursorOutsideWindow -Rect $rect

    $bitmap = New-Object System.Drawing.Bitmap $rect.Width, $rect.Height
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)

    try {
        $graphics.CopyFromScreen(
            (New-Object System.Drawing.Point($rect.Left, $rect.Top)),
            [System.Drawing.Point]::Empty,
            $bitmap.Size
        )
        $cropRect = Get-ContentBoundsFromBitmap -Bitmap $bitmap
        $cropped = $bitmap.Clone($cropRect, $bitmap.PixelFormat)

        try {
            $cropped.Save($Path, [System.Drawing.Imaging.ImageFormat]::Png)
        }
        finally {
            $cropped.Dispose()
        }
    }
    finally {
        $graphics.Dispose()
        $bitmap.Dispose()
    }
}

$ExePath = Resolve-FullPath -Path $ExePath
$OutputDir = Resolve-FullPath -Path $OutputDir

if (-not (Test-Path -LiteralPath $ExePath)) {
    if ($BuildIfMissing) {
        $repoRoot = Resolve-FullPath -Path (Join-Path $PSScriptRoot '..')
        Push-Location $repoRoot
        try {
            npm run tauri:build
        }
        finally {
            Pop-Location
        }
    }
}

if (-not (Test-Path -LiteralPath $ExePath)) {
    throw "Application executable not found: $ExePath"
}

if (-not (Test-Path -LiteralPath $OutputDir)) {
    New-Item -ItemType Directory -Path $OutputDir | Out-Null
}

$shots = @(
    @{ File = 'preview-mods.png'; Action = { param($h) Send-WindowKeys -Handle $h -Keys '%+m' -DelayMs 1800 } },
    @{ File = 'preview-nexus.png'; Action = { param($h) Send-WindowKeys -Handle $h -Keys '%+n' -DelayMs 2400 } },
    @{ File = 'preview-saves.png'; Action = { param($h) Send-WindowKeys -Handle $h -Keys '%+s' -DelayMs 1800 } },
    @{ File = 'preview-logs.png'; Action = { param($h) Send-WindowKeys -Handle $h -Keys '%+l' -DelayMs 1800 } },
    @{ File = 'preview-settings-nexus.png'; Action = { param($h) Send-WindowKeys -Handle $h -Keys '%+t' -DelayMs 1800 } },
    @{ File = 'preview-settings-translate.png'; Action = { param($h) Send-WindowKeys -Handle $h -Keys '%+w' -DelayMs 2200 } },
    @{ File = 'preview-settings-about.png'; Action = { param($h) Send-WindowKeys -Handle $h -Keys '%+e' -DelayMs 2200 } },
    @{ File = 'preview-game-selector.png'; Action = { param($h) Send-WindowKeys -Handle $h -Keys '%+h' -DelayMs 5200 } }
)

$process = Start-Process -FilePath $ExePath -WorkingDirectory (Split-Path -Parent $ExePath) -PassThru

try {
    $app = Wait-AppWindow -ProcessId $process.Id
    Start-Sleep -Milliseconds $StartupDelayMs

    foreach ($shot in $shots) {
        & $shot.Action $app.MainWindowHandle
        $targetPath = Join-Path $OutputDir $shot.File
        Save-WindowScreenshot -Handle $app.MainWindowHandle -Path $targetPath
        Write-Host "Saved $targetPath"
    }
}
finally {
    if (-not $LeaveAppOpen) {
        $running = Get-Process -Id $process.Id -ErrorAction SilentlyContinue
        if ($running) {
            $running.CloseMainWindow() | Out-Null
            Start-Sleep -Milliseconds 800
            if (-not $running.HasExited) {
                Stop-Process -Id $running.Id -Force
            }
        }
    }
}

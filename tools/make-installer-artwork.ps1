# ==============================================================================
# SAYL installer artwork generator
#
# Deterministically regenerates the four installer branding bitmaps:
#   apps/desktop/src-tauri/icons/wix-dialog.bmp   (WixUIDialogBmp, 493x312)
#   apps/desktop/src-tauri/icons/wix-banner.bmp   (WixUIBannerBmp, 493x58)
#   apps/desktop/src-tauri/icons/nsis-header.bmp  (MUI2 header,   150x57)
#   apps/desktop/src-tauri/icons/nsis-sidebar.bmp (MUI2 sidebar,  164x314)
#
# Design (INST-001): the flowing teal "sayl" calligraphy lives ONLY in the
# navy left panel; every area where MSI/NSIS draws text stays flat light so
# the default black dialog text remains readable. The latin "SAYL" wordmark
# is removed. The calligraphy pixels are reused from the existing
# nsis-sidebar.bmp (never redrawn, never rasterized from SVG).
#
# Wordmark removal is a selective fill inside a fixed rectangle
# (x 46..118, y 194..225) and is idempotent: pixels already navy are skipped,
# teal strokes are preserved, so regenerating from an already-cleaned file
# produces byte-identical output. The script proves that itself by generating
# twice (before and after overwriting the outputs) and byte-comparing.
#
# Only PowerShell 7 + System.Drawing is used. No network, no external tools.
# ==============================================================================

[CmdletBinding()]
param(
    [string]$RepoRoot = ""
)

$ErrorActionPreference = 'Stop'
if (-not $RepoRoot) {
    $RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
}
Set-StrictMode -Version Latest

Add-Type -AssemblyName System.Drawing

# ------------------------------------------------------------------------------
# Brand constants
# ------------------------------------------------------------------------------
$Navy      = [System.Drawing.Color]::FromArgb(11, 26, 78)     # #0B1A4E
$LightBg   = [System.Drawing.Color]::FromArgb(251, 251, 253)  # #FBFBFD
$WhiteBg   = [System.Drawing.Color]::FromArgb(255, 255, 255)  # #FFFFFF

$IconsDir    = Join-Path $RepoRoot "apps/desktop/src-tauri/icons"
$SidebarPath = Join-Path $IconsDir "nsis-sidebar.bmp"
$IconPath    = Join-Path $RepoRoot "assets/brand/icon-128.png"

$OutputNames = @('wix-dialog.bmp', 'wix-banner.bmp', 'nsis-header.bmp', 'nsis-sidebar.bmp')

# ------------------------------------------------------------------------------
# Helpers
# ------------------------------------------------------------------------------

# Load a bitmap from disk without keeping the file locked, and without the
# GDI+ "stream must stay alive" trap: the returned bitmap is a standalone
# deep copy made through LockBits/Marshal.Copy (no reference to the source
# stream or file remains).
function Load-Bmp {
    param([string]$Path)
    $bytes = [System.IO.File]::ReadAllBytes($Path)
    $stream = [System.IO.MemoryStream]::new($bytes)
    $loaded = [System.Drawing.Bitmap]::FromStream($stream)
    $w = $loaded.Width
    $h = $loaded.Height
    $rect = [System.Drawing.Rectangle]::new(0, 0, $w, $h)
    $src = $loaded.LockBits($rect, [System.Drawing.Imaging.ImageLockMode]::ReadOnly, $loaded.PixelFormat)
    $copy = [System.Drawing.Bitmap]::new($w, $h, $src.PixelFormat)
    $dst = $copy.LockBits($rect, [System.Drawing.Imaging.ImageLockMode]::WriteOnly, $src.PixelFormat)
    $len = [Math]::Abs($src.Stride) * $h
    $buffer = [byte[]]::new($len)
    [System.Runtime.InteropServices.Marshal]::Copy($src.Scan0, $buffer, 0, $len)
    [System.Runtime.InteropServices.Marshal]::Copy($buffer, 0, $dst.Scan0, $len)
    $copy.UnlockBits($dst)
    $loaded.UnlockBits($src)
    $loaded.Dispose()
    $stream.Dispose()
    return $copy
}

# Save a bitmap as 24bpp uncompressed BI_RGB BMP (bottom-up).
function Save-Bmp24 {
    param(
        [System.Drawing.Bitmap]$Bmp,
        [string]$Path
    )
    $rect = [System.Drawing.Rectangle]::new(0, 0, $Bmp.Width, $Bmp.Height)
    $clone = $Bmp.Clone($rect, [System.Drawing.Imaging.PixelFormat]::Format24bppRgb)
    $clone.Save($Path, [System.Drawing.Imaging.ImageFormat]::Bmp)
    $clone.Dispose()
}

# Remove the white latin wordmark from the sidebar artwork.
# Selective + idempotent: inside the fixed rect, only pixels that are neither
# navy nor teal-family are repainted with flat navy, so teal calligraphy that
# passes near the rect is preserved and re-running is a no-op.
function Remove-Wordmark {
    param([System.Drawing.Bitmap]$Sidebar)
    for ($y = 194; $y -le 225; $y++) {
        for ($x = 46; $x -le 118; $x++) {
            $c = $Sidebar.GetPixel($x, $y)
            $r = [int]$c.R; $g = [int]$c.G; $b = [int]$c.B
            $nearNavy = ([Math]::Abs($r - 11) -le 20 -and [Math]::Abs($g - 26) -le 20 -and [Math]::Abs($b - 78) -le 20)
            $tealFamily = ($g -ge 100 -and $b -ge 100 -and $r -le 90)
            if (-not $nearNavy -and -not $tealFamily) {
                $Sidebar.SetPixel($x, $y, $Navy)
            }
        }
    }
}

# Generate one complete artwork set into $OutDir, reading the sources from
# the icons directory / brand assets as they exist right now.
function New-ArtworkSet {
    param([string]$OutDir)

    $sidebar = Load-Bmp -Path $SidebarPath
    if ($sidebar.Width -ne 164 -or $sidebar.Height -ne 314) {
        throw "nsis-sidebar.bmp must be 164x314 (got $($sidebar.Width)x$($sidebar.Height))"
    }
    Remove-Wordmark -Sidebar $sidebar

    # -- nsis-sidebar.bmp: cleaned original ----------------------------------
    Save-Bmp24 -Bmp $sidebar -Path (Join-Path $OutDir 'nsis-sidebar.bmp')

    # -- wix-dialog.bmp: navy left panel (calligraphy) + flat light body -----
    $dialog = [System.Drawing.Bitmap]::new(493, 312, [System.Drawing.Imaging.PixelFormat]::Format24bppRgb)
    $g = [System.Drawing.Graphics]::FromImage($dialog)
    $g.Clear($LightBg)
    $g.FillRectangle([System.Drawing.SolidBrush]::new($Navy), 0, 0, 164, 312)
    $g.Dispose()
    # Pixel-exact copy of the top 312 rows of the cleaned sidebar (no GDI+
    # interpolation on the artwork itself).
    for ($y = 0; $y -lt 312; $y++) {
        for ($x = 0; $x -lt 164; $x++) {
            $dialog.SetPixel($x, $y, $sidebar.GetPixel($x, $y))
        }
    }
    Save-Bmp24 -Bmp $dialog -Path (Join-Path $OutDir 'wix-dialog.bmp')
    $dialog.Dispose()

    # -- brand mark source ----------------------------------------------------
    $mark = Load-Bmp -Path $IconPath
    if ($mark.Width -ne 128 -or $mark.Height -ne 128) {
        throw "assets/brand/icon-128.png must be 128x128 (got $($mark.Width)x$($mark.Height))"
    }

    # -- wix-banner.bmp: flat light, small mark at the right edge -------------
    $banner = [System.Drawing.Bitmap]::new(493, 58, [System.Drawing.Imaging.PixelFormat]::Format24bppRgb)
    $g = [System.Drawing.Graphics]::FromImage($banner)
    $g.Clear($LightBg)
    $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $g.CompositingQuality = [System.Drawing.Drawing2D.CompositingQuality]::HighQuality
    $g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
    $g.DrawImage($mark, [System.Drawing.Rectangle]::new(447, 10, 38, 38))
    $g.Dispose()
    Save-Bmp24 -Bmp $banner -Path (Join-Path $OutDir 'wix-banner.bmp')
    $banner.Dispose()

    # -- nsis-header.bmp: flat white (blends with the MUI2 white header), ------
    #    small mark on the right side of the canvas
    $header = [System.Drawing.Bitmap]::new(150, 57, [System.Drawing.Imaging.PixelFormat]::Format24bppRgb)
    $g = [System.Drawing.Graphics]::FromImage($header)
    $g.Clear($WhiteBg)
    $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $g.CompositingQuality = [System.Drawing.Drawing2D.CompositingQuality]::HighQuality
    $g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
    $g.DrawImage($mark, [System.Drawing.Rectangle]::new(106, 10, 36, 36))
    $g.Dispose()
    Save-Bmp24 -Bmp $header -Path (Join-Path $OutDir 'nsis-header.bmp')
    $header.Dispose()

    $mark.Dispose()
    $sidebar.Dispose()
}

# ------------------------------------------------------------------------------
# Generate, install, regenerate, and byte-compare (idempotency proof)
# ------------------------------------------------------------------------------

$tmpA = Join-Path $env:TEMP ("sayl-artwork-a-" + [Guid]::NewGuid().ToString('N'))
$tmpB = Join-Path $env:TEMP ("sayl-artwork-b-" + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $tmpA | Out-Null
New-Item -ItemType Directory -Path $tmpB | Out-Null

try {
    if (-not (Test-Path -LiteralPath $SidebarPath)) { throw "Missing source: $SidebarPath" }
    if (-not (Test-Path -LiteralPath $IconPath))    { throw "Missing source: $IconPath" }

    Write-Host "[1/3] Generating artwork set from current sources..." -ForegroundColor Yellow
    New-ArtworkSet -OutDir $tmpA

    Write-Host "[2/3] Installing the regenerated bitmaps into $IconsDir" -ForegroundColor Yellow
    foreach ($name in $OutputNames) {
        Copy-Item -LiteralPath (Join-Path $tmpA $name) -Destination (Join-Path $IconsDir $name) -Force
    }

    Write-Host "[3/3] Regenerating from the installed sources and byte-comparing (idempotency)..." -ForegroundColor Yellow
    New-ArtworkSet -OutDir $tmpB

    $mismatch = @()
    foreach ($name in $OutputNames) {
        $a = [System.IO.File]::ReadAllBytes((Join-Path $tmpA $name))
        $b = [System.IO.File]::ReadAllBytes((Join-Path $tmpB $name))
        $installed = [System.IO.File]::ReadAllBytes((Join-Path $IconsDir $name))
        if (($a.Length -ne $b.Length) -or ($a.Length -ne $installed.Length)) { $mismatch += $name; continue }
        for ($i = 0; $i -lt $a.Length; $i++) {
            if ($a[$i] -ne $b[$i] -or $a[$i] -ne $installed[$i]) { $mismatch += $name; break }
        }
    }

    if ($mismatch.Count -gt 0) {
        Write-Host "[FAIL] Regeneration is not byte-identical for: $($mismatch -join ', ')" -ForegroundColor Red
        exit 1
    }

    foreach ($name in $OutputNames) {
        $len = (Get-Item -LiteralPath (Join-Path $IconsDir $name)).Length
        Write-Host "  [OK] $name ($len bytes)" -ForegroundColor Green
    }
    Write-Host "[SUCCESS] Installer artwork regenerated deterministically. (Exit 0)" -ForegroundColor Green
    exit 0
}
finally {
    foreach ($d in @($tmpA, $tmpB)) {
        if (Test-Path -LiteralPath $d) { Remove-Item -LiteralPath $d -Recurse -Force }
    }
}

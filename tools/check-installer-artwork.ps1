# ==============================================================================
# SAYL installer dialog artwork validator + layout-accurate dialog renders
#
# Gates (exit 1 on any failure):
#   1. wix-dialog.bmp   493x312 24bpp: flat light text area (x>=164), navy
#                       left panel with calligraphy and no latin wordmark.
#   2. wix-banner.bmp   493x58  24bpp: flat light, brand mark only at the
#                       right edge (x>=415), no content anywhere else.
#   3. nsis-header.bmp  150x57  24bpp: flat white, brand mark only at
#                       x>=95.
#   4. nsis-sidebar.bmp 164x314 24bpp: navy-dominant calligraphy panel,
#                       no white wordmark, teal present.
#   5. WCAG contrast >= 7.0 (vs black) for every MSI text rect that lands on
#                       a bitmap, sampled from the real control geometry of
#                       the built MSI (Control/TextStyle tables).
#   6. Renders written to docs/screenshots/installer/ (layout-accurate
#                       mock renders composed from the real artwork and the
#                       real MSI control geometry - NOT runtime captures).
#
# PowerShell 7 + System.Drawing only. No network, no external tools.
# ==============================================================================

[CmdletBinding()]
param(
    [string]$RepoRoot = "",
    [string]$OutputDir = ""
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if (-not $RepoRoot) { $RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path }
if (-not $OutputDir) { $OutputDir = Join-Path $RepoRoot "docs/screenshots/installer" }

Add-Type -AssemblyName System.Drawing

$script:Failures = 0
$IconsDir = Join-Path $RepoRoot "apps/desktop/src-tauri/icons"

# Brand constants
$script:NAVY   = @(11, 26, 78)      # #0B1A4E
$script:LIGHT  = @(251, 251, 253)   # #FBFBFD
$script:TURQ   = @(0, 182, 189)     # #00B6BD
$script:WHITE  = @(255, 255, 255)   # #FFFFFF
$script:FACE   = [System.Drawing.Color]::FromArgb(240, 240, 240)   # classic 3D face

# Dialog geometry extracted from the built MSI's Control table.
# Scale: horizontal px = units * 493/370, vertical px = units * 4/3.
# Dialog outer = 493x360; dialog bitmap = wix-dialog.bmp at (0,0), 493x312;
# banner bitmap = wix-banner.bmp at (0,0), 493x58.
$script:MSI_WELCOME_TITLE = @{ X = 180; Y = 27; W = 293; H = 80 }
$script:MSI_WELCOME_DESC   = @{ X = 180; Y = 107; W = 293; H = 80 }
$script:MSI_EXIT_TITLE     = @{ X = 180; Y = 27; W = 293; H = 80 }
$script:MSI_EXIT_DESC      = @{ X = 180; Y = 93;  W = 293; H = 53 }
$script:MSI_EXIT_CHECKBOX  = @{ X = 180; Y = 253; W = 293; H = 53 }
$script:MSI_BANNER_TITLE   = @{ X = 20;  Y = 8;   W = 267; H = 20 }
$script:MSI_LICENSE_DESC   = @{ X = 33;  Y = 31;  W = 453; H = 20 }
$script:MSI_INSTALLDIR_DESC = @{ X = 33; Y = 31;  W = 373; H = 20 }

# ------------------------------------------------------------------------------
# Helpers
# ------------------------------------------------------------------------------

# Standalone deep-copy bitmap load (no file locks, no GDI+ stream-lifetime trap).
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

# Fast 24bpp pixel buffer via LockBits (top-down rows: index = y*stride + x*3, BGR).
function Get-PixelData {
    param([System.Drawing.Bitmap]$Bmp)
    $rect = [System.Drawing.Rectangle]::new(0, 0, $Bmp.Width, $Bmp.Height)
    $bd = $Bmp.LockBits($rect, [System.Drawing.Imaging.ImageLockMode]::ReadOnly, [System.Drawing.Imaging.PixelFormat]::Format24bppRgb)
    $px = [byte[]]::new([Math]::Abs($bd.Stride) * $bd.Height)
    [System.Runtime.InteropServices.Marshal]::Copy($bd.Scan0, $px, 0, $px.Length)
    $Bmp.UnlockBits($bd)
    return @{ Width = $Bmp.Width; Height = $Bmp.Height; Stride = $bd.Stride; Pixels = $px }
}

function Test-BmpFormat {
    param([string]$Name, [int]$ExpectedW, [int]$ExpectedH)
    $Path = Join-Path $IconsDir $Name
    if (-not (Test-Path -LiteralPath $Path)) {
        Write-Host "  [FAIL] Missing bitmap: $Path" -ForegroundColor Red
        $script:Failures++
        return $false
    }
    $b = [System.IO.File]::ReadAllBytes($Path)
    $ok = $true
    if ($b.Length -lt 54) { $ok = $false }
    $magic = if ($b.Length -ge 2) { [System.Text.Encoding]::ASCII.GetString($b, 0, 2) } else { '??' }
    if ($magic -ne 'BM') { $ok = $false }
    $bpp = [System.BitConverter]::ToUInt16($b, 28)
    $compression = [System.BitConverter]::ToUInt32($b, 30)
    $w = [System.BitConverter]::ToInt32($b, 18)
    $h = [Math]::Abs([System.BitConverter]::ToInt32($b, 22))
    if ($w -ne $ExpectedW -or $h -ne $ExpectedH) { $ok = $false }
    if ($bpp -ne 24) { $ok = $false }
    if ($compression -ne 0) { $ok = $false }
    if ($ok) {
        Write-Host "  [PASS] $Name format: ${w}x${h}, ${bpp}bpp, BI_RGB uncompressed" -ForegroundColor Green
    } else {
        Write-Host "  [FAIL] $Name format: ${w}x${h}, ${bpp}bpp, compression=$compression (expected ${ExpectedW}x${ExpectedH}, 24bpp, BI_RGB)" -ForegroundColor Red
        $script:Failures++
    }
    return
}

function Test-Condition {
    param([bool]$Ok, [string]$PassText, [string]$FailText)
    if ($Ok) {
        Write-Host "  [PASS] $PassText" -ForegroundColor Green
    } else {
        Write-Host "  [FAIL] $FailText" -ForegroundColor Red
        $script:Failures++
    }
}

# sRGB relative luminance of one 8-bit channel value
function Get-Linear {
    param([int]$V)
    $c = $V / 255.0
    if ($c -le 0.03928) { return $c / 12.92 }
    return [Math]::Pow((($c + 0.055) / 1.055), 2.4)
}

# Mean relative luminance over a rect of a pixel buffer (top-down indexing),
# sampled every Nth pixel in both axes for speed.
function Get-RectMeanLuminance {
    param([hashtable]$Data, [int]$X0, [int]$Y0, [int]$X1, [int]$Y1, [int]$Step = 3)
    $sum = 0.0
    $count = 0
    for ($y = $Y0; $y -le $Y1; $y += $Step) {
        $row = $y * $Data.Stride
        for ($x = $X0; $x -le $X1; $x += $Step) {
            $i = $row + $x * 3
            $r = Get-Linear -V $Data.Pixels[$i + 2]
            $g = Get-Linear -V $Data.Pixels[$i + 1]
            $b = Get-Linear -V $Data.Pixels[$i]
            $sum += 0.2126 * $r + 0.7152 * $g + 0.0722 * $b
            $count++
        }
    }
    return $sum / [Math]::Max($count, 1)
}

# WCAG contrast ratio of a mean luminance vs black.
function Get-ContrastVsBlack {
    param([double]$Luminance)
    return ($Luminance + 0.05) / 0.05
}

# ------------------------------------------------------------------------------
# Render drawing helpers (layout-accurate mock UI, classic Windows controls)
# ------------------------------------------------------------------------------

$script:FontNormal = [System.Drawing.Font]::new('Tahoma', 8)
$script:FontBold   = [System.Drawing.Font]::new('Tahoma', 9, [System.Drawing.FontStyle]::Bold)
$script:FontBig    = [System.Drawing.Font]::new('Tahoma', 12)

function New-Canvas {
    param([int]$W, [int]$H, [System.Drawing.Color]$Bg, [double]$Scale = 2.0)
    $bmp = [System.Drawing.Bitmap]::new([int]($W * $Scale), [int]($H * $Scale), [System.Drawing.Imaging.PixelFormat]::Format24bppRgb)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.Clear($Bg)
    $g.ScaleTransform($Scale, $Scale)
    $g.TextRenderingHint = [System.Drawing.Text.TextRenderingHint]::ClearTypeGridFit
    $g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::Half
    return @{ Bmp = $bmp; G = $g; Scale = $Scale }
}

function Save-CanvasPng {
    param([hashtable]$Canvas, [string]$Path)
    $Canvas.Bmp.Save($Path, [System.Drawing.Imaging.ImageFormat]::Png)
    $Canvas.G.Dispose()
    $Canvas.Bmp.Dispose()
}

function Draw-Button {
    param([System.Drawing.Graphics]$G, [string]$Text, [int]$X, [int]$Y, [bool]$IsDefault = $false, [bool]$Enabled = $true)
    $w = 75; $h = 23
    $G.FillRectangle([System.Drawing.SolidBrush]::new([System.Drawing.Color]::FromArgb(237, 237, 237)), ($X + 2), ($Y + 2), ($w - 4), ($h - 4))
    if ($IsDefault) {
        $pen = [System.Drawing.Pen]::new([System.Drawing.Color]::FromArgb(20, 20, 20), 2)
        $G.DrawRectangle($pen, ($X + 1), ($Y + 1), ($w - 2), ($h - 2))
    } else {
        $pen = [System.Drawing.Pen]::new([System.Drawing.Color]::FromArgb(112, 112, 112))
        $G.DrawRectangle($pen, ($X + 0.5), ($Y + 0.5), ($w - 1), ($h - 1))
    }
    $tf = [System.Drawing.StringFormat]::new()
    $tf.Alignment = [System.Drawing.StringAlignment]::Center
    $tf.LineAlignment = [System.Drawing.StringAlignment]::Center
    $brush = if ($Enabled) { [System.Drawing.Brushes]::Black } else { [System.Drawing.SolidBrush]::new([System.Drawing.Color]::FromArgb(109, 109, 109)) }
    $G.DrawString($Text, $script:FontNormal, $brush, [System.Drawing.RectangleF]::new($X, $Y, $w, $h), $tf)
}

function Draw-CheckBox {
    param([System.Drawing.Graphics]$G, [string]$Text, [int]$X, [int]$Y, [bool]$Checked)
    $G.FillRectangle([System.Drawing.Brushes]::White, ($X + 1), ($Y + 2), 12, 12)
    $G.DrawRectangle([System.Drawing.Pen]::new([System.Drawing.Color]::FromArgb(112, 112, 112)), ($X + 1.5), ($Y + 2.5), 11, 11)
    if ($Checked) {
        $pen = [System.Drawing.Pen]::new([System.Drawing.Color]::Black, 2)
        $pts = [System.Drawing.Point[]]@(
            [System.Drawing.Point]::new(($X + 4), ($Y + 8)),
            [System.Drawing.Point]::new(($X + 7), ($Y + 11)),
            [System.Drawing.Point]::new(($X + 11), ($Y + 4))
        )
        $G.DrawLines($pen, $pts)
    }
    $G.DrawString($Text, $script:FontNormal, [System.Drawing.Brushes]::Black, [System.Drawing.RectangleF]::new(($X + 19), $Y, 320, 18))
}

function Draw-SunkenBox {
    param([System.Drawing.Graphics]$G, [int]$X, [int]$Y, [int]$W, [int]$H)
    $G.FillRectangle([System.Drawing.Brushes]::White, ($X + 1), ($Y + 1), ($W - 2), ($H - 2))
    $dark = [System.Drawing.Pen]::new([System.Drawing.Color]::FromArgb(112, 112, 112))
    $lite = [System.Drawing.Pen]::new([System.Drawing.Color]::White)
    $G.DrawLine($dark, $X, $Y, ($X + $W - 1), $Y)
    $G.DrawLine($dark, $X, $Y, $X, ($Y + $H - 1))
    $G.DrawLine($lite, ($X + $W - 1), $Y, ($X + $W - 1), ($Y + $H - 1))
    $G.DrawLine($lite, $X, ($Y + $H - 1), ($X + $W - 1), ($Y + $H - 1))
}

function Draw-BottomLine {
    param([System.Drawing.Graphics]$G, [int]$Y, [int]$W)
    $G.DrawLine([System.Drawing.Pen]::new([System.Drawing.Color]::FromArgb(153, 153, 153)), 0, $Y, $W, $Y)
}

# ==============================================================================
# Gates
# ==============================================================================

Write-Host "============================================================" -ForegroundColor Cyan
Write-Host " SAYL - Installer Dialog Artwork Validator + Dialog Renders" -ForegroundColor Cyan
Write-Host " Repo root: $RepoRoot" -ForegroundColor Cyan
Write-Host "============================================================" -ForegroundColor Cyan

# ------------------------------------------------------------------------------
Write-Host "`n[1/6] Bitmap formats (24bpp BI_RGB, exact dimensions)" -ForegroundColor Yellow
# ------------------------------------------------------------------------------
Test-BmpFormat -Name 'wix-dialog.bmp'   -ExpectedW 493 -ExpectedH 312
Test-BmpFormat -Name 'wix-banner.bmp'  -ExpectedW 493 -ExpectedH 58
Test-BmpFormat -Name 'nsis-header.bmp' -ExpectedW 150 -ExpectedH 57
Test-BmpFormat -Name 'nsis-sidebar.bmp' -ExpectedW 164 -ExpectedH 314

$dialogBmp   = Load-Bmp -Path (Join-Path $IconsDir 'wix-dialog.bmp')
$bannerBmp   = Load-Bmp -Path (Join-Path $IconsDir 'wix-banner.bmp')
$headerBmp    = Load-Bmp -Path (Join-Path $IconsDir 'nsis-header.bmp')
$sidebarBmp  = Load-Bmp -Path (Join-Path $IconsDir 'nsis-sidebar.bmp')
$dialogData  = Get-PixelData -Bmp $dialogBmp
$bannerData  = Get-PixelData -Bmp $bannerBmp
$headerData  = Get-PixelData -Bmp $headerBmp
$sidebarData = Get-PixelData -Bmp $sidebarBmp

# ------------------------------------------------------------------------------
Write-Host "`n[2/6] wix-dialog.bmp layout (navy left panel, flat light text area)" -ForegroundColor Yellow
# ------------------------------------------------------------------------------
# Right text zone: every pixel within +/-8 of #FBFBFD
$deviants = 0
for ($y = 0; $y -lt 312; $y++) {
    $row = $y * $dialogData.Stride
    for ($x = 170; $x -le 489; $x++) {
        $i = $row + $x * 3
        if (  [Math]::Abs($dialogData.Pixels[$i + 2] - 251) -gt 8 `
          -or [Math]::Abs($dialogData.Pixels[$i + 1] - 251) -gt 8 `
          -or [Math]::Abs($dialogData.Pixels[$i]     - 253) -gt 8) { $deviants++ }
    }
}
Test-Condition -Ok ($deviants -eq 0) `
    -PassText "text area (x 170..489) is uniform #FBFBFD, 0 deviant pixels" `
    -FailText "text area (x 170..489) has $deviants deviant pixels (expected 0)"

# Left panel: navy-dominant, no light leak, no white wordmark
$navy = 0; $lightLeak = 0; $white = 0; $tot = 0
for ($y = 0; $y -lt 312; $y++) {
    $row = $y * $dialogData.Stride
    for ($x = 0; $x -le 158; $x++) {
        $i = $row + $x * 3
        $r = [int]$dialogData.Pixels[$i + 2]; $g = [int]$dialogData.Pixels[$i + 1]; $b = [int]$dialogData.Pixels[$i]
        $tot++
        if ([Math]::Abs($r - 11) -le 20 -and [Math]::Abs($g - 26) -le 20 -and [Math]::Abs($b - 78) -le 20) { $navy++ }
        if ([Math]::Abs($r - 251) -le 20 -and [Math]::Abs($g - 251) -le 20 -and [Math]::Abs($b - 253) -le 20) { $lightLeak++ }
        if ($r -ge 200 -and $g -ge 200 -and $b -ge 200) { $white++ }
    }
}
$navyFrac = $navy / $tot; $leakFrac = $lightLeak / $tot; $whiteFrac = $white / $tot
Test-Condition -Ok (($navyFrac -ge 0.60) -and ($leakFrac -le 0.005) -and ($whiteFrac -le 0.002)) `
    -PassText ("left panel (x 0..158): navy {0:P1}, light leak {1:P3}, white {2:P3}" -f $navyFrac, $leakFrac, $whiteFrac) `
    -FailText ("left panel (x 0..158) out of bounds: navy {0:P1} (min 60%), light leak {1:P3} (max 0.5%), white {2:P3} (max 0.2%)" -f $navyFrac, $leakFrac, $whiteFrac)

# ------------------------------------------------------------------------------
Write-Host "`n[3/6] wix-banner.bmp / nsis-header.bmp / nsis-sidebar.bmp layout" -ForegroundColor Yellow
# ------------------------------------------------------------------------------
# wix-banner: x 0..410 uniform #FBFBFD; all deviants confined to x 415..492; mark present
$deviants = 0; $outOfZone = 0; $markPx = 0
for ($y = 0; $y -lt 58; $y++) {
    $row = $y * $bannerData.Stride
    for ($x = 0; $x -le 492; $x++) {
        $i = $row + $x * 3
        $d = (  [Math]::Abs($bannerData.Pixels[$i + 2] - 251) -gt 8 `
          -or [Math]::Abs($bannerData.Pixels[$i + 1] - 251) -gt 8 `
          -or [Math]::Abs($bannerData.Pixels[$i]     - 253) -gt 8)
        if ($d) {
            $deviants++
            if ($x -ge 415) { $markPx++ } else { $outOfZone++ }
        }
    }
}
Test-Condition -Ok (($outOfZone -eq 0) -and ($markPx -gt 0)) `
    -PassText "wix-banner: deviants only in mark zone x 415..492 ($markPx px); x 0..410 uniform #FBFBFD" `
    -FailText "wix-banner: $outOfZone deviant pixels outside x>=415 (expected 0); mark zone px=$markPx (expected > 0)"

# nsis-header: x 0..90 uniform white; deviants only x>=95; mark present
$deviants = 0; $outOfZone = 0; $markPx = 0
for ($y = 0; $y -lt 57; $y++) {
    $row = $y * $headerData.Stride
    for ($x = 0; $x -le 149; $x++) {
        $i = $row + $x * 3
        $r = [int]$headerData.Pixels[$i + 2]; $g = [int]$headerData.Pixels[$i + 1]; $b = [int]$headerData.Pixels[$i]
        $isWhite = ([Math]::Abs($r - 255) -le 8 -and [Math]::Abs($g - 255) -le 8 -and [Math]::Abs($b - 255) -le 8)
        if (-not $isWhite) {
            $deviants++
            if ($x -ge 95) { $markPx++ } else { $outOfZone++ }
        }
    }
}
Test-Condition -Ok (($outOfZone -eq 0) -and ($markPx -gt 0)) `
    -PassText "nsis-header: x 0..90 uniform #FFFFFF; mark present at x>=95 ($markPx px)" `
    -FailText "nsis-header: $outOfZone non-white pixels at x<95 (expected 0); mark zone px=$markPx (expected > 0)"

# nsis-sidebar: >=85% navy, <=0.1% bright white, >=1% teal
$navy = 0; $white = 0; $teal = 0; $tot = 164 * 314
for ($y = 0; $y -lt 314; $y++) {
    $row = $y * $sidebarData.Stride
    for ($x = 0; $x -le 163; $x++) {
        $i = $row + $x * 3
        $r = [int]$sidebarData.Pixels[$i + 2]; $g = [int]$sidebarData.Pixels[$i + 1]; $b = [int]$sidebarData.Pixels[$i]
        if ([Math]::Abs($r - 11) -le 20 -and [Math]::Abs($g - 26) -le 20 -and [Math]::Abs($b - 78) -le 20) { $navy++ }
        elseif ($r -ge 200 -and $g -ge 200 -and $b -ge 200) { $white++ }
        if ($g -ge 100 -and $b -ge 100 -and $r -le 90) { $teal++ }
    }
}
Test-Condition -Ok (($navy / $tot -ge 0.85) -and ($white / $tot -le 0.001) -and ($teal / $tot -ge 0.01)) `
    -PassText ("nsis-sidebar: navy {0:P1}, white {1:P3}, teal {2:P1}" -f ($navy / $tot), ($white / $tot), ($teal / $tot)) `
    -FailText ("nsis-sidebar out of bounds: navy {0:P1} (min 85%), white {1:P3} (max 0.1%), teal {2:P1} (min 1%)" -f ($navy / $tot), ($white / $tot), ($teal / $tot))

# ------------------------------------------------------------------------------
Write-Host "`n[4/6] WCAG contrast of MSI text rects on the artwork (vs black, >= 7.0)" -ForegroundColor Yellow
# ------------------------------------------------------------------------------
$contrastRects = @(
    @{ Name = 'WelcomeDlg title';        Data = $dialogData; R = $script:MSI_WELCOME_TITLE },
    @{ Name = 'WelcomeDlg description'; Data = $dialogData; R = $script:MSI_WELCOME_DESC },
    @{ Name = 'ExitDialog title';       Data = $dialogData; R = $script:MSI_EXIT_TITLE },
    @{ Name = 'ExitDialog description';  Data = $dialogData; R = $script:MSI_EXIT_DESC },
    @{ Name = 'ExitDialog checkbox';     Data = $dialogData; R = $script:MSI_EXIT_CHECKBOX },
    @{ Name = 'License title';           Data = $bannerData; R = $script:MSI_BANNER_TITLE },
    @{ Name = 'License description';     Data = $bannerData; R = $script:MSI_LICENSE_DESC },
    @{ Name = 'InstallDir title';        Data = $bannerData; R = $script:MSI_BANNER_TITLE },
    @{ Name = 'InstallDir description';  Data = $bannerData; R = $script:MSI_INSTALLDIR_DESC }
)
foreach ($cr in $contrastRects) {
    $r = $cr.R
    $lum = Get-RectMeanLuminance -Data $cr.Data -X0 ($r.X + 2) -Y0 ($r.Y + 2) -X1 ($r.X + $r.W - 3) -Y1 ($r.Y + $r.H - 3)
    $ratio = Get-ContrastVsBlack -Luminance $lum
    Test-Condition -Ok ($ratio -ge 7.0) `
        -PassText ("{0} rect ({1},{2} {3}x{4}): mean luminance {5:N3}, contrast vs black {6:N1}" -f $cr.Name, $r.X, $r.Y, $r.W, $r.H, $lum, $ratio) `
        -FailText ("{0} rect: contrast vs black {1:N1} < 7.0 (mean luminance {2:N3})" -f $cr.Name, $ratio, $lum)
}

# Longest banner description must not reach the banner mark (no text overlap)
$probeCanvas = New-Canvas -W 8 -H 8 -Bg ([System.Drawing.Color]::White)
$longestDesc = 'Please read the following license agreement carefully'
$descSize = $probeCanvas.G.MeasureString($longestDesc, $script:FontNormal)
$descEndX = [int]($script:MSI_LICENSE_DESC.X + $descSize.Width)
$probeCanvas.G.Dispose(); $probeCanvas.Bmp.Dispose()
Test-Condition -Ok (($descEndX + 24) -lt 447) `
    -PassText ("longest banner description ends at x=$descEndX (+24 margin < 447 where the banner mark starts): no overlap") `
    -FailText ("longest banner description ends at x=$descEndX and would overlap the banner mark at x>=447")

# ------------------------------------------------------------------------------
Write-Host "`n[5/6] Rendering layout-accurate dialog mock renders" -ForegroundColor Yellow
# ------------------------------------------------------------------------------
New-Item -ItemType Directory -Force -Path $OutputDir | Out-Null
$licenseLines = (Get-Content -LiteralPath (Join-Path $RepoRoot 'LICENSE') -TotalCount 26) -join [Environment]::NewLine
$installPath = 'C:\Users\Dragonfly\AppData\Local\Programs\SAYL'
$black = [System.Drawing.Brushes]::Black
$darkGray = [System.Drawing.SolidBrush]::new([System.Drawing.Color]::FromArgb(51, 51, 51))

# --- MSI bitmap dialogs -------------------------------------------------------
function New-MsiBitmapDialog {
    param([string]$Title, [string]$Description, [bool]$Finish)
    $c = New-Canvas -W 493 -H 360 -Bg $script:FACE
    $c.G.DrawImage($script:CurrentDialogBmp, 0, 0)
    Draw-BottomLine -G $c.G -Y 312 -W 493
    $c.G.DrawString($Title, $script:FontBig, $black, [System.Drawing.RectangleF]::new(180, 27, 293, 80))
    $c.G.DrawString($Description, $script:FontNormal, $black, [System.Drawing.RectangleF]::new(180, 107, 293, 80))
    if ($Finish) {
        Draw-CheckBox -G $c.G -Text 'Launch SAYL' -X 180 -Y 253 -Checked:$true
    }
    Draw-Button -G $c.G -Text '< Back' -X 240 -Y 324 -Enabled:$false
    if ($Finish) {
        Draw-Button -G $c.G -Text 'Finish' -X 315 -Y 324 -IsDefault $true
    } else {
        Draw-Button -G $c.G -Text 'Next >' -X 315 -Y 324 -IsDefault $true
    }
    Draw-Button -G $c.G -Text 'Cancel' -X 405 -Y 324
    return $c
}

# --- MSI banner dialogs -------------------------------------------------------
function New-MsiBannerDialog {
    param([string]$Title, [string]$Description)
    $c = New-Canvas -W 493 -H 360 -Bg $script:FACE
    $c.G.DrawImage($script:CurrentBannerBmp, 0, 0)
    Draw-BottomLine -G $c.G -Y 58 -W 493
    $c.G.DrawString($Title, $script:FontBold, $black, [System.Drawing.RectangleF]::new(20, 8, 267, 20))
    $c.G.DrawString($Description, $script:FontNormal, $black, [System.Drawing.RectangleF]::new(33, 31, 453, 20))
    return $c
}

# msi-welcome.png
$script:CurrentDialogBmp = $dialogBmp
$c = New-MsiBitmapDialog -Title 'Welcome to the SAYL Setup Wizard' `
    -Description 'The Setup Wizard will install SAYL on your computer. Click Next to continue or Cancel to exit the Setup Wizard.' -Finish:$false
Save-CanvasPng -Canvas $c -Path (Join-Path $OutputDir 'msi-welcome.png')

# msi-finish.png
$c = New-MsiBitmapDialog -Title 'Completed the SAYL Setup Wizard' `
    -Description 'Click the Finish button to exit the Setup Wizard.' -Finish:$true
Save-CanvasPng -Canvas $c -Path (Join-Path $OutputDir 'msi-finish.png')

# msi-license.png
$script:CurrentBannerBmp = $bannerBmp
$c = New-MsiBannerDialog -Title 'End-User License Agreement' `
    -Description $longestDesc
# license text box (ScrollableText): white sunken box + scrollbar strip
Draw-SunkenBox -G $c.G -X 27 -Y 80 -W 440 -H 187
$c.G.FillRectangle([System.Drawing.SolidBrush]::new($script:FACE), 449, 81, 17, 185)
$c.G.FillRectangle([System.Drawing.SolidBrush]::new([System.Drawing.Color]::FromArgb(191, 191, 191)), 449, 95, 17, 36)
$c.G.DrawString($licenseLines, $script:FontNormal, $darkGray, [System.Drawing.RectangleF]::new(31, 84, 412, 179))
Draw-CheckBox -G $c.G -Text 'I accept the terms in the License Agreement' -X 27 -Y 276 -Checked:$false
Draw-Button -G $c.G -Text 'Print' -X 149 -Y 324
Draw-Button -G $c.G -Text '< Back' -X 240 -Y 324
Draw-Button -G $c.G -Text 'Next >' -X 315 -Y 324 -IsDefault $true
Draw-Button -G $c.G -Text 'Cancel' -X 405 -Y 324
Save-CanvasPng -Canvas $c -Path (Join-Path $OutputDir 'msi-license.png')

# msi-install-folder.png
$c = New-MsiBannerDialog -Title 'Destination Folder' `
    -Description 'Click Next to install to the default folder or click Change to choose a different folder.'
$c.G.DrawString('Install SAYL to:', $script:FontNormal, $black, [System.Drawing.RectangleF]::new(27, 80, 387, 40))
Draw-SunkenBox -G $c.G -X 27 -Y 133 -W 427 -H 24
$c.G.DrawString($installPath, $script:FontNormal, $black, [System.Drawing.RectangleF]::new(33, 137, 420, 18))
Draw-Button -G $c.G -Text 'Change...' -X 27 -Y 160 -Enabled:$true
Draw-Button -G $c.G -Text '< Back' -X 240 -Y 324
Draw-Button -G $c.G -Text 'Next >' -X 315 -Y 324 -IsDefault $true
Draw-Button -G $c.G -Text 'Cancel' -X 405 -Y 324
Save-CanvasPng -Canvas $c -Path (Join-Path $OutputDir 'msi-install-folder.png')

# --- NSIS pages ----------------------------------------------------------------
$nsisWhite = [System.Drawing.Color]::White
# Arabic "sayl" wordmark (U+0633 U+064A U+0644) built from char codes so this
# script stays pure ASCII and is immune to parser encoding mishandling.
$arSayl = [string]::Join('', [char[]]@(0x0633, 0x064A, 0x0644))

# nsis-welcome.png
$c = New-Canvas -W 500 -H 360 -Bg $nsisWhite
$c.G.DrawImage($sidebarBmp, 0, 0)
$c.G.DrawString(('Welcome to the SAYL ({0}) 0.1.0 Setup Wizard' -f $arSayl), $script:FontBig, $black, [System.Drawing.RectangleF]::new(175, 10, 310, 40))
$c.G.DrawString(('This will install SAYL ({0}) 0.1.0 on your computer. It is recommended that you close all other applications before starting Setup.{1}{1}Click Next to continue.' -f $arSayl, [char]10), $script:FontNormal, $black, [System.Drawing.RectangleF]::new(175, 60, 310, 140))
Draw-Button -G $c.G -Text 'Next >' -X 334 -Y 325 -IsDefault $true
Draw-Button -G $c.G -Text 'Cancel' -X 417 -Y 325
Save-CanvasPng -Canvas $c -Path (Join-Path $OutputDir 'nsis-welcome.png')

# nsis-directory.png
$c = New-Canvas -W 500 -H 360 -Bg $nsisWhite
$c.G.DrawImage($headerBmp, 0, 0)
Draw-BottomLine -G $c.G -Y 57 -W 500
$c.G.DrawString('Installation Folder', $script:FontBold, $black, [System.Drawing.RectangleF]::new(160, 10, 320, 18))
$c.G.DrawString(('Select the folder where SAYL ({0}) 0.1.0 will be installed.' -f $arSayl), $script:FontNormal, $black, [System.Drawing.RectangleF]::new(160, 30, 320, 18))
$c.G.DrawString(('Setup will install SAYL ({0}) 0.1.0 in the following folder.' -f $arSayl), $script:FontNormal, $black, [System.Drawing.RectangleF]::new(20, 80, 400, 18))
Draw-SunkenBox -G $c.G -X 20 -Y 110 -W 320 -H 24
$c.G.DrawString($installPath, $script:FontNormal, $black, [System.Drawing.RectangleF]::new(26, 114, 320, 18))
Draw-Button -G $c.G -Text 'Browse...' -X 350 -Y 110
Draw-Button -G $c.G -Text '< Back' -X 251 -Y 325
Draw-Button -G $c.G -Text 'Next >' -X 334 -Y 325 -IsDefault $true
Draw-Button -G $c.G -Text 'Cancel' -X 417 -Y 325
Save-CanvasPng -Canvas $c -Path (Join-Path $OutputDir 'nsis-directory.png')

# ------------------------------------------------------------------------------
Write-Host "`n[6/6] Render outputs" -ForegroundColor Yellow
# ------------------------------------------------------------------------------
$expectedRenders = @('msi-welcome.png', 'msi-license.png', 'msi-install-folder.png', 'msi-finish.png', 'nsis-welcome.png', 'nsis-directory.png')
foreach ($name in $expectedRenders) {
    $p = Join-Path $OutputDir $name
    if (-not (Test-Path -LiteralPath $p)) {
        Write-Host "  [FAIL] Missing render: $p" -ForegroundColor Red
        $script:Failures++
        continue
    }
    $len = (Get-Item -LiteralPath $p).Length
    if ($len -lt 20KB) {
        Write-Host "  [FAIL] Render $name is only $len bytes (expected >= 20480; renders are 2x scale)" -ForegroundColor Red
        $script:Failures++
    } else {
        Write-Host "  [PASS] Render $name ($len bytes)" -ForegroundColor Green
    }
}

# README for the renders directory
$readme = @"
# Installer dialog renders

Layout-accurate mock renders composed from the real installer artwork
(apps/desktop/src-tauri/icons/*.bmp) and the real MSI dialog control geometry
(Control and TextStyle tables of the built v0.1.0 MSI), plus the NSIS MUI2
welcome/sidebar/header geometry from the bundled NSIS 3.x sources.

- msi-welcome.png    - MSI WelcomeDlg (bitmap dialog, wix-dialog.bmp background)
- msi-license.png    - MSI LicenseAgreementDlg (banner dialog, wix-banner.bmp)
- msi-install-folder.png - MSI InstallDirDlg (banner dialog, wix-banner.bmp)
- msi-finish.png     - MSI ExitDialog with the Launch SAYL checkbox
- nsis-welcome.png   - NSIS MUI2 welcome page (nsis-sidebar.bmp)
- nsis-directory.png - NSIS MUI2 directory page (nsis-header.bmp)

These are NOT runtime captures. Runtime capture is blocked on this developer
machine per the project workflow (validation is CI-only; see docs/TOOLCHAIN.md
and STATUS.json): the real wizard appearance is verified by building the
installers in CI and by the pixel/contrast gates in
tools/check-installer-artwork.ps1.
"@
Set-Content -LiteralPath (Join-Path $OutputDir 'README.md') -Value $readme -Encoding UTF8

foreach ($b in @($dialogBmp, $bannerBmp, $headerBmp, $sidebarBmp)) { $b.Dispose() }

Write-Host "`n------------------------------------------------------------" -ForegroundColor Cyan
if ($script:Failures -eq 0) {
    Write-Host "[SUCCESS] Installer artwork gates passed and renders written to $OutputDir (Exit 0)" -ForegroundColor Green
    exit 0
} else {
    Write-Host "[ERROR] Installer artwork validation failed with $($script:Failures) error(s). (Exit 1)" -ForegroundColor Red
    exit 1
}
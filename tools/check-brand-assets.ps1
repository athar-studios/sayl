[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$Failures = 0

Write-Host "============================================================" -ForegroundColor Cyan
Write-Host " SAYL - Brand Assets and Installer Configurations Validator" -ForegroundColor Cyan
Write-Host " Workspace Root: $RepoRoot" -ForegroundColor Cyan
Write-Host "============================================================" -ForegroundColor Cyan

# ------------------------------------------------------------------------------
# 1. Validate Multi-Frame .ico Files
# ------------------------------------------------------------------------------
Write-Host "`n[1/3] Validating Multi-Frame .ico Assets..." -ForegroundColor Yellow

$RequiredIcoSizes = @(16, 32, 48, 256)
$IcoTargets = @(
    (Join-Path $RepoRoot "assets\brand\icon.ico"),
    (Join-Path $RepoRoot "apps/desktop/src-tauri/icons/icon.ico")
)

function Test-IcoFile([string]$FilePath, [int[]]$RequiredSizes) {
    if (-not (Test-Path -LiteralPath $FilePath)) {
        Write-Host "  [FAIL] Missing .ico file: $FilePath" -ForegroundColor Red
        return $false
    }

    $bytes = [System.IO.File]::ReadAllBytes($FilePath)
    if ($bytes.Length -lt 6) {
        Write-Host "  [FAIL] .ico file is too short: $FilePath" -ForegroundColor Red
        return $false
    }

    $reserved = [System.BitConverter]::ToUInt16($bytes, 0)
    $type = [System.BitConverter]::ToUInt16($bytes, 2)
    $count = [System.BitConverter]::ToUInt16($bytes, 4)

    if ($reserved -ne 0 -or $type -ne 1) {
        Write-Host "  [FAIL] Invalid .ico header (reserved=$reserved, type=$type): $FilePath" -ForegroundColor Red
        return $false
    }

    $presentSizes = [System.Collections.Generic.HashSet[int]]::new()
    for ($i = 0; $i -lt $count; $i++) {
        $offset = 6 + ($i * 16)
        if ($offset + 16 -gt $bytes.Length) { break }
        $w = [int]$bytes[$offset]
        $h = [int]$bytes[$offset + 1]
        if ($w -eq 0) { $w = 256 }
        if ($h -eq 0) { $h = 256 }
        [void]$presentSizes.Add($w)
    }

    $allPresent = $true
    $missingSizes = @()
    foreach ($req in $RequiredSizes) {
        if (-not $presentSizes.Contains($req)) {
            $allPresent = $false
            $missingSizes += $req
        }
    }

    $sizeList = ($presentSizes | Sort-Object) -join ', '
    if ($allPresent) {
        Write-Host "  [PASS] $FilePath" -ForegroundColor Green
        Write-Host "         Total frames: $count (Sizes: $sizeList px)" -ForegroundColor DarkGray
        return $true
    } else {
        Write-Host "  [FAIL] $FilePath is missing required sizes: $($missingSizes -join ', ') (Found: $sizeList)" -ForegroundColor Red
        return $false
    }
}

foreach ($ico in $IcoTargets) {
    if (-not (Test-IcoFile -FilePath $ico -RequiredSizes $RequiredIcoSizes)) {
        $Failures++
    }
}

# ------------------------------------------------------------------------------
# 2. Validate Installer BMP Assets (NSIS & WiX)
# ------------------------------------------------------------------------------
Write-Host "`n[2/3] Validating Installer BMP Dimensions and Format..." -ForegroundColor Yellow

$BmpTargets = @(
    @{ Path = (Join-Path $RepoRoot "apps/desktop/src-tauri/icons/nsis-header.bmp");  Width = 150; Height = 57  },
    @{ Path = (Join-Path $RepoRoot "apps/desktop/src-tauri/icons/nsis-sidebar.bmp"); Width = 164; Height = 314 },
    @{ Path = (Join-Path $RepoRoot "apps/desktop/src-tauri/icons/wix-banner.bmp");   Width = 493; Height = 58  },
    @{ Path = (Join-Path $RepoRoot "apps/desktop/src-tauri/icons/wix-dialog.bmp");   Width = 493; Height = 312 }
)

function Test-BmpFile([string]$FilePath, [int]$ExpectedW, [int]$ExpectedH) {
    if (-not (Test-Path -LiteralPath $FilePath)) {
        Write-Host "  [FAIL] Missing BMP file: $FilePath" -ForegroundColor Red
        return $false
    }

    $bytes = [System.IO.File]::ReadAllBytes($FilePath)
    if ($bytes.Length -lt 54) {
        Write-Host "  [FAIL] BMP header truncated: $FilePath" -ForegroundColor Red
        return $false
    }

    $magic = [System.Text.Encoding]::ASCII.GetString($bytes, 0, 2)
    if ($magic -ne 'BM') {
        Write-Host "  [FAIL] Invalid BMP magic '$magic' (expected 'BM'): $FilePath" -ForegroundColor Red
        return $false
    }

    $headerSize = [System.BitConverter]::ToUInt32($bytes, 14)
    $width = [System.BitConverter]::ToInt32($bytes, 18)
    $height = [System.BitConverter]::ToInt32($bytes, 22)
    $planes = [System.BitConverter]::ToUInt16($bytes, 26)
    $bpp = [System.BitConverter]::ToUInt16($bytes, 28)
    $compression = [System.BitConverter]::ToUInt32($bytes, 30)

    # Positive height is bottom-up (standard), negative is top-down
    $absHeight = [Math]::Abs($height)

    $errors = @()
    if ($width -ne $ExpectedW -or $absHeight -ne $ExpectedH) {
        $errors += "Dimensions ${width}x${absHeight} do not match expected ${ExpectedW}x${ExpectedH}"
    }
    if ($bpp -ne 24 -and $bpp -ne 32) {
        $errors += "Bit depth $bpp bpp is not 24 or 32"
    }
    if ($compression -ne 0) {
        $errors += "Compression $compression is not 0 (BI_RGB uncompressed)"
    }

    if ($errors.Count -eq 0) {
        Write-Host "  [PASS] $FilePath (${width}x${absHeight}, ${bpp}bpp, uncompressed)" -ForegroundColor Green
        return $true
    } else {
        Write-Host "  [FAIL] $FilePath : $($errors -join '; ')" -ForegroundColor Red
        return $false
    }
}

foreach ($bmp in $BmpTargets) {
    if (-not (Test-BmpFile -FilePath $bmp.Path -ExpectedW $bmp.Width -ExpectedH $bmp.Height)) {
        $Failures++
    }
}

# ------------------------------------------------------------------------------
# 2b. Installer Artwork Layout Invariants
#
# Light structural checks for INST-001 (navy calligraphy confined to the
# left panel, flat light text areas, no latin wordmark, marks confined to
# their zones). The full gates, WCAG contrast checks and the layout-accurate
# dialog renders live in tools/check-installer-artwork.ps1.
# ------------------------------------------------------------------------------
Write-Host "`n[2b] Validating Installer Artwork Layout Invariants..." -ForegroundColor Yellow

Add-Type -AssemblyName System.Drawing

# Fast 24bpp pixel buffer via LockBits (top-down rows, index = y*stride + x*3, BGR).
function Get-ArtworkPixelData {
    param([string]$Path)
    $bytes = [System.IO.File]::ReadAllBytes($Path)
    $stream = [System.IO.MemoryStream]::new($bytes)
    $loaded = [System.Drawing.Bitmap]::FromStream($stream)
    $rect = [System.Drawing.Rectangle]::new(0, 0, $loaded.Width, $loaded.Height)
    $bd = $loaded.LockBits($rect, [System.Drawing.Imaging.ImageLockMode]::ReadOnly, [System.Drawing.Imaging.PixelFormat]::Format24bppRgb)
    $px = [byte[]]::new([Math]::Abs($bd.Stride) * $bd.Height)
    [System.Runtime.InteropServices.Marshal]::Copy($bd.Scan0, $px, 0, $px.Length)
    $loaded.UnlockBits($bd)
    $loaded.Dispose()
    $stream.Dispose()
    return @{ Width = $bd.Width; Height = $bd.Height; Stride = $bd.Stride; Pixels = $px }
}

function Test-ArtworkLayout {
    param(
        [string]$Name,
        [scriptblock]$Probe
    )
    $data = Get-ArtworkPixelData -Path (Join-Path $RepoRoot "apps/desktop/src-tauri/icons/$Name")
    $detail = & $Probe $data
    if ($detail.Pass) {
        Write-Host "  [PASS] $Name : $($detail.Text)" -ForegroundColor Green
    } else {
        Write-Host "  [FAIL] $Name : $($detail.Text)" -ForegroundColor Red
        $script:Failures++
    }
}

# wix-dialog.bmp: flat light text area (x >= 170) and navy left panel without
# light leaks or a white latin wordmark.
Test-ArtworkLayout -Name 'wix-dialog.bmp' -Probe {
    param($d)
    $deviants = 0
    for ($y = 0; $y -lt 312; $y++) {
        $row = $y * $d.Stride
        for ($x = 170; $x -le 489; $x++) {
            $i = $row + $x * 3
            if ([Math]::Abs($d.Pixels[$i+2] - 251) -gt 8 -or [Math]::Abs($d.Pixels[$i+1] - 251) -gt 8 -or [Math]::Abs($d.Pixels[$i] - 253) -gt 8) { $deviants++ }
        }
    }
    $navy = 0; $white = 0; $tot = 0
    for ($y = 0; $y -lt 312; $y++) {
        $row = $y * $d.Stride
        for ($x = 0; $x -le 158; $x++) {
            $i = $row + $x * 3
            $r = [int]$d.Pixels[$i+2]; $g = [int]$d.Pixels[$i+1]; $b = [int]$d.Pixels[$i]
            $tot++
            if ([Math]::Abs($r-11) -le 20 -and [Math]::Abs($g-26) -le 20 -and [Math]::Abs($b-78) -le 20) { $navy++ }
            if ($r -ge 200 -and $g -ge 200 -and $b -ge 200) { $white++ }
        }
    }
    $pass = ($deviants -eq 0) -and ($navy / $tot -ge 0.60) -and ($white / $tot -le 0.002)
    @{ Pass = $pass; Text = "text zone deviants=$deviants (0), left panel navy={0:P1} (>=60%), white={1:P3} (<=0.2%)" -f ($navy/$tot), ($white/$tot) }
}

# wix-banner.bmp: flat light everywhere except the brand mark zone (x >= 415).
Test-ArtworkLayout -Name 'wix-banner.bmp' -Probe {
    param($d)
    $outOfZone = 0; $markPx = 0
    for ($y = 0; $y -lt 58; $y++) {
        $row = $y * $d.Stride
        for ($x = 0; $x -le 492; $x++) {
            $i = $row + $x * 3
            if ([Math]::Abs($d.Pixels[$i+2] - 251) -gt 8 -or [Math]::Abs($d.Pixels[$i+1] - 251) -gt 8 -or [Math]::Abs($d.Pixels[$i] - 253) -gt 8) {
                if ($x -ge 415) { $markPx++ } else { $outOfZone++ }
            }
        }
    }
    @{ Pass = ($outOfZone -eq 0 -and $markPx -gt 0); Text = "deviants outside mark zone=$outOfZone (0), mark px=$markPx (>0)" }
}

# nsis-header.bmp: flat white matching the MUI2 white header, mark only at x >= 95.
Test-ArtworkLayout -Name 'nsis-header.bmp' -Probe {
    param($d)
    $outOfZone = 0; $markPx = 0
    for ($y = 0; $y -lt 57; $y++) {
        $row = $y * $d.Stride
        for ($x = 0; $x -le 149; $x++) {
            $i = $row + $x * 3
            $r = [int]$d.Pixels[$i+2]; $g = [int]$d.Pixels[$i+1]; $b = [int]$d.Pixels[$i]
            if (-not ([Math]::Abs($r-255) -le 8 -and [Math]::Abs($g-255) -le 8 -and [Math]::Abs($b-255) -le 8)) {
                if ($x -ge 95) { $markPx++ } else { $outOfZone++ }
            }
        }
    }
    @{ Pass = ($outOfZone -eq 0 -and $markPx -gt 0); Text = "non-white outside mark zone=$outOfZone (0), mark px=$markPx (>0)" }
}

# nsis-sidebar.bmp: navy-dominant calligraphy panel, no white wordmark, teal present.
Test-ArtworkLayout -Name 'nsis-sidebar.bmp' -Probe {
    param($d)
    $navy = 0; $white = 0; $teal = 0; $tot = 164 * 314
    for ($y = 0; $y -lt 314; $y++) {
        $row = $y * $d.Stride
        for ($x = 0; $x -le 163; $x++) {
            $i = $row + $x * 3
            $r = [int]$d.Pixels[$i+2]; $g = [int]$d.Pixels[$i+1]; $b = [int]$d.Pixels[$i]
            if ([Math]::Abs($r-11) -le 20 -and [Math]::Abs($g-26) -le 20 -and [Math]::Abs($b-78) -le 20) { $navy++ }
            if ($r -ge 200 -and $g -ge 200 -and $b -ge 200) { $white++ }
            if ($g -ge 100 -and $b -ge 100 -and $r -le 90) { $teal++ }
        }
    }
    @{ Pass = (($navy/$tot) -ge 0.85 -and ($white/$tot) -le 0.001 -and ($teal/$tot) -ge 0.01); Text = "navy={0:P1} (>=85%), white={1:P3} (<=0.1%), teal={2:P1} (>=1%)" -f ($navy/$tot), ($white/$tot), ($teal/$tot) }
}

# ------------------------------------------------------------------------------
# 3. Validate tauri.conf.json Paths
# ------------------------------------------------------------------------------
Write-Host "`n[3/3] Validating tauri.conf.json Windows Bundle Configurations..." -ForegroundColor Yellow

$TauriConfPath = Join-Path $RepoRoot "apps/desktop/src-tauri/tauri.conf.json"
$SrcTauriDir = Join-Path $RepoRoot "apps/desktop/src-tauri"

if (-not (Test-Path -LiteralPath $TauriConfPath)) {
    Write-Host "  [FAIL] Missing tauri.conf.json: $TauriConfPath" -ForegroundColor Red
    $Failures++
} else {
    $raw = Get-Content -LiteralPath $TauriConfPath -Raw -Encoding UTF8
    $conf = $raw | ConvertFrom-Json

    # Check icons list
    if ($conf.bundle.icon) {
        foreach ($iconRel in $conf.bundle.icon) {
            $target = [System.IO.Path]::GetFullPath((Join-Path $SrcTauriDir $iconRel))
            if (-not (Test-Path -LiteralPath $target)) {
                Write-Host "  [FAIL] bundle.icon path not found: $iconRel -> $target" -ForegroundColor Red
                $Failures++
            } else {
                Write-Host "  [PASS] bundle.icon: $iconRel" -ForegroundColor Green
            }
        }
    } else {
        Write-Host "  [FAIL] bundle.icon missing in tauri.conf.json" -ForegroundColor Red
        $Failures++
    }

    # Check windows nsis
    $nsis = $conf.bundle.windows.nsis
    if ($nsis) {
        $nsisChecks = @(
            @{ Name = "installerIcon"; Path = $nsis.installerIcon },
            @{ Name = "headerImage";   Path = $nsis.headerImage },
            @{ Name = "sidebarImage";  Path = $nsis.sidebarImage },
            @{ Name = "installerHooks"; Path = $nsis.installerHooks }
        )
        foreach ($item in $nsisChecks) {
            if (-not $item.Path) {
                Write-Host "  [FAIL] bundle.windows.nsis.$($item.Name) is not specified" -ForegroundColor Red
                $Failures++
            } else {
                $target = [System.IO.Path]::GetFullPath((Join-Path $SrcTauriDir $item.Path))
                if (-not (Test-Path -LiteralPath $target)) {
                    Write-Host "  [FAIL] bundle.windows.nsis.$($item.Name) not found: $($item.Path) -> $target" -ForegroundColor Red
                    $Failures++
                } else {
                    Write-Host "  [PASS] bundle.windows.nsis.$($item.Name): $($item.Path)" -ForegroundColor Green
                }
            }
        }
    } else {
        Write-Host "  [FAIL] bundle.windows.nsis section missing in tauri.conf.json" -ForegroundColor Red
        $Failures++
    }

    # Check windows wix
    $wix = $conf.bundle.windows.wix
    if ($wix) {
        $wixChecks = @(
            @{ Name = "bannerPath";      Path = $wix.bannerPath },
            @{ Name = "dialogImagePath"; Path = $wix.dialogImagePath }
        )
        foreach ($item in $wixChecks) {
            if (-not $item.Path) {
                Write-Host "  [FAIL] bundle.windows.wix.$($item.Name) is not specified" -ForegroundColor Red
                $Failures++
            } else {
                $target = [System.IO.Path]::GetFullPath((Join-Path $SrcTauriDir $item.Path))
                if (-not (Test-Path -LiteralPath $target)) {
                    Write-Host "  [FAIL] bundle.windows.wix.$($item.Name) not found: $($item.Path) -> $target" -ForegroundColor Red
                    $Failures++
                } else {
                    Write-Host "  [PASS] bundle.windows.wix.$($item.Name): $($item.Path)" -ForegroundColor Green
                }
            }
        }
        if ($wix.fragmentPaths) {
            foreach ($frag in $wix.fragmentPaths) {
                $target = [System.IO.Path]::GetFullPath((Join-Path $SrcTauriDir $frag))
                if (-not (Test-Path -LiteralPath $target)) {
                    Write-Host "  [FAIL] bundle.windows.wix.fragmentPaths not found: $frag -> $target" -ForegroundColor Red
                    $Failures++
                } else {
                    Write-Host "  [PASS] bundle.windows.wix.fragmentPaths: $frag" -ForegroundColor Green
                }
            }
        } else {
            Write-Host "  [FAIL] bundle.windows.wix.fragmentPaths missing in tauri.conf.json" -ForegroundColor Red
            $Failures++
        }
    } else {
        Write-Host "  [FAIL] bundle.windows.wix section missing in tauri.conf.json" -ForegroundColor Red
        $Failures++
    }

    # Config sanity check: base tauri.conf.json must not declare bundle.resources
    if ($conf.bundle.PSObject.Properties['resources']) {
        Write-Host "  [FAIL] base tauri.conf.json should not declare bundle.resources (use installer/tauri.bundle-resources.json)" -ForegroundColor Red
        $Failures++
    } else {
        Write-Host "  [PASS] base tauri.conf.json has no bundle.resources (clean for fresh checkouts)" -ForegroundColor Green
    }

    # Validate release overlay installer/tauri.bundle-resources.json when present
    $OverlayPath = Join-Path $RepoRoot "installer/tauri.bundle-resources.json"
    if (Test-Path -LiteralPath $OverlayPath) {
        try {
            $overlayRaw = Get-Content -LiteralPath $OverlayPath -Raw -Encoding UTF8
            $overlayConf = $overlayRaw | ConvertFrom-Json
            if ($overlayConf.bundle.resources) {
                Write-Host "  [PASS] Release overlay installer/tauri.bundle-resources.json valid with bundle.resources" -ForegroundColor Green
            } else {
                Write-Host "  [FAIL] installer/tauri.bundle-resources.json missing bundle.resources block" -ForegroundColor Red
                $Failures++
            }
        } catch {
            Write-Host "  [FAIL] installer/tauri.bundle-resources.json is invalid JSON: $_" -ForegroundColor Red
            $Failures++
        }
    }
}

Write-Host "`n------------------------------------------------------------" -ForegroundColor Cyan
if ($Failures -eq 0) {
    Write-Host "[SUCCESS] All brand assets and configuration paths verified! (Exit 0)" -ForegroundColor Green
    exit 0
} else {
    Write-Host "[ERROR] Validation failed with $Failures error(s). (Exit 1)" -ForegroundColor Red
    exit 1
}

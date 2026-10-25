[CmdletBinding()]
param (
    [Parameter(Mandatory = $true, ValueFromRemainingArguments = $true, Position = 0)]
    [string[]]$Path,

    [Parameter(Mandatory = $false)]
    [string]$PngOutDir
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

Add-Type -AssemblyName System.Drawing

Write-Host "============================================================" -ForegroundColor Cyan
Write-Host " SAYL - Installer Binary Icon Brand Verifier" -ForegroundColor Cyan
Write-Host "============================================================" -ForegroundColor Cyan

$OverallFailures = 0

# Thresholds:
# Brand icon has dominant Teal (#00b6bd) and Navy (#0b1a4e) pixels (~95-100%).
# Wrong default installer icon has grayscale dominance (>40%) and saturated red (CD arrow).
$MinBrandPercentage = 45.0
$MaxRedPercentage = 1.5

foreach ($filePath in $Path) {
    Write-Host "`nChecking: $filePath" -ForegroundColor Yellow

    if (-not (Test-Path -LiteralPath $filePath)) {
        Write-Host "  [FAIL] File does not exist: $filePath" -ForegroundColor Red
        $OverallFailures++
        continue
    }

    $resolvedPath = (Resolve-Path -LiteralPath $filePath).Path

    $icon = $null
    try {
        $icon = [System.Drawing.Icon]::ExtractAssociatedIcon($resolvedPath)
    } catch {
        Write-Host "  [FAIL] ExtractAssociatedIcon threw an exception: $_" -ForegroundColor Red
        $OverallFailures++
        continue
    }

    if (-not $icon) {
        Write-Host "  [FAIL] Could not extract associated icon from $resolvedPath" -ForegroundColor Red
        $OverallFailures++
        continue
    }

    $bmp = $icon.ToBitmap()
    $w = $bmp.Width
    $h = $bmp.Height

    # Save PNG representation
    $outDir = if ($PngOutDir) {
        $PngOutDir
    } else {
        [System.IO.Path]::GetDirectoryName($resolvedPath)
    }
    if (-not (Test-Path -LiteralPath $outDir)) {
        New-Item -ItemType Directory -Path $outDir -Force | Out-Null
    }
    $baseName = [System.IO.Path]::GetFileNameWithoutExtension($resolvedPath)
    $ext = [System.IO.Path]::GetExtension($resolvedPath).TrimStart('.')
    $pngPath = Join-Path $outDir "${baseName}_${ext}_icon.png"

    try {
        $bmp.Save($pngPath, [System.Drawing.Imaging.ImageFormat]::Png)
        Write-Host "  Extracted icon saved to: $pngPath (${w}x${h} px)" -ForegroundColor DarkGray
    } catch {
        Write-Host "  [WARNING] Could not save extracted icon PNG: $_" -ForegroundColor DarkGray
    }

    # Analyze pixels
    $totalVisible = 0
    $tealCount = 0
    $navyCount = 0
    $redCount = 0
    $grayCount = 0

    for ($y = 0; $y -lt $h; $y++) {
        for ($x = 0; $x -lt $w; $x++) {
            $pixel = $bmp.GetPixel($x, $y)
            # Ignore transparent / near-transparent pixels
            if ($pixel.A -lt 32) {
                continue
            }
            $totalVisible++
            $r = [int]$pixel.R
            $g = [int]$pixel.G
            $b = [int]$pixel.B

            # Saturated red: red is high and dominant
            if ($r -gt 120 -and $r -gt ($g + 35) -and $r -gt ($b + 35)) {
                $redCount++
            }
            # Brand Teal: G and B are high and dominant over R
            elseif ($g -ge 70 -and $b -ge 70 -and $b -gt ($r + 15) -and $g -gt ($r + 15)) {
                $tealCount++
            }
            # Brand Navy: B is dominant, R and G are low/deep
            elseif ($b -gt $r -and $b -gt $g -and $b -ge 30 -and $r -le 80 -and $g -le 100) {
                $navyCount++
            }
            # Neutral gray
            elseif ([Math]::Abs($r - $g) -le 20 -and [Math]::Abs($g - $b) -le 20 -and [Math]::Abs($r - $b) -le 20) {
                $grayCount++
            }
        }
    }

    if ($totalVisible -eq 0) {
        Write-Host "  [FAIL] Icon has no visible (non-transparent) pixels." -ForegroundColor Red
        $OverallFailures++
        continue
    }

    $brandCount = $tealCount + $navyCount
    $brandPct = [Math]::Round(($brandCount / $totalVisible) * 100, 2)
    $tealPct  = [Math]::Round(($tealCount / $totalVisible) * 100, 2)
    $navyPct  = [Math]::Round(($navyCount / $totalVisible) * 100, 2)
    $redPct   = [Math]::Round(($redCount / $totalVisible) * 100, 2)
    $grayPct  = [Math]::Round(($grayCount / $totalVisible) * 100, 2)

    Write-Host "  Visible pixels : $totalVisible"
    Write-Host "  Brand (Teal+Navy) share : $brandPct% (Teal: $tealPct%, Navy: $navyPct%) [Min: $MinBrandPercentage%]"
    Write-Host "  Saturated red share     : $redPct% [Max: $MaxRedPercentage%]"
    Write-Host "  Neutral gray share      : $grayPct%"

    $passed = $true
    if ($brandPct -lt $MinBrandPercentage) {
        Write-Host "  [FAIL] Insufficient brand Teal/Navy share ($brandPct% < $MinBrandPercentage%)" -ForegroundColor Red
        $passed = $false
    }
    if ($redPct -gt $MaxRedPercentage) {
        Write-Host "  [FAIL] Saturated red exceeds brand tolerance ($redPct% > $MaxRedPercentage%)" -ForegroundColor Red
        $passed = $false
    }

    if ($passed) {
        Write-Host "  Status: PASS" -ForegroundColor Green
    } else {
        Write-Host "  Status: FAIL" -ForegroundColor Red
        $OverallFailures++
    }
}

Write-Host "`n------------------------------------------------------------" -ForegroundColor Cyan
if ($OverallFailures -eq 0) {
    Write-Host "[SUCCESS] All files passed icon brand validation. (Exit 0)" -ForegroundColor Green
    exit 0
} else {
    Write-Host "[ERROR] Icon brand validation failed for $OverallFailures target(s). (Exit 1)" -ForegroundColor Red
    exit 1
}

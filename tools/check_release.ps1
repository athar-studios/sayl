[CmdletBinding()]
param (
    [Parameter(Mandatory = $false)]
    [string]$ReleasesDir,

    [Parameter(Mandatory = $false)]
    [string]$Version
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

# ------------------------------------------------------------------------------
# 1. Resolve Root, Releases Directory & Version
# ------------------------------------------------------------------------------
$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path

if (-not $ReleasesDir) {
    $ReleasesDir = Join-Path $RepoRoot "releases"
} elseif (-not [System.IO.Path]::IsPathRooted($ReleasesDir)) {
    $ReleasesDir = Join-Path $RepoRoot $ReleasesDir
}

$VersionFile = Join-Path $RepoRoot "installer\version.txt"
if (-not $Version) {
    if (Test-Path -LiteralPath $VersionFile) {
        $Version = (Get-Content -LiteralPath $VersionFile -Raw).Trim()
    } else {
        Write-Host "[خطأ / ERROR] ملف الإصدار غير موجود: $VersionFile" -ForegroundColor Red
        Write-Host "[خطأ / ERROR] Version file not found: $VersionFile" -ForegroundColor Red
        exit 1
    }
}

Write-Host "============================================================" -ForegroundColor Cyan
Write-Host " SAYL ('سيل') - Release Artefacts Verification" -ForegroundColor Cyan
Write-Host " Target Version: $Version" -ForegroundColor Cyan
Write-Host " Releases Dir:   $ReleasesDir" -ForegroundColor Cyan
Write-Host "============================================================" -ForegroundColor Cyan

# Expected artefact filenames
$ExpectedFiles = @(
    "SAYL-Setup-x64-$Version.exe",
    "SAYL-Setup-x64-$Version.msi",
    "SAYL-$Version-source.zip",
    "SHA256SUMS.txt"
)

$IssuesFound = [System.Collections.Generic.List[string]]::new()

# ------------------------------------------------------------------------------
# 2. Check Releases Directory Existence
# ------------------------------------------------------------------------------
if (-not (Test-Path -LiteralPath $ReleasesDir -PathType Container)) {
    Write-Host "`n[خطأ / ERROR] مجلد الإصدارات غير موجود: $ReleasesDir" -ForegroundColor Red
    Write-Host "[خطأ / ERROR] Releases directory does not exist: $ReleasesDir" -ForegroundColor Red
    Write-Host "`nالمخرجات المطلوبة المفقودة / Missing Required Artefacts:" -ForegroundColor Yellow
    foreach ($file in $ExpectedFiles) {
        Write-Host "  [مفقود / MISSING] $file" -ForegroundColor Red
    }
    Write-Host "`n[فشل التحقق / VERIFICATION FAILED] لم يتم العثور على مجلد مخرجات الإصدار (ExitCode: 1)." -ForegroundColor Red
    exit 1
}

# ------------------------------------------------------------------------------
# 3. Check for Missing and Extra Files in releases/
# ------------------------------------------------------------------------------
$ExistingItems = @(Get-ChildItem -LiteralPath $ReleasesDir -File -ErrorAction SilentlyContinue)
$ExistingNames = @($ExistingItems | ForEach-Object { $_.Name })

if ($ExistingItems.Count -eq 0) {
    Write-Host "`n[خطأ / ERROR] مجلد الإصدارات فارغ تماماً: $ReleasesDir" -ForegroundColor Red
    Write-Host "[خطأ / ERROR] Releases directory is completely empty: $ReleasesDir" -ForegroundColor Red
    Write-Host "`nالمخرجات المطلوبة المفقودة / Missing Required Artefacts:" -ForegroundColor Yellow
    foreach ($file in $ExpectedFiles) {
        Write-Host "  [مفقود / MISSING] $file" -ForegroundColor Red
    }
    Write-Host "`n[فشل التحقق / VERIFICATION FAILED] لا توجد أي ملفات داخل مجلد الإصدارات (ExitCode: 1)." -ForegroundColor Red
    exit 1
}

Write-Host "`n[فحص وجود الملفات / Checking Expected Artefacts]:" -ForegroundColor Yellow
foreach ($expected in $ExpectedFiles) {
    $fullPath = Join-Path $ReleasesDir $expected
    if (Test-Path -LiteralPath $fullPath) {
        Write-Host "  [موجود / FOUND]   $expected" -ForegroundColor Green
    } else {
        Write-Host "  [مفقود / MISSING] $expected" -ForegroundColor Red
        $IssuesFound.Add("الملف المطلوب مفقود / Missing required file: $expected")
    }
}

# Check for unexpected extra files
Write-Host "`n[فحص الملفات الإضافية / Checking Unexpected Artefacts]:" -ForegroundColor Yellow
$HasExtra = $false
foreach ($existing in $ExistingNames) {
    if ($existing -notin $ExpectedFiles) {
        $HasExtra = $true
        Write-Host "  [ملف إضافي / EXTRA] $existing" -ForegroundColor Yellow
        $IssuesFound.Add("ملف غير متوقع في مجلد الإصدار / Unexpected file in releases directory: $existing")
    }
}
if (-not $HasExtra) {
    Write-Host "  [سليم / CLEAN] لا توجد أي ملفات إضافية غير متوقعة." -ForegroundColor Green
}

# ------------------------------------------------------------------------------
# 4. Verify Hashes Against SHA256SUMS.txt
# ------------------------------------------------------------------------------
$ShaFile = Join-Path $ReleasesDir "SHA256SUMS.txt"
if (Test-Path -LiteralPath $ShaFile) {
    Write-Host "`n[تدقيق التجزئات عبر SHA256SUMS.txt / Verifying SHA256 Checksums]:" -ForegroundColor Yellow
    $ShaLines = @(Get-Content -LiteralPath $ShaFile | Where-Object { -not [string]::IsNullOrWhiteSpace($_) })
    
    $ParsedSums = @{}
    foreach ($line in $ShaLines) {
        # Format: <hash>  <filename>
        $trimmed = $line.Trim()
        if ($trimmed -match '^([0-9a-fA-F]{64})\s+(.+)$') {
            $hash = $matches[1].ToLowerInvariant()
            $fileName = [System.IO.Path]::GetFileName($matches[2].Trim())
            $ParsedSums[$fileName] = $hash
        } else {
            $IssuesFound.Add("سطر تالف أو غير صالح في SHA256SUMS.txt / Corrupted line in SHA256SUMS.txt: '$line'")
        }
    }

    # Verify each non-checksum expected file
    $ArtefactsToCheck = $ExpectedFiles | Where-Object { $_ -ne "SHA256SUMS.txt" }
    foreach ($expected in $ArtefactsToCheck) {
        if (-not $ParsedSums.ContainsKey($expected)) {
            $IssuesFound.Add("الملف غير مدرج في جدول SHA256SUMS.txt / File not listed in SHA256SUMS.txt: $expected")
            Write-Host "  [غير مسجل / NOT LISTED] $expected" -ForegroundColor Red
            continue
        }

        $targetPath = Join-Path $ReleasesDir $expected
        if (Test-Path -LiteralPath $targetPath) {
            $expectedHash = $ParsedSums[$expected]
            $actualHash = (Get-FileHash -LiteralPath $targetPath -Algorithm SHA256).Hash.ToLowerInvariant()

            if ($expectedHash -eq $actualHash) {
                Write-Host "  [تطابق / MATCH]    $expected" -ForegroundColor Green
                Write-Host "                     Hash: $actualHash" -ForegroundColor DarkGray
            } else {
                Write-Host "  [عدم تطابق / MISMATCH] $expected" -ForegroundColor Red
                Write-Host "                         Expected: $expectedHash" -ForegroundColor Red
                Write-Host "                         Actual:   $actualHash" -ForegroundColor Red
                $IssuesFound.Add("عدم تطابق تجزئة SHA-256 للملف / SHA-256 mismatch for: $expected")
            }
        }
    }
} else {
    $IssuesFound.Add("ملف التجزئات مفقود / SHA256SUMS.txt is missing.")
}

# ------------------------------------------------------------------------------
# 5. Final Report & Exit Code
# ------------------------------------------------------------------------------
Write-Host "`n============================================================" -ForegroundColor Cyan
if ($IssuesFound.Count -gt 0) {
    Write-Host "[فشل التحقق / VERIFICATION FAILED] تم رصد المشاكل التالية / Issues detected:" -ForegroundColor Red
    foreach ($issue in $IssuesFound) {
        Write-Host "  - $issue" -ForegroundColor Red
    }
    Write-Host "============================================================" -ForegroundColor Cyan
    exit 1
} else {
    Write-Host "[نجاح كامل / VERIFICATION PASSED] جميع المخرجات الأربعة متطابقة وصحيحة!" -ForegroundColor Green
    Write-Host "All 4 release artefacts are present, clean, and cryptographically verified." -ForegroundColor Green
    Write-Host "============================================================" -ForegroundColor Cyan
    exit 0
}
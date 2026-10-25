<#
.SYNOPSIS
    Builds, validates, and packages the SAYL Chromium and Firefox extensions into distribution ZIPs.

.DESCRIPTION
    Validates manifest files (Manifest V3, permissions, icons, locales, pinned Chromium key /
    Firefox gecko ID), executes `node --check` across all JavaScript sources, cleans the dist
    directory, and packages both extensions with manifest.json at the ZIP root.
#>

[CmdletBinding()]
param ()

$ErrorActionPreference = "Stop"

# Assemblies required for ZIP packaging
Add-Type -AssemblyName System.IO.Compression
Add-Type -AssemblyName System.IO.Compression.FileSystem

$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$ChromiumDir = Join-Path $RepoRoot "extensions\browser\chromium"
$FirefoxDir = Join-Path $RepoRoot "extensions\browser\firefox"
$DistDir = Join-Path $RepoRoot "extensions\browser\dist"

$EXPECTED_CHROMIUM_ID = "cpigidmkbhcfkgljofmmpnfdhdaoldoj"
$EXPECTED_FIREFOX_ID = "sayl@atharstudios.com"
$REQUIRED_PERMISSIONS = @("nativeMessaging", "contextMenus", "notifications")

function Get-ChromiumExtensionId {
    param ([string]$Base64Key)

    if ([string]::IsNullOrWhiteSpace($Base64Key)) {
        throw "Pinned key is missing or empty."
    }

    $derBytes = [Convert]::FromBase64String($Base64Key)
    $sha256 = [System.Security.Cryptography.SHA256]::Create()
    try {
        $hash = $sha256.ComputeHash($derBytes)
    } finally {
        $sha256.Dispose()
    }

    $sb = New-Object System.Text.StringBuilder
    for ($i = 0; $i -lt 16; $i++) {
        $b = $hash[$i]
        $hi = [char](97 + [int](($b -shr 4) -band 0x0F))
        $lo = [char](97 + [int]($b -band 0x0F))
        [void]$sb.Append($hi)
        [void]$sb.Append($lo)
    }
    return $sb.ToString()
}

function Validate-ExtensionPackage {
    param (
        [string]$TargetBrowser, # "chromium" or "firefox"
        [string]$PackageDir
    )

    Write-Host "`n=== Validating $TargetBrowser extension in $PackageDir ===" -ForegroundColor Cyan

    if (-not (Test-Path -LiteralPath $PackageDir)) {
        throw "Package directory does not exist: $PackageDir"
    }

    # 1. Run `node --check` on all JS files in package
    $jsFiles = Get-ChildItem -LiteralPath $PackageDir -Recurse -File -Filter "*.js"
    if ($jsFiles.Count -eq 0) {
        throw "No JavaScript files found in $PackageDir"
    }

    foreach ($js in $jsFiles) {
        $relJs = $js.FullName.Substring($PackageDir.Length).TrimStart('\', '/')
        Write-Host "  [JS Check] node --check $relJs"
        & node --check $js.FullName
        if ($LASTEXITCODE -ne 0) {
            throw "JavaScript syntax check failed for: $($js.FullName)"
        }
    }

    # 2. Parse and validate manifest.json
    $manifestPath = Join-Path $PackageDir "manifest.json"
    if (-not (Test-Path -LiteralPath $manifestPath)) {
        throw "manifest.json missing in $PackageDir"
    }

    $manifestRaw = Get-Content -LiteralPath $manifestPath -Raw -Encoding UTF8
    try {
        $manifest = $manifestRaw | ConvertFrom-Json
    } catch {
        throw "Failed to parse JSON in $manifestPath : $_"
    }

    # Manifest version
    if ($manifest.manifest_version -ne 3) {
        throw "Expected manifest_version: 3, got: $($manifest.manifest_version) in $manifestPath"
    }

    # Version
    if ([string]::IsNullOrWhiteSpace($manifest.version)) {
        throw "Extension version is missing or empty in $manifestPath"
    }
    Write-Host "  [Manifest] manifest_version: 3, version: $($manifest.version)"

    # Background
    if ($null -eq $manifest.background) {
        throw "Missing 'background' section in $manifestPath"
    }
    if ($TargetBrowser -eq "chromium") {
        if ([string]::IsNullOrWhiteSpace($manifest.background.service_worker)) {
            throw "Chromium manifest requires 'background.service_worker'"
        }
        $swPath = Join-Path $PackageDir $manifest.background.service_worker
        if (-not (Test-Path -LiteralPath $swPath)) {
            throw "background.service_worker file not found: $swPath"
        }
    } elseif ($TargetBrowser -eq "firefox") {
        if ($null -eq $manifest.background.scripts -or $manifest.background.scripts.Count -eq 0) {
            throw "Firefox manifest requires 'background.scripts' array"
        }
        foreach ($script in $manifest.background.scripts) {
            $scriptPath = Join-Path $PackageDir $script
            if (-not (Test-Path -LiteralPath $scriptPath)) {
                throw "background.scripts file not found: $scriptPath"
            }
        }
    }

    # Action / browser_action
    $action = if ($manifest.action) { $manifest.action } else { $manifest.browser_action }
    if ($null -eq $action) {
        throw "Missing 'action' (or 'browser_action') section in $manifestPath"
    }
    if (-not [string]::IsNullOrWhiteSpace($action.default_popup)) {
        $popupPath = Join-Path $PackageDir $action.default_popup
        if (-not (Test-Path -LiteralPath $popupPath)) {
            throw "default_popup file not found: $popupPath"
        }
    }
    if ($action.default_icon) {
        $action.default_icon.psobject.properties | ForEach-Object {
            $iconPath = Join-Path $PackageDir $_.Value
            if (-not (Test-Path -LiteralPath $iconPath)) {
                throw "Action icon file not found: $iconPath"
            }
        }
    }

    # Icons
    if ($null -eq $manifest.icons) {
        throw "Missing 'icons' section in $manifestPath"
    }
    $manifest.icons.psobject.properties | ForEach-Object {
        $iconPath = Join-Path $PackageDir $_.Value
        if (-not (Test-Path -LiteralPath $iconPath)) {
            throw "Icon file not found: $iconPath"
        }
    }

    # Locales
    if ([string]::IsNullOrWhiteSpace($manifest.default_locale)) {
        throw "Missing 'default_locale' in $manifestPath"
    }
    $localesDir = Join-Path $PackageDir "_locales"
    if (-not (Test-Path -LiteralPath $localesDir)) {
        throw "Missing '_locales' directory in $PackageDir"
    }
    $defaultLocaleFile = Join-Path $localesDir "$($manifest.default_locale)\messages.json"
    if (-not (Test-Path -LiteralPath $defaultLocaleFile)) {
        throw "Default locale message file missing: $defaultLocaleFile"
    }
    # Check that ar and en locales both exist
    foreach ($lang in @("ar", "en")) {
        $langFile = Join-Path $localesDir "$lang\messages.json"
        if (-not (Test-Path -LiteralPath $langFile)) {
            throw "Locale message file missing: $langFile"
        }
        try {
            $null = (Get-Content -LiteralPath $langFile -Raw -Encoding UTF8) | ConvertFrom-Json
        } catch {
            throw "Locale JSON invalid in: $langFile"
        }
    }

    # Permissions
    if ($null -eq $manifest.permissions) {
        throw "Missing 'permissions' in $manifestPath"
    }
    foreach ($perm in $REQUIRED_PERMISSIONS) {
        if ($manifest.permissions -notcontains $perm) {
            throw "Required permission '$perm' missing from $manifestPath"
        }
    }

    # Browser-specific ID validation
    if ($TargetBrowser -eq "chromium") {
        if ([string]::IsNullOrWhiteSpace($manifest.key)) {
            throw "Chromium manifest must have a pinned 'key'."
        }
        $computedId = Get-ChromiumExtensionId -Base64Key $manifest.key
        if ($computedId -ne $EXPECTED_CHROMIUM_ID) {
            throw "Chromium extension ID mismatch! Expected '$EXPECTED_CHROMIUM_ID', computed '$computedId'"
        }
        Write-Host "  [Key] Pinned public key matches expected extension ID: $computedId"
    } elseif ($TargetBrowser -eq "firefox") {
        $gecko = $manifest.browser_specific_settings.gecko
        if ($null -eq $gecko) {
            throw "Firefox manifest missing 'browser_specific_settings.gecko'"
        }
        if ($gecko.id -ne $EXPECTED_FIREFOX_ID) {
            throw "Firefox add-on ID mismatch! Expected '$EXPECTED_FIREFOX_ID', got '$($gecko.id)'"
        }
        if ([string]::IsNullOrWhiteSpace($gecko.strict_min_version)) {
            throw "Firefox manifest missing 'browser_specific_settings.gecko.strict_min_version'"
        }
        Write-Host "  [Gecko ID] Add-on ID matches expected: $($gecko.id) (min: $($gecko.strict_min_version))"
    }

    Write-Host "  [OK] $TargetBrowser extension validation passed." -ForegroundColor Green
    return $manifest.version
}

function Build-ExtensionZip {
    param (
        [string]$TargetBrowser,
        [string]$PackageDir,
        [string]$Version,
        [string]$DistDirectory
    )

    $zipFileName = "sayl-$TargetBrowser-$Version.zip"
    $zipPath = Join-Path $DistDirectory $zipFileName
    $resolvedPackageDir = (Resolve-Path -LiteralPath $PackageDir).Path

    if (Test-Path -LiteralPath $zipPath) {
        Remove-Item -LiteralPath $zipPath -Force
    }

    Write-Host "`nPackaging $zipFileName from $resolvedPackageDir..." -ForegroundColor Cyan

    $zipStream = [System.IO.File]::Open($zipPath, [System.IO.FileMode]::Create)
    $archive = New-Object System.IO.Compression.ZipArchive($zipStream, [System.IO.Compression.ZipArchiveMode]::Create)

    try {
        $files = Get-ChildItem -LiteralPath $resolvedPackageDir -Recurse -File
        foreach ($file in $files) {
            $rel = $file.FullName.Substring($resolvedPackageDir.Length).TrimStart('\', '/') -replace '\\', '/'

            # Exclude VCS, OS metadata, node_modules, dist
            if ($rel -match '^(node_modules|dist|\.git)' -or $rel -match '\.DS_Store$' -or $rel -match 'Thumbs\.db$') {
                continue
            }

            [System.IO.Compression.ZipFileExtensions]::CreateEntryFromFile(
                $archive,
                $file.FullName,
                $rel,
                [System.IO.Compression.CompressionLevel]::Optimal
            ) | Out-Null
        }
    } finally {
        $archive.Dispose()
        $zipStream.Dispose()
    }

    # Verify created ZIP structure
    $verifyZip = [System.IO.Compression.ZipFile]::OpenRead($zipPath)
    try {
        $entries = $verifyZip.Entries
        $hasManifestAtRoot = $false
        foreach ($entry in $entries) {
            if ($entry.FullName -eq "manifest.json") {
                $hasManifestAtRoot = $true
            }
            if ($entry.FullName -match '^(node_modules|dist|\.git)') {
                throw "Unwanted directory found in ZIP: $($entry.FullName)"
            }
        }

        if (-not $hasManifestAtRoot) {
            throw "ZIP archive does not contain 'manifest.json' at the root: $zipPath"
        }
        if ($entries.Count -eq 0) {
            throw "ZIP archive is empty: $zipPath"
        }

        $fileInfo = Get-Item -LiteralPath $zipPath
        $sizeKb = [math]::Round($fileInfo.Length / 1KB, 2)
        Write-Host "  Artifact: $($fileInfo.FullName)" -ForegroundColor Green
        Write-Host "  Size: $($fileInfo.Length) bytes ($sizeKb KB)"
        Write-Host "  Total entries: $($entries.Count)"
        foreach ($e in $entries) {
            $eName = $e.FullName
            $eLen = $e.Length
            Write-Host "    - $eName ($eLen bytes)"
        }
    } finally {
        $verifyZip.Dispose()
    }

    return $zipPath
}

# --- Execution ---

Write-Host "=== SAYL Browser Extensions Builder ===" -ForegroundColor Magenta

# Validate packages
$chromiumVersion = Validate-ExtensionPackage -TargetBrowser "chromium" -PackageDir $ChromiumDir
$firefoxVersion  = Validate-ExtensionPackage -TargetBrowser "firefox" -PackageDir $FirefoxDir

# Clean only dist directory
if (-not (Test-Path -LiteralPath $DistDir)) {
    New-Item -ItemType Directory -Path $DistDir -Force | Out-Null
} else {
    Write-Host "`nCleaning dist directory: $DistDir" -ForegroundColor Yellow
    Get-ChildItem -LiteralPath $DistDir -File | Where-Object { $_.Name -like "sayl-*.zip" } | Remove-Item -Force
}

# Build ZIPs
$chromiumZip = Build-ExtensionZip -TargetBrowser "chromium" -PackageDir $ChromiumDir -Version $chromiumVersion -DistDirectory $DistDir
$firefoxZip  = Build-ExtensionZip -TargetBrowser "firefox"  -PackageDir $FirefoxDir  -Version $firefoxVersion  -DistDirectory $DistDir

Write-Host "`n=== Build Completed Successfully ===" -ForegroundColor Green
Write-Host "Chromium artifact: $chromiumZip"
Write-Host "Firefox artifact:  $firefoxZip"

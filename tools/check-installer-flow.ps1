[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$Failures = 0

Write-Host "============================================================" -ForegroundColor Cyan
Write-Host " SAYL - Installer Flow & Browser Extension Policy Validator" -ForegroundColor Cyan
Write-Host " Workspace Root: $RepoRoot" -ForegroundColor Cyan
Write-Host "============================================================" -ForegroundColor Cyan

function Assert-Condition([bool]$Condition, [string]$PassMsg, [string]$FailMsg) {
    if ($Condition) {
        Write-Host "  [PASS] $PassMsg" -ForegroundColor Green
    } else {
        Write-Host "  [FAIL] $FailMsg" -ForegroundColor Red
        $script:Failures++
    }
}

$InstallerFiles = @(
    @{ Path = "installer/nsis/hooks.nsh";      Kind = "NSIS_HOOK" },
    @{ Path = "installer/nsis/sayl.nsi";       Kind = "NSIS_SCRIPT" },
    @{ Path = "installer/wix/native-host.wxs"; Kind = "WIX_FRAGMENT" },
    @{ Path = "installer/wix/sayl.wxs";        Kind = "WIX_PACKAGE" }
)

# ------------------------------------------------------------------------------
# 1. Inspect Installer Files for Policy Compliance
# ------------------------------------------------------------------------------
Write-Host "`n[1/4] Inspecting NSIS & WiX Sources for Policy Requirements..." -ForegroundColor Yellow

foreach ($item in $InstallerFiles) {
    $relPath = $item.Path
    $fullPath = Join-Path $RepoRoot $relPath

    Write-Host "`n  Checking $relPath ($($item.Kind)):" -ForegroundColor Cyan

    if (-not (Test-Path -LiteralPath $fullPath)) {
        Write-Host "  [FAIL] File missing: $relPath" -ForegroundColor Red
        $Failures++
        continue
    }

    $content = Get-Content -LiteralPath $fullPath -Raw -Encoding UTF8

    # Assertion 1: No PowerShell invocation for end-users
    $hasPowerShell = ($content -match '(?i)\b(powershell|pwsh)(\.exe)?\b')
    Assert-Condition (-not $hasPowerShell) `
        "No PowerShell/pwsh invocations found (per-user clean execution)" `
        "Found PowerShell/pwsh invocation in $relPath (forbidden for end-users)"

    # Assertion 2: Both --register-host and --unregister-host present
    $hasRegister = ($content -match '--register-host')
    $hasUnregister = ($content -match '--unregister-host')
    Assert-Condition ($hasRegister -and $hasUnregister) `
        "Contains both --register-host and --unregister-host" `
        "Missing --register-host or --unregister-host in $relPath"

    # Assertion 3: No hardcoded extension ID passed to --register-host
    $hasHardcodedExtId = ($content -match '--register-host[^\r\n]*--extension-id')
    Assert-Condition (-not $hasHardcodedExtId) `
        "--register-host invoked with default pinned ID (no hardcoded argument)" `
        "--register-host has hardcoded --extension-id argument in $relPath"

    # Assertion 4: Staging destinations under LOCALAPPDATA for chromium and firefox
    $hasChromiumLocal = ($content -match '(?i)LOCALAPPDATA[%\\/]+Sayl\\extension\\chromium')
    $hasFirefoxLocal  = ($content -match '(?i)LOCALAPPDATA[%\\/]+Sayl\\extension\\firefox')
    Assert-Condition ($hasChromiumLocal -and $hasFirefoxLocal) `
        "Staging destinations present for Chromium and Firefox under %LOCALAPPDATA%\Sayl\extension" `
        "Missing Chromium or Firefox LocalAppData destination in $relPath"

    # Assertion 5: No inline manifest writing (allowed_origins / allowed_extensions)
    $hasInlineManifest = ($content -match 'allowed_origins|allowed_extensions')
    Assert-Condition (-not $hasInlineManifest) `
        "No inline allowed_origins/allowed_extensions manifest writing (bridge CLI owns manifests)" `
        "Found inline allowed_origins or allowed_extensions manifest writing in $relPath"

    # Assertion 6: No manual installer-side manifest writing (FileWrite of JSON)
    $hasFileWriteJson = ($content -match 'FileWrite[^\r\n]*["'']\s*[{"]')
    Assert-Condition (-not $hasFileWriteJson) `
        "No manual FileWrite JSON manifest composition" `
        "Found manual FileWrite JSON manifest composition in $relPath"

    # Assertion 7: Uninstall cleanup of LOCALAPPDATA\Sayl\extension and LOCALAPPDATA\Sayl\host
    $hasExtCleanup = ($content -match '(?i)(RMDir\s+/r|rd\s+/s\s+/q)[^\r\n]*LOCALAPPDATA[%\\/]+Sayl\\extension')
    $hasHostCleanup = ($content -match '(?i)(RMDir\s+/r|rd\s+/s\s+/q)[^\r\n]*LOCALAPPDATA[%\\/]+Sayl\\host')
    Assert-Condition ($hasExtCleanup -and $hasHostCleanup) `
        "Uninstall cleans up %LOCALAPPDATA%\Sayl\extension and %LOCALAPPDATA%\Sayl\host" `
        "Missing uninstall cleanup of LocalAppData extension or host directory in $relPath"

    # Assertion 8: Autostart Run\SAYL registry key not deleted by installer
    # The app owns HKCU\...\Run\SAYL, installer must not remove it unless proving ownership
    $deletesRunKey = ($content -match '(?i)(DeleteRegValue[^\r\n]*Run[^\r\n]*SAYL|<RegistryValue[^\r\n]*Run[^\r\n]*Action="remove")')
    Assert-Condition (-not $deletesRunKey) `
        "Preserves app-owned autostart key (no uninstall deletion of Run\SAYL)" `
        "Deletes HKCU\...\Run\SAYL on uninstall in $relPath (app owns autostart)"
}

# ------------------------------------------------------------------------------
# 2. Inspect Bundle Resources Overlay
# ------------------------------------------------------------------------------
Write-Host "`n[2/4] Validating installer/tauri.bundle-resources.json..." -ForegroundColor Yellow

$OverlayFile = Join-Path $RepoRoot "installer/tauri.bundle-resources.json"
if (-not (Test-Path -LiteralPath $OverlayFile)) {
    Write-Host "  [FAIL] Missing $OverlayFile" -ForegroundColor Red
    $Failures++
} else {
    $overlayJson = Get-Content -LiteralPath $OverlayFile -Raw -Encoding UTF8 | ConvertFrom-Json
    $res = $overlayJson.bundle.resources

    $hasBridge = $false
    $hasChromium = $false
    $hasFirefox = $false

    foreach ($prop in $res.PSObject.Properties) {
        $val = $prop.Value
        if ($val -like "*sayl-bridge.exe*") { $hasBridge = $true }
        if ($val -like "*extensions/browser/chromium/*") { $hasChromium = $true }
        if ($val -like "*extensions/browser/firefox/*") { $hasFirefox = $true }
    }

    Assert-Condition ($hasBridge) "Bundle overlay maps sayl-bridge.exe" "Bundle overlay missing sayl-bridge.exe"
    Assert-Condition ($hasChromium) "Bundle overlay maps extensions/browser/chromium/" "Bundle overlay missing chromium extension"
    Assert-Condition ($hasFirefox) "Bundle overlay maps extensions/browser/firefox/" "Bundle overlay missing firefox extension"
}

# ------------------------------------------------------------------------------
# 3. Inspect tools/release.ps1 Staging Logic
# ------------------------------------------------------------------------------
Write-Host "`n[3/4] Validating tools/release.ps1 Staging Pipeline..." -ForegroundColor Yellow

$ReleaseScript = Join-Path $RepoRoot "tools/release.ps1"
if (-not (Test-Path -LiteralPath $ReleaseScript)) {
    Write-Host "  [FAIL] Missing $ReleaseScript" -ForegroundColor Red
    $Failures++
} else {
    $releaseContent = Get-Content -LiteralPath $ReleaseScript -Raw -Encoding UTF8

    $stagesChromium = ($releaseContent -match 'extensions\\browser\\chromium')
    $stagesFirefox  = ($releaseContent -match 'extensions\\browser\\firefox')

    Assert-Condition ($stagesChromium -and $stagesFirefox) `
        "tools/release.ps1 stages both Chromium and Firefox browser extensions" `
        "tools/release.ps1 does not stage both Chromium and Firefox browser extensions"
}

# ------------------------------------------------------------------------------
# 4. Dry-Run Verification of tools/release.ps1
# ------------------------------------------------------------------------------
Write-Host "`n[4/4] Verifying tools/release.ps1 -DryRun Execution..." -ForegroundColor Yellow

try {
    $dryRunOutput = & pwsh -NoProfile -File $ReleaseScript -DryRun -SkipFrontend 2>&1
    $exitCode = $LASTEXITCODE

    Assert-Condition ($exitCode -eq 0) `
        "tools/release.ps1 -DryRun completed with exit code 0" `
        "tools/release.ps1 -DryRun failed with exit code $exitCode"

    $hasChromiumDry = ($dryRunOutput -match 'extensions/browser/chromium/\*\*')
    $hasFirefoxDry  = ($dryRunOutput -match 'extensions/browser/firefox/\*\*')
    Assert-Condition ($hasChromiumDry -and $hasFirefoxDry) `
        "Dry-run output confirms staging for both Chromium and Firefox" `
        "Dry-run output missing Chromium or Firefox staging output"
} catch {
    Write-Host "  [FAIL] Execution exception in release.ps1 -DryRun: $_" -ForegroundColor Red
    $Failures++
}

# ------------------------------------------------------------------------------
# Summary
# ------------------------------------------------------------------------------
Write-Host "`n------------------------------------------------------------" -ForegroundColor Cyan
if ($Failures -eq 0) {
    Write-Host "[SUCCESS] All installer flow policies verified successfully! (Exit 0)" -ForegroundColor Green
    exit 0
} else {
    Write-Host "[ERROR] Installer flow validation failed with $Failures error(s). (Exit 1)" -ForegroundColor Red
    exit 1
}

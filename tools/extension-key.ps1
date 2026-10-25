<#
.SYNOPSIS
    Manages the SAYL Chromium browser extension key pair and validates pinned extension IDs.

.DESCRIPTION
    Provides subcommands to generate a 2048-bit RSA private key outside the repository,
    display public key and computed 32-character extension ID, and verify consistency
    between manifest.json, build scripts, Rust bridge constants, and host manifests.

.PARAMETER Generate
    Generates a new 2048-bit RSA key pair at $KeyPath (outside the repo). Refuses to
    overwrite an existing key unless -Force is specified.

.PARAMETER Show
    Reads the private key at $KeyPath, prints public key and computed ID, and verifies
    it matches manifest.json.

.PARAMETER Verify
    Verifies that the manifest.json "key" derives the expected 32-character ID and that
    all hardcoded constants in Rust, build tools, and host manifests match. (Default)

.PARAMETER Force
    Allows overwriting an existing private key during -Generate.

.PARAMETER KeyPath
    Path to the private key file. Defaults to "$env:USERPROFILE\.sayl\extension-private-key.pem".
#>

[CmdletBinding(DefaultParameterSetName = "Verify")]
param (
    [Parameter(ParameterSetName = "Generate")]
    [switch]$Generate,

    [Parameter(ParameterSetName = "Show")]
    [switch]$Show,

    [Parameter(ParameterSetName = "Verify")]
    [switch]$Verify,

    [Parameter(ParameterSetName = "Generate")]
    [switch]$Force,

    [Parameter()]
    [string]$KeyPath = (Join-Path $env:USERPROFILE ".sayl\extension-private-key.pem")
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $false

$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$ManifestPath = Join-Path $RepoRoot "extensions\browser\chromium\manifest.json"
$HostManifestPath = Join-Path $RepoRoot "extensions\browser\host\com.sayl.downloader.json"
$BridgeInstallPath = Join-Path $RepoRoot "crates\bridge\src\install.rs"
$BuildExtensionsScript = Join-Path $RepoRoot "tools\build-browser-extensions.ps1"
$ExtensionsReadmePath = Join-Path $RepoRoot "extensions\browser\README.md"

function Find-OpenSsl {
    if ($env:OPENSSL_BIN -and (Test-Path -LiteralPath $env:OPENSSL_BIN)) {
        return $env:OPENSSL_BIN
    }
    $cmd = Get-Command "openssl" -ErrorAction SilentlyContinue
    if ($cmd) {
        return $cmd.Source
    }
    $candidates = @(
        "C:\Program Files\Git\usr\bin\openssl.exe",
        "C:\Program Files\OpenSSL-Win64\bin\openssl.exe",
        "C:\Program Files (x86)\OpenSSL-Win32\bin\openssl.exe",
        "C:\Program Files\Git\bin\openssl.exe"
    )
    foreach ($cand in $candidates) {
        if (Test-Path -LiteralPath $cand) {
            return $cand
        }
    }
    throw "OpenSSL executable not found. Please install Git for Windows or set OPENSSL_BIN."
}

function Get-DerPublicKeyBytes {
    param ([string]$PrivateKeyPath)

    $openSsl = Find-OpenSsl
    $tempPub = [System.IO.Path]::GetTempFileName()
    $errLog = [System.IO.Path]::GetTempFileName()
    try {
        $proc = Start-Process -FilePath $openSsl -ArgumentList @("rsa", "-in", $PrivateKeyPath, "-pubout", "-outform", "DER", "-out", $tempPub) -NoNewWindow -Wait -PassThru -RedirectStandardError $errLog
        if ($proc.ExitCode -ne 0 -or -not (Test-Path -LiteralPath $tempPub)) {
            $errMsg = if (Test-Path -LiteralPath $errLog) { Get-Content -LiteralPath $errLog -Raw } else { "" }
            throw "Failed to extract public key from $PrivateKeyPath via OpenSSL. ExitCode: $($proc.ExitCode). $errMsg"
        }
        return [System.IO.File]::ReadAllBytes($tempPub)
    } finally {
        if (Test-Path -LiteralPath $tempPub) {
            Remove-Item -LiteralPath $tempPub -Force -ErrorAction SilentlyContinue
        }
        if (Test-Path -LiteralPath $errLog) {
            Remove-Item -LiteralPath $errLog -Force -ErrorAction SilentlyContinue
        }
    }
}

function Get-ExtensionIdFromDerBytes {
    param ([byte[]]$DerBytes)

    if ($null -eq $DerBytes -or $DerBytes.Length -eq 0) {
        throw "DER bytes cannot be empty."
    }
    $sha256 = [System.Security.Cryptography.SHA256]::Create()
    try {
        $hash = $sha256.ComputeHash($DerBytes)
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

function Get-ExtensionIdFromBase64Key {
    param ([string]$Base64Key)

    if ([string]::IsNullOrWhiteSpace($Base64Key)) {
        throw "Base64 key cannot be empty."
    }
    $derBytes = [Convert]::FromBase64String($Base64Key.Trim())
    return Get-ExtensionIdFromDerBytes -DerBytes $derBytes
}

if ($Generate) {
    if ((Test-Path -LiteralPath $KeyPath) -and -not $Force) {
        Write-Error "Private key already exists at: $KeyPath. Use -Force to overwrite."
        exit 1
    }

    $parentDir = Split-Path -Parent $KeyPath
    if (-not (Test-Path -LiteralPath $parentDir)) {
        New-Item -ItemType Directory -Path $parentDir -Force | Out-Null
    }

    $openSsl = Find-OpenSsl
    $errLog = [System.IO.Path]::GetTempFileName()
    try {
        $proc = Start-Process -FilePath $openSsl -ArgumentList @("genrsa", "-out", $KeyPath, "2048") -NoNewWindow -Wait -PassThru -RedirectStandardError $errLog
        if ($proc.ExitCode -ne 0 -or -not (Test-Path -LiteralPath $KeyPath)) {
            $errMsg = if (Test-Path -LiteralPath $errLog) { Get-Content -LiteralPath $errLog -Raw } else { "" }
            Write-Error "OpenSSL failed to generate RSA private key. ExitCode: $($proc.ExitCode). $errMsg"
            exit 1
        }
    } finally {
        if (Test-Path -LiteralPath $errLog) {
            Remove-Item -LiteralPath $errLog -Force -ErrorAction SilentlyContinue
        }
    }

    # Restrict permissions on Windows to current user only
    try {
        $acl = Get-Acl -LiteralPath $KeyPath
        $acl.SetAccessRuleProtection($true, $false)
        $currentUser = [System.Security.Principal.WindowsIdentity]::GetCurrent().Name
        $rule = New-Object System.Security.AccessControl.FileSystemAccessRule($currentUser, "FullControl", "Allow")
        $acl.SetAccessRule($rule)
        Set-Acl -LiteralPath $KeyPath -AclObject $acl
    } catch {
        Write-Warning "Could not restrict file ACL: $_"
    }

    $derBytes = Get-DerPublicKeyBytes -PrivateKeyPath $KeyPath
    $base64Key = [Convert]::ToBase64String($derBytes)
    $extensionId = Get-ExtensionIdFromDerBytes -DerBytes $derBytes

    Write-Host "Generated new RSA-2048 extension key pair successfully." -ForegroundColor Green
    Write-Host "Private Key Path : $KeyPath"
    Write-Host "Extension ID     : $extensionId"
    Write-Host "Public Key Length: $($derBytes.Length) bytes ($($base64Key.Length) base64 chars)"
    Write-Host "Public Key (base64):"
    Write-Host $base64Key
    exit 0
}

if ($Show) {
    if (-not (Test-Path -LiteralPath $KeyPath)) {
        Write-Error "Private key file not found at: $KeyPath"
        exit 1
    }

    $derBytes = Get-DerPublicKeyBytes -PrivateKeyPath $KeyPath
    $base64Key = [Convert]::ToBase64String($derBytes)
    $extensionId = Get-ExtensionIdFromDerBytes -DerBytes $derBytes

    Write-Host "Private Key Path : $KeyPath"
    Write-Host "Extension ID     : $extensionId"
    Write-Host "Public Key Length: $($derBytes.Length) bytes ($($base64Key.Length) base64 chars)"
    Write-Host "Public Key (base64):"
    Write-Host $base64Key

    if (Test-Path -LiteralPath $ManifestPath) {
        $manifestJson = Get-Content -LiteralPath $ManifestPath -Raw -Encoding UTF8 | ConvertFrom-Json
        if ($manifestJson.key -ne $base64Key) {
            Write-Error "Mismatch: Key in manifest.json does NOT match the key at $KeyPath!"
            exit 1
        }
        Write-Host "Verified: Public key matches manifest.json ($ManifestPath)" -ForegroundColor Green
    } else {
        Write-Warning "manifest.json not found at $ManifestPath; skipped manifest comparison."
    }
    exit 0
}

# Default mode: Verify
Write-Host "=== Verifying Extension ID Consistency ===" -ForegroundColor Cyan

$mismatches = [System.Collections.Generic.List[string]]::new()

# 1. Manifest public key & derived ID
if (-not (Test-Path -LiteralPath $ManifestPath)) {
    Write-Error "manifest.json not found: $ManifestPath"
    exit 1
}

$manifestRaw = Get-Content -LiteralPath $ManifestPath -Raw -Encoding UTF8
$manifestJson = $manifestRaw | ConvertFrom-Json
$manifestKey = $manifestJson.key

if ([string]::IsNullOrWhiteSpace($manifestKey)) {
    $mismatches.Add("manifest.json has missing or empty 'key' field.")
    $expectedId = $null
} else {
    try {
        $expectedId = Get-ExtensionIdFromBase64Key -Base64Key $manifestKey
        Write-Host "  [PASS] manifest.json key -> ID: $expectedId" -ForegroundColor Green
    } catch {
        $mismatches.Add("Failed to derive ID from manifest.json 'key': $_")
        $expectedId = $null
    }
}

if ($expectedId) {
    # 2. tools/build-browser-extensions.ps1
    if (Test-Path -LiteralPath $BuildExtensionsScript) {
        $buildScriptContent = Get-Content -LiteralPath $BuildExtensionsScript -Raw -Encoding UTF8
        if ($buildScriptContent -match '\$EXPECTED_CHROMIUM_ID\s*=\s*"([^"]+)"') {
            $foundId = $matches[1]
            if ($foundId -eq $expectedId) {
                Write-Host "  [PASS] tools/build-browser-extensions.ps1 EXPECTED_CHROMIUM_ID matches ($foundId)" -ForegroundColor Green
            } else {
                $mismatches.Add("tools/build-browser-extensions.ps1 EXPECTED_CHROMIUM_ID ($foundId) != manifest ID ($expectedId)")
            }
        } else {
            $mismatches.Add("tools/build-browser-extensions.ps1: `$EXPECTED_CHROMIUM_ID constant not found.")
        }
    } else {
        $mismatches.Add("File not found: $BuildExtensionsScript")
    }

    # 3. crates/bridge/src/install.rs
    if (Test-Path -LiteralPath $BridgeInstallPath) {
        $bridgeContent = Get-Content -LiteralPath $BridgeInstallPath -Raw -Encoding UTF8
        if ($bridgeContent -match 'pub const PINNED_CHROMIUM_EXTENSION_ID:\s*&str\s*=\s*"([^"]+)";') {
            $foundId = $matches[1]
            if ($foundId -eq $expectedId) {
                Write-Host "  [PASS] crates/bridge/src/install.rs PINNED_CHROMIUM_EXTENSION_ID matches ($foundId)" -ForegroundColor Green
            } else {
                $mismatches.Add("crates/bridge/src/install.rs PINNED_CHROMIUM_EXTENSION_ID ($foundId) != manifest ID ($expectedId)")
            }
        } else {
            $mismatches.Add("crates/bridge/src/install.rs: PINNED_CHROMIUM_EXTENSION_ID constant not found.")
        }

        # Check origin in tests inside install.rs
        $expectedOrigin = "chrome-extension://$expectedId/"
        if (-not $bridgeContent.Contains($expectedOrigin)) {
            $mismatches.Add("crates/bridge/src/install.rs does not contain expected origin: $expectedOrigin")
        } else {
            Write-Host "  [PASS] crates/bridge/src/install.rs contains origin ($expectedOrigin)" -ForegroundColor Green
        }
    } else {
        $mismatches.Add("File not found: $BridgeInstallPath")
    }

    # 4. extensions/browser/host/com.sayl.downloader.json
    if (Test-Path -LiteralPath $HostManifestPath) {
        $hostManifestRaw = Get-Content -LiteralPath $HostManifestPath -Raw -Encoding UTF8
        $hostJson = $hostManifestRaw | ConvertFrom-Json
        $expectedOrigin = "chrome-extension://$expectedId/"
        if ($hostJson.allowed_origins -contains $expectedOrigin) {
            Write-Host "  [PASS] extensions/browser/host/com.sayl.downloader.json allowed_origins matches ($expectedOrigin)" -ForegroundColor Green
        } else {
            $mismatches.Add("extensions/browser/host/com.sayl.downloader.json allowed_origins missing: $expectedOrigin")
        }
    } else {
        $mismatches.Add("File not found: $HostManifestPath")
    }

    # 5. extensions/browser/README.md
    if (Test-Path -LiteralPath $ExtensionsReadmePath) {
        $readmeContent = Get-Content -LiteralPath $ExtensionsReadmePath -Raw -Encoding UTF8
        if ($readmeContent.Contains($expectedId)) {
            Write-Host "  [PASS] extensions/browser/README.md contains extension ID ($expectedId)" -ForegroundColor Green
        } else {
            $mismatches.Add("extensions/browser/README.md does not contain extension ID ($expectedId)")
        }
    }
}

if ($mismatches.Count -gt 0) {
    Write-Host "`nExtension ID verification failed with $($mismatches.Count) mismatch(es):" -ForegroundColor Red
    foreach ($err in $mismatches) {
        Write-Host "  - $err" -ForegroundColor Red
    }
    exit 1
}

Write-Host "`nAll extension ID constants match: $expectedId" -ForegroundColor Green
exit 0

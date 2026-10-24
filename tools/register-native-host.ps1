<#
.SYNOPSIS
    Registers or unregisters the SAYL native messaging host for Chromium and Microsoft Edge.

.DESCRIPTION
    PowerShell 5+ compatible script to register the host manifest under:
    - HKCU:\Software\Google\Chrome\NativeMessagingHosts\com.sayl.downloader
    - HKCU:\Software\Microsoft\Edge\NativeMessagingHosts\com.sayl.downloader

.PARAMETER HostExe
    Path to the compiled sayl-bridge.exe binary.

.PARAMETER ManifestPath
    Path to the com.sayl.downloader.json manifest file.

.PARAMETER ExtensionId
    The Chromium/Edge extension ID (allowed_origins entry).

.PARAMETER Unregister
    Removes the registry entries for Chrome and Edge.
#>

[CmdletBinding()]
param (
    [Parameter(Mandatory = $false)]
    [string]$HostExe,

    [Parameter(Mandatory = $false)]
    [string]$ManifestPath,

    [Parameter(Mandatory = $false)]
    [string]$ExtensionId,

    [Parameter(Mandatory = $false)]
    [switch]$Unregister
)

$ChromeKey = "HKCU:\Software\Google\Chrome\NativeMessagingHosts\com.sayl.downloader"
$EdgeKey = "HKCU:\Software\Microsoft\Edge\NativeMessagingHosts\com.sayl.downloader"

if ($Unregister) {
    $keys = @($ChromeKey, $EdgeKey)
    foreach ($key in $keys) {
        if (Test-Path -LiteralPath $key) {
            Remove-Item -LiteralPath $key -Recurse -Force
            Write-Host "Unregistered: $key"
        } else {
            Write-Host "Key not found (already unregistered): $key"
        }
    }
    exit 0
}

# Registration requires ManifestPath
if ([string]::IsNullOrWhiteSpace($ManifestPath)) {
    Write-Error "Registration requires -ManifestPath pointing to the host manifest JSON file."
    exit 1
}

if (-not (Test-Path -LiteralPath $ManifestPath)) {
    Write-Error "Refusing registration: Manifest file does not exist on disk: $ManifestPath"
    exit 1
}

$resolvedManifest = (Resolve-Path -LiteralPath $ManifestPath).Path

try {
    $rawJson = Get-Content -LiteralPath $resolvedManifest -Raw -Encoding UTF8
    $manifestData = $rawJson | ConvertFrom-Json
} catch {
    Write-Error "Failed to parse JSON in manifest: $resolvedManifest"
    exit 1
}

# Determine host executable path to validate
$targetExe = if (-not [string]::IsNullOrWhiteSpace($HostExe)) {
    $HostExe
} else {
    $manifestData.path
}

if ([string]::IsNullOrWhiteSpace($targetExe) -or -not (Test-Path -LiteralPath $targetExe)) {
    Write-Error "Refusing registration: Host executable does not exist on disk: $targetExe"
    exit 1
}

$resolvedHostExe = (Resolve-Path -LiteralPath $targetExe).Path

# Update manifest content if HostExe or ExtensionId was provided
$dirty = $false
if ($manifestData.path -ne $resolvedHostExe) {
    $manifestData.path = $resolvedHostExe
    $dirty = $true
}

if (-not [string]::IsNullOrWhiteSpace($ExtensionId)) {
    $newOrigin = "chrome-extension://$ExtensionId/"
    $manifestData.allowed_origins = @($newOrigin)
    $dirty = $true
}

if ($dirty) {
    $updatedJson = $manifestData | ConvertTo-Json -Depth 5
    Set-Content -LiteralPath $resolvedManifest -Value $updatedJson -Encoding UTF8
}

# Write registry keys for Chrome and Edge
$targetKeys = @($ChromeKey, $EdgeKey)

foreach ($key in $targetKeys) {
    if (-not (Test-Path -LiteralPath $key)) {
        New-Item -Path $key -Force | Out-Null
    }
    Set-ItemProperty -LiteralPath $key -Name "(default)" -Value $resolvedManifest -Type String
    Write-Host "Registered: $key -> $resolvedManifest"
}

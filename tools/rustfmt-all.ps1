# Applies rustfmt the way `cargo fmt --all` would, without invoking cargo, so it
# can run where cargo builds are not allowed (for example on a machine where
# Smart App Control holds freshly linked binaries).
#
# usage (PowerShell, from the repository root):
#   pwsh -NoProfile -File tools/rustfmt-all.ps1           # write formatting
#   pwsh -NoProfile -File tools/rustfmt-all.ps1 -Check    # fail if unformatted
#
# Every .rs file outside target/, node_modules/ and .git/ is formatted with the
# workspace edition (2021). rustfmt is located in the rustup toolchain that
# rust-toolchain.toml pins, falling back to %USERPROFILE%\.cargo\bin.

[CmdletBinding()]
param (
    [switch]$Check
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
Push-Location $RepoRoot
try {
    $rustfmt = $null
    $candidates = @(
        (Join-Path $env:USERPROFILE '.rustup\toolchains\stable-x86_64-pc-windows-msvc\bin\rustfmt.exe'),
        (Join-Path $env:USERPROFILE '.rustup\toolchains\stable-x86_64-pc-windows-gnu\bin\rustfmt.exe'),
        (Join-Path $env:USERPROFILE '.cargo\bin\rustfmt.exe')
    )
    foreach ($c in $candidates) {
        if (Test-Path -LiteralPath $c) { $rustfmt = $c; break }
    }
    if (-not $rustfmt) {
        Write-Error "rustfmt not found; install the rustfmt component with: rustup component add rustfmt"
        exit 1
    }

    $files = Get-ChildItem -Path $RepoRoot -Recurse -File -Filter '*.rs' |
        Where-Object { $_.FullName -notmatch '[\\/](target|node_modules|\.git)[\\/]' }

    $failures = 0
    foreach ($file in $files) {
        $rel = $file.FullName.Substring($RepoRoot.Length).TrimStart('\', '/') -replace '\\', '/'
        $fmtArgs = @('--edition', '2021')
        if ($Check) { $fmtArgs = @('--check') + $fmtArgs }
        $fmtArgs += $rel
        & $rustfmt @fmtArgs
        if ($LASTEXITCODE -ne 0) {
            if ($Check) { Write-Host "would reformat: $rel" -ForegroundColor Yellow }
            $failures++
        }
    }

    $mode = if ($Check) { 'checked' } else { 'formatted' }
    Write-Host "rustfmt $mode $($files.Count) Rust files with $rustfmt ($failures failure(s))"
    if ($failures -gt 0) { exit 1 }
    exit 0
} finally {
    Pop-Location
}

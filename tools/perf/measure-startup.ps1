[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$Exe,
    [int]$Runs = 3,
    [int]$TimeoutSeconds = 30
)

$ErrorActionPreference = "Stop"

if (-not (Test-Path $Exe)) {
    Write-Error "Target executable not found at '$Exe'"
    exit 1
}

$resolvedExe = (Resolve-Path $Exe).Path
Write-Host "Measuring startup performance for: $resolvedExe"
Write-Host "Runs: $Runs, Timeout per run: ${TimeoutSeconds}s"

function Get-Median {
    param([double[]]$Values)
    if ($Values.Count -eq 0) { return $null }
    $sorted = $Values | Sort-Object
    $mid = [math]::Floor($sorted.Count / 2)
    if ($sorted.Count % 2 -eq 1) {
        return $sorted[$mid]
    } else {
        return ($sorted[$mid - 1] + $sorted[$mid]) / 2.0
    }
}

$workspaceRoot = $PSScriptRoot
while ($workspaceRoot -and -not (Test-Path (Join-Path $workspaceRoot "Cargo.toml"))) {
    $workspaceRoot = Split-Path -Parent $workspaceRoot
}
if (-not $workspaceRoot) {
    $workspaceRoot = $PWD
}

$runResults = @()

for ($i = 1; $i -le $Runs; $i++) {
    Write-Host "--- Run $i of $Runs ---"
    $tempGuid = [guid]::NewGuid().ToString("N")
    $baseTemp = Join-Path $workspaceRoot "target\perf_run_$tempGuid"
    $dataDir = Join-Path $baseTemp "data"
    $dlDir = Join-Path $baseTemp "dl"

    New-Item -ItemType Directory -Path $dataDir -Force | Out-Null
    New-Item -ItemType Directory -Path $dlDir -Force | Out-Null

    $logFile = Join-Path $dataDir "perf.log"
    $stdoutFile = Join-Path $baseTemp "stdout.log"
    $stderrFile = Join-Path $baseTemp "stderr.log"

    $origPerf = $env:SAYL_PERF
    $origData = $env:SAYL_DATA_DIR
    $origDl = $env:SAYL_DOWNLOAD_DIR

    $env:SAYL_PERF = "1"
    $env:SAYL_DATA_DIR = $dataDir
    $env:SAYL_DOWNLOAD_DIR = $dlDir

    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    $proc = Start-Process -FilePath $resolvedExe `
        -RedirectStandardOutput $stdoutFile `
        -RedirectStandardError $stderrFile `
        -PassThru

    if ($null -eq $proc) {
        Write-Error "Failed to start process: $resolvedExe"
        Remove-Item -Recurse -Force $baseTemp -ErrorAction SilentlyContinue
        exit 1
    }

    $windowShownMs = $null
    $firstPaintMs = $null
    $logFound = $false
    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)

    try {
        while ((Get-Date) -lt $deadline) {
            if (Test-Path $logFile) {
                $logFound = $true
                $lines = @()
                try {
                    $lines = Get-Content -Path $logFile -ErrorAction SilentlyContinue
                } catch {
                    # Retry next tick if locked
                }

                foreach ($line in $lines) {
                    if ([string]::IsNullOrWhiteSpace($line)) { continue }
                    try {
                        $json = $line | ConvertFrom-Json
                        if ($json.event -eq "window_shown" -and $null -eq $windowShownMs) {
                            $windowShownMs = [double]$json.elapsed_ms
                        }
                        if ($json.event -eq "frontend_perf") {
                            if ($null -ne $json.first_paint_ms) {
                                $firstPaintMs = [double]$json.first_paint_ms
                            } elseif ($null -ne $json.firstPaintMs) {
                                $firstPaintMs = [double]$json.firstPaintMs
                            }
                        }
                    } catch {}
                }

                if ($null -ne $windowShownMs) {
                    # Brief wait for frontend_perf if pending
                    $graceEnd = (Get-Date).AddMilliseconds(1000)
                    while ((Get-Date) -lt $graceEnd -and $null -eq $firstPaintMs -and -not $proc.HasExited) {
                        Start-Sleep -Milliseconds 100
                        try {
                            $lines = Get-Content -Path $logFile -ErrorAction SilentlyContinue
                            foreach ($line in $lines) {
                                if ([string]::IsNullOrWhiteSpace($line)) { continue }
                                $json = $line | ConvertFrom-Json
                                if ($json.event -eq "frontend_perf") {
                                    if ($null -ne $json.first_paint_ms) {
                                        $firstPaintMs = [double]$json.first_paint_ms
                                    } elseif ($null -ne $json.firstPaintMs) {
                                        $firstPaintMs = [double]$json.firstPaintMs
                                    }
                                }
                            }
                        } catch {}
                    }
                    break
                }
            }

            if ($proc.HasExited) {
                break
            }

            Start-Sleep -Milliseconds 50
        }
    } finally {
        $sw.Stop()
        if (-not $proc.HasExited) {
            try {
                Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
                $proc.WaitForExit(3000) | Out-Null
            } catch {}
        }
        $env:SAYL_PERF = $origPerf
        $env:SAYL_DATA_DIR = $origData
        $env:SAYL_DOWNLOAD_DIR = $origDl
    }

    if (-not $logFound) {
        Remove-Item -Recurse -Force $baseTemp -ErrorAction SilentlyContinue
        Write-Error "ERROR: perf.log never appeared within timeout of ${TimeoutSeconds}s"
        exit 1
    }

    $runRecord = [PSCustomObject]@{
        run = $i
        process_elapsed_ms = $sw.ElapsedMilliseconds
        window_shown_ms = $windowShownMs
        first_paint_ms = $firstPaintMs
    }
    $runResults += $runRecord
    Write-Host ("Run {0}: window_shown = {1} ms, first_paint = {2} ms, wall_elapsed = {3} ms" -f $i, $windowShownMs, $firstPaintMs, $sw.ElapsedMilliseconds)

    Start-Sleep -Milliseconds 200
    Remove-Item -Recurse -Force $baseTemp -ErrorAction SilentlyContinue
}

$validWindowShown = @($runResults | Where-Object { $null -ne $_.window_shown_ms } | ForEach-Object { [double]$_.window_shown_ms })
$validFirstPaint = @($runResults | Where-Object { $null -ne $_.first_paint_ms } | ForEach-Object { [double]$_.first_paint_ms })

$summary = [PSCustomObject]@{
    runs = $runResults
    median_window_shown_ms = Get-Median -Values $validWindowShown
    median_first_paint_ms = Get-Median -Values $validFirstPaint
}

Write-Host "=== Performance Summary ==="
$summary | ConvertTo-Json -Depth 5
exit 0

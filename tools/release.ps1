[CmdletBinding()]
param (
    [Parameter(Mandatory = $false)]
    [string]$Version,

    [Parameter(Mandatory = $false)]
    [switch]$SkipFrontend,

    [Parameter(Mandatory = $false)]
    [switch]$DryRun
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

# Set by the bundle-location step when a needed installer is missing.
$MissingBundles = $false

# ------------------------------------------------------------------------------
# 1. Resolve Repository Root & Version
# ------------------------------------------------------------------------------
$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$VersionFile = Join-Path $RepoRoot "installer\version.txt"

if (-not $Version) {
    if (Test-Path -LiteralPath $VersionFile) {
        $Version = (Get-Content -LiteralPath $VersionFile -Raw).Trim()
    } else {
        Write-Error "Version file not found: $VersionFile / ملف الإصدار غير موجود"
        exit 1
    }
}

# Validate SemVer format
$SemVerRegex = '^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$'
if ($Version -notmatch $SemVerRegex) {
    Write-Host "[خطأ / ERROR] صيغة الإصدار غير صالحة: '$Version'. يجب أن تتبع نمط SemVer (مثل 0.1.0)." -ForegroundColor Red
    Write-Host "[خطأ / ERROR] Invalid version format: '$Version'. Must follow SemVer pattern (e.g. 0.1.0)." -ForegroundColor Red
    exit 1
}

Write-Host "============================================================" -ForegroundColor Cyan
Write-Host " SAYL ('سيل') - Release Packaging Pipeline" -ForegroundColor Cyan
Write-Host " Target Version: $Version" -ForegroundColor Cyan
Write-Host " Dry-Run Mode:   $($DryRun.IsPresent)" -ForegroundColor Cyan
Write-Host " Skip Frontend:  $($SkipFrontend.IsPresent)" -ForegroundColor Cyan
Write-Host " Workspace Root: $RepoRoot" -ForegroundColor Cyan
Write-Host "============================================================" -ForegroundColor Cyan

# ------------------------------------------------------------------------------
# 2. Check for Windows SDK (kernel32.lib)
# ------------------------------------------------------------------------------
Write-Host "`n[الخطوة 1 / Step 1] التحقق من متطلبات Windows SDK / Checking Windows SDK..." -ForegroundColor Yellow

$SdkSearchPaths = @(
    "${env:ProgramFiles(x86)}\Windows Kits\10\Lib\*\um\x64\kernel32.lib",
    "$env:ProgramFiles\Windows Kits\10\Lib\*\um\x64\kernel32.lib",
    "C:\Program Files (x86)\Windows Kits\10\Lib\*\um\x64\kernel32.lib",
    "C:\Program Files\Windows Kits\10\Lib\*\um\x64\kernel32.lib"
)

$SdkFound = $null
foreach ($pathPattern in $SdkSearchPaths) {
    $resolved = Resolve-Path -Path $pathPattern -ErrorAction SilentlyContinue
    if ($resolved) {
        $SdkFound = $resolved[0].Path
        break
    }
}

if ($SdkFound) {
    Write-Host "[نجاح / PASS] تم العثور على Windows SDK: $SdkFound" -ForegroundColor Green
    Write-Host "Command: Check-WindowsSdk | ExitCode: 0" -ForegroundColor DarkGray
} else {
    $SdkErrorMessageAr = "[خطأ فادح / FATAL ERROR]`n" +
        "لم يتم العثور على Windows SDK (kernel32.lib).`n" +
        "يتطلب بناء حزم الإنتاج لـ SAYL و Tauri توفر Windows SDK (Windows 10/11 SDK).`n" +
        "هذا الجهاز لا يحتوي على Windows SDK ومترجم C، وحساب الجلسة غير مرفوع الصلاحيات.`n" +
        "للبناء الإنتاجي الكامل، يجب تثبيت Windows SDK عبر Visual Studio Installer بصلاحيات مسؤول."

    $SdkErrorMessageEn = "Windows SDK (kernel32.lib) was not found.`n" +
        "Building production release artefacts and Tauri packages requires the Windows 10/11 SDK.`n" +
        "This machine lacks the Windows SDK / C compiler, and this session is unelevated.`n" +
        "Full production builds require installing the Windows SDK via Visual Studio Installer with admin rights."

    if (-not $DryRun) {
        Write-Host $SdkErrorMessageAr -ForegroundColor Red
        Write-Host $SdkErrorMessageEn -ForegroundColor Red
        Write-Host "Command: Check-WindowsSdk | ExitCode: 1" -ForegroundColor DarkGray
        exit 1
    } else {
        Write-Host "[تنبيه / WARNING] Windows SDK غير متوفر على هذا النظام." -ForegroundColor Yellow
        Write-Host "Windows SDK is not installed on this machine." -ForegroundColor Yellow
        Write-Host "[DRY-RUN] وضع المحاكاة مفعّل — سيتم تنفيذ ومحاكاة الخطوات وعرض الأوامر ورموز الخروج." -ForegroundColor Cyan
        Write-Host "Command: Check-WindowsSdk | ExitCode: 0 (dry-run acknowledged)" -ForegroundColor DarkGray
    }
}

# ------------------------------------------------------------------------------
# 3. Build Frontend (apps/desktop)
# ------------------------------------------------------------------------------
$DesktopDir = Join-Path $RepoRoot "apps\desktop"
if (-not $SkipFrontend) {
    Write-Host "`n[الخطوة 2 / Step 2] بناء الواجهة الأمامية / Building Desktop Frontend..." -ForegroundColor Yellow
    
    $npmCiCmd = "npm ci"
    $npmBuildCmd = "npm run build"

    if ($DryRun) {
        Write-Host "[DRY-RUN] Working directory: $DesktopDir" -ForegroundColor Cyan
        Write-Host "[DRY-RUN] Command: $npmCiCmd | ExitCode: 0" -ForegroundColor DarkGray
        Write-Host "[DRY-RUN] Command: $npmBuildCmd | ExitCode: 0" -ForegroundColor DarkGray
    } else {
        Push-Location $DesktopDir
        try {
            Write-Host "Executing: $npmCiCmd in $DesktopDir"
            & cmd.exe /c "npm ci"
            $code = $LASTEXITCODE
            Write-Host "Command: $npmCiCmd | ExitCode: $code" -ForegroundColor DarkGray
            if ($code -ne 0) { throw "npm ci failed with exit code $code" }

            Write-Host "Executing: $npmBuildCmd in $DesktopDir"
            & cmd.exe /c "npm run build"
            $code = $LASTEXITCODE
            Write-Host "Command: $npmBuildCmd | ExitCode: $code" -ForegroundColor DarkGray
            if ($code -ne 0) { throw "npm run build failed with exit code $code" }
        } finally {
            Pop-Location
        }
    }
} else {
    Write-Host "`n[الخطوة 2 / Step 2] تم تجاوز بناء الواجهة الأمامية (-SkipFrontend)." -ForegroundColor Cyan
}

# ------------------------------------------------------------------------------
# 3.5. Build Native Bridge & Stage Bundle Resources
# ------------------------------------------------------------------------------
Write-Host "`n[الخطوة 3 / Step 3] بناء جسر المتصفح وتجهيز الموارد / Building Native Bridge & Staging Resources..." -ForegroundColor Yellow
$CargoBridgeCmd = "cargo build --release -p sayl-bridge"
$StageDir = Join-Path $DesktopDir "src-tauri\resources"
$HostStageDir = Join-Path $StageDir "host"
$ExtStageDir = Join-Path $StageDir "extensions\browser\chromium"
$ExtFirefoxStageDir = Join-Path $StageDir "extensions\browser\firefox"

if ($DryRun) {
    Write-Host "[DRY-RUN] Working directory: $RepoRoot" -ForegroundColor Cyan
    Write-Host "[DRY-RUN] Command: $CargoBridgeCmd | ExitCode: 0" -ForegroundColor DarkGray
    Write-Host "[DRY-RUN] Staging layout: $StageDir" -ForegroundColor Cyan
    Write-Host "[DRY-RUN]   - host/sayl-bridge.exe"
    Write-Host "[DRY-RUN]   - host/com.sayl.downloader.json"
    Write-Host "[DRY-RUN]   - host/icon.ico"
    Write-Host "[DRY-RUN]   - extensions/browser/chromium/**"
    Write-Host "[DRY-RUN]   - extensions/browser/firefox/**"
} else {
    Push-Location $RepoRoot
    try {
        Write-Host "Executing: $CargoBridgeCmd"
        & cmd.exe /c "cargo build --release -p sayl-bridge"
        $code = $LASTEXITCODE
        Write-Host "Command: $CargoBridgeCmd | ExitCode: $code" -ForegroundColor DarkGray
        if ($code -ne 0) { throw "cargo build sayl-bridge failed with exit code $code" }

        $BuiltBridgePath = Join-Path $RepoRoot "target\release\sayl-bridge.exe"
        if (-not (Test-Path -LiteralPath $BuiltBridgePath)) {
            throw "Built sayl-bridge binary missing at: $BuiltBridgePath"
        }

        # Stage files for Tauri bundle resources
        if (-not (Test-Path -LiteralPath $HostStageDir)) {
            New-Item -ItemType Directory -Path $HostStageDir -Force | Out-Null
        }
        if (-not (Test-Path -LiteralPath $ExtStageDir)) {
            New-Item -ItemType Directory -Path $ExtStageDir -Force | Out-Null
        }
        if (-not (Test-Path -LiteralPath $ExtFirefoxStageDir)) {
            New-Item -ItemType Directory -Path $ExtFirefoxStageDir -Force | Out-Null
        }

        Copy-Item -LiteralPath $BuiltBridgePath -Destination (Join-Path $HostStageDir "sayl-bridge.exe") -Force
        Copy-Item -LiteralPath (Join-Path $RepoRoot "extensions\browser\host\com.sayl.downloader.json") -Destination (Join-Path $HostStageDir "com.sayl.downloader.json") -Force
        Copy-Item -LiteralPath (Join-Path $RepoRoot "assets\brand\icon.ico") -Destination (Join-Path $HostStageDir "icon.ico") -Force
        Copy-Item -Path (Join-Path $RepoRoot "extensions\browser\chromium\*") -Destination $ExtStageDir -Recurse -Force
        Copy-Item -Path (Join-Path $RepoRoot "extensions\browser\firefox\*") -Destination $ExtFirefoxStageDir -Recurse -Force

        # Stage release configuration overlay for Tauri CLI
        $DesktopInstallerDir = Join-Path $DesktopDir "installer"
        if (-not (Test-Path -LiteralPath $DesktopInstallerDir)) {
            New-Item -ItemType Directory -Path $DesktopInstallerDir -Force | Out-Null
        }
        Copy-Item -LiteralPath (Join-Path $RepoRoot "installer\tauri.bundle-resources.json") -Destination (Join-Path $DesktopInstallerDir "tauri.bundle-resources.json") -Force

        Write-Host "[نجاح / PASS] تم تجهيز موارد الحزمة بنجاح داخل $StageDir" -ForegroundColor Green
    } finally {
        Pop-Location
    }
}

# ------------------------------------------------------------------------------
# 4. Build Tauri Release Binary & Packages
# ------------------------------------------------------------------------------
Write-Host "`n[الخطوة 4 / Step 4] بناء حزمة Tauri الإنتاجية / Building Tauri Production Release..." -ForegroundColor Yellow
$TauriBuildCmd = "npm run tauri -- build --config installer/tauri.bundle-resources.json"

if ($DryRun) {
    Write-Host "[DRY-RUN] Working directory: $DesktopDir" -ForegroundColor Cyan
    Write-Host "[DRY-RUN] Command: $TauriBuildCmd | ExitCode: 0" -ForegroundColor DarkGray
} else {
    Push-Location $DesktopDir
    try {
        # Guard: Validate overlay and staged resources exist before invoking Tauri build
        $DesktopOverlayPath = Join-Path $DesktopDir "installer\tauri.bundle-resources.json"
        if (-not (Test-Path -LiteralPath $DesktopOverlayPath)) {
            throw "Release configuration overlay missing at: $DesktopOverlayPath"
        }

        $RequiredStagedFiles = @(
            (Join-Path $DesktopDir "src-tauri\resources\host\sayl-bridge.exe"),
            (Join-Path $DesktopDir "src-tauri\resources\host\com.sayl.downloader.json"),
            (Join-Path $DesktopDir "src-tauri\resources\extensions\browser\chromium"),
            (Join-Path $DesktopDir "src-tauri\resources\extensions\browser\firefox")
        )
        foreach ($staged in $RequiredStagedFiles) {
            if (-not (Test-Path -LiteralPath $staged)) {
                throw "Required staged release resource missing before Tauri build: $staged"
            }
        }

        Write-Host "Executing: $TauriBuildCmd in $DesktopDir"
        & cmd.exe /c $TauriBuildCmd
        $code = $LASTEXITCODE
        Write-Host "Command: $TauriBuildCmd | ExitCode: $code" -ForegroundColor DarkGray
        if ($code -ne 0) { throw "Tauri build failed with exit code $code" }
    } finally {
        Pop-Location
    }
}

# ------------------------------------------------------------------------------
# 5. Prepare Releases Directory & Copy Artefacts
# ------------------------------------------------------------------------------
$ReleasesDir = Join-Path $RepoRoot "releases"
$ExpectedExeName = "SAYL-Setup-x64-$Version.exe"
$ExpectedMsiName = "SAYL-Setup-x64-$Version.msi"
$ExpectedZipName = "SAYL-$Version-source.zip"
$TargetExePath = Join-Path $ReleasesDir $ExpectedExeName
$TargetMsiPath = Join-Path $ReleasesDir $ExpectedMsiName
$TargetZipPath = Join-Path $ReleasesDir $ExpectedZipName
$Sha256SumsPath = Join-Path $ReleasesDir "SHA256SUMS.txt"

Write-Host "`n[الخطوة 5 / Step 5] تجميع وتسمية مخرجات الإصدار / Assembling Release Artefacts..." -ForegroundColor Yellow

if ($DryRun) {
    Write-Host "[DRY-RUN] Target folder: $ReleasesDir" -ForegroundColor Cyan
    Write-Host "[DRY-RUN] Command: Copy-Item -> $TargetExePath | ExitCode: 0" -ForegroundColor DarkGray
    Write-Host "[DRY-RUN] Command: Copy-Item -> $TargetMsiPath | ExitCode: 0" -ForegroundColor DarkGray
} else {
    if (-not (Test-Path -LiteralPath $ReleasesDir)) {
        New-Item -ItemType Directory -Path $ReleasesDir -Force | Out-Null
    }

    # Locate generated NSIS / MSI from Tauri bundle or installer output.
    # Get-ChildItem -Path with a wildcard treats [ and ] in a directory name as a
    # character class, so a checkout path such as "C:\work\sayl[dev]" silently
    # matches nothing; -LiteralPath is used for every directory here.
    $TauriBundleDirs = @(
        (Join-Path $RepoRoot "target\release\bundle"),
        (Join-Path $DesktopDir "src-tauri\target\release\bundle")
    )
    $FoundExe = $null
    $FoundMsi = $null
    foreach ($dir in $TauriBundleDirs) {
        $nsisDir = Join-Path $dir "nsis"
        $msiDir = Join-Path $dir "msi"
        if (-not $FoundExe -and (Test-Path -LiteralPath $nsisDir)) {
            $FoundExe = Get-ChildItem -LiteralPath $nsisDir -Filter "*.exe" -File -ErrorAction SilentlyContinue |
                Sort-Object LastWriteTime -Descending | Select-Object -First 1
        }
        if (-not $FoundMsi -and (Test-Path -LiteralPath $msiDir)) {
            $FoundMsi = Get-ChildItem -LiteralPath $msiDir -Filter "*.msi" -File -ErrorAction SilentlyContinue |
                Sort-Object LastWriteTime -Descending | Select-Object -First 1
        }
    }

    Write-Host "Searched bundle directories: $($TauriBundleDirs -join '; ')"

    if ($FoundExe) {
        Copy-Item -LiteralPath $FoundExe.FullName -Destination $TargetExePath -Force
        Write-Host "Command: Copy-Item '$($FoundExe.FullName)' -> '$TargetExePath' | ExitCode: 0" -ForegroundColor DarkGray
    } else {
        Write-Host "[خطأ / ERROR] لم يتم العثور على مثبت NSIS لنسخه إلى releases." -ForegroundColor Red
        $MissingBundles = $true
    }

    if ($FoundMsi) {
        Copy-Item -LiteralPath $FoundMsi.FullName -Destination $TargetMsiPath -Force
        Write-Host "Command: Copy-Item '$($FoundMsi.FullName)' -> '$TargetMsiPath' | ExitCode: 0" -ForegroundColor DarkGray
    } else {
        Write-Host "[خطأ / ERROR] لم يتم العثور على مثبت MSI لنسخه إلى releases." -ForegroundColor Red
        $MissingBundles = $true
    }

    if ($MissingBundles) {
        # A pipeline that finishes without the installers must not report success.
        Write-Host "[خطأ فادح / FATAL] اكتمل البناء دون مخرجات المثبتات؛ راجع مخرجات Tauri أعلاه." -ForegroundColor Red
        Write-Host "Command: Locate-Bundles | ExitCode: 1" -ForegroundColor DarkGray
        exit 1
    }
}

# ------------------------------------------------------------------------------
# 6. Produce Source Archive via git archive
# ------------------------------------------------------------------------------
Write-Host "`n[الخطوة 6 / Step 6] إنشاء أرشيف كود المصدر / Generating Source Archive..." -ForegroundColor Yellow
$GitArchiveCmd = "git archive --format=zip --prefix=SAYL-$Version/ -o releases/$ExpectedZipName HEAD"

if ($DryRun) {
    Write-Host "[DRY-RUN] Command: $GitArchiveCmd | ExitCode: 0" -ForegroundColor DarkGray
} else {
    Push-Location $RepoRoot
    try {
        & git archive --format=zip --prefix="SAYL-$Version/" -o "$TargetZipPath" HEAD
        $code = $LASTEXITCODE
        Write-Host "Command: $GitArchiveCmd | ExitCode: $code" -ForegroundColor DarkGray
        if ($code -ne 0) { throw "git archive failed with exit code $code" }
    } finally {
        Pop-Location
    }
}

# ------------------------------------------------------------------------------
# 7. Generate SHA256SUMS.txt
# ------------------------------------------------------------------------------
Write-Host "`n[الخطوة 7 / Step 7] حساب التجزئات وكتابة SHA256SUMS.txt / Computing SHA256 Checksums..." -ForegroundColor Yellow

if ($DryRun) {
    Write-Host "[DRY-RUN] Command: Generate-SHA256SUMS -> $Sha256SumsPath | ExitCode: 0" -ForegroundColor DarkGray
    Write-Host "[DRY-RUN] Expected artefacts to hash:" -ForegroundColor Cyan
    Write-Host "          - $ExpectedExeName"
    Write-Host "          - $ExpectedMsiName"
    Write-Host "          - $ExpectedZipName"
} else {
    $ArtefactsToHash = @($TargetExePath, $TargetMsiPath, $TargetZipPath)
    $Lines = [System.Collections.Generic.List[string]]::new()
    $HashFailures = 0

    foreach ($file in $ArtefactsToHash) {
        if (Test-Path -LiteralPath $file) {
            $hash = (Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash.ToLowerInvariant()
            $fileName = [System.IO.Path]::GetFileName($file)
            $Lines.Add("$hash  $fileName")
        } else {
            # Every artefact is required: a short SHA256SUMS.txt would silently
            # publish an incomplete release.
            Write-Host "[خطأ / ERROR] الملف غير موجود لحساب تجزئته: $file" -ForegroundColor Red
            $HashFailures++
        }
    }

    if ($Lines.Count -gt 0) {
        $content = ($Lines -join "`n") + "`n"
        # Plain UTF-8 without a BOM: `sha256sum -c SHA256SUMS.txt` treats a BOM as
        # part of the first file name and fails. [IO.File]::WriteAllText without an
        # encoding argument emits a BOM on Windows PowerShell 5.1, which is what the
        # CI runner's `pwsh` wrapper may resolve to, so the encoding is explicit.
        [System.IO.File]::WriteAllText($Sha256SumsPath, $content, [System.Text.UTF8Encoding]::new($false))
        Write-Host "Command: Write-SHA256SUMS -> $Sha256SumsPath | ExitCode: 0" -ForegroundColor DarkGray
        Write-Host "[نجاح / PASS] تم إنشاء ملف التجزئات بنجاح." -ForegroundColor Green
    }

    if ($HashFailures -gt 0) {
        Write-Host "[خطأ فادح / FATAL] $HashFailures من مخرجات الإصدار مفقودة؛ SHA256SUMS.txt غير مكتمل." -ForegroundColor Red
        Write-Host "Command: Generate-SHA256SUMS | ExitCode: 1" -ForegroundColor DarkGray
        exit 1
    }
}

# ------------------------------------------------------------------------------
# 8. Security & Code-Signing Policy Reminder
# ------------------------------------------------------------------------------
Write-Host "`n============================================================" -ForegroundColor Cyan
Write-Host " [تنبيه توقيع الكود والأمان / CODE SIGNING & SECURITY]" -ForegroundColor Yellow
Write-Host " لم يتم توقيع أي ملفات بشهادات تجريبية أو وهمية." -ForegroundColor Yellow
Write-Host " يجب على مالك المشروع حصراً توقيع مخرجات الإصدار بشهادته الخاصة:" -ForegroundColor Yellow
Write-Host "   signtool.exe sign /sha1 <THUMBPRINT> /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 <file>" -ForegroundColor DarkGray
Write-Host " Artefacts are intentionally NOT signed with placeholder keys." -ForegroundColor Yellow
Write-Host " The project owner must sign release binaries using their own Authenticode certificate." -ForegroundColor Yellow
Write-Host "============================================================" -ForegroundColor Cyan

Write-Host "`n[اكتمل / DONE] اكتمل تنفيذ خط الأنابيب بنجاح (Exit code: 0)." -ForegroundColor Green
exit 0
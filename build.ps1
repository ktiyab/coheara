#Requires -Version 5.1
<#
.SYNOPSIS
    Coheara Local Build System for Windows (.msi + .exe installers)
.DESCRIPTION
    Mirrors build.sh but targets Windows with NSIS and MSI bundles.
    Requires: Node.js 20+, Rust 1.80+, Perl (for OpenSSL), JDK 21+, Android SDK, vcpkg (for Tesseract).
.EXAMPLE
    .\build.ps1 desktop -NoSign         # First build (unsigned, no credentials)
    .\build.ps1 desktop -SkipMobile     # Desktop only (use pre-staged mobile artifacts)
    .\build.ps1 desktop                 # Signed desktop build (includes mobile)
    .\build.ps1 android -NoSign         # Unsigned Android APK
    .\build.ps1 all                     # Everything, signed
    .\build.ps1 clean                   # Remove all build artifacts
#>

[CmdletBinding()]
param(
    [Parameter(Position = 0, Mandatory = $true)]
    [ValidateSet("desktop", "android", "all", "clean", "setup", "help")]
    [string]$Command,

    [switch]$NoSign,
    [switch]$SkipMobile,
    [switch]$Verbose_
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

# ── Constants ──────────────────────────────────────────────────────────────
$ProjectRoot = $PSScriptRoot
$PackageDir  = Join-Path $ProjectRoot "package"
$KeysDir     = Join-Path $ProjectRoot "Specs\build\keys"
$MobileDir   = Join-Path $ProjectRoot "mobile"
$TauriDir    = Join-Path $ProjectRoot "src-tauri"
$AndroidDir  = Join-Path $MobileDir "android"

$CargoPath = if ($env:CARGO) { $env:CARGO }
             elseif (Get-Command cargo -ErrorAction SilentlyContinue) { "cargo" }
             elseif (Test-Path "$env:USERPROFILE\.cargo\bin\cargo.exe") { "$env:USERPROFILE\.cargo\bin\cargo.exe" }
             else { $null }

# Ensure cargo directory is in PATH
if ($CargoPath -and (Test-Path $CargoPath)) {
    $cargoDir = Split-Path $CargoPath -Parent
    if ($env:PATH -notlike "*$cargoDir*") {
        $env:PATH = "$cargoDir;$env:PATH"
    }
}

$Version = (Get-Content (Join-Path $ProjectRoot "package.json") -Raw | ConvertFrom-Json).version
$Sign = -not $NoSign
$MobileSkipped = $SkipMobile.IsPresent

# ── Logging ────────────────────────────────────────────────────────────────
function Log-Info  { param([string]$Msg) Write-Host "[INFO]  $Msg" -ForegroundColor Cyan }
function Log-Ok    { param([string]$Msg) Write-Host "[OK]    $Msg" -ForegroundColor Green }
function Log-Warn  { param([string]$Msg) Write-Host "[WARN]  $Msg" -ForegroundColor Yellow }
function Log-Error { param([string]$Msg) Write-Host "[ERROR] $Msg" -ForegroundColor Red }
function Log-Step  { param([string]$Msg) Write-Host "`n==> $Msg" -ForegroundColor White -BackgroundColor DarkGray }

$timer = [System.Diagnostics.Stopwatch]::StartNew()
function Get-Elapsed { "{0}m {1}s" -f [math]::Floor($timer.Elapsed.TotalMinutes), $timer.Elapsed.Seconds }

# ── Usage ──────────────────────────────────────────────────────────────────
function Show-Usage {
    Write-Host @"
Coheara Local Build System v$Version (Windows)

Usage: .\build.ps1 <command> [options]

Commands:
  setup       First-time setup: install deps, verify toolchain, build i18n
  desktop     Build Windows installers (.msi + .exe via NSIS)
              Also builds mobile artifacts (bundled inside installer)
  android     Build standalone signed Android APK
  all         Build everything (desktop + standalone APK)
  clean       Remove all build artifacts and intermediates
  help        Show this help

Options:
  -NoSign      Build without signing (faster, for testing)
  -SkipMobile  Skip mobile build (use pre-staged PWA + APK from prior build)
  -Verbose_    Show detailed command output

Credential Priority (passwords only - key files read from disk):
  1. Environment variables (TAURI_SIGNING_PRIVATE_KEY_PASSWORD, etc.)
  2. .env file in project root
  3. Interactive prompt

Signing Keys (read directly from files):
  Tauri:   Specs\build\keys\coheara-tauri.key
  Android: Specs\build\keys\coheara-release.jks

Output:
  All artifacts collected in .\package\

Examples:
  .\build.ps1 desktop -NoSign         # Quick unsigned build for testing
  .\build.ps1 desktop -SkipMobile     # Desktop only (use pre-staged mobile artifacts)
  .\build.ps1 android                 # Signed Android APK
  .\build.ps1 all                     # Everything, signed
  .\build.ps1 clean                   # Remove all build artifacts

Note: -SkipMobile is auto-detected when pre-staged artifacts exist in
      src-tauri\resources\mobile-pwa\ and mobile-apk\coheara.apk
"@
}

# ── Credential Loading ─────────────────────────────────────────────────────
function Load-Credentials {
    if (-not $Sign) {
        Log-Info "Signing disabled (-NoSign)"
        return
    }

    # Tier 2: .env file (only whitelisted variables)
    $envFile = Join-Path $ProjectRoot ".env"
    if (Test-Path $envFile) {
        Log-Info "Loading credentials from .env"
        $whitelisted = @(
            "TAURI_SIGNING_PRIVATE_KEY_PASSWORD",
            "ANDROID_KEYSTORE_PASSWORD",
            "ANDROID_KEY_PASSWORD",
            "ANDROID_KEY_ALIAS"
        )
        foreach ($line in Get-Content $envFile) {
            $line = $line.Trim()
            if (-not $line -or $line.StartsWith("#")) { continue }
            $parts = $line -split "=", 2
            if ($parts.Count -ne 2) { continue }
            $key = $parts[0].Trim()
            $value = $parts[1].Trim().Trim('"').Trim("'")
            if ($key -in $whitelisted -and -not [Environment]::GetEnvironmentVariable($key)) {
                [Environment]::SetEnvironmentVariable($key, $value, "Process")
            }
        }
    }

    # Tier 3: Interactive prompt for missing passwords
    if (-not $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD) {
        $secure = Read-Host "Enter TAURI_SIGNING_PRIVATE_KEY_PASSWORD" -AsSecureString
        $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = [Runtime.InteropServices.Marshal]::PtrToStringAuto(
            [Runtime.InteropServices.Marshal]::SecureStringToBSTR($secure))
    }

    if (-not $MobileSkipped) {
        if (-not $env:ANDROID_KEYSTORE_PASSWORD) {
            $secure = Read-Host "Enter ANDROID_KEYSTORE_PASSWORD" -AsSecureString
            $env:ANDROID_KEYSTORE_PASSWORD = [Runtime.InteropServices.Marshal]::PtrToStringAuto(
                [Runtime.InteropServices.Marshal]::SecureStringToBSTR($secure))
        }

        # PKCS12: key password = store password
        if (-not $env:ANDROID_KEY_PASSWORD) {
            $env:ANDROID_KEY_PASSWORD = $env:ANDROID_KEYSTORE_PASSWORD
        }
        if (-not $env:ANDROID_KEY_ALIAS) {
            $env:ANDROID_KEY_ALIAS = "coheara"
        }
    }

    Log-Ok "Credentials loaded"
}

# ── Auto-install Helpers ──────────────────────────────────────────────────

function Find-Git {
    # Check PATH first
    $git = Get-Command git -ErrorAction SilentlyContinue
    if ($git) { return $git.Source }

    # Search common Windows install locations
    $candidates = @(
        "$env:ProgramFiles\Git\cmd\git.exe",
        "${env:ProgramFiles(x86)}\Git\cmd\git.exe",
        "$env:LOCALAPPDATA\Programs\Git\cmd\git.exe",
        "$env:USERPROFILE\scoop\shims\git.exe"
    )
    foreach ($path in $candidates) {
        if (Test-Path $path) { return $path }
    }

    return $null
}

function Install-Git {
    # Try winget first (available on Windows 10 1809+ and Windows 11)
    if (Get-Command winget -ErrorAction SilentlyContinue) {
        Write-Host ""
        Log-Warn "Git is required but not found in PATH."
        Write-Host "  This will install Git for Windows via winget (~2 min)" -ForegroundColor Yellow
        Write-Host ""
        $answer = Read-Host "  Install Git automatically? [Y/n]"
        if ($answer -and $answer -notin @("Y", "y", "yes", "Yes")) {
            return $null
        }

        Log-Step "Installing Git via winget"
        $prevEAP = $ErrorActionPreference; $ErrorActionPreference = "Continue"
        & winget install Git.Git --accept-package-agreements --accept-source-agreements
        $exitCode = $LASTEXITCODE; $ErrorActionPreference = $prevEAP
        if ($exitCode -ne 0) { Log-Error "Failed to install Git via winget"; return $null }

        # Refresh PATH to find newly installed git
        $env:PATH = [Environment]::GetEnvironmentVariable("PATH", "Machine") + ";" + [Environment]::GetEnvironmentVariable("PATH", "User")
        $gitPath = Find-Git
        if ($gitPath) {
            Log-Ok "Git installed successfully"
            return $gitPath
        }

        # winget sometimes needs a new shell — check default location
        $defaultGit = "$env:ProgramFiles\Git\cmd\git.exe"
        if (Test-Path $defaultGit) {
            $gitDir = Split-Path $defaultGit -Parent
            $env:PATH = "$gitDir;$env:PATH"
            Log-Ok "Git installed successfully (added to PATH for this session)"
            return $defaultGit
        }

        Log-Error "Git was installed but could not be found. Please restart PowerShell and retry."
        return $null
    }

    Log-Error "Git is not installed and winget is not available for auto-install."
    Write-Host "  Install Git manually: https://git-scm.com/download/win" -ForegroundColor Yellow
    return $null
}

function Find-LibClang {
    # Check LIBCLANG_PATH env var
    if ($env:LIBCLANG_PATH -and (Test-Path (Join-Path $env:LIBCLANG_PATH "libclang.dll"))) {
        return $true
    }

    # Search common locations
    $candidates = @(
        "$env:ProgramFiles\LLVM\bin",
        "${env:ProgramFiles(x86)}\LLVM\bin",
        "$env:ProgramFiles\LLVM\lib",
        "$env:LOCALAPPDATA\Programs\LLVM\bin"
    )

    # Also check Visual Studio Clang tools
    $vsWhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
    if (Test-Path $vsWhere) {
        $vsPath = & $vsWhere -latest -property installationPath 2>$null
        if ($vsPath) {
            $candidates += "$vsPath\VC\Tools\Llvm\x64\bin"
            $candidates += "$vsPath\VC\Tools\Llvm\bin"
        }
    }

    foreach ($dir in $candidates) {
        if (Test-Path (Join-Path $dir "libclang.dll")) {
            $env:LIBCLANG_PATH = $dir
            Log-Info "Found libclang at $dir"
            return $true
        }
    }

    return $false
}

function Install-Llvm {
    if (-not (Get-Command winget -ErrorAction SilentlyContinue)) {
        Log-Error "LLVM/libclang is not installed and winget is not available."
        Write-Host "  Install LLVM manually: https://releases.llvm.org/" -ForegroundColor Yellow
        return $false
    }

    Write-Host ""
    Log-Warn "LLVM/libclang is required for Rust FFI binding generation."
    Write-Host "  This will install LLVM via winget (~2 min)" -ForegroundColor Yellow
    Write-Host ""
    $answer = Read-Host "  Install LLVM automatically? [Y/n]"
    if ($answer -and $answer -notin @("Y", "y", "yes", "Yes")) {
        return $false
    }

    Log-Step "Installing LLVM via winget"
    $prevEAP = $ErrorActionPreference; $ErrorActionPreference = "Continue"
    & winget install LLVM.LLVM --accept-package-agreements --accept-source-agreements
    $exitCode = $LASTEXITCODE; $ErrorActionPreference = $prevEAP
    if ($exitCode -ne 0) { Log-Error "Failed to install LLVM via winget"; return $false }

    # Refresh PATH and find libclang
    $env:PATH = [Environment]::GetEnvironmentVariable("PATH", "Machine") + ";" + [Environment]::GetEnvironmentVariable("PATH", "User")

    if (Find-LibClang) {
        Log-Ok "LLVM installed successfully (LIBCLANG_PATH=$($env:LIBCLANG_PATH))"
        return $true
    }

    # winget installs to Program Files by default
    $defaultPath = "$env:ProgramFiles\LLVM\bin"
    if (Test-Path (Join-Path $defaultPath "libclang.dll")) {
        $env:LIBCLANG_PATH = $defaultPath
        Log-Ok "LLVM installed successfully (LIBCLANG_PATH=$defaultPath)"
        return $true
    }

    Log-Error "LLVM was installed but libclang.dll could not be found. Please restart PowerShell and retry."
    return $false
}

function Find-Perl {
    # Check PATH first
    $perl = Get-Command perl -ErrorAction SilentlyContinue
    if ($perl) { return $perl.Source }

    # Search common Windows install locations (Strawberry Perl, ActivePerl)
    $candidates = @(
        "$env:ProgramFiles\Strawberry\perl\bin\perl.exe",
        "${env:ProgramFiles(x86)}\Strawberry\perl\bin\perl.exe",
        "C:\Strawberry\perl\bin\perl.exe",
        "$env:ProgramFiles\Perl64\bin\perl.exe",
        "${env:ProgramFiles(x86)}\Perl64\bin\perl.exe",
        "$env:USERPROFILE\scoop\shims\perl.exe"
    )
    foreach ($path in $candidates) {
        if (Test-Path $path) { return $path }
    }

    return $null
}

function Install-Perl {
    if (-not (Get-Command winget -ErrorAction SilentlyContinue)) {
        Log-Error "Perl is not installed and winget is not available for auto-install."
        Write-Host "  Install Strawberry Perl manually: https://strawberryperl.com/" -ForegroundColor Yellow
        return $null
    }

    Write-Host ""
    Log-Warn "Perl is required for OpenSSL compilation (used by the openssl-sys Rust crate)."
    Write-Host "  This will install Strawberry Perl via winget (~2 min)" -ForegroundColor Yellow
    Write-Host ""
    $answer = Read-Host "  Install Perl automatically? [Y/n]"
    if ($answer -and $answer -notin @("Y", "y", "yes", "Yes")) {
        return $null
    }

    Log-Step "Installing Strawberry Perl via winget"
    $prevEAP = $ErrorActionPreference; $ErrorActionPreference = "Continue"
    & winget install StrawberryPerl.StrawberryPerl --accept-package-agreements --accept-source-agreements
    $exitCode = $LASTEXITCODE; $ErrorActionPreference = $prevEAP
    if ($exitCode -ne 0) { Log-Error "Failed to install Perl via winget"; return $null }

    # Refresh PATH to find newly installed perl
    $env:PATH = [Environment]::GetEnvironmentVariable("PATH", "Machine") + ";" + [Environment]::GetEnvironmentVariable("PATH", "User")
    $perlPath = Find-Perl
    if ($perlPath) {
        Log-Ok "Perl installed successfully"
        return $perlPath
    }

    # winget installs Strawberry Perl to C:\Strawberry by default
    $defaultPerl = "C:\Strawberry\perl\bin\perl.exe"
    if (Test-Path $defaultPerl) {
        $perlDir = Split-Path $defaultPerl -Parent
        $env:PATH = "$perlDir;$env:PATH"
        Log-Ok "Perl installed successfully (added to PATH for this session)"
        return $defaultPerl
    }

    Log-Error "Perl was installed but could not be found. Please restart PowerShell and retry."
    return $null
}

function Install-VcpkgTesseract {
    $vcpkgRoot = "C:\vcpkg"

    Write-Host ""
    Log-Warn "Tesseract OCR is required for the desktop build (OCR module)."
    Write-Host "  This will:" -ForegroundColor Yellow
    Write-Host "    1. Clone vcpkg to $vcpkgRoot (~1 min)" -ForegroundColor Yellow
    Write-Host "    2. Install tesseract:x64-windows-static-md (~5 min)" -ForegroundColor Yellow
    Write-Host ""
    $answer = Read-Host "  Install vcpkg + Tesseract automatically? [Y/n]"
    if ($answer -and $answer -notin @("Y", "y", "yes", "Yes")) {
        return $false
    }

    # Find or install git (needed by vcpkg)
    $gitExe = Find-Git
    if (-not $gitExe) {
        $gitExe = Install-Git
        if (-not $gitExe) { return $false }
    }

    # Temporarily allow stderr output from native commands to flow through
    # (PowerShell 5.1 treats stderr as errors when ErrorActionPreference=Stop)
    $prevEAP = $ErrorActionPreference
    $ErrorActionPreference = "Continue"

    # Clone vcpkg if needed
    if (-not (Test-Path $vcpkgRoot)) {
        Log-Step "Cloning vcpkg to $vcpkgRoot"
        & $gitExe clone https://github.com/microsoft/vcpkg.git $vcpkgRoot
        if ($LASTEXITCODE -ne 0) {
            $ErrorActionPreference = $prevEAP
            Log-Error "Failed to clone vcpkg"
            return $false
        }
    }

    # Bootstrap if needed
    $vcpkgExe = Join-Path $vcpkgRoot "vcpkg.exe"
    if (-not (Test-Path $vcpkgExe)) {
        Log-Step "Bootstrapping vcpkg"
        & (Join-Path $vcpkgRoot "bootstrap-vcpkg.bat") -disableMetrics
        if ($LASTEXITCODE -ne 0) {
            $ErrorActionPreference = $prevEAP
            Log-Error "Failed to bootstrap vcpkg"
            return $false
        }
    }

    # Set environment for this session + persist for future sessions
    $env:VCPKG_ROOT = $vcpkgRoot
    $env:PATH = "$vcpkgRoot;$env:PATH"
    [Environment]::SetEnvironmentVariable("VCPKG_ROOT", $vcpkgRoot, "User")
    Log-Ok "VCPKG_ROOT set to $vcpkgRoot (persisted for future sessions)"

    # Install Tesseract
    Log-Step "Installing Tesseract via vcpkg (this may take several minutes)"
    & $vcpkgExe install tesseract:x64-windows-static-md
    if ($LASTEXITCODE -ne 0) {
        $ErrorActionPreference = $prevEAP
        Log-Error "Failed to install Tesseract via vcpkg"
        return $false
    }

    # Restore strict error handling
    $ErrorActionPreference = $prevEAP

    Log-Ok "vcpkg + Tesseract installed successfully"
    return $true
}

# ── Dependency Checking ────────────────────────────────────────────────────
function Check-Dependencies {
    param([string]$Target)
    $missing = @()

    # Common
    if (-not (Get-Command node -ErrorAction SilentlyContinue)) {
        $missing += "Node.js >= 20 (https://nodejs.org/)"
    }
    if (-not (Get-Command npm -ErrorAction SilentlyContinue)) {
        $missing += "npm"
    }
    if (-not $CargoPath) {
        $missing += "Rust/cargo (https://rustup.rs/)"
    }

    # Desktop
    if ($Target -in @("desktop", "all")) {
        # Check for Visual Studio Build Tools (MSVC)
        $vsWhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
        if (Test-Path $vsWhere) {
            $vsPath = & $vsWhere -latest -property installationPath 2>$null
            if (-not $vsPath) {
                $missing += "Visual Studio Build Tools with C++ workload"
            }
        } else {
            # Try checking for cl.exe in PATH
            if (-not (Get-Command cl -ErrorAction SilentlyContinue)) {
                $missing += "Visual Studio Build Tools with C++ workload (https://visualstudio.microsoft.com/downloads/#build-tools)"
            }
        }

        # Check for vcpkg Tesseract (needed for OCR)
        $tesseractFound = $false
        if ($env:VCPKG_ROOT -and (Test-Path (Join-Path $env:VCPKG_ROOT "installed\x64-windows-static-md\lib\tesseract*.lib"))) {
            $tesseractFound = $true
        } elseif (Get-Command tesseract -ErrorAction SilentlyContinue) {
            $tesseractFound = $true
        }
        if (-not $tesseractFound) {
            if (-not (Install-VcpkgTesseract)) {
                $missing += "Tesseract OCR (install via: vcpkg install tesseract:x64-windows-static-md)"
            }
        }

        # Check for libclang (needed by bindgen for FFI generation)
        if (-not (Find-LibClang)) {
            if (-not (Install-Llvm)) {
                $missing += "LLVM/libclang (install via: winget install LLVM.LLVM)"
            }
        }

        # Check for Perl (needed by openssl-sys crate to configure OpenSSL)
        if (-not (Find-Perl)) {
            $perlPath = Install-Perl
            if (-not $perlPath) {
                $missing += "Perl (install via: winget install StrawberryPerl.StrawberryPerl)"
            }
        } else {
            # Ensure perl's directory is in PATH for cargo build
            $perlPath = Find-Perl
            $perlDir = Split-Path $perlPath -Parent
            if ($env:PATH -notlike "*$perlDir*") {
                $env:PATH = "$perlDir;$env:PATH"
            }
        }

        if ($Sign) {
            $tauriKey = Join-Path $KeysDir "coheara-tauri.key"
            if (-not (Test-Path $tauriKey)) {
                $missing += "Tauri signing key: $tauriKey"
            }
        }
    }

    # Android / Mobile (desktop also bundles mobile artifacts)
    if (-not $MobileSkipped -and $Target -in @("desktop", "android", "all")) {
        if (-not (Get-Command java -ErrorAction SilentlyContinue)) {
            $missing += "Java JDK 21+ (https://adoptium.net/)"
        } else {
            $javaVer = & java -version 2>&1 | Select-Object -First 1
            if ($javaVer -match '"(\d+)') {
                if ([int]$Matches[1] -lt 21) {
                    $missing += "Java 21+ (found: $($Matches[1]))"
                }
            }
        }

        if (-not $env:ANDROID_HOME -and -not $env:ANDROID_SDK_ROOT) {
            $sdkPaths = @(
                "$env:LOCALAPPDATA\Android\Sdk",
                "$env:USERPROFILE\Android\Sdk",
                "$env:USERPROFILE\AppData\Local\Android\Sdk"
            )
            foreach ($p in $sdkPaths) {
                if (Test-Path $p) {
                    $env:ANDROID_HOME = $p
                    break
                }
            }
            if (-not $env:ANDROID_HOME) {
                $missing += "ANDROID_HOME (Android SDK not found)"
            }
        }

        if ($Sign) {
            $androidKey = Join-Path $KeysDir "coheara-release.jks"
            if (-not (Test-Path $androidKey)) {
                $missing += "Android keystore: $androidKey"
            }
        }
    }

    if ($missing.Count -gt 0) {
        Log-Error "Missing dependencies:"
        foreach ($dep in $missing) {
            Write-Host "  - $dep"
        }
        exit 1
    }

    Log-Ok "All dependencies satisfied"
}

# ── Pre-staged Mobile Detection ───────────────────────────────────────────

function Detect-PrestagedMobile {
    if ($script:MobileSkipped) {
        Log-Info "Mobile build: SKIPPED (-SkipMobile flag)"
        return
    }

    $pwaDest = Join-Path $TauriDir "resources\mobile-pwa"
    $apkDest = Join-Path $TauriDir "resources\mobile-apk"
    $hasPwa = $false
    $hasApk = $false

    if (Test-Path $pwaDest) {
        $pwaFiles = Get-ChildItem $pwaDest -Exclude ".gitkeep", ".gitignore" -File -Recurse -ErrorAction SilentlyContinue
        if ($pwaFiles -and $pwaFiles.Count -gt 0) { $hasPwa = $true }
    }

    $apkPath = Join-Path $apkDest "coheara.apk"
    if (Test-Path $apkPath) { $hasApk = $true }

    if ($hasPwa -and $hasApk) {
        $script:MobileSkipped = $true
        $apkSize = "{0:N1} MB" -f ((Get-Item $apkPath).Length / 1MB)
        Log-Info "Mobile build: SKIPPED (pre-staged artifacts detected)"
        Log-Info "  PWA: $pwaDest ($($pwaFiles.Count) files)"
        Log-Info "  APK: $apkPath ($apkSize)"
    } elseif ($hasPwa -or $hasApk) {
        Log-Warn "Partial pre-staged mobile artifacts found:"
        if ($hasPwa) { Log-Info "  PWA: present" } else { Log-Warn "  PWA: missing" }
        if ($hasApk) { Log-Info "  APK: present" } else { Log-Warn "  APK: missing" }
        Log-Info "Building mobile from source (use -SkipMobile to use partial artifacts)"
    }
}

# ── Build Phases ───────────────────────────────────────────────────────────

function Build-Frontend {
    Log-Step "Building desktop frontend (SvelteKit)"
    Push-Location $ProjectRoot
    try {
        # --force: bypass EBADPLATFORM for cross-platform lockfile (WSL2 + Windows)
        & npm install --prefer-offline --force
        if ($LASTEXITCODE -ne 0) { throw "npm install failed" }
        & npm run build
        if ($LASTEXITCODE -ne 0) { throw "npm run build failed" }
        Log-Ok "Frontend built -> .\build\"
    } finally { Pop-Location }
}

function Build-MobilePwa {
    Log-Step "Building mobile PWA"
    Push-Location $MobileDir
    try {
        & npm install --prefer-offline --force
        if ($LASTEXITCODE -ne 0) { throw "npm install failed" }
        & npm run build
        if ($LASTEXITCODE -ne 0) { throw "npm run build failed" }
        Log-Ok "Mobile PWA built -> mobile\build\"
    } finally { Pop-Location }
}

function Build-AndroidApk {
    Log-Step "Building Android APK"

    if (-not (Test-Path (Join-Path $MobileDir "build"))) {
        Build-MobilePwa
    }

    Push-Location $MobileDir
    try {
        Log-Info "Syncing Capacitor to Android"
        & npx cap sync android
        if ($LASTEXITCODE -ne 0) { throw "Capacitor sync failed" }
    } finally { Pop-Location }

    if ($Sign) {
        Log-Info "Configuring Android signing"
        $jksSrc = Join-Path $KeysDir "coheara-release.jks"
        $jksDst = Join-Path $AndroidDir "app\release-key.jks"
        Copy-Item $jksSrc $jksDst -Force
        $env:ANDROID_KEYSTORE_FILE = "release-key.jks"
    }

    Push-Location $AndroidDir
    try {
        $gradlew = Join-Path $AndroidDir "gradlew.bat"
        & $gradlew assembleRelease
        if ($LASTEXITCODE -ne 0) { throw "Gradle build failed" }
    } finally { Pop-Location }

    # Signed builds produce app-release.apk; unsigned produce app-release-unsigned.apk
    $apkDir = Join-Path $AndroidDir "app\build\outputs\apk\release"
    $apkPath = $null
    $signed = Join-Path $apkDir "app-release.apk"
    $unsigned = Join-Path $apkDir "app-release-unsigned.apk"
    if (Test-Path $signed) { $apkPath = $signed }
    elseif (Test-Path $unsigned) { $apkPath = $unsigned }

    if ($apkPath) {
        $size = "{0:N1} MB" -f ((Get-Item $apkPath).Length / 1MB)
        Log-Ok "Android APK built ($size) -> $apkPath"
    } else {
        Log-Error "APK not found in $apkDir"
        if (Test-Path $apkDir) { Get-ChildItem $apkDir | Format-Table Name, Length }
        exit 1
    }
}

function Stage-MobileResources {
    Log-Step "Staging mobile resources into Tauri"

    $pwaDest = Join-Path $TauriDir "resources\mobile-pwa"
    $apkDest = Join-Path $TauriDir "resources\mobile-apk"

    New-Item -ItemType Directory -Path $pwaDest -Force | Out-Null
    New-Item -ItemType Directory -Path $apkDest -Force | Out-Null

    # Clean previous staged files (preserve .gitkeep and .gitignore)
    Get-ChildItem $pwaDest -Exclude ".gitkeep", ".gitignore" -Recurse | Remove-Item -Recurse -Force -ErrorAction SilentlyContinue
    Get-ChildItem $apkDest -Exclude ".gitkeep", ".gitignore" -Recurse | Remove-Item -Recurse -Force -ErrorAction SilentlyContinue

    # Stage PWA
    $pwaSrc = Join-Path $MobileDir "build"
    if (Test-Path $pwaSrc) {
        Copy-Item (Join-Path $pwaSrc "*") $pwaDest -Recurse -Force
        Log-Ok "PWA staged -> $pwaDest"
    } else {
        Log-Warn "Mobile PWA not built - skipping"
    }

    # Stage APK (renamed for distribution server)
    $apkDir = Join-Path $AndroidDir "app\build\outputs\apk\release"
    $apkSrc = $null
    $signed = Join-Path $apkDir "app-release.apk"
    $unsigned = Join-Path $apkDir "app-release-unsigned.apk"
    if (Test-Path $signed) { $apkSrc = $signed }
    elseif (Test-Path $unsigned) { $apkSrc = $unsigned }

    if ($apkSrc) {
        Copy-Item $apkSrc (Join-Path $apkDest "coheara.apk") -Force
        Log-Ok "APK staged -> $apkDest\coheara.apk"
    } else {
        Log-Warn "Android APK not built - skipping"
    }
}

function Build-Desktop {
    Log-Step "Building Tauri desktop installer (Windows)"

    if (-not (Test-Path (Join-Path $ProjectRoot "build"))) {
        Build-Frontend
    }

    $bundles = "nsis,msi"

    # Set signing environment
    $tauriKey = Join-Path $KeysDir "coheara-tauri.key"
    if ($Sign -and (Test-Path $tauriKey)) {
        $env:TAURI_SIGNING_PRIVATE_KEY = Get-Content $tauriKey -Raw
        Log-Info "Tauri updater signing: ENABLED"
    } else {
        Remove-Item Env:\TAURI_SIGNING_PRIVATE_KEY -ErrorAction SilentlyContinue
        Remove-Item Env:\TAURI_SIGNING_PRIVATE_KEY_PASSWORD -ErrorAction SilentlyContinue
        Log-Warn "Tauri updater signing: DISABLED"
    }

    Log-Info "Building bundles: $bundles"

    Push-Location $ProjectRoot
    try {
        if ($Sign) {
            & npx tauri build --bundles $bundles
            if ($LASTEXITCODE -ne 0) { throw "Tauri build failed" }
        } else {
            # When unsigned, Tauri may error about missing private key if pubkey exists
            # in tauri.conf.json. The bundles are still created before the error.
            & npx tauri build --bundles $bundles
            $tauriExit = $LASTEXITCODE

            if ($tauriExit -ne 0) {
                $tauriOut = Join-Path $TauriDir "target\release\bundle"
                $hasBundles = $false
                $nsisDir = Join-Path $tauriOut "nsis"
                $msiDir  = Join-Path $tauriOut "msi"
                if ((Test-Path $nsisDir) -and (Get-ChildItem $nsisDir -Filter "*.exe" -ErrorAction SilentlyContinue)) { $hasBundles = $true }
                if ((Test-Path $msiDir) -and (Get-ChildItem $msiDir -Filter "*.msi" -ErrorAction SilentlyContinue)) { $hasBundles = $true }

                if ($hasBundles) {
                    Log-Warn "Tauri exited with error (signing skipped) but bundles were created"
                } else {
                    Log-Error "Tauri build failed and no bundles were created"
                    exit 1
                }
            }
        }
    } finally { Pop-Location }

    Log-Ok "Desktop build complete"
}

# ── Artifact Collection ────────────────────────────────────────────────────

function Collect-Artifacts {
    Log-Step "Collecting artifacts into package\"

    New-Item -ItemType Directory -Path $PackageDir -Force | Out-Null

    $tauriOut = Join-Path $TauriDir "target\release\bundle"

    # NSIS .exe installer
    $nsisDir = Join-Path $tauriOut "nsis"
    if (Test-Path $nsisDir) {
        Get-ChildItem $nsisDir -Filter "*.exe" | ForEach-Object {
            Copy-Item $_.FullName $PackageDir -Force
            Log-Ok "Collected: $($_.Name)"
        }
    }

    # MSI installer
    $msiDir = Join-Path $tauriOut "msi"
    if (Test-Path $msiDir) {
        Get-ChildItem $msiDir -Filter "*.msi" | ForEach-Object {
            Copy-Item $_.FullName $PackageDir -Force
            Log-Ok "Collected: $($_.Name)"
        }
    }

    # Updater signatures (.sig files)
    $sigCount = 0
    Get-ChildItem $tauriOut -Filter "*.sig" -Recurse -ErrorAction SilentlyContinue | ForEach-Object {
        Copy-Item $_.FullName $PackageDir -Force
        $sigCount++
    }
    if ($sigCount -gt 0) {
        Log-Ok "Collected: $sigCount .sig files"
    }

    # Android APK
    $apkDir = Join-Path $AndroidDir "app\build\outputs\apk\release"
    $apkSrc = $null
    $signed = Join-Path $apkDir "app-release.apk"
    $unsigned = Join-Path $apkDir "app-release-unsigned.apk"
    if (Test-Path $signed) { $apkSrc = $signed }
    elseif (Test-Path $unsigned) { $apkSrc = $unsigned }

    if ($apkSrc) {
        Copy-Item $apkSrc (Join-Path $PackageDir "coheara-$Version.apk") -Force
        Log-Ok "Collected: .apk"
    }

    Write-Host ""
    Log-Step "Build artifacts in $PackageDir`:"
    if (Test-Path $PackageDir) {
        Get-ChildItem $PackageDir | Format-Table Name, @{N="Size";E={"{0:N1} MB" -f ($_.Length / 1MB)}} -AutoSize
    } else {
        Write-Host "  (empty)"
    }
    Log-Ok "Build completed in $(Get-Elapsed)"
}

# ── Clean ──────────────────────────────────────────────────────────────────

function Invoke-Clean {
    Log-Step "Cleaning build artifacts"

    $dirsToClean = @(
        (Join-Path $ProjectRoot "build"),
        (Join-Path $ProjectRoot ".svelte-kit"),
        (Join-Path $MobileDir "build"),
        (Join-Path $MobileDir ".svelte-kit")
    )
    foreach ($dir in $dirsToClean) {
        if (Test-Path $dir) { Remove-Item $dir -Recurse -Force }
    }
    Log-Ok "Cleaned: frontend outputs"

    # Gradle clean
    $gradlew = Join-Path $AndroidDir "gradlew.bat"
    if (Test-Path $gradlew) {
        Push-Location $AndroidDir
        try { & $gradlew clean 2>$null } catch { }
        finally { Pop-Location }

        $releaseKey = Join-Path $AndroidDir "app\release-key.jks"
        if (Test-Path $releaseKey) { Remove-Item $releaseKey -Force }
        Log-Ok "Cleaned: Android build"
    }

    # Clean staged mobile resources (preserve .gitkeep and .gitignore)
    $pwaDest = Join-Path $TauriDir "resources\mobile-pwa"
    $apkDest = Join-Path $TauriDir "resources\mobile-apk"
    if (Test-Path $pwaDest) {
        Get-ChildItem $pwaDest -Exclude ".gitkeep", ".gitignore" -Recurse | Remove-Item -Recurse -Force -ErrorAction SilentlyContinue
    }
    if (Test-Path $apkDest) {
        Get-ChildItem $apkDest -Exclude ".gitkeep", ".gitignore" -Recurse | Remove-Item -Recurse -Force -ErrorAction SilentlyContinue
    }
    Log-Ok "Cleaned: staged mobile resources"

    if (Test-Path $PackageDir) { Remove-Item $PackageDir -Recurse -Force }
    Log-Ok "Cleaned: package\"

    Log-Ok "Clean complete"
}

# ── Setup ─────────────────────────────────────────────────────────────────

function Invoke-Setup {
    Log-Step "Setting up Coheara development environment"

    # 1. Install npm dependencies
    Log-Info "Installing npm dependencies..."
    Push-Location $ProjectRoot
    try {
        # --force: bypass EBADPLATFORM for cross-platform lockfile (WSL2 + Windows)
        & npm install --force
        if ($LASTEXITCODE -ne 0) { throw "npm install failed" }
        Log-Ok "npm dependencies installed"
    } finally { Pop-Location }

    # 2. Verify key packages
    Push-Location $ProjectRoot
    try {
        $fbVer = & node -e "console.log(require('./node_modules/flowbite-svelte/package.json').version)" 2>$null
        if ($fbVer) { Log-Ok "flowbite-svelte@$fbVer" } else { Log-Error "flowbite-svelte not found" }

        $icVer = & node -e "console.log(require('./node_modules/flowbite-svelte-icons/package.json').version)" 2>$null
        if ($icVer) { Log-Ok "flowbite-svelte-icons@$icVer" } else { Log-Error "flowbite-svelte-icons not found" }
    } finally { Pop-Location }

    # 3. Build i18n locale files
    Log-Info "Building i18n locale files..."
    Push-Location $ProjectRoot
    try {
        & node src/lib/i18n/build-locales.js
        Log-Ok "i18n locales built"
    } finally { Pop-Location }

    # 4. Check Rust toolchain
    if ($CargoPath) {
        $rustVer = & $CargoPath --version 2>$null | Select-Object -First 1
        Log-Ok "Rust: $rustVer"
    } else {
        Log-Warn "Rust: not found (frontend-only mode will still work)"
    }

    # 5. npm audit summary
    Log-Info "Running npm audit..."
    Push-Location $ProjectRoot
    try {
        & npm audit 2>$null | Select-Object -Last 5 | ForEach-Object { Log-Info "  $_" }
    } finally { Pop-Location }

    Write-Host ""
    Log-Ok "Setup complete. Run: .\build.ps1 desktop -NoSign  (build) or use dev.sh in WSL2 (dev)"
}

# ── Orchestration ──────────────────────────────────────────────────────────

function Invoke-Desktop {
    Detect-PrestagedMobile
    Check-Dependencies "desktop"
    Load-Credentials
    Build-Frontend
    if (-not $MobileSkipped) {
        Build-MobilePwa
        Build-AndroidApk
        Stage-MobileResources
    }
    Build-Desktop
    Collect-Artifacts
}

function Invoke-Android {
    if ($MobileSkipped) {
        Log-Error "-SkipMobile cannot be used with 'android' command"
        exit 1
    }
    Check-Dependencies "android"
    Load-Credentials
    Build-MobilePwa
    Build-AndroidApk
    Collect-Artifacts
}

function Invoke-All {
    Detect-PrestagedMobile
    Check-Dependencies "all"
    Load-Credentials
    Build-Frontend
    if (-not $MobileSkipped) {
        Build-MobilePwa
        Build-AndroidApk
        Stage-MobileResources
    }
    Build-Desktop
    Collect-Artifacts
}

# ── Main ───────────────────────────────────────────────────────────────────

Write-Host "Coheara Build System v$Version (Windows)" -ForegroundColor White
Write-Host "Command: $Command  Signing: $Sign  Platform: Windows" -ForegroundColor Gray
Write-Host ""

switch ($Command) {
    "setup"   { Invoke-Setup }
    "desktop" { Invoke-Desktop }
    "android" { Invoke-Android }
    "all"     { Invoke-All }
    "clean"   { Invoke-Clean }
    "help"    { Show-Usage }
}

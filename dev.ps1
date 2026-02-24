#Requires -Version 5.1
<#
.SYNOPSIS
    Coheara Development Server — fast iteration without full builds.
.DESCRIPTION
    Wraps Tauri dev mode, frontend-only mode, type checking, and test runs.
    No signing keys, no installer packaging — just code and see results.
    Use -Rebuild to wipe all dev data and caches for a clean start.
.EXAMPLE
    .\dev.ps1                       # Full stack: Svelte HMR + Rust backend
    .\dev.ps1 frontend              # Frontend only (no Rust)
    .\dev.ps1 setup                 # First-time setup
    .\dev.ps1 full -Rebuild         # Wipe everything, fresh start
    .\dev.ps1 frontend -Rebuild     # Wipe + UI-only dev
    .\dev.ps1 check                 # Type-check everything
    .\dev.ps1 test                  # Run all tests
    .\dev.ps1 test:watch            # Watch mode for frontend tests
#>

[CmdletBinding()]
param(
    [Parameter(Position = 0)]
    [ValidateSet("full", "frontend", "check", "test", "test:watch", "setup", "help")]
    [string]$Command = "full",

    [switch]$Rebuild
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

# ── Constants ─────────────────────────────────────────────────────────────
$ProjectRoot = $PSScriptRoot
$TauriDir    = Join-Path $ProjectRoot "src-tauri"

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

# ── Logging ───────────────────────────────────────────────────────────────
function Log-Info  { param([string]$Msg) Write-Host "[INFO]  $Msg" -ForegroundColor Cyan }
function Log-Ok    { param([string]$Msg) Write-Host "[OK]    $Msg" -ForegroundColor Green }
function Log-Warn  { param([string]$Msg) Write-Host "[WARN]  $Msg" -ForegroundColor Yellow }
function Log-Error { param([string]$Msg) Write-Host "[ERROR] $Msg" -ForegroundColor Red }
function Log-Step  { param([string]$Msg) Write-Host "`n==> $Msg" -ForegroundColor White -BackgroundColor DarkGray }

# ── Usage ─────────────────────────────────────────────────────────────────
function Show-Usage {
    Write-Host @"
Coheara Development Server

Usage: .\dev.ps1 [command] [options]

Commands:
  setup       First-time setup: install deps, verify toolchain, build i18n
              Run this after cloning or when package.json changes.

  full        Full stack: Svelte HMR + Rust backend (default)
              Frontend changes: instant (<1s via HMR)
              Rust changes: ~10-30s (incremental compilation)

  frontend    Frontend only: Svelte + Vite dev server
              No Rust compilation. Tauri IPC calls will fail.
              Use when working on UI layout, styling, i18n, components.

  check       Type-check everything without running
              Runs: npm run check + cargo check

  test        Run all test suites
              Runs: npx vitest run + cargo nextest (or cargo test fallback)

  test:watch  Watch mode for frontend tests
              Re-runs affected tests on file save.

  help        Show this help

Options:
  -Rebuild    Hard reset before starting: wipe ALL dev data, caches, and
              node_modules. Use after deep changes (schema, migrations,
              data model). Everything rebuilt fresh.
              Deletes: ~/Coheara-dev, .svelte-kit, node_modules, i18n generated.

Iteration speeds:
  Svelte component change  ->  <1 second (HMR)
  Rust code change         ->  10-30 seconds (incremental)
  Full production build    ->  5-30 minutes (use build.ps1 instead)

Examples:
  .\dev.ps1                      # Full stack (default)
  .\dev.ps1 frontend             # UI-only dev
  .\dev.ps1 full -Rebuild        # Wipe everything, fresh start
  .\dev.ps1 frontend -Rebuild    # Wipe + UI-only dev
"@
}

# ── Hard Reset (-Rebuild) ─────────────────────────────────────────────────
function Invoke-Rebuild {
    $devDataDir = Join-Path $env:USERPROFILE "Coheara-dev"
    $svelteKitDir = Join-Path $ProjectRoot ".svelte-kit"
    $nodeModulesDir = Join-Path $ProjectRoot "node_modules"
    $generatedDir = Join-Path $ProjectRoot "src\lib\i18n\locales\_generated"

    Log-Step "HARD RESET - wiping all dev data and caches"
    Write-Host ""
    Log-Warn "This will permanently delete:"
    if (Test-Path $devDataDir) { Log-Warn "  * $devDataDir (profiles, databases, encrypted files)" }
    if (Test-Path $svelteKitDir) { Log-Warn "  * .svelte-kit\ (SvelteKit cache)" }
    if (Test-Path $nodeModulesDir) { Log-Warn "  * node_modules\ (npm packages)" }
    if (Test-Path $generatedDir) { Log-Warn "  * i18n generated locales" }
    $targetDir = Join-Path $TauriDir "target"
    if (Test-Path $targetDir) { Log-Warn "  * Rust build artifacts (target/)" }
    Write-Host ""

    $answer = Read-Host "  Proceed with hard reset? [y/N]"
    if ($answer -notin @("y", "Y", "yes", "Yes")) {
        Log-Info "Aborted."
        exit 0
    }

    # 1. Dev app data (profiles, SQLite DBs, encrypted files, models)
    if (Test-Path $devDataDir) {
        Remove-Item $devDataDir -Recurse -Force
        Log-Ok "Deleted $devDataDir"
    }

    # 2. SvelteKit cache
    if (Test-Path $svelteKitDir) {
        Remove-Item $svelteKitDir -Recurse -Force
        Log-Ok "Deleted .svelte-kit\"
    }

    # 3. node_modules (forces fresh npm ci)
    if (Test-Path $nodeModulesDir) {
        Remove-Item $nodeModulesDir -Recurse -Force
        Log-Ok "Deleted node_modules\"
    }

    # 4. Generated i18n locales (rebuilt by Ensure-Deps)
    if (Test-Path $generatedDir) {
        Remove-Item $generatedDir -Recurse -Force
        Log-Ok "Deleted i18n generated locales"
    }

    # 5. Rust build artifacts (full clean, forces fresh compilation)
    $targetDir = Join-Path $TauriDir "target"
    if (Test-Path $targetDir) {
        Remove-Item $targetDir -Recurse -Force
        Log-Ok "Deleted Rust target/ (full rebuild on next compile)"
    }

    Write-Host ""
    Log-Ok "Hard reset complete - everything will be rebuilt fresh"
    Write-Host ""
}

# ── PDFium bootstrap ─────────────────────────────────────────────────────
# R3: pdfium-render requires the PDFium dynamic library at runtime.
# Downloads pre-built binary from bblanchon/pdfium-binaries (Chromium PDFium).
$PdfiumVersion = "chromium/7690"
$PdfiumCacheDir = Join-Path $TauriDir "resources\pdfium"

function Ensure-Pdfium {
    $libName = "pdfium.dll"
    $platform = "win-x64"
    $libPath = Join-Path $PdfiumCacheDir "bin\$libName"

    # Skip if already cached
    if (Test-Path $libPath) {
        $env:PDFIUM_DYNAMIC_LIB_PATH = $libPath
        Log-Ok "PDFium ready: $libPath"
        return
    }

    Log-Step "Downloading PDFium ($platform)"
    New-Item -ItemType Directory -Path $PdfiumCacheDir -Force | Out-Null

    $url = "https://github.com/bblanchon/pdfium-binaries/releases/download/$PdfiumVersion/pdfium-$platform.tgz"
    $tmpFile = Join-Path $env:TEMP "pdfium-$([guid]::NewGuid().ToString('N').Substring(0,8)).tgz"

    try {
        Invoke-WebRequest -Uri $url -OutFile $tmpFile -UseBasicParsing
        & tar xzf $tmpFile -C $PdfiumCacheDir
        Remove-Item $tmpFile -Force -ErrorAction SilentlyContinue

        if (Test-Path $libPath) {
            $env:PDFIUM_DYNAMIC_LIB_PATH = $libPath
            Log-Ok "PDFium downloaded: $libPath"
        } else {
            Log-Error "PDFium archive extracted but $libName not found in $PdfiumCacheDir"
            Log-Info "Expected: $libPath"
            if (Test-Path $PdfiumCacheDir) { Get-ChildItem $PdfiumCacheDir | Format-Table Name }
            throw "PDFium extraction failed"
        }
    } catch {
        Remove-Item $tmpFile -Force -ErrorAction SilentlyContinue
        Log-Error "Failed to download PDFium from $url"
        Log-Info "Manually download and set PDFIUM_DYNAMIC_LIB_PATH=\path\to\$libName"
        throw
    }
}

# ── Dependency bootstrap ──────────────────────────────────────────────────
function Ensure-Deps {
    # Install node_modules if missing
    if (-not (Test-Path (Join-Path $ProjectRoot "node_modules"))) {
        Log-Step "Installing npm dependencies"
        Push-Location $ProjectRoot
        # --force: lockfile may have been generated on WSL2 (Linux) with
        # platform-specific optional deps (e.g. @rollup/rollup-linux-x64-gnu).
        # --force bypasses EBADPLATFORM and adds Windows binaries alongside Linux ones,
        # so both WSL2 and Windows share the same node_modules.
        try {
            & npm install --force
            if ($LASTEXITCODE -ne 0) {
                Log-Error "npm install failed"
                exit 1
            }
        } finally { Pop-Location }
    }

    # Build i18n locale files if missing
    $generatedDir = Join-Path $ProjectRoot "src\lib\i18n\locales\_generated"
    if (-not (Test-Path $generatedDir) -or
        (Get-ChildItem $generatedDir -File -ErrorAction SilentlyContinue).Count -eq 0) {
        Log-Step "Building i18n locale files"
        Push-Location $ProjectRoot
        try { & node src/lib/i18n/build-locales.js } finally { Pop-Location }
    }

    # Ensure PDFium for vision-based PDF extraction (R3)
    Ensure-Pdfium
}

# ── Commands ──────────────────────────────────────────────────────────────

function Invoke-Full {
    Log-Step "Starting full-stack dev server (Svelte + Rust)"
    Ensure-Deps

    if (-not $CargoPath) {
        Log-Error "Cargo not found. Full-stack mode requires Rust."
        Log-Info "Install Rust: winget install Rustlang.Rustup"
        Log-Info "Or use frontend-only mode: .\dev.ps1 frontend"
        exit 1
    }

    # Set dev-friendly defaults (OBS-01, DX-03)
    if (-not $env:RUST_LOG) { $env:RUST_LOG = "coheara=debug" }

    Log-Info "Frontend: http://localhost:1420 (Vite HMR)"
    Log-Info "Backend:  Tauri native window (Rust debug build)"
    Log-Info "DevTools: Enabled (F12 to inspect)"
    Log-Info "Logging:  $($env:RUST_LOG)"
    Log-Info "Data dir: ~/Coheara-dev"
    Log-Info "Stop:     Ctrl+C"
    Write-Host ""

    Push-Location $ProjectRoot
    try {
        & npx tauri dev --features devtools --config src-tauri/tauri.dev.conf.json
    } finally { Pop-Location }
}

function Invoke-Frontend {
    Log-Step "Starting frontend-only dev server (no Rust)"
    Ensure-Deps

    Log-Info "Server:   http://localhost:1420"
    Log-Info "HMR:      Enabled (changes appear instantly)"
    Log-Info "Backend:  NONE - Tauri IPC calls will fail"
    Log-Info "Stop:     Ctrl+C"
    Write-Host ""
    Log-Warn "This mode is for UI work only. Use '.\dev.ps1 full' to test IPC commands."
    Write-Host ""

    Push-Location $ProjectRoot
    try { & npm run dev } finally { Pop-Location }
}

function Invoke-Check {
    Log-Step "Type-checking all layers"
    Ensure-Deps

    $exitCode = 0

    # Svelte/TypeScript check
    Log-Info "Running Svelte/TypeScript check..."
    $prevEAP = $ErrorActionPreference; $ErrorActionPreference = "Continue"
    & npm run check --prefix $ProjectRoot
    if ($LASTEXITCODE -eq 0) {
        Log-Ok "Svelte/TypeScript: clean"
    } else {
        Log-Error "Svelte/TypeScript: errors found"
        $exitCode = 1
    }

    # Rust check
    if ($CargoPath) {
        Log-Info "Running Rust cargo check..."
        & $CargoPath check --manifest-path (Join-Path $TauriDir "Cargo.toml")
        if ($LASTEXITCODE -eq 0) {
            Log-Ok "Rust: clean"
        } else {
            Log-Error "Rust: errors found"
            $exitCode = 1
        }
    } else {
        Log-Warn "Rust: skipped (cargo not found)"
    }

    $ErrorActionPreference = $prevEAP

    Write-Host ""
    if ($exitCode -eq 0) {
        Log-Ok "All type checks passed"
    } else {
        Log-Error "Type check failures detected"
        exit $exitCode
    }
}

function Invoke-Test {
    Log-Step "Running all test suites"
    Ensure-Deps

    $exitCode = 0
    $prevEAP = $ErrorActionPreference; $ErrorActionPreference = "Continue"

    # Frontend tests
    Log-Info "Running frontend tests (Vitest)..."
    Push-Location $ProjectRoot
    try {
        & npx vitest run
        if ($LASTEXITCODE -eq 0) {
            Log-Ok "Frontend tests: all passed"
        } else {
            Log-Error "Frontend tests: failures"
            $exitCode = 1
        }
    } finally { Pop-Location }

    # Rust tests — prefer nextest (parallel) with cargo test fallback
    if ($CargoPath) {
        Write-Host ""
        $nextest = Get-Command cargo-nextest -ErrorAction SilentlyContinue
        if ($nextest) {
            Log-Info "Running Rust tests (cargo nextest — parallel)..."
            & $CargoPath nextest run --manifest-path (Join-Path $TauriDir "Cargo.toml")
            if ($LASTEXITCODE -eq 0) {
                Log-Ok "Rust tests: all passed"
            } else {
                Log-Error "Rust tests: failures"
                $exitCode = 1
            }
            # Doctests not supported by nextest — run separately
            Log-Info "Running Rust doctests..."
            & $CargoPath test --manifest-path (Join-Path $TauriDir "Cargo.toml") --doc 2>$null
            if ($LASTEXITCODE -ne 0) {
                Log-Warn "Rust doctests: some failures (non-blocking)"
            }
        } else {
            Log-Info "Running Rust tests (cargo test)..."
            & $CargoPath test --manifest-path (Join-Path $TauriDir "Cargo.toml")
            if ($LASTEXITCODE -eq 0) {
                Log-Ok "Rust tests: all passed"
            } else {
                Log-Error "Rust tests: failures"
                $exitCode = 1
            }
        }
    } else {
        Log-Warn "Rust tests: skipped (cargo not found)"
    }

    $ErrorActionPreference = $prevEAP

    Write-Host ""
    if ($exitCode -eq 0) {
        Log-Ok "All tests passed"
    } else {
        Log-Error "Test failures detected"
        exit $exitCode
    }
}

function Invoke-TestWatch {
    Log-Step "Starting frontend test watch mode"
    Ensure-Deps

    Log-Info "Watching for changes - tests re-run on save"
    Log-Info "Stop: Ctrl+C"
    Write-Host ""

    Push-Location $ProjectRoot
    try { & npx vitest } finally { Pop-Location }
}

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
    } finally { Pop-Location }

    # 3. Build i18n locale files
    Log-Info "Building i18n locale files..."
    Push-Location $ProjectRoot
    try {
        & node src/lib/i18n/build-locales.js
        Log-Ok "i18n locales built"
    } finally { Pop-Location }

    # 4. Ensure PDFium for vision-based PDF extraction (R3)
    Ensure-Pdfium

    # 5. Check Rust toolchain
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
    Log-Ok "Setup complete. Run: .\dev.ps1 frontend  (UI dev) or .\dev.ps1 full (full stack)"
}

# ── Main ──────────────────────────────────────────────────────────────────
Write-Host "Coheara Dev - $($Command.ToUpper()) mode" -ForegroundColor White
if ($Rebuild) {
    Write-Host "  -Rebuild active" -ForegroundColor Yellow
}
Write-Host ""

# Execute rebuild before any command
if ($Rebuild) {
    Invoke-Rebuild
}

switch ($Command) {
    "full"       { Invoke-Full }
    "frontend"   { Invoke-Frontend }
    "check"      { Invoke-Check }
    "test"       { Invoke-Test }
    "test:watch" { Invoke-TestWatch }
    "setup"      { Invoke-Setup }
    "help"       { Show-Usage }
}

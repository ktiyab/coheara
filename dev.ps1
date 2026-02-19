#Requires -Version 5.1
<#
.SYNOPSIS
    Coheara Development Server — fast iteration without full builds.
.DESCRIPTION
    Wraps Tauri dev mode, frontend-only mode, type checking, and test runs.
    No signing keys, no installer packaging — just code and see results.
.EXAMPLE
    .\dev.ps1                   # Full stack: Svelte HMR + Rust backend
    .\dev.ps1 frontend          # Frontend only (no Rust)
    .\dev.ps1 check             # Type-check everything
    .\dev.ps1 test              # Run all tests
    .\dev.ps1 test:watch        # Watch mode for frontend tests
#>

[CmdletBinding()]
param(
    [Parameter(Position = 0)]
    [ValidateSet("full", "frontend", "check", "test", "test:watch", "help")]
    [string]$Command = "full"
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

Usage: .\dev.ps1 [command]

Commands:
  full        Full stack: Svelte HMR + Rust backend (default)
              Frontend changes: instant (<1s via HMR)
              Rust changes: ~10-30s (incremental compilation)

  frontend    Frontend only: Svelte + Vite dev server
              No Rust compilation. Tauri IPC calls will fail.
              Use when working on UI layout, styling, i18n, components.

  check       Type-check everything without running
              Runs: npm run check + cargo check

  test        Run all test suites
              Runs: npx vitest run + cargo test

  test:watch  Watch mode for frontend tests
              Re-runs affected tests on file save.

  help        Show this help

Iteration speeds:
  Svelte component change  ->  <1 second (HMR)
  Rust code change         ->  10-30 seconds (incremental)
  Full production build    ->  5-30 minutes (use build.ps1 instead)
"@
}

# ── Dependency bootstrap ──────────────────────────────────────────────────
function Ensure-Deps {
    # Install node_modules if missing
    if (-not (Test-Path (Join-Path $ProjectRoot "node_modules"))) {
        Log-Step "Installing npm dependencies"
        Push-Location $ProjectRoot
        try { & npm ci } finally { Pop-Location }
    }

    # Build i18n locale files if missing
    $generatedDir = Join-Path $ProjectRoot "src\lib\i18n\locales\_generated"
    if (-not (Test-Path $generatedDir) -or
        (Get-ChildItem $generatedDir -File -ErrorAction SilentlyContinue).Count -eq 0) {
        Log-Step "Building i18n locale files"
        Push-Location $ProjectRoot
        try { & node src/lib/i18n/build-locales.js } finally { Pop-Location }
    }
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

    # Rust tests
    if ($CargoPath) {
        Write-Host ""
        Log-Info "Running Rust tests (cargo test)..."
        & $CargoPath test --manifest-path (Join-Path $TauriDir "Cargo.toml")
        if ($LASTEXITCODE -eq 0) {
            Log-Ok "Rust tests: all passed"
        } else {
            Log-Error "Rust tests: failures"
            $exitCode = 1
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

# ── Main ──────────────────────────────────────────────────────────────────
Write-Host "Coheara Dev - $($Command.ToUpper()) mode" -ForegroundColor White
Write-Host ""

switch ($Command) {
    "full"       { Invoke-Full }
    "frontend"   { Invoke-Frontend }
    "check"      { Invoke-Check }
    "test"       { Invoke-Test }
    "test:watch" { Invoke-TestWatch }
    "help"       { Show-Usage }
}

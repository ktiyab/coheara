#Requires -Version 5.1
<#
.SYNOPSIS
    Coheara Security Audit — scans Rust and npm dependencies for known vulnerabilities.
.DESCRIPTION
    Runs cargo audit (Rust), npm audit (frontend), and npm audit (mobile) to detect
    CVEs in the dependency chain. Results are written to AUDIT.txt.
    Requires: Rust/cargo, Node.js/npm. cargo-audit is auto-installed if missing.
.EXAMPLE
    .\audit.ps1              # Full audit, output to AUDIT.txt
    .\audit.ps1 -CI          # Exit 1 on critical/high (CI gate mode)
    .\audit.ps1 -Fix         # Attempt npm audit fix for resolvable issues
#>

[CmdletBinding()]
param(
    [switch]$CI,
    [switch]$Fix,
    [switch]$Help
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

# ── Constants ─────────────────────────────────────────────────────────────
$ProjectRoot = $PSScriptRoot
$TauriDir    = Join-Path $ProjectRoot "src-tauri"
$MobileDir   = Join-Path $ProjectRoot "mobile"
$AuditFile   = Join-Path $ProjectRoot "AUDIT.txt"

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
if ($Help) {
    Write-Host @"
Coheara Security Audit

Usage: .\audit.ps1 [options]

Options:
  -CI    Exit 1 on critical/high vulnerabilities (for CI pipelines)
  -Fix   Attempt npm audit fix for auto-resolvable issues
  -Help  Show this help

Output:
  Results written to .\AUDIT.txt

Components scanned:
  1. Rust crates (via cargo audit)
  2. Frontend npm packages (./package.json)
  3. Mobile npm packages (./mobile/package.json)
"@
    exit 0
}

# ── Counters ──────────────────────────────────────────────────────────────
$script:TotalCritical = 0
$script:TotalHigh     = 0
$script:TotalModerate = 0
$script:TotalLow      = 0

# ── AUDIT.txt helpers ─────────────────────────────────────────────────────
function Init-AuditFile {
    $timestamp = (Get-Date).ToUniversalTime().ToString("yyyy-MM-dd HH:mm:ss 'UTC'")
    @"
================================================================================
COHEARA SECURITY AUDIT REPORT
================================================================================
Date:     $timestamp
Host:     $env:COMPUTERNAME
Platform: Windows $([Environment]::OSVersion.Version)
================================================================================

"@ | Set-Content $AuditFile -Encoding UTF8
}

function Append-Audit {
    param([string]$Text)
    $Text | Add-Content $AuditFile -Encoding UTF8
}

function Append-Section {
    param([string]$Title)
    @"

--------------------------------------------------------------------------------
$Title
--------------------------------------------------------------------------------

"@ | Add-Content $AuditFile -Encoding UTF8
}

# ── Rust Audit ────────────────────────────────────────────────────────────
function Audit-Rust {
    Log-Step "Auditing Rust dependencies (cargo audit)"
    Append-Section "RUST DEPENDENCIES (cargo audit)"

    if (-not $CargoPath) {
        Log-Warn "Cargo not found - skipping Rust audit"
        Append-Audit "SKIPPED: Cargo not found"
        return
    }

    # Install cargo-audit if missing
    $cargoAudit = Get-Command cargo-audit -ErrorAction SilentlyContinue
    if (-not $cargoAudit) {
        Log-Info "Installing cargo-audit..."
        $prevEAP = $ErrorActionPreference; $ErrorActionPreference = "Continue"
        & $CargoPath install cargo-audit --quiet 2>&1 | Out-Null
        $ErrorActionPreference = $prevEAP

        $cargoAudit = Get-Command cargo-audit -ErrorAction SilentlyContinue
        if (-not $cargoAudit) {
            Log-Error "Failed to install cargo-audit"
            Append-Audit "SKIPPED: Failed to install cargo-audit"
            return
        }
    }

    $prevEAP = $ErrorActionPreference; $ErrorActionPreference = "Continue"
    $auditOutput = & cargo audit --manifest-path (Join-Path $TauriDir "Cargo.toml") 2>&1 | Out-String
    $auditExit = $LASTEXITCODE
    $ErrorActionPreference = $prevEAP

    Append-Audit $auditOutput

    if ($auditExit -eq 0) {
        Log-Ok "Rust: No known vulnerabilities found"
        Append-Audit "RESULT: CLEAN - No known vulnerabilities"
    } else {
        $critCount = ([regex]::Matches($auditOutput, "(?i)critical")).Count
        $highCount = ([regex]::Matches($auditOutput, "(?i)high")).Count
        $vulnCount = ([regex]::Matches($auditOutput, "RUSTSEC-")).Count

        $script:TotalCritical += $critCount
        $script:TotalHigh += $highCount

        if ($critCount -gt 0) {
            Log-Error "Rust: $vulnCount advisories found ($critCount critical)"
        } elseif ($highCount -gt 0) {
            Log-Warn "Rust: $vulnCount advisories found ($highCount high)"
        } else {
            Log-Warn "Rust: $vulnCount advisories found"
        }
        Append-Audit "RESULT: $vulnCount advisories (critical=$critCount, high=$highCount)"
    }
}

# ── npm Audit (generic) ──────────────────────────────────────────────────
function Audit-Npm {
    param(
        [string]$Label,
        [string]$Dir
    )

    Log-Step "Auditing $Label npm dependencies"
    Append-Section "$Label NPM DEPENDENCIES (npm audit)"

    if (-not (Test-Path $Dir)) {
        Log-Warn "$Label`: Directory not found ($Dir) - skipping"
        Append-Audit "SKIPPED: Directory not found ($Dir)"
        return
    }

    $packageJson = Join-Path $Dir "package.json"
    if (-not (Test-Path $packageJson)) {
        Log-Warn "$Label`: No package.json found - skipping"
        Append-Audit "SKIPPED: No package.json found"
        return
    }

    # Run npm audit in JSON mode for structured parsing
    $prevEAP = $ErrorActionPreference; $ErrorActionPreference = "Continue"
    $auditJson = & npm audit --json --prefix $Dir 2>$null | Out-String
    $ErrorActionPreference = $prevEAP

    # Parse JSON for severity counts
    $critical = 0; $high = 0; $moderate = 0; $low = 0
    try {
        $parsed = $auditJson | ConvertFrom-Json -ErrorAction Stop
        $vulns = if ($parsed.metadata.vulnerabilities) { $parsed.metadata.vulnerabilities }
                 elseif ($parsed.vulnerabilities) { $parsed.vulnerabilities }
                 else { $null }
        if ($vulns) {
            $critical = [int]($vulns.critical)
            $high     = [int]($vulns.high)
            $moderate = [int]($vulns.moderate)
            $low      = [int]($vulns.low)
        }
    } catch {
        Log-Warn "$Label`: Could not parse npm audit JSON — falling back to text output"
    }

    # Write human-readable output
    $prevEAP = $ErrorActionPreference; $ErrorActionPreference = "Continue"
    $auditReadable = & npm audit --prefix $Dir 2>&1 | Out-String
    $ErrorActionPreference = $prevEAP

    Append-Audit $auditReadable

    $total = $critical + $high + $moderate + $low
    $script:TotalCritical += $critical
    $script:TotalHigh     += $high
    $script:TotalModerate += $moderate
    $script:TotalLow      += $low

    $severitySummary = "critical=$critical, high=$high, moderate=$moderate, low=$low"

    if ($total -eq 0) {
        Log-Ok "$Label`: No known vulnerabilities found"
        Append-Audit "RESULT: CLEAN - No known vulnerabilities"
    } else {
        if ($critical -gt 0) {
            Log-Error "$Label`: $total vulnerabilities ($severitySummary)"
        } elseif ($high -gt 0) {
            Log-Warn "$Label`: $total vulnerabilities ($severitySummary)"
        } else {
            Log-Info "$Label`: $total vulnerabilities ($severitySummary)"
        }
        Append-Audit "RESULT: $total vulnerabilities ($severitySummary)"
    }

    # Optional fix
    if ($Fix -and $total -gt 0) {
        Log-Info "$Label`: Attempting npm audit fix..."
        Append-Audit ""
        Append-Audit "--- npm audit fix ---"
        $prevEAP = $ErrorActionPreference; $ErrorActionPreference = "Continue"
        $fixOutput = & npm audit fix --prefix $Dir 2>&1 | Out-String
        $ErrorActionPreference = $prevEAP
        Append-Audit $fixOutput
        Log-Ok "$Label`: npm audit fix completed (re-run audit to verify)"
    }
}

# ── Summary ───────────────────────────────────────────────────────────────
function Write-AuditSummary {
    $total = $script:TotalCritical + $script:TotalHigh + $script:TotalModerate + $script:TotalLow

    Append-Section "SUMMARY"
    Append-Audit "  Critical:  $($script:TotalCritical)"
    Append-Audit "  High:      $($script:TotalHigh)"
    Append-Audit "  Moderate:  $($script:TotalModerate)"
    Append-Audit "  Low:       $($script:TotalLow)"
    Append-Audit "  -----------------"
    Append-Audit "  Total:     $total"
    Append-Audit ""

    if ($total -eq 0) {
        Append-Audit "VERDICT: CLEAN - No known vulnerabilities across all components."
        Log-Ok "All components clean - no known vulnerabilities"
    } elseif ($script:TotalCritical -gt 0 -or $script:TotalHigh -gt 0) {
        Append-Audit "VERDICT: FAIL - $($script:TotalCritical) critical + $($script:TotalHigh) high vulnerabilities require attention."
        Log-Error "FAIL: $($script:TotalCritical) critical + $($script:TotalHigh) high vulnerabilities found"
    } else {
        Append-Audit "VERDICT: WARN - $total vulnerabilities found (moderate/low only)."
        Log-Warn "WARN: $total vulnerabilities (moderate/low) - review recommended"
    }

    Append-Audit ""
    Append-Audit "Report written: $AuditFile"

    Write-Host ""
    Log-Step "Audit report written to: $AuditFile"
}

# ── Main ──────────────────────────────────────────────────────────────────
Write-Host "Coheara Security Audit" -ForegroundColor White
$modeLabel = if ($CI) { "CI (strict)" } else { "Report" }
Write-Host "Mode: $modeLabel  Fix: $Fix" -ForegroundColor Gray
Write-Host ""

Init-AuditFile
Audit-Rust
Audit-Npm -Label "FRONTEND" -Dir $ProjectRoot
Audit-Npm -Label "MOBILE" -Dir $MobileDir
Write-AuditSummary

# CI gate: exit 1 if critical or high vulnerabilities found
if ($CI -and ($script:TotalCritical -gt 0 -or $script:TotalHigh -gt 0)) {
    Log-Error "CI gate: Failing due to critical/high vulnerabilities"
    exit 1
}

exit 0

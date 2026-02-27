<#
.SYNOPSIS
    Configure Ollama for AMD GPU acceleration on Windows.

.DESCRIPTION
    This script configures environment variables to enable GPU acceleration
    in Ollama for AMD graphics cards. It supports two backends:
    - Vulkan (works with any AMD GPU)
    - ROCm (works with officially supported AMD GPUs)

    The script does NOT install Ollama itself. Install it from https://ollama.com/download

.PARAMETER Backend
    GPU backend to configure: "vulkan" (default) or "rocm".
    Vulkan works with any GPU. ROCm requires a supported GPU + HIP SDK.

.PARAMETER AllowRemote
    If set, configures Ollama to accept connections from WSL2 and other hosts.
    Sets OLLAMA_HOST=0.0.0.0 (listens on all interfaces).

.PARAMETER GfxOverride
    ROCm architecture override (e.g., "10.3.0" for gfx1030).
    Only used with -Backend rocm. Use when your GPU is not officially supported.

.PARAMETER Undo
    Remove all Ollama GPU environment variables set by this script.

.PARAMETER Check
    Check current GPU configuration and Ollama status without making changes.

.EXAMPLE
    .\setup-ollama-gpu.ps1
    # Configures Vulkan backend (default, works with any AMD GPU)

.EXAMPLE
    .\setup-ollama-gpu.ps1 -Backend vulkan -AllowRemote
    # Configures Vulkan + allows WSL2 connections

.EXAMPLE
    .\setup-ollama-gpu.ps1 -Backend rocm -GfxOverride "10.3.0"
    # Configures ROCm with gfx1030 architecture override

.EXAMPLE
    .\setup-ollama-gpu.ps1 -Check
    # Shows current configuration and GPU detection status

.EXAMPLE
    .\setup-ollama-gpu.ps1 -Undo
    # Removes all GPU-related environment variables
#>

[CmdletBinding()]
param(
    [ValidateSet("vulkan", "rocm")]
    [string]$Backend = "vulkan",

    [switch]$AllowRemote,

    [string]$GfxOverride,

    [switch]$Undo,

    [switch]$Check
)

$ErrorActionPreference = "Stop"

# --- Helper Functions ---

function Write-Header {
    param([string]$Text)
    Write-Host ""
    Write-Host "=== $Text ===" -ForegroundColor Cyan
    Write-Host ""
}

function Write-Status {
    param([string]$Label, [string]$Value, [string]$Color = "White")
    Write-Host "  $Label : " -NoNewline
    Write-Host $Value -ForegroundColor $Color
}

function Get-EnvVar {
    param([string]$Name)
    $user = [System.Environment]::GetEnvironmentVariable($Name, 'User')
    $machine = [System.Environment]::GetEnvironmentVariable($Name, 'Machine')
    if ($user) { return @{ Value = $user; Scope = "User" } }
    if ($machine) { return @{ Value = $machine; Scope = "Machine" } }
    return $null
}

function Set-EnvVarSafe {
    param([string]$Name, [string]$Value)
    try {
        [System.Environment]::SetEnvironmentVariable($Name, $Value, 'User')
        Write-Host "  SET $Name=$Value" -ForegroundColor Green
        return $true
    }
    catch {
        Write-Host "  FAILED to set $Name : $_" -ForegroundColor Red
        return $false
    }
}

function Remove-EnvVarSafe {
    param([string]$Name)
    $existing = Get-EnvVar $Name
    if ($existing) {
        try {
            [System.Environment]::SetEnvironmentVariable($Name, $null, $existing.Scope)
            Write-Host "  REMOVED $Name (was: $($existing.Value))" -ForegroundColor Yellow
        }
        catch {
            Write-Host "  FAILED to remove $Name : $_" -ForegroundColor Red
        }
    }
    else {
        Write-Host "  SKIP $Name (not set)" -ForegroundColor DarkGray
    }
}

function Get-GpuInfo {
    try {
        $gpus = Get-CimInstance Win32_VideoController | Select-Object Name, DriverVersion, AdapterRAM
        return $gpus
    }
    catch {
        return $null
    }
}

function Get-OllamaProcess {
    return Get-Process -Name "ollama*" -ErrorAction SilentlyContinue
}

function Test-OllamaInstalled {
    $path = Get-Command ollama -ErrorAction SilentlyContinue
    return $null -ne $path
}

# --- Check Mode ---

if ($Check) {
    Write-Header "GPU Information"

    $gpus = Get-GpuInfo
    if ($gpus) {
        foreach ($gpu in $gpus) {
            $vramGB = if ($gpu.AdapterRAM) { [math]::Round($gpu.AdapterRAM / 1GB, 1) } else { "Unknown" }
            Write-Status "GPU" $gpu.Name "White"
            Write-Status "Driver" $gpu.DriverVersion "White"
            Write-Status "VRAM (reported)" "$vramGB GB" "White"
        }
    }
    else {
        Write-Host "  No GPU detected" -ForegroundColor Red
    }

    Write-Header "Ollama Installation"

    if (Test-OllamaInstalled) {
        $ollamaPath = (Get-Command ollama).Source
        Write-Status "Installed" $ollamaPath "Green"
        try {
            $version = & ollama --version 2>&1
            Write-Status "Version" $version "White"
        }
        catch {
            Write-Status "Version" "Could not determine" "Yellow"
        }
    }
    else {
        Write-Status "Installed" "NOT FOUND — install from https://ollama.com/download" "Red"
    }

    $procs = Get-OllamaProcess
    if ($procs) {
        Write-Status "Running" "Yes (PID: $($procs.Id -join ', '))" "Green"
    }
    else {
        Write-Status "Running" "No" "Yellow"
    }

    Write-Header "Environment Variables"

    $vars = @(
        "OLLAMA_VULKAN",
        "OLLAMA_HOST",
        "HSA_OVERRIDE_GFX_VERSION",
        "HIP_VISIBLE_DEVICES",
        "GGML_VK_VISIBLE_DEVICES",
        "OLLAMA_GPU_OVERHEAD",
        "OLLAMA_FLASH_ATTENTION",
        "OLLAMA_CONTEXT_LENGTH",
        "OLLAMA_KEEP_ALIVE"
    )

    foreach ($var in $vars) {
        $env = Get-EnvVar $var
        if ($env) {
            Write-Status $var "$($env.Value) [$($env.Scope)]" "Green"
        }
        else {
            Write-Status $var "(not set)" "DarkGray"
        }
    }

    Write-Header "ROCm Kernel Targets (shipped with Ollama)"

    $ollamaDir = "$env:LOCALAPPDATA\Programs\Ollama\lib\ollama\rocm"
    if (Test-Path $ollamaDir) {
        $targets = Get-ChildItem $ollamaDir -Recurse -Filter "*.hsaco" |
            ForEach-Object { if ($_.Name -match 'gfx(\d+)') { $Matches[0] } } |
            Sort-Object -Unique
        if ($targets) {
            Write-Status "Compiled targets" ($targets -join ", ") "White"
        }
        else {
            Write-Status "Compiled targets" "None found" "Yellow"
        }
    }
    else {
        Write-Status "ROCm directory" "Not found at $ollamaDir" "Yellow"
    }

    Write-Host ""
    exit 0
}

# --- Undo Mode ---

if ($Undo) {
    Write-Header "Removing Ollama GPU Environment Variables"

    Remove-EnvVarSafe "OLLAMA_VULKAN"
    Remove-EnvVarSafe "HSA_OVERRIDE_GFX_VERSION"

    # Only remove OLLAMA_HOST if it was set to 0.0.0.0 (our change)
    $host_env = Get-EnvVar "OLLAMA_HOST"
    if ($host_env -and $host_env.Value -eq "0.0.0.0") {
        Remove-EnvVarSafe "OLLAMA_HOST"
    }
    elseif ($host_env) {
        Write-Host "  SKIP OLLAMA_HOST (value '$($host_env.Value)' was not set by this script)" -ForegroundColor DarkGray
    }

    Write-Host ""
    Write-Host "Done. Restart Ollama for changes to take effect." -ForegroundColor Cyan
    Write-Host "  To restart: Stop-Process -Name ollama -Force; ollama serve" -ForegroundColor White
    Write-Host ""
    exit 0
}

# --- Configure Mode ---

Write-Header "Ollama AMD GPU Configuration"

# Pre-flight checks
if (-not (Test-OllamaInstalled)) {
    Write-Host "  Ollama is not installed." -ForegroundColor Red
    Write-Host "  Download from: https://ollama.com/download" -ForegroundColor Yellow
    Write-Host ""
    exit 1
}

$gpus = Get-GpuInfo
if ($gpus) {
    $amdGpu = $gpus | Where-Object { $_.Name -like "*AMD*" -or $_.Name -like "*Radeon*" }
    if ($amdGpu) {
        Write-Status "AMD GPU detected" $amdGpu.Name "Green"
    }
    else {
        Write-Host "  No AMD GPU detected. Found: $($gpus.Name -join ', ')" -ForegroundColor Yellow
        Write-Host "  Vulkan may still work with non-AMD GPUs." -ForegroundColor Yellow
    }
}

Write-Header "Configuring Backend: $Backend"

$changes = @()

if ($Backend -eq "vulkan") {
    # Enable Vulkan
    if (Set-EnvVarSafe "OLLAMA_VULKAN" "1") {
        $changes += "OLLAMA_VULKAN=1"
    }

    # Remove ROCm override if present (avoid conflicts)
    $existing_hsa = Get-EnvVar "HSA_OVERRIDE_GFX_VERSION"
    if ($existing_hsa) {
        Write-Host "  Removing HSA_OVERRIDE_GFX_VERSION (not needed for Vulkan)" -ForegroundColor Yellow
        Remove-EnvVarSafe "HSA_OVERRIDE_GFX_VERSION"
    }
}
elseif ($Backend -eq "rocm") {
    # Disable Vulkan (ROCm takes priority)
    $existing_vulkan = Get-EnvVar "OLLAMA_VULKAN"
    if ($existing_vulkan) {
        Write-Host "  Removing OLLAMA_VULKAN (using ROCm instead)" -ForegroundColor Yellow
        Remove-EnvVarSafe "OLLAMA_VULKAN"
    }

    # Set GFX override if provided
    if ($GfxOverride) {
        if ($GfxOverride -notmatch '^\d+\.\d+\.\d+$') {
            Write-Host "  Invalid GfxOverride format. Expected: X.Y.Z (e.g., 10.3.0)" -ForegroundColor Red
            exit 1
        }
        if (Set-EnvVarSafe "HSA_OVERRIDE_GFX_VERSION" $GfxOverride) {
            $changes += "HSA_OVERRIDE_GFX_VERSION=$GfxOverride"
        }
        Write-Host ""
        Write-Host "  NOTE: ROCm with architecture override requires AMD HIP SDK installed." -ForegroundColor Yellow
        Write-Host "  Download: https://www.amd.com/en/developer/resources/rocm-hub/hip-sdk.html" -ForegroundColor Yellow
    }
}

# Remote access (WSL2 support)
if ($AllowRemote) {
    Write-Header "Configuring Remote Access (WSL2)"
    if (Set-EnvVarSafe "OLLAMA_HOST" "0.0.0.0") {
        $changes += "OLLAMA_HOST=0.0.0.0"
    }
    Write-Host ""
    Write-Host "  Ollama will listen on all interfaces (port 11434)." -ForegroundColor White
    Write-Host "  WSL2 can connect via: OLLAMA_HOST=http://<windows-ip>:11434" -ForegroundColor White
    Write-Host "  You may need to allow port 11434 in Windows Firewall." -ForegroundColor Yellow
}

# Summary
Write-Header "Configuration Complete"

if ($changes.Count -gt 0) {
    Write-Host "  Changes applied:" -ForegroundColor Green
    foreach ($change in $changes) {
        Write-Host "    $change" -ForegroundColor White
    }
}
else {
    Write-Host "  No changes were needed." -ForegroundColor Yellow
}

Write-Host ""
Write-Host "  Next steps:" -ForegroundColor Cyan
Write-Host "    1. Restart Ollama (close and reopen, or run: ollama serve)" -ForegroundColor White
Write-Host "    2. Load a model: ollama run llama3.2 'hello'" -ForegroundColor White
Write-Host "    3. Check GPU usage: ollama ps" -ForegroundColor White
Write-Host "    4. Verify with: .\setup-ollama-gpu.ps1 -Check" -ForegroundColor White
Write-Host ""

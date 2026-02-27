#!/usr/bin/env bash
# ============================================================================
# setup-ollama-gpu.sh — Configure Ollama for AMD GPU acceleration (Linux/WSL2)
# ============================================================================
#
# Usage:
#   ./setup-ollama-gpu.sh [OPTIONS]
#
# Options:
#   --backend <vulkan|rocm>   GPU backend (default: vulkan)
#   --wsl2                    Configure WSL2 to connect to Windows Ollama
#   --undo                    Remove all GPU-related configuration
#   --check                   Show current configuration (no changes)
#   --help                    Show this help
#
# Examples:
#   ./setup-ollama-gpu.sh                        # Vulkan on native Linux
#   ./setup-ollama-gpu.sh --backend vulkan       # Explicit Vulkan
#   ./setup-ollama-gpu.sh --wsl2                 # WSL2 → Windows Ollama
#   ./setup-ollama-gpu.sh --check                # Inspect current state
#   ./setup-ollama-gpu.sh --undo                 # Remove configuration
# ============================================================================

set -euo pipefail

# --- Defaults ---
BACKEND="vulkan"
WSL2_MODE=false
UNDO_MODE=false
CHECK_MODE=false
BASHRC="$HOME/.bashrc"

# --- Colors ---
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
CYAN='\033[0;36m'
GRAY='\033[0;90m'
NC='\033[0m'

# --- Parse Arguments ---
while [[ $# -gt 0 ]]; do
    case "$1" in
        --backend)
            BACKEND="$2"
            shift 2
            ;;
        --wsl2)
            WSL2_MODE=true
            shift
            ;;
        --undo)
            UNDO_MODE=true
            shift
            ;;
        --check)
            CHECK_MODE=true
            shift
            ;;
        --help|-h)
            head -20 "$0" | tail -18
            exit 0
            ;;
        *)
            echo -e "${RED}Unknown option: $1${NC}"
            echo "Use --help for usage information."
            exit 1
            ;;
    esac
done

# --- Helper Functions ---

header() {
    echo ""
    echo -e "${CYAN}=== $1 ===${NC}"
    echo ""
}

status() {
    local label="$1"
    local value="$2"
    local color="${3:-$NC}"
    printf "  %-28s %b%s%b\n" "$label:" "$color" "$value" "$NC"
}

is_wsl2() {
    grep -qi microsoft /proc/version 2>/dev/null
}

get_windows_ip() {
    ip route show default 2>/dev/null | awk '{print $3}'
}

bashrc_has() {
    grep -qF "$1" "$BASHRC" 2>/dev/null
}

bashrc_add() {
    if ! bashrc_has "$1"; then
        echo "$1" >> "$BASHRC"
        echo -e "  ${GREEN}ADDED to $BASHRC:${NC} $1"
    else
        echo -e "  ${GRAY}SKIP (already in $BASHRC):${NC} $1"
    fi
}

bashrc_remove() {
    if bashrc_has "$1"; then
        local tmp
        tmp=$(mktemp)
        grep -vF "$1" "$BASHRC" > "$tmp"
        mv "$tmp" "$BASHRC"
        echo -e "  ${YELLOW}REMOVED from $BASHRC:${NC} $1"
    else
        echo -e "  ${GRAY}SKIP (not in $BASHRC):${NC} $1"
    fi
}

# --- Check Mode ---

if $CHECK_MODE; then
    header "System Information"

    if is_wsl2; then
        status "Platform" "WSL2" "$YELLOW"
        status "Windows IP" "$(get_windows_ip)" "$NC"
    else
        status "Platform" "Native Linux" "$GREEN"
    fi

    # GPU detection
    if command -v lspci &>/dev/null; then
        gpu_info=$(lspci 2>/dev/null | grep -i 'vga\|3d\|display' | head -3)
        if [[ -n "$gpu_info" ]]; then
            while IFS= read -r line; do
                status "GPU" "$line" "$NC"
            done <<< "$gpu_info"
        else
            status "GPU" "None detected via lspci" "$RED"
        fi
    else
        status "GPU" "lspci not available" "$GRAY"
    fi

    # ROCm
    if command -v rocminfo &>/dev/null; then
        rocm_gpu=$(rocminfo 2>/dev/null | grep "Name:" | grep -v "CPU" | head -3)
        if [[ -n "$rocm_gpu" ]]; then
            status "ROCm GPUs" "$rocm_gpu" "$GREEN"
        else
            status "ROCm" "Installed but no GPUs found" "$YELLOW"
        fi
    else
        status "ROCm" "Not installed" "$GRAY"
    fi

    # Device nodes
    [[ -e /dev/kfd ]] && status "/dev/kfd" "Present (ROCm capable)" "$GREEN" || status "/dev/kfd" "Absent" "$GRAY"
    [[ -e /dev/dxg ]] && status "/dev/dxg" "Present (DirectX passthrough)" "$YELLOW" || status "/dev/dxg" "Absent" "$GRAY"

    header "Ollama Installation"

    if command -v ollama &>/dev/null; then
        status "Installed" "$(command -v ollama)" "$GREEN"
        version=$(ollama --version 2>&1 || echo "unknown")
        status "Version" "$version" "$NC"

        if pgrep -x ollama &>/dev/null; then
            pid=$(pgrep -x ollama | head -1)
            status "Running" "Yes (PID: $pid)" "$GREEN"
        else
            status "Running" "No" "$YELLOW"
        fi
    else
        status "Installed" "NOT FOUND" "$RED"
        echo -e "  Install: ${CYAN}curl -fsSL https://ollama.com/install.sh | sh${NC}"
    fi

    header "Environment Variables"

    for var in OLLAMA_VULKAN OLLAMA_HOST HSA_OVERRIDE_GFX_VERSION HIP_VISIBLE_DEVICES \
               GGML_VK_VISIBLE_DEVICES OLLAMA_GPU_OVERHEAD OLLAMA_FLASH_ATTENTION \
               OLLAMA_CONTEXT_LENGTH OLLAMA_KEEP_ALIVE; do
        val="${!var:-}"
        if [[ -n "$val" ]]; then
            status "$var" "$val" "$GREEN"
        else
            status "$var" "(not set)" "$GRAY"
        fi
    done

    # Check bashrc entries
    header "Persisted Configuration ($BASHRC)"

    for pattern in "OLLAMA_VULKAN" "OLLAMA_HOST" "HSA_OVERRIDE_GFX_VERSION"; do
        line=$(grep "$pattern" "$BASHRC" 2>/dev/null | tail -1)
        if [[ -n "$line" ]]; then
            status "$pattern" "$line" "$GREEN"
        else
            status "$pattern" "(not in bashrc)" "$GRAY"
        fi
    done

    echo ""
    exit 0
fi

# --- Undo Mode ---

if $UNDO_MODE; then
    header "Removing Ollama GPU Configuration"

    bashrc_remove 'export OLLAMA_VULKAN="1"'
    bashrc_remove 'export OLLAMA_VULKAN=1'
    bashrc_remove 'export HSA_OVERRIDE_GFX_VERSION='

    # Only remove OLLAMA_HOST if it points to a Windows IP (our WSL2 setup)
    host_line=$(grep "OLLAMA_HOST" "$BASHRC" 2>/dev/null | tail -1)
    if [[ "$host_line" == *"11434"* ]]; then
        bashrc_remove "$host_line"
    fi

    echo ""
    echo -e "  ${CYAN}Done. Run 'source ~/.bashrc' to apply.${NC}"
    echo -e "  ${NC}Restart Ollama if running: sudo systemctl restart ollama${NC}"
    echo ""
    exit 0
fi

# --- Configure Mode ---

header "Ollama AMD GPU Configuration"

# Detect platform
if is_wsl2; then
    echo -e "  ${YELLOW}Detected: WSL2${NC}"
    if ! $WSL2_MODE; then
        echo ""
        echo -e "  ${YELLOW}You're running in WSL2. AMD GPU acceleration requires${NC}"
        echo -e "  ${YELLOW}Ollama on the Windows side. Use --wsl2 flag to configure${NC}"
        echo -e "  ${YELLOW}this machine as a client to Windows Ollama.${NC}"
        echo ""
        echo -e "  ${CYAN}On Windows, run the PowerShell script first:${NC}"
        echo -e "  ${NC}  .\\setup-ollama-gpu.ps1 -Backend vulkan -AllowRemote${NC}"
        echo ""
        echo -e "  ${CYAN}Then here:${NC}"
        echo -e "  ${NC}  ./setup-ollama-gpu.sh --wsl2${NC}"
        echo ""
        exit 0
    fi
fi

# --- WSL2 Client Mode ---

if $WSL2_MODE; then
    header "Configuring WSL2 → Windows Ollama"

    WINDOWS_IP=$(get_windows_ip)
    if [[ -z "$WINDOWS_IP" ]]; then
        echo -e "  ${RED}Could not detect Windows IP.${NC}"
        echo -e "  ${NC}Try: ip route show default | awk '{print \$3}'${NC}"
        exit 1
    fi

    status "Windows IP" "$WINDOWS_IP" "$GREEN"
    OLLAMA_URL="http://${WINDOWS_IP}:11434"

    # Test connectivity
    echo ""
    echo -e "  Testing connection to $OLLAMA_URL ..."
    if curl -s --max-time 5 "$OLLAMA_URL/" &>/dev/null; then
        echo -e "  ${GREEN}Connected to Windows Ollama.${NC}"
    else
        echo -e "  ${RED}Cannot reach Windows Ollama at $OLLAMA_URL${NC}"
        echo ""
        echo -e "  ${YELLOW}Checklist:${NC}"
        echo -e "  1. Is Ollama running on Windows? (check Task Manager)"
        echo -e "  2. Was OLLAMA_HOST=0.0.0.0 set on Windows?"
        echo -e "  3. Is Windows Firewall allowing port 11434?"
        echo ""
        echo -e "  ${CYAN}Run the PowerShell script on Windows first:${NC}"
        echo -e "  ${NC}  .\\setup-ollama-gpu.ps1 -Backend vulkan -AllowRemote${NC}"
        echo ""
        exit 1
    fi

    # Persist OLLAMA_HOST
    # Remove any old OLLAMA_HOST entries first
    if bashrc_has "OLLAMA_HOST"; then
        old_line=$(grep "OLLAMA_HOST" "$BASHRC" | tail -1)
        bashrc_remove "$old_line"
    fi

    bashrc_add "export OLLAMA_HOST=\"$OLLAMA_URL\""

    echo ""
    echo -e "  ${CYAN}Done. Run 'source ~/.bashrc' to apply, then:${NC}"
    echo -e "  ${NC}  ollama list     # See Windows models${NC}"
    echo -e "  ${NC}  ollama ps       # Check GPU status${NC}"
    echo ""
    exit 0
fi

# --- Native Linux GPU Configuration ---

header "Configuring Backend: $BACKEND"

if [[ "$BACKEND" == "vulkan" ]]; then
    bashrc_add 'export OLLAMA_VULKAN="1"'

    # Remove ROCm override if present
    if bashrc_has "HSA_OVERRIDE_GFX_VERSION"; then
        echo -e "  ${YELLOW}Removing HSA_OVERRIDE_GFX_VERSION (not needed for Vulkan)${NC}"
        old_line=$(grep "HSA_OVERRIDE_GFX_VERSION" "$BASHRC" | tail -1)
        bashrc_remove "$old_line"
    fi

elif [[ "$BACKEND" == "rocm" ]]; then
    # Remove Vulkan if present
    if bashrc_has "OLLAMA_VULKAN"; then
        echo -e "  ${YELLOW}Removing OLLAMA_VULKAN (using ROCm instead)${NC}"
        bashrc_remove 'export OLLAMA_VULKAN="1"'
        bashrc_remove 'export OLLAMA_VULKAN=1'
    fi

    # Check ROCm availability
    if ! command -v rocminfo &>/dev/null; then
        echo ""
        echo -e "  ${YELLOW}ROCm does not appear to be installed.${NC}"
        echo -e "  ${NC}Install: https://rocm.docs.amd.com/projects/install-on-linux/en/latest/${NC}"
    fi

    if [[ ! -e /dev/kfd ]]; then
        echo -e "  ${YELLOW}/dev/kfd not found — ROCm kernel driver may not be loaded.${NC}"
    fi
else
    echo -e "${RED}Unknown backend: $BACKEND (use vulkan or rocm)${NC}"
    exit 1
fi

header "Configuration Complete"

echo -e "  ${CYAN}Next steps:${NC}"
echo -e "  1. Apply: ${NC}source ~/.bashrc${NC}"
echo -e "  2. Restart Ollama: ${NC}sudo systemctl restart ollama${NC}"
echo -e "  3. Load a model: ${NC}ollama run llama3.2 'hello'${NC}"
echo -e "  4. Check GPU: ${NC}ollama ps${NC}"
echo -e "  5. Verify: ${NC}./setup-ollama-gpu.sh --check${NC}"
echo ""

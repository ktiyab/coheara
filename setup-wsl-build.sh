#!/usr/bin/env bash
set -euo pipefail

# ── WSL2 Rust Build Performance Setup ───────────────────────────────────────
# Idempotent — safe to re-run on fresh installs and existing setups.
#
# Optimizations:
#   1. CARGO_TARGET_DIR on native Linux FS (ext4 vs 9P bridge)
#   2. mold linker + clang (multi-threaded linking)
#   3. cargo-nextest (parallel test execution)
#   4. Dev profile debug reduction (Cargo.toml)
#   5. Windows Defender exclusion guidance (print only)
#
# Usage:
#   ./setup-wsl-build.sh          # Run all steps
#   ./setup-wsl-build.sh --check  # Dry run — show current state only
# ────────────────────────────────────────────────────────────────────────────

# ── Colors ──────────────────────────────────────────────────────────────────
if [[ -z "${NO_COLOR:-}" && -t 1 ]]; then
    GREEN='\033[0;32m'
    YELLOW='\033[0;33m'
    RED='\033[0;31m'
    CYAN='\033[0;36m'
    DIM='\033[0;90m'
    BOLD='\033[1m'
    RESET='\033[0m'
else
    GREEN='' YELLOW='' RED='' CYAN='' DIM='' BOLD='' RESET=''
fi

ok()   { echo -e "  ${GREEN}[OK]${RESET} $1"; }
skip() { echo -e "  ${DIM}[SKIP]${RESET} $1 ${DIM}(already configured)${RESET}"; }
info() { echo -e "  ${CYAN}[INFO]${RESET} $1"; }
warn() { echo -e "  ${YELLOW}[WARN]${RESET} $1"; }
err()  { echo -e "  ${RED}[ERR]${RESET} $1"; }
step() { echo -e "\n${BOLD}── $1 ──${RESET}"; }

DRY_RUN=false
if [[ "${1:-}" == "--check" ]]; then
    DRY_RUN=true
    echo -e "${BOLD}Dry run mode — showing current state only${RESET}"
fi

CHANGED_BASHRC=false
CHANGES_MADE=()
ALREADY_OK=()

# ── Resolve paths ───────────────────────────────────────────────────────────
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TAURI_DIR="$SCRIPT_DIR/src-tauri"
CARGO="${CARGO:-${HOME}/.cargo/bin/cargo}"
if [[ ! -x "$CARGO" ]]; then
    if command -v cargo >/dev/null 2>&1; then
        CARGO="$(command -v cargo)"
    fi
fi
CARGO_DIR="$(dirname "${CARGO:-/root/.cargo/bin/cargo}")"

# ── Pre-flight ──────────────────────────────────────────────────────────────
step "Pre-flight checks"

# Detect WSL2
if grep -qi microsoft /proc/version 2>/dev/null; then
    ok "WSL2 detected"
else
    warn "Not running on WSL2 — some optimizations (Defender, CARGO_TARGET_DIR) are WSL-specific"
    warn "Continuing anyway — mold, nextest, and dev profile are useful everywhere"
fi

# Check cargo
if [[ -n "$CARGO" && -x "$CARGO" ]]; then
    ok "cargo found: $CARGO ($($CARGO --version 2>/dev/null || echo 'unknown'))"
else
    err "cargo not found — install Rust first: https://rustup.rs"
    exit 1
fi

# ════════════════════════════════════════════════════════════════════════════
# STEP 1: CARGO_TARGET_DIR on native Linux filesystem
# ════════════════════════════════════════════════════════════════════════════
step "Step 1/5: CARGO_TARGET_DIR on native Linux FS"

TARGET_DIR="${HOME}/cargo-targets"

# Check .bashrc
if grep -q 'CARGO_TARGET_DIR' ~/.bashrc 2>/dev/null; then
    skip "CARGO_TARGET_DIR already in ~/.bashrc"
    ALREADY_OK+=("CARGO_TARGET_DIR in .bashrc")
else
    if [[ "$DRY_RUN" == true ]]; then
        info "Would add CARGO_TARGET_DIR=$TARGET_DIR to ~/.bashrc"
    else
        echo "" >> ~/.bashrc
        echo "# WSL2 build perf: move cargo artifacts to native Linux FS (ext4 vs 9P)" >> ~/.bashrc
        echo "export CARGO_TARGET_DIR=\"\$HOME/cargo-targets\"" >> ~/.bashrc
        ok "Added CARGO_TARGET_DIR to ~/.bashrc"
        CHANGED_BASHRC=true
        CHANGES_MADE+=("CARGO_TARGET_DIR → ~/.bashrc")
    fi
fi

# Export for current session
export CARGO_TARGET_DIR="$TARGET_DIR"

# Create directory
if [[ -d "$TARGET_DIR" ]]; then
    skip "Directory $TARGET_DIR already exists"
    ALREADY_OK+=("Target directory exists")
else
    if [[ "$DRY_RUN" == true ]]; then
        info "Would create $TARGET_DIR"
    else
        mkdir -p "$TARGET_DIR"
        ok "Created $TARGET_DIR"
        CHANGES_MADE+=("Created $TARGET_DIR")
    fi
fi

# Check for old target/ on Windows FS
OLD_TARGET="$TAURI_DIR/target"
if [[ -d "$OLD_TARGET" ]]; then
    OLD_SIZE=$(du -sh "$OLD_TARGET" 2>/dev/null | cut -f1 || echo "unknown")
    warn "Old target/ found on Windows FS: $OLD_TARGET ($OLD_SIZE)"
    info "Delete manually to reclaim disk space: rm -rf \"$OLD_TARGET\""
    info "This is safe — cargo will rebuild into $TARGET_DIR"
fi

# ════════════════════════════════════════════════════════════════════════════
# STEP 2: mold linker + clang
# ════════════════════════════════════════════════════════════════════════════
step "Step 2/5: mold linker + clang"

NEED_APT=()
if command -v mold >/dev/null 2>&1; then
    skip "mold already installed ($(mold --version 2>/dev/null))"
    ALREADY_OK+=("mold installed")
else
    NEED_APT+=(mold)
fi

if command -v clang >/dev/null 2>&1; then
    skip "clang already installed ($(clang --version 2>/dev/null | head -1))"
    ALREADY_OK+=("clang installed")
else
    NEED_APT+=(clang)
fi

if [[ ${#NEED_APT[@]} -gt 0 ]]; then
    if [[ "$DRY_RUN" == true ]]; then
        info "Would install: ${NEED_APT[*]}"
    else
        info "Installing: ${NEED_APT[*]}"
        sudo apt-get update -qq && sudo apt-get install -y -qq "${NEED_APT[@]}"
        ok "Installed ${NEED_APT[*]}"
        CHANGES_MADE+=("apt install ${NEED_APT[*]}")
    fi
fi

# Configure ~/.cargo/config.toml
CARGO_CONFIG="$HOME/.cargo/config.toml"
MOLD_CONFIG='[target.x86_64-unknown-linux-gnu]
linker = "clang"
rustflags = ["-C", "link-arg=-fuse-ld=mold"]'

if [[ -f "$CARGO_CONFIG" ]] && grep -q 'fuse-ld=mold' "$CARGO_CONFIG" 2>/dev/null; then
    skip "mold already configured in $CARGO_CONFIG"
    ALREADY_OK+=("mold in cargo config")
else
    if [[ "$DRY_RUN" == true ]]; then
        info "Would configure mold in $CARGO_CONFIG"
    else
        mkdir -p "$(dirname "$CARGO_CONFIG")"
        # Append if file exists, create if not
        if [[ -f "$CARGO_CONFIG" ]]; then
            echo "" >> "$CARGO_CONFIG"
            echo "$MOLD_CONFIG" >> "$CARGO_CONFIG"
        else
            echo "$MOLD_CONFIG" > "$CARGO_CONFIG"
        fi
        ok "Configured mold in $CARGO_CONFIG"
        CHANGES_MADE+=("mold → $CARGO_CONFIG")
    fi
fi

# ════════════════════════════════════════════════════════════════════════════
# STEP 3: cargo-nextest (parallel test runner)
# ════════════════════════════════════════════════════════════════════════════
step "Step 3/5: cargo-nextest"

if command -v cargo-nextest >/dev/null 2>&1 || [[ -x "$CARGO_DIR/cargo-nextest" ]]; then
    NEXTEST_VER=$("$CARGO_DIR/cargo-nextest" nextest --version 2>/dev/null || cargo-nextest nextest --version 2>/dev/null || echo "installed")
    skip "cargo-nextest already installed ($NEXTEST_VER)"
    ALREADY_OK+=("cargo-nextest installed")
else
    if [[ "$DRY_RUN" == true ]]; then
        info "Would install cargo-nextest"
    else
        info "Installing cargo-nextest (this may take a few minutes)..."
        "$CARGO" install cargo-nextest
        ok "Installed cargo-nextest"
        CHANGES_MADE+=("cargo install cargo-nextest")
    fi
fi

# ════════════════════════════════════════════════════════════════════════════
# STEP 4: Dev profile optimization (Cargo.toml)
# ════════════════════════════════════════════════════════════════════════════
step "Step 4/5: Dev profile optimization"

CARGO_TOML="$TAURI_DIR/Cargo.toml"
if [[ ! -f "$CARGO_TOML" ]]; then
    warn "Cargo.toml not found at $CARGO_TOML — skipping"
else
    if grep -q '\[profile\.dev\]' "$CARGO_TOML" 2>/dev/null; then
        skip "[profile.dev] already present in Cargo.toml"
        ALREADY_OK+=("Dev profile in Cargo.toml")
    else
        if [[ "$DRY_RUN" == true ]]; then
            info "Would add [profile.dev] to $CARGO_TOML"
        else
            cat >> "$CARGO_TOML" << 'TOML'

# ── Dev build optimization ──────────────────────────────────────────────────
# Reduces compile + link time by skipping full debug info for dependencies.
# Backtraces still work (line-tables-only preserves file:line info).
[profile.dev]
debug = "line-tables-only"

[profile.dev.package."*"]
debug = false
TOML
            ok "Added [profile.dev] optimization to Cargo.toml"
            CHANGES_MADE+=("Dev profile → Cargo.toml")
        fi
    fi
fi

# ════════════════════════════════════════════════════════════════════════════
# STEP 5: Windows Defender exclusion (guidance only)
# ════════════════════════════════════════════════════════════════════════════
step "Step 5/5: Windows Defender exclusion"

if grep -qi microsoft /proc/version 2>/dev/null; then
    # Get Windows-native path via wslpath (most reliable)
    if command -v wslpath >/dev/null 2>&1; then
        DEFENDER_PATH=$(wslpath -w "$TARGET_DIR" 2>/dev/null || echo "")
    fi
    # Fallback: construct manually
    if [[ -z "${DEFENDER_PATH:-}" ]]; then
        WSL_DISTRO="${WSL_DISTRO_NAME:-Ubuntu}"
        DEFENDER_PATH="\\\\wsl\$\\${WSL_DISTRO}${TARGET_DIR}"
    fi

    info "This step requires Windows PowerShell (Run as Administrator)"
    echo ""
    echo -e "  ${BOLD}Copy-paste this into an elevated PowerShell:${RESET}"
    echo ""
    echo -e "  ${CYAN}# Exclude only cargo build artifacts (deterministic compiler output)${RESET}"
    # Use printf %s to preserve backslashes from wslpath output
    printf '  %bAdd-MpPreference -ExclusionPath "%s"%b\n' "$BOLD" "$DEFENDER_PATH" "$RESET"
    echo ""
    info "This excludes ONLY build artifacts — not source code, not your home folder"
    info "Measured impact: 40-70% overhead on file I/O operations (Cargo #5028)"
else
    skip "Not on WSL2 — Defender exclusion not applicable"
fi

# ════════════════════════════════════════════════════════════════════════════
# Summary
# ════════════════════════════════════════════════════════════════════════════
step "Summary"

echo ""
if [[ ${#CHANGES_MADE[@]} -gt 0 ]]; then
    echo -e "  ${GREEN}Changes made:${RESET}"
    for change in "${CHANGES_MADE[@]}"; do
        echo -e "    ${GREEN}+${RESET} $change"
    done
fi

if [[ ${#ALREADY_OK[@]} -gt 0 ]]; then
    echo -e "  ${DIM}Already configured:${RESET}"
    for item in "${ALREADY_OK[@]}"; do
        echo -e "    ${DIM}-${RESET} $item"
    done
fi

echo ""
echo -e "  ${BOLD}Current configuration:${RESET}"
echo -e "    CARGO_TARGET_DIR = ${CARGO_TARGET_DIR:-NOT SET}"
echo -e "    Linker           = $(command -v mold >/dev/null 2>&1 && echo "mold ($(mold --version 2>/dev/null))" || echo "default (GNU ld)")"
echo -e "    Test runner      = $(command -v cargo-nextest >/dev/null 2>&1 && echo "cargo-nextest (parallel)" || echo "cargo test (serial)")"
echo -e "    Dev debug info   = $(grep -q '\[profile\.dev\]' "$CARGO_TOML" 2>/dev/null && echo "line-tables-only (optimized)" || echo "full (default)")"

if [[ "$CHANGED_BASHRC" == true ]]; then
    echo ""
    warn "Run 'source ~/.bashrc' or start a new terminal to activate env changes"
fi

if [[ ${#CHANGES_MADE[@]} -eq 0 ]]; then
    echo ""
    ok "Everything already configured — no changes needed"
fi

echo ""
info "Impact order: CARGO_TARGET_DIR > Defender exclusion > mold > nextest > dev profile"
info "First build will be full (new target dir). Subsequent builds use incremental cache."
echo ""

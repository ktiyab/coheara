#!/usr/bin/env bash
set -euo pipefail

# ── Coheara Development Server ────────────────────────────────────────────
# Fast iteration loop — no full build required.
#
# Usage:
#   ./dev.sh              # Full stack: Svelte HMR + Rust backend (default)
#   ./dev.sh frontend     # Frontend only: Svelte + Vite (no Rust)
#   ./dev.sh check        # Type-check everything (Svelte + Rust)
#   ./dev.sh test         # Run all tests (frontend + backend)
#   ./dev.sh test:watch   # Watch mode for frontend tests
# ──────────────────────────────────────────────────────────────────────────

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$SCRIPT_DIR"
TAURI_DIR="$PROJECT_ROOT/src-tauri"

CARGO="${CARGO:-/root/.cargo/bin/cargo}"
if [[ ! -x "$CARGO" ]]; then
    if command -v cargo >/dev/null 2>&1; then
        CARGO="cargo"
    elif [[ -x "$HOME/.cargo/bin/cargo" ]]; then
        CARGO="$HOME/.cargo/bin/cargo"
    else
        CARGO=""
    fi
fi

# Ensure cargo's directory is in PATH
if [[ -n "$CARGO" && -x "$CARGO" ]]; then
    CARGO_DIR="$(dirname "$CARGO")"
    case ":$PATH:" in
        *":$CARGO_DIR:"*) ;;
        *) export PATH="$CARGO_DIR:$PATH" ;;
    esac
fi

COMMAND="${1:-full}"

# ── Colors ────────────────────────────────────────────────────────────────
if [[ -z "${NO_COLOR:-}" && -t 1 ]]; then
    RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[0;33m'
    BLUE='\033[0;34m'; BOLD='\033[1m'; NC='\033[0m'
else
    RED=''; GREEN=''; YELLOW=''; BLUE=''; BOLD=''; NC=''
fi

log_info()  { echo -e "${BLUE}[INFO]${NC}  $*"; }
log_ok()    { echo -e "${GREEN}[OK]${NC}    $*"; }
log_warn()  { echo -e "${YELLOW}[WARN]${NC}  $*"; }
log_error() { echo -e "${RED}[ERROR]${NC} $*" >&2; }
log_step()  { echo -e "\n${BOLD}==> $*${NC}"; }

# ── Usage ─────────────────────────────────────────────────────────────────
usage() {
    cat <<EOF
Coheara Development Server

Usage: ./dev.sh [command]

Commands:
  full        Full stack: Svelte HMR + Rust backend (default)
              Frontend changes: instant (<1s via HMR)
              Rust changes: ~10-30s (incremental compilation)

  frontend    Frontend only: Svelte + Vite dev server
              No Rust compilation. Tauri IPC calls will fail.
              Use when working on UI layout, styling, i18n, components.

  check       Type-check everything without running
              Runs: npm run check + cargo check (in parallel)

  test        Run all test suites
              Runs: npx vitest run + cargo test (in parallel)

  test:watch  Watch mode for frontend tests
              Re-runs affected tests on file save.

  help        Show this help

Iteration speeds:
  Svelte component change  →  <1 second (HMR)
  Rust code change         →  10-30 seconds (incremental)
  Full production build    →  5-30 minutes (use build.sh instead)
EOF
}

# ── Dependency bootstrap ──────────────────────────────────────────────────
ensure_deps() {
    # Install node_modules if missing
    if [[ ! -d "$PROJECT_ROOT/node_modules" ]]; then
        log_step "Installing npm dependencies"
        cd "$PROJECT_ROOT" && npm ci
    fi

    # Build i18n locale files if missing
    local generated_dir="$PROJECT_ROOT/src/lib/i18n/locales/_generated"
    if [[ ! -d "$generated_dir" ]] || [[ -z "$(ls -A "$generated_dir" 2>/dev/null)" ]]; then
        log_step "Building i18n locale files"
        cd "$PROJECT_ROOT" && node src/lib/i18n/build-locales.js
    fi
}

# ── Commands ──────────────────────────────────────────────────────────────

cmd_full() {
    log_step "Starting full-stack dev server (Svelte + Rust)"
    ensure_deps

    if [[ -z "$CARGO" || ! -x "$CARGO" ]]; then
        log_error "Cargo not found. Full-stack mode requires Rust."
        log_info "Install Rust: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
        log_info "Or use frontend-only mode: ./dev.sh frontend"
        exit 1
    fi

    # Set dev-friendly defaults (OBS-01, DX-03)
    export RUST_LOG="${RUST_LOG:-coheara=debug}"

    log_info "Frontend: http://localhost:1420 (Vite HMR)"
    log_info "Backend:  Tauri native window (Rust debug build)"
    log_info "DevTools: Enabled (F12 to inspect)"
    log_info "Logging:  ${RUST_LOG}"
    log_info "Data dir: ~/Coheara-dev"
    log_info "Stop:     Ctrl+C"
    echo ""

    cd "$PROJECT_ROOT"
    npx tauri dev \
        --features devtools \
        --config src-tauri/tauri.dev.conf.json
}

cmd_frontend() {
    log_step "Starting frontend-only dev server (no Rust)"
    ensure_deps

    log_info "Server:   http://localhost:1420"
    log_info "HMR:      Enabled (changes appear instantly)"
    log_info "Backend:  NONE — Tauri IPC calls will fail"
    log_info "Stop:     Ctrl+C"
    echo ""
    log_warn "This mode is for UI work only. Use './dev.sh full' to test IPC commands."
    echo ""

    cd "$PROJECT_ROOT"
    npm run dev
}

cmd_check() {
    log_step "Type-checking all layers"
    ensure_deps

    local exit_code=0

    # Run both checks in parallel
    log_info "Starting Svelte/TypeScript check..."
    npm run check --prefix "$PROJECT_ROOT" &
    local svelte_pid=$!

    if [[ -n "$CARGO" && -x "$CARGO" ]]; then
        log_info "Starting Rust cargo check..."
        "$CARGO" check --manifest-path "$TAURI_DIR/Cargo.toml" &
        local rust_pid=$!
    fi

    # Wait for Svelte check
    if wait $svelte_pid; then
        log_ok "Svelte/TypeScript: clean"
    else
        log_error "Svelte/TypeScript: errors found"
        exit_code=1
    fi

    # Wait for Rust check
    if [[ -n "${rust_pid:-}" ]]; then
        if wait $rust_pid; then
            log_ok "Rust: clean"
        else
            log_error "Rust: errors found"
            exit_code=1
        fi
    else
        log_warn "Rust: skipped (cargo not found)"
    fi

    if [[ $exit_code -eq 0 ]]; then
        echo ""
        log_ok "All type checks passed"
    else
        echo ""
        log_error "Type check failures detected"
    fi

    return $exit_code
}

cmd_test() {
    log_step "Running all test suites"
    ensure_deps

    local exit_code=0

    # Run frontend tests
    log_info "Running frontend tests (Vitest)..."
    if cd "$PROJECT_ROOT" && npx vitest run; then
        log_ok "Frontend tests: all passed"
    else
        log_error "Frontend tests: failures"
        exit_code=1
    fi

    # Run Rust tests
    if [[ -n "$CARGO" && -x "$CARGO" ]]; then
        echo ""
        log_info "Running Rust tests (cargo test)..."
        if "$CARGO" test --manifest-path "$TAURI_DIR/Cargo.toml"; then
            log_ok "Rust tests: all passed"
        else
            log_error "Rust tests: failures"
            exit_code=1
        fi
    else
        log_warn "Rust tests: skipped (cargo not found)"
    fi

    if [[ $exit_code -eq 0 ]]; then
        echo ""
        log_ok "All tests passed"
    else
        echo ""
        log_error "Test failures detected"
    fi

    return $exit_code
}

cmd_test_watch() {
    log_step "Starting frontend test watch mode"
    ensure_deps

    log_info "Watching for changes — tests re-run on save"
    log_info "Stop: Ctrl+C"
    echo ""

    cd "$PROJECT_ROOT"
    npx vitest
}

# ── Main ──────────────────────────────────────────────────────────────────
echo -e "${BOLD}Coheara Dev${NC} — $(echo "$COMMAND" | tr '[:lower:]' '[:upper:]') mode"
echo ""

case "$COMMAND" in
    full)       cmd_full ;;
    frontend)   cmd_frontend ;;
    check)      cmd_check ;;
    test)       cmd_test ;;
    test:watch) cmd_test_watch ;;
    help|--help|-h) usage ;;
    *)
        log_error "Unknown command: $COMMAND"
        echo ""
        usage
        exit 1
        ;;
esac

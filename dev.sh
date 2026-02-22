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

Usage: ./dev.sh [command] [options]

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
              Runs: npm run check + cargo check (in parallel)

  test        Run all test suites
              Runs: npx vitest run + cargo test (in parallel)

  test:watch  Watch mode for frontend tests
              Re-runs affected tests on file save.

  help        Show this help

Options:
  --rebuild   Hard reset before starting: wipe ALL dev data, caches, and
              node_modules. Use after deep changes (schema, migrations,
              data model). Skips incremental sync — everything rebuilt fresh.
              Deletes: ~/Coheara-dev, .svelte-kit, node_modules, i18n generated,
              and WSL2 temp dirs (/tmp/coheara-dev, /tmp/coheara-vite-cache).

Iteration speeds:
  Svelte component change  →  <1 second (HMR)
  Rust code change         →  10-30 seconds (incremental)
  Full production build    →  5-30 minutes (use build.sh instead)

Examples:
  ./dev.sh                       # Full stack (default)
  ./dev.sh frontend              # UI-only dev
  ./dev.sh full --rebuild        # Wipe everything, fresh start
  ./dev.sh frontend --rebuild    # Wipe + UI-only dev
EOF
}

# ── Argument parsing ─────────────────────────────────────────────────────
COMMAND=""
REBUILD=false

for arg in "$@"; do
    case "$arg" in
        --rebuild) REBUILD=true ;;
        setup|full|frontend|check|test|test:watch|help|--help|-h)
            COMMAND="$arg" ;;
        *)
            log_error "Unknown argument: $arg"
            echo ""
            usage
            exit 1
            ;;
    esac
done

COMMAND="${COMMAND:-full}"

# ── WSL2 filesystem detection ────────────────────────────────────────────
# /mnt/c/ (Windows 9P mount) is ~100x slower than native Linux I/O.
# Vite's module runner times out because each file read takes 10-100ms.
# Solution: rsync the project to native Linux fs before starting Vite.
LINUX_DEV_DIR="/tmp/coheara-dev"
IS_CROSS_FS=false
if [[ "$PROJECT_ROOT" == /mnt/* ]]; then
    IS_CROSS_FS=true
fi

sync_to_linux_fs() {
    log_step "Syncing project to native Linux filesystem (${LINUX_DEV_DIR})" >&2
    log_info "Source: ${PROJECT_ROOT} (Windows 9P mount — slow)" >&2
    log_info "Target: ${LINUX_DEV_DIR} (native ext4 — fast)" >&2

    mkdir -p "$LINUX_DEV_DIR"

    rsync -a --delete \
        --exclude='.git' \
        --exclude='target' \
        --exclude='node_modules' \
        --exclude='.svelte-kit' \
        "$PROJECT_ROOT/" "$LINUX_DEV_DIR/"

    # Install/update node_modules on Linux fs (faster than syncing 50k files)
    # Re-install if: missing, or lockfile changed since last install
    local lockfile_hash
    lockfile_hash=$(md5sum "$LINUX_DEV_DIR/package-lock.json" 2>/dev/null | cut -d' ' -f1)
    local cached_hash=""
    [[ -f "$LINUX_DEV_DIR/node_modules/.lockfile-hash" ]] && cached_hash=$(cat "$LINUX_DEV_DIR/node_modules/.lockfile-hash")

    if [[ ! -d "$LINUX_DEV_DIR/node_modules" ]] || [[ "$lockfile_hash" != "$cached_hash" ]]; then
        log_info "Installing npm dependencies on Linux fs..." >&2
        cd "$LINUX_DEV_DIR" && npm ci >&2
        echo "$lockfile_hash" > "$LINUX_DEV_DIR/node_modules/.lockfile-hash"
    fi

    # Rebuild i18n if needed
    local generated_dir="$LINUX_DEV_DIR/src/lib/i18n/locales/_generated"
    if [[ ! -d "$generated_dir" ]] || [[ -z "$(ls -A "$generated_dir" 2>/dev/null)" ]]; then
        cd "$LINUX_DEV_DIR" && node src/lib/i18n/build-locales.js >&2
    fi

    log_ok "Synced to $LINUX_DEV_DIR" >&2
}

# ── Hard Reset (--rebuild) ────────────────────────────────────────────────
do_rebuild() {
    local home_dir
    home_dir="$(eval echo '~')"
    local dev_data_dir="$home_dir/Coheara-dev"

    log_step "HARD RESET — wiping all dev data and caches"
    echo ""
    log_warn "This will permanently delete:"
    [[ -d "$dev_data_dir" ]] && log_warn "  • $dev_data_dir (profiles, databases, encrypted files)"
    [[ -d "$PROJECT_ROOT/.svelte-kit" ]] && log_warn "  • .svelte-kit/ (SvelteKit cache)"
    [[ -d "$PROJECT_ROOT/node_modules" ]] && log_warn "  • node_modules/ (npm packages)"
    local generated_dir="$PROJECT_ROOT/src/lib/i18n/locales/_generated"
    [[ -d "$generated_dir" ]] && log_warn "  • i18n generated locales"
    [[ -d "$LINUX_DEV_DIR" ]] && log_warn "  • $LINUX_DEV_DIR (WSL2 synced copy)"
    [[ -d "/tmp/coheara-vite-cache" ]] && log_warn "  • /tmp/coheara-vite-cache (Vite cache)"
    [[ -d "$TAURI_DIR/target" ]] && log_warn "  • Rust build artifacts (cargo clean)"
    echo ""

    read -rp "  Proceed with hard reset? [y/N]: " answer
    if [[ ! "${answer,,}" =~ ^(y|yes)$ ]]; then
        log_info "Aborted."
        exit 0
    fi

    # 1. Dev app data (profiles, SQLite DBs, encrypted files, models)
    if [[ -d "$dev_data_dir" ]]; then
        rm -rf "$dev_data_dir"
        log_ok "Deleted $dev_data_dir"
    fi

    # 2. SvelteKit cache
    if [[ -d "$PROJECT_ROOT/.svelte-kit" ]]; then
        rm -rf "$PROJECT_ROOT/.svelte-kit"
        log_ok "Deleted .svelte-kit/"
    fi

    # 3. node_modules (forces fresh npm ci)
    if [[ -d "$PROJECT_ROOT/node_modules" ]]; then
        rm -rf "$PROJECT_ROOT/node_modules"
        log_ok "Deleted node_modules/"
    fi

    # 4. Generated i18n locales (rebuilt by ensure_deps)
    if [[ -d "$generated_dir" ]]; then
        rm -rf "$generated_dir"
        log_ok "Deleted i18n generated locales"
    fi

    # 5. WSL2 synced copy (forces fresh sync, not incremental)
    if [[ -d "$LINUX_DEV_DIR" ]]; then
        rm -rf "$LINUX_DEV_DIR"
        log_ok "Deleted $LINUX_DEV_DIR"
    fi

    # 6. Vite cache
    if [[ -d "/tmp/coheara-vite-cache" ]]; then
        rm -rf "/tmp/coheara-vite-cache"
        log_ok "Deleted /tmp/coheara-vite-cache"
    fi

    # 7. Rust build artifacts (full clean, forces fresh compilation)
    if [[ -d "$TAURI_DIR/target" ]]; then
        rm -rf "$TAURI_DIR/target"
        log_ok "Deleted Rust target/ (full rebuild on next compile)"
    fi

    echo ""
    log_ok "Hard reset complete — everything will be rebuilt fresh"
    echo ""
}

# ── PDFium bootstrap ─────────────────────────────────────────────────────
# R3: pdfium-render requires the PDFium dynamic library at runtime.
# Downloads pre-built binary from bblanchon/pdfium-binaries (Chromium PDFium).
PDFIUM_VERSION="chromium/7690"
PDFIUM_CACHE_DIR="$TAURI_DIR/resources/pdfium"

ensure_pdfium() {
    local lib_name="libpdfium.so"
    local platform="linux-x64"
    case "$(uname -s)" in
        Darwin)
            lib_name="libpdfium.dylib"
            if [[ "$(uname -m)" == "arm64" ]]; then
                platform="mac-arm64"
            else
                platform="mac-x64"
            fi
            ;;
    esac

    local lib_path="$PDFIUM_CACHE_DIR/lib/$lib_name"

    # Skip if already cached
    if [[ -f "$lib_path" ]]; then
        export PDFIUM_DYNAMIC_LIB_PATH="$lib_path"
        log_ok "PDFium ready: $lib_path"
        return
    fi

    log_step "Downloading PDFium ($platform)"
    mkdir -p "$PDFIUM_CACHE_DIR"

    local url="https://github.com/bblanchon/pdfium-binaries/releases/download/${PDFIUM_VERSION}/pdfium-${platform}.tgz"
    local tmp_file
    tmp_file=$(mktemp /tmp/pdfium-XXXXXX.tgz)

    if curl -fsSL -o "$tmp_file" "$url"; then
        tar xzf "$tmp_file" -C "$PDFIUM_CACHE_DIR"
        rm -f "$tmp_file"
        if [[ -f "$lib_path" ]]; then
            export PDFIUM_DYNAMIC_LIB_PATH="$lib_path"
            log_ok "PDFium downloaded: $lib_path"
        else
            log_error "PDFium archive extracted but $lib_name not found in $PDFIUM_CACHE_DIR"
            log_info "Expected: $lib_path"
            ls -la "$PDFIUM_CACHE_DIR/" 2>/dev/null
            return 1
        fi
    else
        rm -f "$tmp_file"
        log_error "Failed to download PDFium from $url"
        log_info "Manually download and set PDFIUM_DYNAMIC_LIB_PATH=/path/to/$lib_name"
        return 1
    fi
}

# ── Dependency bootstrap ──────────────────────────────────────────────────
ensure_deps() {
    # Install node_modules if missing
    if [[ ! -d "$PROJECT_ROOT/node_modules" ]]; then
        log_step "Installing npm dependencies" >&2
        cd "$PROJECT_ROOT" && npm ci >&2
    fi

    # Build i18n locale files if missing
    local generated_dir="$PROJECT_ROOT/src/lib/i18n/locales/_generated"
    if [[ ! -d "$generated_dir" ]] || [[ -z "$(ls -A "$generated_dir" 2>/dev/null)" ]]; then
        log_step "Building i18n locale files" >&2
        cd "$PROJECT_ROOT" && node src/lib/i18n/build-locales.js >&2
    fi

    # Ensure PDFium for vision-based PDF extraction (R3)
    ensure_pdfium
}

# Resolve which directory to run Vite from
get_dev_dir() {
    if [[ "$IS_CROSS_FS" == true ]]; then
        sync_to_linux_fs
        echo "$LINUX_DEV_DIR"
    else
        ensure_deps
        echo "$PROJECT_ROOT"
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

    if [[ "$IS_CROSS_FS" == true ]]; then
        # WSL2: Sync frontend to Linux fs, start Vite there, then Tauri separately
        local dev_dir
        dev_dir="$(get_dev_dir)"

        log_info "Frontend: http://localhost:1420 (Vite from $dev_dir)"
        log_info "Backend:  Tauri native window (Rust debug build)"
        log_info "DevTools: Enabled (F12 to inspect)"
        log_info "Logging:  ${RUST_LOG}"
        log_info "Data dir: ~/Coheara-dev"
        log_info "Stop:     Ctrl+C"
        echo ""

        # Start Vite from Linux fs (fast) — in background
        cd "$dev_dir"
        npm run i18n:build
        npx vite dev &
        local vite_pid=$!

        # Cleanup on any exit (Ctrl+C, error, normal exit)
        cleanup() {
            log_info "Shutting down..."
            kill $vite_pid 2>/dev/null
            wait $vite_pid 2>/dev/null
            log_ok "Servers stopped"
        }
        trap cleanup EXIT INT TERM

        # Wait for Vite to be ready
        log_info "Waiting for Vite..."
        local attempts=0
        while ! curl -s --max-time 2 http://localhost:1420/ > /dev/null 2>&1; do
            sleep 2
            attempts=$((attempts + 1))
            if [[ $attempts -gt 30 ]]; then
                log_error "Vite failed to start after 60s"
                exit 1
            fi
        done
        log_ok "Vite ready"

        # Start Tauri (it will connect to the already-running Vite).
        # Override beforeDevCommand to empty — we already started Vite above.
        cd "$PROJECT_ROOT"
        npx tauri dev \
            --features devtools \
            --config src-tauri/tauri.dev.conf.json \
            --config '{"build":{"beforeDevCommand":""}}' \
            --no-dev-server
    else
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
    fi
}

cmd_frontend() {
    log_step "Starting frontend-only dev server (no Rust)"

    local dev_dir
    dev_dir="$(get_dev_dir)"

    log_info "Server:   http://localhost:1420"
    if [[ "$IS_CROSS_FS" == true ]]; then
        log_info "Source:   $dev_dir (synced from $PROJECT_ROOT)"
        log_info "Re-sync:  Run ./dev.sh frontend again after code changes"
    fi
    log_info "HMR:      Enabled (changes appear instantly)"
    log_info "Backend:  NONE — Tauri IPC calls will fail"
    log_info "Stop:     Ctrl+C"
    echo ""
    log_warn "This mode is for UI work only. Use './dev.sh full' to test IPC commands."
    echo ""

    cd "$dev_dir"
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

cmd_setup() {
    log_step "Setting up Coheara development environment"

    # 1. Install npm dependencies
    log_info "Installing npm dependencies..."
    cd "$PROJECT_ROOT" && npm ci
    log_ok "npm dependencies installed"

    # 2. Verify key packages
    local flowbite_ver
    flowbite_ver=$(node -e "console.log(require('./node_modules/flowbite-svelte/package.json').version)" 2>/dev/null || echo "MISSING")
    if [[ "$flowbite_ver" == "MISSING" ]]; then
        log_error "flowbite-svelte not found — check package.json"
    else
        log_ok "flowbite-svelte@${flowbite_ver}"
    fi

    # 3. Build i18n locale files
    log_info "Building i18n locale files..."
    node src/lib/i18n/build-locales.js
    log_ok "i18n locales built"

    # 4. Ensure PDFium for vision-based PDF extraction (R3)
    ensure_pdfium

    # 5. Check Rust toolchain
    if [[ -n "$CARGO" && -x "$CARGO" ]]; then
        local rust_ver
        rust_ver=$("$CARGO" --version 2>/dev/null | head -1)
        log_ok "Rust: $rust_ver"
    else
        log_warn "Rust: not found (frontend-only mode will still work)"
    fi

    # 5. npm audit summary
    log_info "Running npm audit..."
    local audit_result
    audit_result=$(npm audit --json 2>/dev/null | node -e "
        const d=require('fs').readFileSync('/dev/stdin','utf8');
        try { const j=JSON.parse(d); const v=j.metadata?.vulnerabilities||{};
        console.log('critical:'+( v.critical||0)+' high:'+(v.high||0)+' moderate:'+(v.moderate||0)); }
        catch(e) { console.log('parse-error'); }
    " 2>/dev/null || echo "unavailable")
    log_info "Audit: $audit_result"

    echo ""
    log_ok "Setup complete. Run: ./dev.sh frontend  (UI dev) or ./dev.sh full (full stack)"
}

# ── Main ──────────────────────────────────────────────────────────────────
echo -e "${BOLD}Coheara Dev${NC} — $(echo "$COMMAND" | tr '[:lower:]' '[:upper:]') mode"
if [[ "$REBUILD" == true ]]; then
    echo -e "  ${YELLOW}--rebuild${NC} active"
fi
echo ""

# Execute rebuild before any command
if [[ "$REBUILD" == true ]]; then
    do_rebuild
fi

case "$COMMAND" in
    setup)      cmd_setup ;;
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

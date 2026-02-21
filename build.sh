#!/usr/bin/env bash
set -euo pipefail

# ── Constants ──────────────────────────────────────────────────────────────
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$SCRIPT_DIR"
PACKAGE_DIR="$PROJECT_ROOT/package"
KEYS_DIR="$PROJECT_ROOT/Specs/build/keys"
MOBILE_DIR="$PROJECT_ROOT/mobile"
TAURI_DIR="$PROJECT_ROOT/src-tauri"
ANDROID_DIR="$MOBILE_DIR/android"

CARGO="${CARGO:-/root/.cargo/bin/cargo}"

# Ensure cargo's directory is in PATH (needed by tools like npx tauri build)
CARGO_DIR="$(dirname "$CARGO")"
case ":$PATH:" in
    *":$CARGO_DIR:"*) ;;
    *) export PATH="$CARGO_DIR:$PATH" ;;
esac

VERSION=$(grep '"version"' "$PROJECT_ROOT/package.json" | head -1 | sed 's/.*"version": *"\([^"]*\)".*/\1/')
TIMESTAMP=$(date +%Y%m%d-%H%M%S)

SIGN=true
VERBOSE=false
SKIP_MOBILE=false
COMMAND=""

# ── Colors (respects NO_COLOR) ─────────────────────────────────────────────
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

SECONDS=0
elapsed() { echo "$((SECONDS / 60))m $((SECONDS % 60))s"; }

# ── Usage ──────────────────────────────────────────────────────────────────
usage() {
    cat <<EOF
Coheara Local Build System v${VERSION}

Usage: ./build.sh <command> [options]

Commands:
  setup       First-time setup: install deps, verify toolchain, build i18n
  desktop     Build desktop installer (Linux .deb/.AppImage)
              Also builds mobile artifacts (bundled inside installer)
  android     Build standalone signed Android APK
  all         Build everything (desktop + standalone APK)
  clean       Remove all build artifacts and intermediates

Options:
  --no-sign      Build without signing (faster, for testing)
  --skip-mobile  Skip mobile build (use pre-staged PWA + APK from prior build)
  --verbose      Show detailed command output
  --help         Show this help

Credential Priority (passwords only — key files read from disk):
  1. Environment variables (TAURI_SIGNING_PRIVATE_KEY_PASSWORD, etc.)
  2. .env file in project root
  3. Interactive prompt

Signing Keys (read directly from files):
  Tauri:   Specs/build/keys/coheara-tauri.key
  Android: Specs/build/keys/coheara-release.jks

Output:
  All artifacts collected in ./package/

Examples:
  ./build.sh desktop              # Full signed desktop build
  ./build.sh desktop --skip-mobile  # Desktop only (use pre-staged mobile artifacts)
  ./build.sh android --no-sign    # Quick unsigned APK for testing
  ./build.sh all                  # Everything, signed
  ./build.sh clean                # Remove all build artifacts

Note: --skip-mobile is auto-detected when pre-staged artifacts exist in
      src-tauri/resources/mobile-pwa/ and mobile-apk/coheara.apk
EOF
}

# ── Credential Loading ─────────────────────────────────────────────────────
load_credentials() {
    if [[ "$SIGN" != true ]]; then
        log_info "Signing disabled (--no-sign)"
        return
    fi

    # Tier 2: .env file (only whitelisted variables, no arbitrary execution)
    if [[ -f "$PROJECT_ROOT/.env" ]]; then
        log_info "Loading credentials from .env"
        while IFS='=' read -r key value; do
            key=$(echo "$key" | xargs)
            [[ -z "$key" || "$key" == \#* ]] && continue
            # Strip surrounding quotes from value
            value=$(echo "$value" | sed -e 's/^"//' -e 's/"$//' -e "s/^'//" -e "s/'$//")
            case "$key" in
                TAURI_SIGNING_PRIVATE_KEY_PASSWORD|\
                ANDROID_KEYSTORE_PASSWORD|\
                ANDROID_KEY_PASSWORD|\
                ANDROID_KEY_ALIAS)
                    if [[ -z "${!key:-}" ]]; then
                        export "$key=$value"
                    fi
                    ;;
            esac
        done < "$PROJECT_ROOT/.env"
    fi

    # Tier 3: Interactive prompt for missing passwords
    if [[ -z "${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:-}" ]]; then
        read -rsp "Enter TAURI_SIGNING_PRIVATE_KEY_PASSWORD: " TAURI_SIGNING_PRIVATE_KEY_PASSWORD
        echo
        export TAURI_SIGNING_PRIVATE_KEY_PASSWORD
    fi

    if [[ "$SKIP_MOBILE" != true ]]; then
        if [[ -z "${ANDROID_KEYSTORE_PASSWORD:-}" ]]; then
            read -rsp "Enter ANDROID_KEYSTORE_PASSWORD: " ANDROID_KEYSTORE_PASSWORD
            echo
            export ANDROID_KEYSTORE_PASSWORD
        fi

        # PKCS12: key password = store password (documented in CREDENTIALS-SECRETS.md)
        export ANDROID_KEY_PASSWORD="${ANDROID_KEY_PASSWORD:-$ANDROID_KEYSTORE_PASSWORD}"
        export ANDROID_KEY_ALIAS="${ANDROID_KEY_ALIAS:-coheara}"
    fi

    log_ok "Credentials loaded"
}

# ── Auto-install Helpers ──────────────────────────────────────────────────

install_linux_deps() {
    local pkgs=(
        build-essential pkg-config perl
        libgtk-3-dev libwebkit2gtk-4.1-dev
        libappindicator3-dev librsvg2-dev patchelf libsoup-3.0-dev
        libjavascriptcoregtk-4.1-dev tesseract-ocr libtesseract-dev
        libleptonica-dev libclang-dev libssl-dev unzip
    )

    echo ""
    log_warn "Missing Linux system libraries."
    echo "  This will install via apt:"
    echo "    ${pkgs[*]}"
    echo ""
    read -rp "  Install missing packages automatically? (requires sudo) [Y/n]: " answer
    if [[ "${answer,,}" =~ ^(y|yes|)$ ]]; then
        log_step "Installing system libraries via apt"
        sudo apt-get update -qq
        sudo apt-get install -y "${pkgs[@]}"
        return $?
    fi
    return 1
}

# ── Tessdata bootstrap ────────────────────────────────────────────────────
ensure_tessdata() {
    local tessdata_dir=""

    # Locate tessdata directory: TESSDATA_PREFIX > system paths
    if [[ -n "${TESSDATA_PREFIX:-}" && -d "$TESSDATA_PREFIX" ]]; then
        tessdata_dir="$TESSDATA_PREFIX"
    else
        local candidates=(
            "/usr/share/tesseract-ocr/5/tessdata"
            "/usr/share/tesseract-ocr/4.00/tessdata"
            "/usr/share/tessdata"
            "/usr/local/share/tessdata"
        )
        for candidate in "${candidates[@]}"; do
            if [[ -d "$candidate" ]]; then
                tessdata_dir="$candidate"
                break
            fi
        done
    fi

    if [[ -z "$tessdata_dir" ]]; then
        log_warn "Tessdata directory not found - OCR will be unavailable for scanned documents"
        log_info "Install: sudo apt install tesseract-ocr"
        return
    fi

    local base_url="https://github.com/tesseract-ocr/tessdata_best/raw/main"
    for lang in eng fra deu; do
        if [[ ! -f "$tessdata_dir/$lang.traineddata" ]]; then
            log_info "Downloading $lang.traineddata..."
            if curl -fsSL -o "$tessdata_dir/$lang.traineddata" "$base_url/$lang.traineddata" 2>/dev/null; then
                log_ok "Downloaded $lang.traineddata"
            else
                log_warn "Failed to download $lang.traineddata (try: sudo apt install tesseract-ocr-$lang)"
            fi
        fi
    done

    export TESSDATA_PREFIX="$tessdata_dir"
    log_ok "Tessdata ready: $tessdata_dir"
}

# ── Dependency Checking ────────────────────────────────────────────────────
check_dependencies() {
    local target="$1"
    local missing=()

    # Common
    command -v node >/dev/null || missing+=("node (Node.js >= 20)")
    command -v npm  >/dev/null || missing+=("npm")
    [[ -x "$CARGO" ]]         || missing+=("cargo (expected at $CARGO)")

    # Desktop
    if [[ "$target" == "desktop" || "$target" == "all" ]]; then
        if [[ "$(uname -s)" == "Linux" ]]; then
            local linux_missing=()
            pkg-config --exists gtk+-3.0       2>/dev/null || linux_missing+=("libgtk-3-dev")
            pkg-config --exists webkit2gtk-4.1 2>/dev/null || linux_missing+=("libwebkit2gtk-4.1-dev")
            pkg-config --exists tesseract      2>/dev/null || linux_missing+=("libtesseract-dev")
            pkg-config --exists lept           2>/dev/null || linux_missing+=("libleptonica-dev")
            command -v patchelf >/dev/null                 || linux_missing+=("patchelf")
            command -v perl    >/dev/null                 || linux_missing+=("perl")

            if [[ ${#linux_missing[@]} -gt 0 ]]; then
                if install_linux_deps; then
                    # Re-verify after install
                    local still_missing=false
                    pkg-config --exists gtk+-3.0       2>/dev/null || still_missing=true
                    pkg-config --exists webkit2gtk-4.1 2>/dev/null || still_missing=true
                    pkg-config --exists tesseract      2>/dev/null || still_missing=true
                    pkg-config --exists lept           2>/dev/null || still_missing=true
                    command -v patchelf >/dev/null                 || still_missing=true
                    command -v perl    >/dev/null                 || still_missing=true
                    if [[ "$still_missing" == true ]]; then
                        missing+=("Linux system libraries (auto-install incomplete, check errors above)")
                    fi
                else
                    missing+=("${linux_missing[@]}")
                fi
            fi
        fi

        if [[ "$SIGN" == true ]]; then
            [[ -f "$KEYS_DIR/coheara-tauri.key" ]] || missing+=("Tauri key: $KEYS_DIR/coheara-tauri.key")
        fi
    fi

    # Android / Mobile (desktop also bundles mobile artifacts)
    if [[ "$SKIP_MOBILE" != true ]] && [[ "$target" == "desktop" || "$target" == "android" || "$target" == "all" ]]; then
        command -v java >/dev/null || missing+=("java (JDK 21)")

        if command -v java >/dev/null; then
            local java_ver
            java_ver=$(java -version 2>&1 | head -1 | sed 's/.*"\([0-9]*\)\..*/\1/')
            [[ "${java_ver:-0}" -ge 21 ]] || missing+=("Java 21+ (found: ${java_ver:-unknown})")
        fi

        if [[ -z "${ANDROID_HOME:-}" && -z "${ANDROID_SDK_ROOT:-}" ]]; then
            for sdk_path in ~/Android/Sdk /opt/android-sdk /usr/lib/android-sdk; do
                if [[ -d "$sdk_path" ]]; then
                    export ANDROID_HOME="$sdk_path"
                    break
                fi
            done
            [[ -n "${ANDROID_HOME:-}" ]] || missing+=("ANDROID_HOME (Android SDK)")
        fi

        if [[ "$SIGN" == true ]]; then
            [[ -f "$KEYS_DIR/coheara-release.jks" ]] || missing+=("Android keystore: $KEYS_DIR/coheara-release.jks")
        fi
    fi

    if [[ ${#missing[@]} -gt 0 ]]; then
        log_error "Missing dependencies:"
        for dep in "${missing[@]}"; do
            echo "  - $dep"
        done
        exit 1
    fi

    log_ok "All dependencies satisfied"
}

# ── Pre-staged Mobile Detection ───────────────────────────────────────────

detect_prestaged_mobile() {
    if [[ "$SKIP_MOBILE" == true ]]; then
        log_info "Mobile build: SKIPPED (--skip-mobile flag)"
        return
    fi

    local pwa_dest="$TAURI_DIR/resources/mobile-pwa"
    local apk_dest="$TAURI_DIR/resources/mobile-apk"
    local has_pwa=false
    local has_apk=false

    if [[ -d "$pwa_dest" ]]; then
        local pwa_count
        pwa_count=$(find "$pwa_dest" -mindepth 1 ! -name '.gitkeep' ! -name '.gitignore' -type f 2>/dev/null | wc -l)
        [[ "$pwa_count" -gt 0 ]] && has_pwa=true
    fi

    [[ -f "$apk_dest/coheara.apk" ]] && has_apk=true

    if [[ "$has_pwa" == true && "$has_apk" == true ]]; then
        SKIP_MOBILE=true
        local apk_size
        apk_size=$(du -h "$apk_dest/coheara.apk" | cut -f1)
        log_info "Mobile build: SKIPPED (pre-staged artifacts detected)"
        log_info "  PWA: $pwa_dest ($pwa_count files)"
        log_info "  APK: $apk_dest/coheara.apk ($apk_size)"
    elif [[ "$has_pwa" == true || "$has_apk" == true ]]; then
        log_warn "Partial pre-staged mobile artifacts found:"
        [[ "$has_pwa" == true ]] && log_info "  PWA: present" || log_warn "  PWA: missing"
        [[ "$has_apk" == true ]] && log_info "  APK: present" || log_warn "  APK: missing"
        log_info "Building mobile from source (use --skip-mobile to use partial artifacts)"
    fi
}

# ── Build Phases ───────────────────────────────────────────────────────────

build_frontend() {
    log_step "Building desktop frontend (SvelteKit)"
    cd "$PROJECT_ROOT"
    npm ci --prefer-offline
    npm run build
    log_ok "Frontend built -> ./build/"
}

build_mobile_pwa() {
    log_step "Building mobile PWA"
    cd "$MOBILE_DIR"
    npm ci --prefer-offline
    npm run build
    log_ok "Mobile PWA built -> mobile/build/"
}

build_android_apk() {
    log_step "Building Android APK"
    cd "$MOBILE_DIR"

    if [[ ! -d "$MOBILE_DIR/build" ]]; then
        build_mobile_pwa
    fi

    log_info "Syncing Capacitor to Android"
    npx cap sync android

    if [[ "$SIGN" == true ]]; then
        log_info "Configuring Android signing"
        cp "$KEYS_DIR/coheara-release.jks" "$ANDROID_DIR/app/release-key.jks"
        export ANDROID_KEYSTORE_FILE="release-key.jks"
    fi

    cd "$ANDROID_DIR"
    chmod +x ./gradlew
    ./gradlew assembleRelease

    # Signed builds produce app-release.apk; unsigned produce app-release-unsigned.apk
    local apk_dir="$ANDROID_DIR/app/build/outputs/apk/release"
    local apk_path=""
    if [[ -f "$apk_dir/app-release.apk" ]]; then
        apk_path="$apk_dir/app-release.apk"
    elif [[ -f "$apk_dir/app-release-unsigned.apk" ]]; then
        apk_path="$apk_dir/app-release-unsigned.apk"
    fi

    if [[ -n "$apk_path" ]]; then
        local size
        size=$(du -h "$apk_path" | cut -f1)
        log_ok "Android APK built ($size) -> $apk_path"
    else
        log_error "APK not found in $apk_dir"
        ls -la "$apk_dir/" 2>/dev/null || true
        exit 1
    fi
}

stage_mobile_resources() {
    log_step "Staging mobile resources into Tauri"

    local pwa_dest="$TAURI_DIR/resources/mobile-pwa"
    local apk_dest="$TAURI_DIR/resources/mobile-apk"

    mkdir -p "$pwa_dest" "$apk_dest"

    # Clean previous staged files (preserve .gitkeep)
    find "$pwa_dest" -mindepth 1 ! -name '.gitkeep' -delete 2>/dev/null || true
    find "$apk_dest" -mindepth 1 ! -name '.gitkeep' -delete 2>/dev/null || true

    # Stage PWA
    if [[ -d "$MOBILE_DIR/build" ]]; then
        cp -r "$MOBILE_DIR/build/"* "$pwa_dest/"
        log_ok "PWA staged -> $pwa_dest"
    else
        log_warn "Mobile PWA not built — skipping"
    fi

    # Stage APK (renamed for distribution server)
    local apk_dir="$ANDROID_DIR/app/build/outputs/apk/release"
    local apk_src=""
    [[ -f "$apk_dir/app-release.apk" ]] && apk_src="$apk_dir/app-release.apk"
    [[ -z "$apk_src" && -f "$apk_dir/app-release-unsigned.apk" ]] && apk_src="$apk_dir/app-release-unsigned.apk"

    if [[ -n "$apk_src" ]]; then
        cp "$apk_src" "$apk_dest/coheara.apk"
        log_ok "APK staged -> $apk_dest/coheara.apk"
    else
        log_warn "Android APK not built — skipping"
    fi
}

build_desktop() {
    log_step "Building Tauri desktop installer"
    cd "$PROJECT_ROOT"

    if [[ ! -d "$PROJECT_ROOT/build" ]]; then
        build_frontend
    fi

    # Platform-specific bundles
    local bundles=""
    case "$(uname -s)" in
        Linux)  bundles="deb,appimage" ;;
        Darwin) bundles="dmg" ;;
        *)
            log_error "Unsupported platform for desktop build: $(uname -s)"
            log_info "Use WSL2 on Windows to build Linux installers"
            exit 1
            ;;
    esac

    # Set signing environment
    if [[ "$SIGN" == true && -f "$KEYS_DIR/coheara-tauri.key" ]]; then
        export TAURI_SIGNING_PRIVATE_KEY
        TAURI_SIGNING_PRIVATE_KEY=$(cat "$KEYS_DIR/coheara-tauri.key")
        log_info "Tauri updater signing: ENABLED"
    else
        unset TAURI_SIGNING_PRIVATE_KEY 2>/dev/null || true
        unset TAURI_SIGNING_PRIVATE_KEY_PASSWORD 2>/dev/null || true
        log_warn "Tauri updater signing: DISABLED"
    fi

    log_info "Building bundles: $bundles"

    if [[ "$SIGN" == true ]]; then
        npx tauri build --bundles "$bundles"
    else
        # When unsigned, Tauri may error about missing private key if pubkey exists
        # in tauri.conf.json. The bundles are still created before the error.
        set +e
        npx tauri build --bundles "$bundles" 2>&1
        local tauri_exit=$?
        set -e

        if [[ $tauri_exit -ne 0 ]]; then
            # Check if bundles were actually created despite the error
            local tauri_out="$TAURI_DIR/target/release/bundle"
            local has_bundles=false
            [[ -d "$tauri_out/deb" ]] && ls "$tauri_out/deb/"*.deb >/dev/null 2>&1 && has_bundles=true
            [[ -d "$tauri_out/appimage" ]] && ls "$tauri_out/appimage/"*.AppImage >/dev/null 2>&1 && has_bundles=true
            [[ -d "$tauri_out/dmg" ]] && ls "$tauri_out/dmg/"*.dmg >/dev/null 2>&1 && has_bundles=true

            if [[ "$has_bundles" == true ]]; then
                log_warn "Tauri exited with error (signing skipped) but bundles were created"
            else
                log_error "Tauri build failed and no bundles were created"
                exit 1
            fi
        fi
    fi

    log_ok "Desktop build complete"
}

# ── Artifact Collection ────────────────────────────────────────────────────

collect_artifacts() {
    log_step "Collecting artifacts into package/"

    mkdir -p "$PACKAGE_DIR"

    local tauri_out="$TAURI_DIR/target/release/bundle"

    # Linux .deb
    if [[ -d "$tauri_out/deb" ]]; then
        find "$tauri_out/deb" -name "*.deb" -exec cp {} "$PACKAGE_DIR/" \; 2>/dev/null && \
            log_ok "Collected: .deb"
    fi

    # Linux .AppImage
    if [[ -d "$tauri_out/appimage" ]]; then
        find "$tauri_out/appimage" -name "*.AppImage" -exec cp {} "$PACKAGE_DIR/" \; 2>/dev/null && \
            log_ok "Collected: .AppImage"
    fi

    # macOS .dmg
    if [[ -d "$tauri_out/dmg" ]]; then
        find "$tauri_out/dmg" -name "*.dmg" -exec cp {} "$PACKAGE_DIR/" \; 2>/dev/null && \
            log_ok "Collected: .dmg"
    fi

    # Updater signatures (.sig files)
    local sig_count=0
    while IFS= read -r -d '' sig; do
        cp "$sig" "$PACKAGE_DIR/"
        sig_count=$((sig_count + 1))
    done < <(find "$tauri_out" -name "*.sig" -print0 2>/dev/null)
    if [[ $sig_count -gt 0 ]]; then
        log_ok "Collected: $sig_count .sig files"
    fi

    # Android APK
    local apk_dir="$ANDROID_DIR/app/build/outputs/apk/release"
    local apk_src=""
    [[ -f "$apk_dir/app-release.apk" ]] && apk_src="$apk_dir/app-release.apk"
    [[ -z "$apk_src" && -f "$apk_dir/app-release-unsigned.apk" ]] && apk_src="$apk_dir/app-release-unsigned.apk"

    if [[ -n "$apk_src" ]]; then
        cp "$apk_src" "$PACKAGE_DIR/coheara-${VERSION}.apk"
        log_ok "Collected: .apk"
    fi

    echo ""
    log_step "Build artifacts in $PACKAGE_DIR:"
    ls -lh "$PACKAGE_DIR/" 2>/dev/null || echo "  (empty)"
    echo ""
    log_ok "Build completed in $(elapsed)"
}

# ── Clean ──────────────────────────────────────────────────────────────────

cmd_clean() {
    log_step "Cleaning build artifacts"

    rm -rf "$PROJECT_ROOT/build"
    rm -rf "$PROJECT_ROOT/.svelte-kit"
    log_ok "Cleaned: desktop frontend"

    rm -rf "$MOBILE_DIR/build"
    rm -rf "$MOBILE_DIR/.svelte-kit"
    log_ok "Cleaned: mobile frontend"

    if [[ -d "$ANDROID_DIR" ]]; then
        cd "$ANDROID_DIR" && ./gradlew clean 2>/dev/null || true
        rm -f "$ANDROID_DIR/app/release-key.jks"
        log_ok "Cleaned: Android build"
    fi

    find "$TAURI_DIR/resources/mobile-pwa" -mindepth 1 ! -name '.gitkeep' -delete 2>/dev/null || true
    find "$TAURI_DIR/resources/mobile-apk" -mindepth 1 ! -name '.gitkeep' -delete 2>/dev/null || true
    log_ok "Cleaned: staged mobile resources"

    rm -rf "$PACKAGE_DIR"
    log_ok "Cleaned: package/"

    log_ok "Clean complete"
}

# ── Setup ─────────────────────────────────────────────────────────────────

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

    # 4. Ensure Tesseract tessdata for OCR
    ensure_tessdata

    # 5. Check Rust toolchain
    if [[ -n "${CARGO:-}" && -x "$CARGO" ]]; then
        local rust_ver
        rust_ver=$("$CARGO" --version 2>/dev/null | head -1)
        log_ok "Rust: $rust_ver"
    else
        log_warn "Rust: not found (frontend-only mode will still work)"
    fi

    # 6. npm audit summary
    log_info "Running npm audit..."
    npm audit 2>/dev/null | tail -5 || true

    echo ""
    log_ok "Setup complete. Run: ./dev.sh frontend  (UI dev) or ./build.sh desktop --no-sign  (build)"
}

# ── Orchestration ──────────────────────────────────────────────────────────

cmd_desktop() {
    detect_prestaged_mobile
    check_dependencies "desktop"
    ensure_tessdata
    load_credentials
    build_frontend
    if [[ "$SKIP_MOBILE" != true ]]; then
        build_mobile_pwa
        build_android_apk
        stage_mobile_resources
    fi
    build_desktop
    collect_artifacts
}

cmd_android() {
    if [[ "$SKIP_MOBILE" == true ]]; then
        log_error "--skip-mobile cannot be used with 'android' command"
        exit 1
    fi
    check_dependencies "android"
    load_credentials
    build_mobile_pwa
    build_android_apk
    collect_artifacts
}

cmd_all() {
    detect_prestaged_mobile
    check_dependencies "all"
    ensure_tessdata
    load_credentials
    build_frontend
    if [[ "$SKIP_MOBILE" != true ]]; then
        build_mobile_pwa
        build_android_apk
        stage_mobile_resources
    fi
    build_desktop
    collect_artifacts
}

# ── Argument Parsing ───────────────────────────────────────────────────────

for arg in "$@"; do
    case "$arg" in
        desktop|android|all|clean|setup) COMMAND="$arg" ;;
        --no-sign)      SIGN=false ;;
        --skip-mobile)  SKIP_MOBILE=true ;;
        --verbose)      VERBOSE=true; set -x ;;
        --help|-h)      usage; exit 0 ;;
        *)          log_error "Unknown argument: $arg"; usage; exit 1 ;;
    esac
done

if [[ -z "$COMMAND" ]]; then
    usage
    exit 1
fi

echo -e "${BOLD}Coheara Build System v${VERSION}${NC}"
echo -e "Command: ${BOLD}$COMMAND${NC}  Signing: ${BOLD}$SIGN${NC}  Platform: ${BOLD}$(uname -s)${NC}"
echo ""

case "$COMMAND" in
    setup)   cmd_setup ;;
    desktop) cmd_desktop ;;
    android) cmd_android ;;
    all)     cmd_all ;;
    clean)   cmd_clean ;;
esac

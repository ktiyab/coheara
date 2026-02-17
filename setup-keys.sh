#!/usr/bin/env bash
set -euo pipefail

# ── Coheara Signing Key Generator ──────────────────────────────────────────
# Creates all signing keys needed to build signed Coheara installers.
#
# What this script does:
#   1. Verifies root .gitignore protects Specs/ (refuses if not)
#   2. Creates Specs/build/keys/ directory with .gitignore defense layer
#   3. Generates Tauri updater signing keypair (minisign, password-protected)
#   4. Generates Android release keystore (PKCS12, RSA 2048-bit)
#   5. Generates iOS distribution private key + CSR (for Apple Developer portal)
#   6. Writes .env with all passwords (for build.sh consumption)
#   7. Prints summary and next steps
#
# Safe to re-run: existing keys are NEVER overwritten unless --force is used.
# ───────────────────────────────────────────────────────────────────────────

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$SCRIPT_DIR"
KEYS_DIR="$PROJECT_ROOT/Specs/build/keys"
BUILD_DIR="$PROJECT_ROOT/Specs/build"
ENV_FILE="$PROJECT_ROOT/.env"

FORCE=false

# ── Colors ─────────────────────────────────────────────────────────────────
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

# ── Usage ──────────────────────────────────────────────────────────────────
usage() {
    cat <<'EOF'
Coheara Signing Key Generator

Usage: ./setup-keys.sh [options]

Generates all signing keys needed for local builds:
  - Tauri updater signing keypair (minisign format)
  - Android release keystore (PKCS12, RSA 2048-bit)
  - iOS distribution private key + CSR

Keys are stored in Specs/build/keys/ (git-ignored).
Passwords are written to .env (git-ignored).

Options:
  --force   Overwrite existing keys (DESTRUCTIVE — cannot be undone)
  --help    Show this help

After running, use: ./build.sh desktop
EOF
}

# ── Argument Parsing ───────────────────────────────────────────────────────
for arg in "$@"; do
    case "$arg" in
        --force) FORCE=true ;;
        --help|-h) usage; exit 0 ;;
        *) log_error "Unknown argument: $arg"; usage; exit 1 ;;
    esac
done

echo -e "${BOLD}Coheara Signing Key Generator${NC}"
echo ""

# ── Step 1: Verify root .gitignore ─────────────────────────────────────────
log_step "Verifying .gitignore safety"

if [[ ! -f "$PROJECT_ROOT/.gitignore" ]]; then
    log_error "No .gitignore found at project root. Refusing to generate keys."
    exit 1
fi

if ! grep -q "^Specs/" "$PROJECT_ROOT/.gitignore"; then
    log_error "Root .gitignore does not contain 'Specs/' exclusion."
    log_error "Refusing to generate keys — they would be at risk of accidental commit."
    log_error ""
    log_error "Add this line to .gitignore first:"
    log_error "  Specs/"
    exit 1
fi

if ! grep -q "^\.env" "$PROJECT_ROOT/.gitignore"; then
    log_error "Root .gitignore does not contain '.env' exclusion."
    log_error "Refusing to generate keys — passwords would be at risk of accidental commit."
    exit 1
fi

log_ok "Root .gitignore protects Specs/ and .env"

# ── Step 2: Create directory structure + defense .gitignore ────────────────
log_step "Creating protected directory structure"

mkdir -p "$KEYS_DIR"

cat > "$BUILD_DIR/.gitignore" << 'GITIGNORE'
# ══════════════════════════════════════════════════════════════
# DEFENSE LAYER 2 — Prevent accidental commit of signing secrets
# (Layer 1: root .gitignore excludes Specs/ entirely)
# ══════════════════════════════════════════════════════════════

# Signing keys and keystores
keys/
*.jks
*.keystore
*.key
*.p12
*.pem
*.b64
*.cer
*.csr
*.mobileprovision

# Credentials documentation
CREDENTIALS*

# macOS metadata
.DS_Store
GITIGNORE

log_ok "Created Specs/build/.gitignore (defense layer 2)"
log_ok "Directory: $KEYS_DIR"

# ── Step 3: Check for existing keys ───────────────────────────────────────
check_exists() {
    local file="$1"
    local label="$2"
    if [[ -f "$file" && "$FORCE" != true ]]; then
        log_warn "$label already exists: $file"
        log_warn "Use --force to overwrite (DESTRUCTIVE)"
        return 1
    fi
    return 0
}

# ── Step 4: Check required tools ──────────────────────────────────────────
log_step "Checking required tools"

CARGO="${CARGO:-/root/.cargo/bin/cargo}"
missing=()

command -v openssl >/dev/null || missing+=("openssl")
command -v keytool >/dev/null || missing+=("keytool (from JDK — install Java 21)")

if [[ -x "$CARGO" ]]; then
    "$CARGO" tauri --version >/dev/null 2>&1 || missing+=("tauri-cli (run: cargo install tauri-cli)")
else
    missing+=("cargo (expected at $CARGO)")
fi

if [[ ${#missing[@]} -gt 0 ]]; then
    log_error "Missing tools:"
    for dep in "${missing[@]}"; do
        echo "  - $dep"
    done
    exit 1
fi

log_ok "All tools available"

# ── Generate random password ──────────────────────────────────────────────
gen_password() {
    openssl rand -base64 24
}

# ── Step 5: Generate Tauri Signing Keypair ─────────────────────────────────
TAURI_KEY="$KEYS_DIR/coheara-tauri.key"
TAURI_PUB="$KEYS_DIR/coheara-tauri.key.pub"
TAURI_PASSWORD=""

generate_tauri_key() {
    log_step "Generating Tauri updater signing keypair"

    if ! check_exists "$TAURI_KEY" "Tauri private key"; then
        TAURI_PASSWORD="EXISTING"
        return
    fi

    TAURI_PASSWORD=$(gen_password)

    "$CARGO" tauri signer generate \
        -w "$TAURI_KEY" \
        -p "$TAURI_PASSWORD" \
        --ci --force

    log_ok "Tauri keypair generated"
    log_ok "  Private: $TAURI_KEY"
    log_ok "  Public:  $TAURI_PUB"
}

# ── Step 6: Generate Android Keystore ──────────────────────────────────────
ANDROID_JKS="$KEYS_DIR/coheara-release.jks"
ANDROID_PASSWORD=""

generate_android_keystore() {
    log_step "Generating Android release keystore"

    if ! check_exists "$ANDROID_JKS" "Android keystore"; then
        ANDROID_PASSWORD="EXISTING"
        return
    fi

    ANDROID_PASSWORD=$(gen_password)

    keytool -genkey -v \
        -keystore "$ANDROID_JKS" \
        -storetype PKCS12 \
        -keyalg RSA \
        -keysize 2048 \
        -validity 10000 \
        -alias coheara \
        -storepass "$ANDROID_PASSWORD" \
        -keypass "$ANDROID_PASSWORD" \
        -dname "CN=Coheara, OU=Mobile, O=Coheara, L=Paris, ST=IDF, C=FR"

    # Base64-encode for GitHub secrets (if needed later)
    base64 -w 0 "$ANDROID_JKS" > "$KEYS_DIR/coheara-release.jks.b64"

    log_ok "Android keystore generated"
    log_ok "  Keystore: $ANDROID_JKS"
    log_ok "  Base64:   $KEYS_DIR/coheara-release.jks.b64"
    log_ok "  Alias:    coheara"
}

# ── Step 7: Generate iOS Private Key + CSR ─────────────────────────────────
IOS_KEY="$KEYS_DIR/coheara-ios-dist.key"
IOS_CSR="$KEYS_DIR/coheara-ios-csr.pem"
IOS_P12_PASSWORD=""

generate_ios_key() {
    log_step "Generating iOS distribution key + CSR"

    if ! check_exists "$IOS_KEY" "iOS private key"; then
        IOS_P12_PASSWORD="EXISTING"
        return
    fi

    IOS_P12_PASSWORD=$(gen_password)

    # Generate RSA 2048-bit private key
    openssl genrsa -out "$IOS_KEY" 2048

    # Generate CSR (for upload to Apple Developer portal)
    openssl req -new \
        -key "$IOS_KEY" \
        -out "$IOS_CSR" \
        -subj "/CN=Coheara Distribution/O=Coheara/C=FR"

    log_ok "iOS key + CSR generated"
    log_ok "  Private key: $IOS_KEY"
    log_ok "  CSR:         $IOS_CSR"
    log_info "Upload the CSR to Apple Developer portal to get a .cer certificate"
    log_info "Then convert to .p12 with:"
    echo ""
    echo "  openssl x509 -inform DER -in $KEYS_DIR/coheara-ios-dist.cer -out /tmp/cert.pem"
    echo "  openssl pkcs12 -export \\"
    echo "    -inkey $IOS_KEY \\"
    echo "    -in /tmp/cert.pem \\"
    echo "    -out $KEYS_DIR/coheara-ios-dist.p12 \\"
    echo "    -passout 'pass:$IOS_P12_PASSWORD'"
    echo "  base64 -w 0 $KEYS_DIR/coheara-ios-dist.p12 > $KEYS_DIR/coheara-ios-dist.p12.b64"
    echo "  rm /tmp/cert.pem"
}

# ── Step 8: Write .env ────────────────────────────────────────────────────
write_env() {
    log_step "Writing .env file"

    if [[ -f "$ENV_FILE" && "$FORCE" != true ]]; then
        log_warn ".env already exists. Skipping (use --force to overwrite)."
        log_warn "If you generated new keys, manually update .env with the new passwords."
        return
    fi

    # Only write .env if at least one key was newly generated
    if [[ "$TAURI_PASSWORD" == "EXISTING" && "$ANDROID_PASSWORD" == "EXISTING" && "$IOS_P12_PASSWORD" == "EXISTING" ]]; then
        log_info "No new keys generated — .env not written."
        log_info "If you need a .env, re-run with --force to regenerate all keys."
        return
    fi

    local content="# Coheara build signing passwords
# Generated by setup-keys.sh on $(date -u +"%Y-%m-%d %H:%M:%S UTC")
# This file is git-ignored. NEVER commit it.
"

    if [[ "$TAURI_PASSWORD" != "EXISTING" ]]; then
        content+="
# Tauri updater signing
TAURI_SIGNING_PRIVATE_KEY_PASSWORD=${TAURI_PASSWORD}"
    fi

    if [[ "$ANDROID_PASSWORD" != "EXISTING" ]]; then
        content+="
# Android keystore
ANDROID_KEYSTORE_PASSWORD=${ANDROID_PASSWORD}
ANDROID_KEY_PASSWORD=${ANDROID_PASSWORD}
ANDROID_KEY_ALIAS=coheara"
    fi

    if [[ "$IOS_P12_PASSWORD" != "EXISTING" ]]; then
        content+="
# iOS P12 export password (used when creating .p12 from .cer)
IOS_P12_PASSWORD=${IOS_P12_PASSWORD}"
    fi

    echo "$content" > "$ENV_FILE"
    log_ok "Passwords written to .env"
}

# ── Step 9: Update tauri.conf.json public key ──────────────────────────────
update_tauri_pubkey() {
    if [[ "$TAURI_PASSWORD" == "EXISTING" ]]; then
        return
    fi

    log_step "Updating tauri.conf.json with new public key"

    if [[ ! -f "$TAURI_PUB" ]]; then
        log_warn "Public key file not found — skipping tauri.conf.json update"
        return
    fi

    local pubkey
    pubkey=$(cat "$TAURI_PUB")
    local conf="$PROJECT_ROOT/src-tauri/tauri.conf.json"

    if [[ ! -f "$conf" ]]; then
        log_warn "tauri.conf.json not found — skipping public key update"
        return
    fi

    # Replace the pubkey value in tauri.conf.json
    local old_pubkey
    old_pubkey=$(grep '"pubkey"' "$conf" | sed 's/.*"pubkey": *"\([^"]*\)".*/\1/')

    if [[ -n "$old_pubkey" ]]; then
        sed -i "s|\"pubkey\": \"$old_pubkey\"|\"pubkey\": \"$pubkey\"|" "$conf"
        log_ok "Updated pubkey in tauri.conf.json"
    else
        log_warn "Could not find pubkey field in tauri.conf.json — update manually"
        log_info "New public key: $pubkey"
    fi
}

# ── Run ────────────────────────────────────────────────────────────────────
generate_tauri_key
generate_android_keystore
generate_ios_key
write_env
update_tauri_pubkey

# ── Summary ────────────────────────────────────────────────────────────────
log_step "Setup complete"

echo ""
echo -e "${BOLD}Generated files:${NC}"
echo ""
echo "  Specs/build/"
echo "  ├── .gitignore              (defense layer 2 — blocks key patterns)"
echo "  └── keys/"

for f in "$KEYS_DIR"/*; do
    if [[ -f "$f" ]]; then
        local_name=$(basename "$f")
        size=$(du -h "$f" | cut -f1)
        echo "      ├── $local_name ($size)"
    fi
done

echo ""
echo "  .env                        (passwords for build.sh)"
echo ""
echo -e "${BOLD}Security layers:${NC}"
echo "  Layer 1: .gitignore excludes Specs/ entirely"
echo "  Layer 2: Specs/build/.gitignore blocks *.jks, *.key, *.p12, etc."
echo "  Layer 3: .env is git-ignored"
echo ""
echo -e "${BOLD}Next steps:${NC}"
echo "  1. Verify: git status  (no Specs/ or .env files should appear)"
echo "  2. Build:  ./build.sh desktop"
echo "  3. Test:   ./build.sh android --no-sign  (quick unsigned APK)"
echo ""

if [[ "$TAURI_PASSWORD" != "EXISTING" ]]; then
    echo -e "${YELLOW}[IMPORTANT]${NC} Back up these files to a secure location:"
    echo "  - Specs/build/keys/  (all key files)"
    echo "  - .env               (all passwords)"
    echo "  Loss of these files = cannot sign updates for existing installations."
    echo ""
fi

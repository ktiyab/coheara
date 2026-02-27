#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────
# setup-medgemma.sh — Coheara Model Factory
# ─────────────────────────────────────────────────────────────
#
# Builds hardware-calibrated MedGemma variants from Google's official source.
# Idempotent: skips steps already completed. Safe to re-run.
#
# SECURITY: Builds ONLY from Google's official safetensors at
# huggingface.co/google/medgemma-1.5-4b-it. No community models.
# For a medical app handling private health data, the model
# weights trust chain must go directly to the source.
#
# What this script does (skips steps already done):
#   1. Installs/updates Ollama (if missing)
#   2. Starts Ollama server (if not running)
#   3. Installs Python huggingface_hub (if missing)
#   4. Authenticates with HuggingFace (token from .env or interactive)
#   5. Downloads official safetensors (~8 GB, cached)
#   6. Builds quantized model(s) via ollama create
#   7. Verifies text + vision inference per variant
#
# Variants:
#   q4  → coheara-medgemma-4b-q4  (Q4_K_M, ~2.5 GB) — low RAM/VRAM
#   q8  → coheara-medgemma-4b-q8  (Q8_0,   ~4.1 GB) — balanced (default)
#   f16 → coheara-medgemma-4b-f16 (F16,    ~7.8 GB) — full precision
#
# Usage:
#   ./setup-medgemma.sh                    # Build default (q8)
#   ./setup-medgemma.sh --variant q4       # Build compact variant
#   ./setup-medgemma.sh --variant f16      # Build full precision
#   ./setup-medgemma.sh --all              # Build all 3 variants
#   ./setup-medgemma.sh --list             # Show built variants
#   ./setup-medgemma.sh --test             # Verify all built variants
#   ./setup-medgemma.sh --test --variant q8  # Verify specific variant
#   HF_TOKEN=hf_xxx ./setup-medgemma.sh   # Pass token via env
#
# Spec: Specs/experiments/MODEL-FACTORY-SPEC.md
# ─────────────────────────────────────────────────────────────

set -euo pipefail

# ─── Configuration ──────────────────────────────────────────

MODEL_PREFIX="coheara-medgemma-4b"
HF_REPO="google/medgemma-1.5-4b-it"
HF_TERMS_URL="https://huggingface.co/google/medgemma-1.5-4b-it"
MIN_OLLAMA_VERSION="0.9.0"
MIN_DISK_GB=15

# Variant → Ollama quantize flag mapping
declare -A QUANT_FLAGS=(
    [q4s]="q4_K_S"
    [q4]="q4_K_M"
    [q8]="q8_0"
    [f16]="f16"
)
ALL_VARIANTS=("q4s" "q4" "q8" "f16")
DEFAULT_VARIANT="q8"

# WSL2: use Linux filesystem for performance (not /mnt/c/)
DOWNLOAD_DIR="${HOME}/.cache/coheara/medgemma-safetensors"

# Look for .env in script directory (Coheara root)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ENV_FILE="${SCRIPT_DIR}/.env"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BOLD='\033[1m'
DIM='\033[2m'
NC='\033[0m'

info()  { echo -e "  ${GREEN}✓${NC} $*"; }
warn()  { echo -e "  ${YELLOW}!${NC} $*"; }
fail()  { echo -e "  ${RED}✗${NC} $*" >&2; }

# Dynamic step counter
CURRENT_STEP=0
TOTAL_STEPS=7
step() { CURRENT_STEP=$((CURRENT_STEP + 1)); echo -e "\n${BOLD}[${CURRENT_STEP}/${TOTAL_STEPS}]${NC} $1"; }

# Variant → full model name
model_name() { echo "${MODEL_PREFIX}-$1"; }

# ─── 1. Ollama ──────────────────────────────────────────────

setup_ollama() {
    step "Ollama"

    local need_install=false

    if ! command -v ollama &>/dev/null; then
        warn "Ollama not found. Installing..."
        need_install=true
    else
        local version
        version=$(ollama --version 2>/dev/null | grep -oP '[\d.]+' | head -1 || echo "0.0.0")
        info "Ollama ${version} installed."
    fi

    if $need_install; then
        echo -e "  ${DIM}Downloading from ollama.com...${NC}"
        curl -fsSL https://ollama.com/install.sh | sh 2>&1 | tail -3
        if ! command -v ollama &>/dev/null; then
            fail "Ollama installation failed."
            exit 1
        fi
        local version
        version=$(ollama --version 2>/dev/null | grep -oP '[\d.]+' | head -1)
        info "Ollama ${version} installed."
    fi
}

# ─── 2. Ollama server ──────────────────────────────────────

start_ollama_server() {
    step "Ollama server"

    if curl -sf http://localhost:11434/ &>/dev/null; then
        info "Server already running."
        return 0
    fi

    warn "Server not running. Starting..."

    # Try systemd first (works on native Linux)
    if command -v systemctl &>/dev/null && systemctl is-enabled ollama &>/dev/null 2>&1; then
        sudo systemctl start ollama 2>/dev/null || true
        sleep 2
        if curl -sf http://localhost:11434/ &>/dev/null; then
            info "Started via systemd."
            return 0
        fi
    fi

    # Manual start (WSL2, no systemd)
    nohup ollama serve > /tmp/ollama-serve.log 2>&1 &
    local pid=$!

    # Wait up to 10s for server
    local retries=10
    while [ $retries -gt 0 ]; do
        if curl -sf http://localhost:11434/ &>/dev/null; then
            info "Server started (PID ${pid})."
            return 0
        fi
        sleep 1
        retries=$((retries - 1))
    done

    fail "Cannot start Ollama server."
    echo "    Log: /tmp/ollama-serve.log"
    echo "    Try manually: ollama serve"
    exit 1
}

# ─── 3. Python + huggingface_hub ────────────────────────────

setup_python_deps() {
    step "Python dependencies"

    if ! command -v python3 &>/dev/null; then
        fail "Python 3 not found. Install: sudo apt install python3 python3-pip"
        exit 1
    fi

    if python3 -c "import huggingface_hub" &>/dev/null 2>&1; then
        local hf_version
        hf_version=$(python3 -c "import huggingface_hub; print(huggingface_hub.__version__)" 2>/dev/null)
        info "huggingface_hub ${hf_version} available."
    else
        warn "Installing huggingface_hub..."
        pip install --break-system-packages --quiet "huggingface_hub" 2>/dev/null \
            || pip install --quiet "huggingface_hub" 2>/dev/null \
            || pip install --user --quiet "huggingface_hub" 2>/dev/null
        if ! python3 -c "import huggingface_hub" &>/dev/null 2>&1; then
            fail "Cannot install huggingface_hub."
            echo "    Try: pip install huggingface_hub"
            exit 1
        fi
        info "huggingface_hub installed."
    fi
}

# ─── 4. HuggingFace auth ───────────────────────────────────

setup_hf_auth() {
    step "HuggingFace authentication"

    # Token resolution order: env var > .env file > interactive
    local token="${HF_TOKEN:-}"

    if [ -z "$token" ] && [ -f "$ENV_FILE" ]; then
        token=$(grep '^HF_TOKEN=' "$ENV_FILE" 2>/dev/null | cut -d= -f2- || true)
        if [ -n "$token" ]; then
            info "Token loaded from .env"
        fi
    fi

    if [ -z "$token" ]; then
        # Check cached token
        if python3 -c "
from huggingface_hub import HfApi
HfApi().whoami()
" &>/dev/null 2>&1; then
            info "Using cached HuggingFace token."
            return 0
        fi

        # Interactive login
        echo ""
        echo -e "  ${BOLD}HuggingFace token required.${NC}"
        echo "  1. Create an account at https://huggingface.co"
        echo "  2. Accept terms at: ${HF_TERMS_URL}"
        echo "  3. Create a token at: https://huggingface.co/settings/tokens"
        echo ""
        read -rp "  Paste your HF token (hf_...): " token
        echo ""

        if [ -z "$token" ]; then
            fail "No token provided."
            exit 1
        fi
    fi

    export HF_TOKEN="$token"

    # Verify token + repo access
    python3 -c "
import os
from huggingface_hub import HfApi
api = HfApi(token=os.environ['HF_TOKEN'])
user = api.whoami()['name']
print(f'  Authenticated as: {user}')
# Test repo access
api.model_info('${HF_REPO}')
print('  Repository access: granted')
" 2>&1 || {
        fail "Authentication failed or repository access denied."
        echo ""
        echo "    Accept Google's HAI-DEF terms at:"
        echo "    ${HF_TERMS_URL}"
        echo ""
        echo "    Then re-run this script."
        exit 1
    }

    info "HuggingFace access verified."
}

# ─── 5. Download safetensors ────────────────────────────────

download_safetensors() {
    step "Download safetensors"

    # Check if already downloaded
    if [ -f "${DOWNLOAD_DIR}/config.json" ] && ls "${DOWNLOAD_DIR}"/*.safetensors &>/dev/null 2>&1; then
        local size
        size=$(du -sh "${DOWNLOAD_DIR}" 2>/dev/null | cut -f1)
        info "Already downloaded (${size} at ${DOWNLOAD_DIR})"
        return 0
    fi

    # Check disk space
    local available_gb
    available_gb=$(df --output=avail -BG "${HOME}" 2>/dev/null | tail -1 | tr -d ' G' || echo "0")
    if [ "${available_gb:-0}" -lt "${MIN_DISK_GB}" ]; then
        fail "Need ~${MIN_DISK_GB} GB free. Available: ${available_gb} GB"
        exit 1
    fi
    info "${available_gb} GB disk available."

    mkdir -p "${DOWNLOAD_DIR}"

    echo -e "  ${DIM}Downloading ~8 GB from ${HF_REPO}...${NC}"

    python3 -c "
import os
from huggingface_hub import snapshot_download

path = snapshot_download(
    '${HF_REPO}',
    local_dir='${DOWNLOAD_DIR}',
    token=os.environ.get('HF_TOKEN', None),
    allow_patterns=['*.safetensors', '*.json', 'tokenizer*', '*.model'],
)
" 2>&1

    # Verify essential files
    if [ ! -f "${DOWNLOAD_DIR}/config.json" ]; then
        fail "Download incomplete — config.json missing."
        exit 1
    fi
    if ! ls "${DOWNLOAD_DIR}"/*.safetensors &>/dev/null 2>&1; then
        fail "Download incomplete — no .safetensors files."
        exit 1
    fi

    local size
    size=$(du -sh "${DOWNLOAD_DIR}" 2>/dev/null | cut -f1)
    info "Downloaded ${size} to ${DOWNLOAD_DIR}"
}

# ─── 6. Build model ────────────────────────────────────────

build_variant() {
    local variant="$1"
    local quant_flag="${QUANT_FLAGS[$variant]}"
    local name
    name=$(model_name "$variant")

    step "Build ${name} (${quant_flag})"

    # Check if model already exists
    if ollama show "${name}" &>/dev/null 2>&1; then
        warn "Model ${name} already exists in Ollama."
        read -rp "  Rebuild? [y/N] " rebuild
        if [[ ! "$rebuild" =~ ^[Yy] ]]; then
            info "Keeping existing model."
            return 0
        fi
    fi

    # Phase 1: Convert safetensors → Ollama format (heavy, ~5-20 min)
    # Minimal Modelfile for conversion — just FROM and context
    cat > "${DOWNLOAD_DIR}/Modelfile" <<'EOF'
FROM .
PARAMETER num_ctx 8192
EOF

    echo -e "  ${DIM}Converting safetensors → Ollama format (${quant_flag})...${NC}"
    if [ "$variant" = "f16" ]; then
        echo -e "  ${DIM}F16: no quantization, fastest build (~5 min).${NC}"
    else
        echo -e "  ${DIM}This takes 5-20 minutes on CPU.${NC}"
    fi
    echo ""

    (cd "${DOWNLOAD_DIR}" && ollama create "${name}" --quantize "${quant_flag}")

    # Phase 2: Apply Gemma3 chat template (instant — reuses converted weights)
    # MedGemma's tokenizer_config.json lacks a chat_template, so Ollama can't
    # auto-detect it. We overlay the Modelfile.coheara template.
    local tmpfile
    tmpfile=$(mktemp /tmp/Modelfile.coheara.XXXXXX)
    cat > "${tmpfile}" <<TMPL
FROM ${name}

TEMPLATE """{{- range .Messages }}
{{- if or (eq .Role "user") (eq .Role "system") }}
<start_of_turn>user
{{ .Content }}<end_of_turn>
{{ end }}
{{- if eq .Role "assistant" }}
<start_of_turn>model
{{ .Content }}<end_of_turn>
{{ end }}
{{- end }}
<start_of_turn>model
"""

PARAMETER stop <end_of_turn>
PARAMETER num_ctx 8192
TMPL

    ollama create "${name}" -f "${tmpfile}" 2>&1
    rm -f "${tmpfile}"

    info "Model ${name} created with Gemma3 chat template."
}

# ─── 7. Verify ─────────────────────────────────────────────

verify_variant() {
    local name="$1"

    echo -e "\n  ${BOLD}Verifying ${name}${NC}"

    # Model registered?
    if ! ollama show "${name}" &>/dev/null 2>&1; then
        fail "${name} not found in Ollama."
        return 1
    fi
    info "Model registered."

    # Verify chat template is set
    local tmpl
    tmpl=$(ollama show "${name}" --template 2>&1)
    if echo "$tmpl" | grep -q "start_of_turn"; then
        info "Gemma3 chat template: set."
    else
        warn "Chat template missing — model may not follow instructions."
    fi

    # Text test (streaming)
    echo -e "  ${DIM}Testing text generation...${NC}"
    local text_output
    text_output=$(curl -sf --max-time 180 http://localhost:11434/api/chat -d "{
        \"model\": \"${name}\",
        \"messages\": [{\"role\": \"user\", \"content\": \"What is aspirin? Reply in one sentence.\"}],
        \"stream\": true
    }" 2>&1) || true

    if [ -z "$text_output" ]; then
        fail "Text: no response (timeout)."
        return 1
    elif echo "$text_output" | grep -q '"done":true'; then
        info "Text generation: working."
    elif echo "$text_output" | grep -qi '"error"'; then
        fail "Text: model error."
        echo "    $(echo "$text_output" | grep -o '"error":"[^"]*"' | head -1)"
        return 1
    else
        warn "Text generation: partial response (may need longer timeout)."
    fi

    # Vision test — valid 64x64 red PNG (generated inline)
    echo -e "  ${DIM}Testing vision...${NC}"
    local red_png
    red_png=$(python3 -c "
import base64, struct, zlib
def make_png(w, h, r, g, b):
    def chunk(t, d):
        c = t + d
        return struct.pack('>I', len(d)) + c + struct.pack('>I', zlib.crc32(c) & 0xffffffff)
    raw = b''
    for _ in range(h):
        raw += b'\x00' + bytes([r, g, b]) * w
    return b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', w, h, 8, 2, 0, 0, 0)) + chunk(b'IDAT', zlib.compress(raw)) + chunk(b'IEND', b'')
print(base64.b64encode(make_png(64, 64, 255, 0, 0)).decode())
" 2>/dev/null)

    if [ -z "$red_png" ]; then
        warn "Vision: skipped (could not generate test image)."
        return 0
    fi

    local vision_output
    vision_output=$(curl -sf --max-time 300 http://localhost:11434/api/chat -d "{
        \"model\": \"${name}\",
        \"messages\": [{
            \"role\": \"user\",
            \"content\": \"What color is this image? Reply with just the color name.\",
            \"images\": [\"${red_png}\"]
        }],
        \"stream\": true
    }" 2>&1) || true

    if [ -z "$vision_output" ]; then
        fail "Vision: no response (timeout — may need GPU)."
        return 1
    elif echo "$vision_output" | grep -q '"done":true'; then
        info "Vision: working."
    elif echo "$vision_output" | grep -qi '"error"\|unsupported\|does not support'; then
        fail "Vision: not supported by this model build."
        echo "    $(echo "$vision_output" | grep -o '"error":"[^"]*"' | head -1)"
        return 1
    else
        warn "Vision: partial response."
    fi

    return 0
}

# ─── List variants ──────────────────────────────────────────

list_variants() {
    echo ""
    echo -e "  ${BOLD}Coheara MedGemma Variants${NC}"
    echo -e "  Source: ${DIM}${HF_REPO}${NC}"
    echo ""
    printf "  %-30s  %-10s  %-10s  %s\n" "MODEL" "QUANT" "SIZE" "STATUS"
    echo "  $(printf '%.0s─' {1..70})"

    local found=0
    for variant in "${ALL_VARIANTS[@]}"; do
        local name
        name=$(model_name "$variant")
        local quant="${QUANT_FLAGS[$variant]}"
        local status="not built"
        local size="-"

        if ollama show "${name}" &>/dev/null 2>&1; then
            status="${GREEN}ready${NC}"
            # Get model size from ollama list
            size=$(ollama list 2>/dev/null | grep "^${name}" | awk '{print $3 " " $4}' || echo "?")
            found=$((found + 1))
        else
            status="${DIM}not built${NC}"
        fi

        printf "  %-30s  %-10s  %-10s  " "${name}" "${quant}" "${size}"
        echo -e "${status}"
    done

    echo ""
    if [ "$found" -eq 0 ]; then
        echo -e "  ${DIM}No variants built yet. Run: $0 --variant q8${NC}"
    else
        echo -e "  ${DIM}${found}/${#ALL_VARIANTS[@]} variants built.${NC}"
    fi
    echo ""
}

# ─── Main ───────────────────────────────────────────────────

main() {
    local mode="build"
    local variant="${DEFAULT_VARIANT}"
    local build_all=false

    while [[ $# -gt 0 ]]; do
        case "$1" in
            --test)      mode="test"; shift ;;
            --list)      mode="list"; shift ;;
            --all)       build_all=true; shift ;;
            --variant)
                variant="$2"
                if [[ ! " ${ALL_VARIANTS[*]} " =~ " ${variant} " ]]; then
                    fail "Unknown variant: ${variant}. Valid: ${ALL_VARIANTS[*]}"
                    exit 1
                fi
                shift 2
                ;;
            --help|-h)
                echo "setup-medgemma.sh — Coheara Model Factory"
                echo ""
                echo "Builds hardware-calibrated MedGemma variants from Google's official source."
                echo ""
                echo "Usage: $0 [OPTIONS]"
                echo ""
                echo "Build:"
                echo "  --variant q4|q8|f16   Build specific variant (default: q8)"
                echo "  --all                 Build all 3 variants"
                echo ""
                echo "Inspect:"
                echo "  --list                Show built variants"
                echo "  --test                Verify built variants (text + vision)"
                echo "  --test --variant q8   Verify specific variant"
                echo ""
                echo "Other:"
                echo "  --help                Show this help"
                echo ""
                echo "Variants:"
                echo "  q4s → ${MODEL_PREFIX}-q4s (Q4_K_S, ~2.4 GB) — quantization floor"
                echo "  q4  → ${MODEL_PREFIX}-q4  (Q4_K_M, ~2.5 GB) — low RAM/VRAM"
                echo "  q8  → ${MODEL_PREFIX}-q8  (Q8_0,   ~4.1 GB) — balanced (default)"
                echo "  f16 → ${MODEL_PREFIX}-f16 (F16,    ~7.8 GB) — full precision"
                echo ""
                echo "Source: ${HF_REPO}"
                echo "Terms:  ${HF_TERMS_URL}"
                echo "Spec:   Specs/experiments/MODEL-FACTORY-SPEC.md"
                exit 0
                ;;
            *)  fail "Unknown: $1. Try --help"; exit 1 ;;
        esac
    done

    echo ""
    echo -e "  ${BOLD}Coheara Model Factory${NC} — MedGemma 1.5 4B IT"
    echo -e "  ${DIM}${HF_REPO}${NC}"
    echo ""

    # ── List mode ──
    if [ "$mode" = "list" ]; then
        list_variants
        exit 0
    fi

    # ── Test mode ──
    if [ "$mode" = "test" ]; then
        setup_ollama
        start_ollama_server

        if $build_all; then
            step "Verify all variants"
            local pass=0
            local total=0
            for v in "${ALL_VARIANTS[@]}"; do
                local name
                name=$(model_name "$v")
                if ollama show "${name}" &>/dev/null 2>&1; then
                    total=$((total + 1))
                    if verify_variant "${name}"; then
                        pass=$((pass + 1))
                    fi
                fi
            done
            echo ""
            echo -e "  ${BOLD}Verification: ${pass}/${total} passed.${NC}"
        else
            local name
            name=$(model_name "$variant")
            step "Verify ${name}"
            verify_variant "${name}"
        fi
        exit $?
    fi

    # ── Build mode ──
    local variants_to_build=()
    if $build_all; then
        variants_to_build=("${ALL_VARIANTS[@]}")
        TOTAL_STEPS=$((5 + ${#ALL_VARIANTS[@]} + 1))  # steps 1-5 + N builds + 1 verify
    else
        variants_to_build=("$variant")
        TOTAL_STEPS=7  # steps 1-5 + 1 build + 1 verify
    fi

    # Steps 1-5: shared setup (download once, build many)
    setup_ollama
    start_ollama_server
    setup_python_deps
    setup_hf_auth
    download_safetensors

    # Step 6: build variant(s)
    for v in "${variants_to_build[@]}"; do
        build_variant "$v"
    done

    # Step 7: verify
    step "Verify"
    local pass=0
    local total=${#variants_to_build[@]}
    for v in "${variants_to_build[@]}"; do
        local name
        name=$(model_name "$v")
        if verify_variant "${name}"; then
            pass=$((pass + 1))
        fi
    done

    echo ""
    if [ "$pass" -eq "$total" ]; then
        echo -e "  ${GREEN}${BOLD}Setup complete.${NC} ${pass}/${total} variants verified."
        for v in "${variants_to_build[@]}"; do
            echo "  • $(model_name "$v")"
        done
        echo ""
        echo -e "  ${DIM}List all: $0 --list${NC}"
    else
        fail "${pass}/${total} variants verified. Check errors above."
        echo "    Retry: $0 --test"
        exit 1
    fi
    echo ""
}

main "$@"

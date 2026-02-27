#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────
# gce-build-startup.sh — GCE Worker Script
# ─────────────────────────────────────────────────────────────
#
# Builds ONE MedGemma variant and saves to GCS.
# Optionally pushes to Ollama registry.
#
# Self-contained — does NOT depend on the local repo.
# Configuration comes from GCE instance metadata.
#
# Metadata keys:
#   action       — "build" (default) or "build+push"
#   variant      — One of: q4, q8, f16
#   hf-token     — HuggingFace access token (gated model)
#   gcs-bucket   — GCS bucket name
#   registry-ns  — Ollama registry namespace (push only)
#   ollama-key   — Ollama SSH key content (push only)
#
# Spec: Specs/experiments/CLOUD-BUILD-SPEC.md
# ─────────────────────────────────────────────────────────────

set -euo pipefail

export HOME="${HOME:-/root}"

# ─── Logging ─────────────────────────────────────────────────

LOG_FILE="/var/log/coheara-build.log"

log()       { echo "[$(date -u '+%Y-%m-%d %H:%M:%S UTC')] $*" | tee -a "$LOG_FILE"; }
log_error() { echo "[$(date -u '+%Y-%m-%d %H:%M:%S UTC')] ERROR: $*" | tee -a "$LOG_FILE" >&2; }

# ─── Error Trap ──────────────────────────────────────────────

on_error() {
    local exit_code=$? line_no=$1
    log_error "Script failed at line ${line_no} (exit ${exit_code})"
    if [ -n "${GCS_BUCKET:-}" ] && [ -n "${VARIANT:-}" ]; then
        echo "FAILED at line ${line_no} (exit ${exit_code}) — $(date -u)" \
            | gsutil -q cp - "gs://${GCS_BUCKET}/status/${VARIANT}-error.fail" 2>/dev/null || true
        gsutil -q cp "$LOG_FILE" "gs://${GCS_BUCKET}/status/${VARIANT}-build.log" 2>/dev/null || true
    fi
}
trap 'on_error $LINENO' ERR

# ─── Metadata ────────────────────────────────────────────────

META_URL="http://metadata.google.internal/computeMetadata/v1"
META_HDR="Metadata-Flavor: Google"

get_meta() { curl -sf -H "$META_HDR" "${META_URL}/instance/attributes/$1" 2>/dev/null || echo ""; }

# ─── Configuration ───────────────────────────────────────────

MODEL_PREFIX="coheara-medgemma-4b"
HF_REPO="google/medgemma-1.5-4b-it"
SAFETENSORS_DIR="/tmp/medgemma-safetensors"

declare -A QUANT_FLAGS=(
    [q4s]="q4_K_S"
    [q4]="q4_K_M"
    [q8]="q8_0"
    [f16]="f16"
)

# Populated by load_config
ACTION=""
VARIANT=""
GCS_BUCKET=""
HF_TOKEN=""
REGISTRY_NS=""

load_config() {
    log "Loading config from metadata..."

    ACTION=$(get_meta "action")
    VARIANT=$(get_meta "variant")
    GCS_BUCKET=$(get_meta "gcs-bucket")
    HF_TOKEN=$(get_meta "hf-token")
    REGISTRY_NS=$(get_meta "registry-ns")

    ACTION="${ACTION:-build}"

    # Validate
    if [ -z "$VARIANT" ]; then log_error "variant metadata not set."; exit 1; fi
    if [ -z "$GCS_BUCKET" ]; then log_error "gcs-bucket metadata not set."; exit 1; fi
    if [ -z "${QUANT_FLAGS[$VARIANT]+x}" ]; then log_error "Unknown variant: ${VARIANT}"; exit 1; fi
    if [ -z "$HF_TOKEN" ]; then log_error "hf-token metadata not set."; exit 1; fi

    if [[ "$ACTION" == *"push"* ]]; then
        if [ -z "$REGISTRY_NS" ]; then log_error "registry-ns needed for push."; exit 1; fi
        local key_content
        key_content=$(get_meta "ollama-key")
        if [ -z "$key_content" ]; then log_error "ollama-key needed for push."; exit 1; fi
        # Install key
        mkdir -p /root/.ollama
        echo "$key_content" > /root/.ollama/id_ed25519
        chmod 600 /root/.ollama/id_ed25519
        log "Ollama registry key installed."
    fi

    log "Config: action=${ACTION} variant=${VARIANT} bucket=${GCS_BUCKET}"
}

# ─── GCS Helpers ─────────────────────────────────────────────

gcs_marker() { echo "$2" | gsutil -q cp - "$1"; }

# ─── Install Ollama ──────────────────────────────────────────

install_ollama() {
    if command -v ollama &>/dev/null; then
        log "Ollama already installed."
        return 0
    fi
    log "Installing Ollama..."
    curl -fsSL https://ollama.com/install.sh | sh 2>&1 | tail -5 | tee -a "$LOG_FILE"
    command -v ollama &>/dev/null || { log_error "Ollama install failed."; exit 1; }
    log "Ollama installed."
}

start_ollama() {
    log "Starting Ollama server..."
    OLLAMA_HOST="0.0.0.0:11434" nohup ollama serve > /var/log/ollama-serve.log 2>&1 &
    local retries=30
    while [ $retries -gt 0 ]; do
        curl -sf http://localhost:11434/ &>/dev/null && { log "Ollama server ready."; return 0; }
        sleep 1; retries=$((retries - 1))
    done
    log_error "Ollama server failed to start."; exit 1
}

# ─── Download Safetensors ────────────────────────────────────

download_safetensors() {
    log "Downloading safetensors..."
    mkdir -p "$SAFETENSORS_DIR"

    # Try GCS cache first
    if gsutil -q stat "gs://${GCS_BUCKET}/safetensors/config.json" 2>/dev/null; then
        log "Cache hit — downloading from GCS..."
        gsutil -m cp -r "gs://${GCS_BUCKET}/safetensors/*" "${SAFETENSORS_DIR}/" 2>&1 | tee -a "$LOG_FILE"
        if [ -f "${SAFETENSORS_DIR}/config.json" ] && ls "${SAFETENSORS_DIR}"/*.safetensors &>/dev/null 2>&1; then
            log "Safetensors from GCS cache: $(du -sh "$SAFETENSORS_DIR" | cut -f1)"
            return 0
        fi
        log "GCS cache incomplete, falling back to HuggingFace..."
    fi

    # Download from HuggingFace
    log "Downloading from HuggingFace (${HF_REPO})..."
    if ! python3 -c "import huggingface_hub" &>/dev/null 2>&1; then
        log "Installing huggingface_hub..."
        apt-get update -qq && apt-get install -y -qq python3-pip 2>&1 | tail -3 | tee -a "$LOG_FILE"
        pip install --break-system-packages --quiet huggingface_hub 2>&1 | tee -a "$LOG_FILE"
    fi

    export HF_TOKEN
    python3 -c "
import os
from huggingface_hub import snapshot_download
path = snapshot_download(
    '${HF_REPO}',
    local_dir='${SAFETENSORS_DIR}',
    token=os.environ.get('HF_TOKEN'),
    allow_patterns=['*.safetensors', '*.json', 'tokenizer*', '*.model'],
)
print(f'Downloaded to: {path}')
" 2>&1 | tee -a "$LOG_FILE"

    # Verify
    [ -f "${SAFETENSORS_DIR}/config.json" ] || { log_error "config.json missing."; exit 1; }
    ls "${SAFETENSORS_DIR}"/*.safetensors &>/dev/null 2>&1 || { log_error "No .safetensors files."; exit 1; }
    log "Downloaded: $(du -sh "$SAFETENSORS_DIR" | cut -f1)"

    # Cache to GCS
    log "Caching safetensors to GCS..."
    gsutil -m cp -r "${SAFETENSORS_DIR}/"* "gs://${GCS_BUCKET}/safetensors/" 2>&1 | tee -a "$LOG_FILE"
}

# ─── Build ───────────────────────────────────────────────────

do_build() {
    local name="${MODEL_PREFIX}-${VARIANT}"
    local quant="${QUANT_FLAGS[$VARIANT]}"
    local start_time
    start_time=$(date +%s)

    log "═══ BUILD: ${name} (${quant}) ═══"

    # Quantize
    log "Quantizing safetensors..."
    (cd "${SAFETENSORS_DIR}" && ollama create "${name}" --quantize "${quant}") 2>&1 | tee -a "$LOG_FILE"

    # Apply chat template
    log "Applying Gemma3 chat template..."
    cat > "/tmp/Modelfile.${VARIANT}" <<'TMPL'
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
    # Re-create with template (heredoc can't expand ${name} inside 'TMPL', use sed)
    sed -i "s|\${name}|${name}|g" "/tmp/Modelfile.${VARIANT}"
    ollama create "${name}" -f "/tmp/Modelfile.${VARIANT}" 2>&1 | tee -a "$LOG_FILE"
    rm -f "/tmp/Modelfile.${VARIANT}"

    # Verify
    ollama show "${name}" &>/dev/null 2>&1 || { log_error "${name} not found after create."; exit 1; }

    local model_size
    model_size=$(ollama list 2>/dev/null | grep "^${name}" | awk '{print $3, $4}' || echo "unknown")

    # Export to GCS
    log "Exporting model blob to GCS..."
    local blob_dir="/root/.ollama/models"
    tar -cf "/tmp/${name}.tar" -C "$blob_dir" . 2>&1 | tee -a "$LOG_FILE"
    gsutil -m cp "/tmp/${name}.tar" "gs://${GCS_BUCKET}/models/${name}.tar" 2>&1 | tee -a "$LOG_FILE"
    rm -f "/tmp/${name}.tar"

    local duration=$(( $(date +%s) - start_time ))

    # Write marker
    gcs_marker "gs://${GCS_BUCKET}/status/${VARIANT}.built" \
        "$(date -u '+%Y-%m-%d %H:%M:%S UTC') | ${name} | ${model_size} | ${duration}s"

    log "═══ BUILD DONE: ${name} (${model_size}, ${duration}s) ═══"
}

# ─── Push ────────────────────────────────────────────────────

do_push() {
    local name="${MODEL_PREFIX}-${VARIANT}"
    local registry_name="${REGISTRY_NS}/${name}"
    local start_time
    start_time=$(date +%s)

    log "═══ PUSH: ${registry_name} ═══"

    # Check if model is local (from build), otherwise load from GCS
    if ! ollama show "${name}" &>/dev/null 2>&1; then
        log "Model not local — loading from GCS..."
        local blob="gs://${GCS_BUCKET}/models/${name}.tar"
        gsutil -q stat "$blob" 2>/dev/null || { log_error "No blob at ${blob}. Build first."; exit 1; }

        local blob_dir="/root/.ollama/models"
        mkdir -p "$blob_dir"
        gsutil -m cp "$blob" "/tmp/${name}.tar" 2>&1 | tee -a "$LOG_FILE"
        tar -xf "/tmp/${name}.tar" -C "$blob_dir" 2>&1 | tee -a "$LOG_FILE"
        rm -f "/tmp/${name}.tar"

        ollama show "${name}" &>/dev/null 2>&1 || { log_error "${name} not found after import."; exit 1; }
        log "Model loaded from GCS."
    fi

    # Tag for registry
    ollama cp "${name}" "${registry_name}" 2>&1 | tee -a "$LOG_FILE"

    # Push
    log "Pushing to registry..."
    local push_output
    push_output=$(ollama push "${registry_name}" 2>&1) || {
        log_error "Push failed:"
        echo "$push_output" | tee -a "$LOG_FILE"
        exit 1
    }
    echo "$push_output" | tee -a "$LOG_FILE"

    local duration=$(( $(date +%s) - start_time ))

    gcs_marker "gs://${GCS_BUCKET}/status/${VARIANT}.pushed" \
        "$(date -u '+%Y-%m-%d %H:%M:%S UTC') | ${registry_name} | ${duration}s"

    log "═══ PUSH DONE: ${registry_name} (${duration}s) ═══"
}

# ─── Main ────────────────────────────────────────────────────

main() {
    log "═══════════════════════════════════════════════════"
    log "Coheara Model Worker — ${ACTION:-build} ${VARIANT:-?}"
    log "═══════════════════════════════════════════════════"

    load_config
    install_ollama
    start_ollama
    download_safetensors

    case "$ACTION" in
        build)
            do_build
            ;;
        build+push)
            do_build
            do_push
            ;;
        push)
            do_push
            ;;
        *)
            log_error "Unknown action: ${ACTION}"
            exit 1
            ;;
    esac

    # Upload log
    gsutil -q cp "$LOG_FILE" "gs://${GCS_BUCKET}/status/${VARIANT}-build.log" 2>/dev/null || true

    log "Shutting down..."
    shutdown -h now || true
}

main "$@"

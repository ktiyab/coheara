#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────
# cloud-build-medgemma.sh — Cloud Build Orchestrator
# ─────────────────────────────────────────────────────────────
#
# Manages GCE instances to build and push MedGemma variants.
# Each invocation handles ONE variant, ONE action.
#
# Usage:
#   ./cloud-build-medgemma.sh build q4         Build q4, save to GCS
#   ./cloud-build-medgemma.sh build q8         Build q8, save to GCS
#   ./cloud-build-medgemma.sh build+push q4    Build q4 + push to registry
#   ./cloud-build-medgemma.sh push q4          Push q4 from GCS to registry
#   ./cloud-build-medgemma.sh status           Check all GCS markers
#   ./cloud-build-medgemma.sh pull             Pull from registry to local
#   ./cloud-build-medgemma.sh cleanup          Delete orphaned GCE instance
#
# Workflow (recommended):
#   1. Build one variant, validate it works:
#      ./cloud-build-medgemma.sh build q4
#      ./cloud-build-medgemma.sh status
#
#   2. Build remaining variants:
#      ./cloud-build-medgemma.sh build q8
#      ./cloud-build-medgemma.sh build f16
#
#   3. Push all to registry:
#      ./cloud-build-medgemma.sh push q4
#      ./cloud-build-medgemma.sh push q8
#      ./cloud-build-medgemma.sh push f16
#
#   4. Pull to local Ollama:
#      ./cloud-build-medgemma.sh pull
#
# Environment (.env):
#   OLLAMA_REGISTRY_NS   Ollama username (required)
#   GCS_BUCKET           GCS bucket name (required)
#   HF_TOKEN             HuggingFace token (build only)
#   OLLAMA_KEY_FILE      SSH key path (push only)
#   GCE_MACHINE_TYPE     Default: e2-standard-8
#   GCE_DISK_SIZE        Default: 50GB
#   GCE_DISK_TYPE        Default: pd-ssd
#   GCE_ZONE             Default: us-central1-a
#   GCE_MAX_RUN          Default: 3600s
#
# Spec: Specs/experiments/CLOUD-BUILD-SPEC.md
# ─────────────────────────────────────────────────────────────

set -euo pipefail

# ─── Constants ───────────────────────────────────────────────

INSTANCE_NAME="coheara-model-builder"
IMAGE_FAMILY="ubuntu-2404-lts-amd64"
IMAGE_PROJECT="ubuntu-os-cloud"
MODEL_PREFIX="coheara-medgemma-4b"
ALL_VARIANTS=("q4s" "q4" "q8" "f16")

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
# ENV_FILE="${PROJECT_ROOT}/.env"
ENV_FILE="${SCRIPT_DIR}/.env"
WORKER_SCRIPT="${SCRIPT_DIR}/gce-build-startup.sh"

# ─── Defaults (overridable via .env) ─────────────────────────

GCE_MACHINE_TYPE="${GCE_MACHINE_TYPE:-e2-standard-8}"
GCE_DISK_SIZE="${GCE_DISK_SIZE:-100GB}"
GCE_DISK_TYPE="${GCE_DISK_TYPE:-pd-ssd}"
GCE_ZONE="${GCE_ZONE:-us-central1-a}"
GCE_MAX_RUN="${GCE_MAX_RUN:-3600s}"

GCS_BUCKET=""
REGISTRY_NS=""

# ─── Colors ──────────────────────────────────────────────────

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'
BOLD='\033[1m'; DIM='\033[2m'; NC='\033[0m'

info()  { echo -e "  ${GREEN}✓${NC} $*"; }
warn()  { echo -e "  ${YELLOW}!${NC} $*"; }
fail()  { echo -e "  ${RED}✗${NC} $*" >&2; }

# ─── Load .env ───────────────────────────────────────────────

load_env() {
    if [ -f "$ENV_FILE" ]; then
        set -a; source "$ENV_FILE"; set +a
    fi
    REGISTRY_NS="${OLLAMA_REGISTRY_NS:-}"
    GCS_BUCKET="${GCS_BUCKET:-}"
    [ -n "$REGISTRY_NS" ] || { fail "OLLAMA_REGISTRY_NS not set in .env"; exit 1; }
    [ -n "$GCS_BUCKET" ]  || { fail "GCS_BUCKET not set in .env"; exit 1; }
}

validate_gcloud() {
    command -v gcloud &>/dev/null || { fail "gcloud not found."; exit 1; }
    local acct
    acct=$(gcloud auth list --filter="status:ACTIVE" --format="value(account)" 2>/dev/null | head -1)
    [ -n "$acct" ] || { fail "gcloud not authenticated."; exit 1; }
    info "GCP: ${acct} / $(gcloud config get-value project 2>/dev/null)"
}

ensure_bucket() {
    if gsutil ls "gs://${GCS_BUCKET}/" &>/dev/null 2>&1; then
        info "Bucket: gs://${GCS_BUCKET}/"
    else
        warn "Creating bucket gs://${GCS_BUCKET}/..."
        gsutil mb -l us-central1 "gs://${GCS_BUCKET}/"
    fi
}

# ─── Instance Lifecycle ──────────────────────────────────────

instance_exists() {
    gcloud compute instances describe "$INSTANCE_NAME" \
        --zone="$GCE_ZONE" --format="value(status)" &>/dev/null 2>&1
}

instance_status() {
    gcloud compute instances describe "$INSTANCE_NAME" \
        --zone="$GCE_ZONE" --format="value(status)" 2>/dev/null || echo "NOT_FOUND"
}

delete_instance() {
    if instance_exists; then
        echo -e "  ${DIM}Deleting ${INSTANCE_NAME}...${NC}"
        gcloud compute instances delete "$INSTANCE_NAME" \
            --zone="$GCE_ZONE" --quiet 2>/dev/null || true
        info "Instance deleted."
    fi
}

cleanup_trap() {
    local exit_code=$?
    if instance_exists; then
        warn "Cleaning up GCE instance..."
        delete_instance
    fi
    exit $exit_code
}

# ─── Create + Monitor ────────────────────────────────────────

run_on_gce() {
    local action="$1"
    local variant="$2"

    # Validate worker script
    [ -f "$WORKER_SCRIPT" ] || { fail "Worker not found: ${WORKER_SCRIPT}"; exit 1; }

    # Build metadata
    local metadata="^~^action=${action}~variant=${variant}~gcs-bucket=${GCS_BUCKET}~hf-token=${HF_TOKEN:-}~registry-ns=${REGISTRY_NS}"
    local metadata_files="startup-script=${WORKER_SCRIPT}"

    # Add ollama key for push actions
    if [[ "$action" == *"push"* ]]; then
        local key_file="${OLLAMA_KEY_FILE:-}"
        key_file="${key_file/#\~/$HOME}"
        [ -f "$key_file" ] || { fail "Ollama key not found: ${key_file}"; exit 1; }
        info "Ollama key: ${key_file}"
        metadata_files="${metadata_files},ollama-key=${key_file}"
    fi

    # Validate HF_TOKEN for build actions
    if [[ "$action" == *"build"* ]]; then
        [ -n "${HF_TOKEN:-}" ] || { fail "HF_TOKEN not set in .env"; exit 1; }
    fi

    # Clean ALL old markers for this variant (start fresh)
    info "Cleaning old markers for ${variant}..."
    gsutil -q rm "gs://${GCS_BUCKET}/status/${variant}.*" 2>/dev/null || true
    gsutil -q rm "gs://${GCS_BUCKET}/status/${variant}-*" 2>/dev/null || true
    # Also clean legacy format markers from previous script versions
    gsutil -q rm "gs://${GCS_BUCKET}/status/build-${variant}.*" 2>/dev/null || true
    gsutil -q rm "gs://${GCS_BUCKET}/status/build-complete.done" 2>/dev/null || true
    gsutil -q rm "gs://${GCS_BUCKET}/status/build.log" 2>/dev/null || true

    # Delete existing instance
    if instance_exists; then
        warn "Instance exists. Deleting..."
        delete_instance
    fi

    echo ""
    echo -e "  ${BOLD}GCE: ${action} ${variant}${NC}"
    echo -e "  ${DIM}Machine: ${GCE_MACHINE_TYPE} | Disk: ${GCE_DISK_SIZE} ${GCE_DISK_TYPE} | Zone: ${GCE_ZONE}${NC}"
    echo ""

    gcloud compute instances create "$INSTANCE_NAME" \
        --zone="$GCE_ZONE" \
        --machine-type="$GCE_MACHINE_TYPE" \
        --boot-disk-size="$GCE_DISK_SIZE" \
        --boot-disk-type="$GCE_DISK_TYPE" \
        --image-family="$IMAGE_FAMILY" \
        --image-project="$IMAGE_PROJECT" \
        --scopes=storage-rw,logging-write \
        --metadata="$metadata" \
        --metadata-from-file="$metadata_files" \
        --max-run-duration="$GCE_MAX_RUN" \
        --instance-termination-action="DELETE" \
        --no-restart-on-failure \
        --quiet 2>&1

    info "Instance created."

    # Monitor
    monitor "$action" "$variant"
}

monitor() {
    local action="$1"
    local variant="$2"

    # Determine which marker signals completion
    local done_marker
    if [[ "$action" == *"push"* ]]; then
        done_marker="gs://${GCS_BUCKET}/status/${variant}.pushed"
    else
        done_marker="gs://${GCS_BUCKET}/status/${variant}.built"
    fi
    local error_marker="gs://${GCS_BUCKET}/status/${variant}-error.fail"

    echo ""
    echo -e "  ${BOLD}Monitoring...${NC} ${DIM}(polling every 15s)${NC}"
    echo ""

    local poll=0 max_polls=160  # 160 × 15s = 40 min

    while [ $poll -lt $max_polls ]; do
        # Error?
        if gsutil -q stat "$error_marker" 2>/dev/null; then
            echo ""
            fail "FAILED — ${action} ${variant}"
            gsutil cat "$error_marker" 2>/dev/null | sed 's/^/    /'
            echo ""
            local log_file="gs://${GCS_BUCKET}/status/${variant}-build.log"
            if gsutil -q stat "$log_file" 2>/dev/null; then
                echo -e "  ${BOLD}Last 30 lines:${NC}"
                gsutil cat "$log_file" 2>/dev/null | tail -30 | sed 's/^/    /'
            fi
            return 1
        fi

        # Done?
        if gsutil -q stat "$done_marker" 2>/dev/null; then
            echo ""
            info "Done: ${action} ${variant}"
            gsutil cat "$done_marker" 2>/dev/null | sed 's/^/    /'
            return 0
        fi

        # Instance died?
        local ist
        ist=$(instance_status)
        if [ "$ist" = "NOT_FOUND" ] || [ "$ist" = "TERMINATED" ]; then
            # Check one more time for markers (race condition)
            sleep 5
            if gsutil -q stat "$done_marker" 2>/dev/null; then
                echo ""
                info "Done: ${action} ${variant}"
                gsutil cat "$done_marker" 2>/dev/null | sed 's/^/    /'
                return 0
            fi
            echo ""
            fail "Instance terminated without completion."
            local log_file="gs://${GCS_BUCKET}/status/${variant}-build.log"
            if gsutil -q stat "$log_file" 2>/dev/null; then
                echo -e "  ${BOLD}Build log:${NC}"
                gsutil cat "$log_file" 2>/dev/null | tail -30 | sed 's/^/    /'
            fi
            return 1
        fi

        printf "\r  [%3ds] waiting...  " "$(( poll * 15 ))"
        sleep 15
        poll=$((poll + 1))
    done

    echo ""
    warn "Timeout (40 min). Check: $0 status"
    return 1
}

# ─── Status ──────────────────────────────────────────────────

show_status() {
    echo ""
    echo -e "  ${BOLD}Build Status${NC}"
    echo ""

    for variant in "${ALL_VARIANTS[@]}"; do
        local line="  ${variant}:"
        if gsutil -q stat "gs://${GCS_BUCKET}/status/${variant}.pushed" 2>/dev/null; then
            line="${line} ${GREEN}pushed${NC} — $(gsutil cat "gs://${GCS_BUCKET}/status/${variant}.pushed" 2>/dev/null)"
        elif gsutil -q stat "gs://${GCS_BUCKET}/status/${variant}.built" 2>/dev/null; then
            line="${line} ${YELLOW}built${NC} — $(gsutil cat "gs://${GCS_BUCKET}/status/${variant}.built" 2>/dev/null)"
        elif gsutil -q stat "gs://${GCS_BUCKET}/status/${variant}-error.fail" 2>/dev/null; then
            line="${line} ${RED}failed${NC} — $(gsutil cat "gs://${GCS_BUCKET}/status/${variant}-error.fail" 2>/dev/null)"
        else
            line="${line} ${DIM}not started${NC}"
        fi
        echo -e "$line"
    done

    echo ""
    local ist
    ist=$(instance_status)
    if [ "$ist" != "NOT_FOUND" ]; then
        echo -e "  ${DIM}Instance: ${ist}${NC}"
    fi
    echo ""
}

# ─── Pull from Registry ─────────────────────────────────────

pull_models() {
    echo ""
    echo -e "  ${BOLD}Pulling from Ollama registry${NC}"
    echo ""

    # Find Ollama host
    local ollama_host="${OLLAMA_HOST:-}"
    if [ -z "$ollama_host" ]; then
        local win_ip
        win_ip=$(ip route show default 2>/dev/null | awk '{print $3}' || echo "")
        if [ -n "$win_ip" ] && curl -sf "http://${win_ip}:11434/" &>/dev/null; then
            ollama_host="http://${win_ip}:11434"
            info "Windows Ollama at ${ollama_host}"
        elif curl -sf "http://localhost:11434/" &>/dev/null; then
            ollama_host="http://localhost:11434"
        else
            fail "No Ollama server found."; exit 1
        fi
    fi
    export OLLAMA_HOST="$ollama_host"

    for variant in "${ALL_VARIANTS[@]}"; do
        local reg="${REGISTRY_NS}/${MODEL_PREFIX}-${variant}"
        echo -e "  ${DIM}Pulling ${reg}...${NC}"
        if OLLAMA_HOST="$ollama_host" ollama pull "$reg" 2>&1; then
            OLLAMA_HOST="$ollama_host" ollama cp "$reg" "${MODEL_PREFIX}-${variant}" 2>/dev/null || true
            info "${variant}: pulled."
        else
            fail "${variant}: pull failed."
        fi
    done
    echo ""
}

# ─── Usage ───────────────────────────────────────────────────

usage() {
    cat <<'EOF'
cloud-build-medgemma.sh — Build & push MedGemma variants via GCE

Usage:
  ./cloud-build-medgemma.sh <action> <variant>
  ./cloud-build-medgemma.sh <command>

Actions (one variant at a time):
  build <q4s|q4|q8|f16>        Quantize model, save blob to GCS
  build+push <q4s|q4|q8|f16>   Build + push to Ollama registry
  push <q4s|q4|q8|f16>         Push previously built blob to registry

Commands:
  status                   Show build/push status for all variants
  pull                     Pull all pushed models to local Ollama
  cleanup                  Delete orphaned GCE instance

Recommended workflow:
  1. ./cloud-build-medgemma.sh build q4       # validate one works
  2. ./cloud-build-medgemma.sh build q8
  3. ./cloud-build-medgemma.sh build f16
  4. ./cloud-build-medgemma.sh push q4        # validate push works
  5. ./cloud-build-medgemma.sh push q8
  6. ./cloud-build-medgemma.sh push f16
  7. ./cloud-build-medgemma.sh pull           # pull to local

Environment (.env):
  OLLAMA_REGISTRY_NS   Ollama username           (required)
  GCS_BUCKET           GCS bucket name           (required)
  HF_TOKEN             HuggingFace token         (build)
  OLLAMA_KEY_FILE      SSH key path              (push)
  GCE_MACHINE_TYPE     Default: e2-standard-8
  GCE_DISK_SIZE        Default: 50GB
  GCE_DISK_TYPE        Default: pd-ssd
  GCE_ZONE             Default: us-central1-a
  GCE_MAX_RUN          Default: 3600s
EOF
}

# ─── Main ────────────────────────────────────────────────────

main() {
    [ $# -ge 1 ] || { usage; exit 1; }

    local cmd="$1"
    local variant="${2:-}"

    load_env

    echo ""
    echo -e "  ${BOLD}Coheara Cloud Build${NC} — MedGemma 1.5 4B IT"
    echo -e "  ${DIM}Registry: ${REGISTRY_NS}/${MODEL_PREFIX}-*${NC}"
    echo ""

    case "$cmd" in
        status)
            validate_gcloud
            show_status
            ;;
        cleanup)
            validate_gcloud
            delete_instance
            ;;
        pull)
            pull_models
            ;;
        build|push|build+push)
            # Validate variant
            if [ -z "$variant" ] || [[ ! "$variant" =~ ^(q4s|q4|q8|f16)$ ]]; then
                fail "Usage: $0 ${cmd} <q4s|q4|q8|f16>"
                exit 1
            fi
            validate_gcloud
            ensure_bucket
            trap cleanup_trap EXIT ERR SIGINT SIGTERM
            run_on_gce "$cmd" "$variant"
            ;;
        -h|--help|help)
            usage
            ;;
        *)
            fail "Unknown: ${cmd}. Try: $0 help"
            exit 1
            ;;
    esac
}

main "$@"

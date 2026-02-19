#!/usr/bin/env bash
set -uo pipefail

# ── Coheara Security Audit ────────────────────────────────────────────────
# Scans Rust (cargo audit), frontend npm, and mobile npm dependencies
# for known vulnerabilities (CVEs). Writes results to AUDIT.txt.
#
# Usage:
#   ./audit.sh              # Full audit, output to AUDIT.txt
#   ./audit.sh --ci         # Exit 1 on critical/high (CI gate mode)
#   ./audit.sh --fix        # Attempt npm audit fix for resolvable issues
# ──────────────────────────────────────────────────────────────────────────

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$SCRIPT_DIR"
TAURI_DIR="$PROJECT_ROOT/src-tauri"
MOBILE_DIR="$PROJECT_ROOT/mobile"
AUDIT_FILE="$PROJECT_ROOT/AUDIT.txt"

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

CI_MODE=false
FIX_MODE=false

for arg in "$@"; do
    case "$arg" in
        --ci)  CI_MODE=true ;;
        --fix) FIX_MODE=true ;;
        --help|-h)
            echo "Usage: ./audit.sh [--ci] [--fix]"
            echo "  --ci   Exit 1 on critical/high vulnerabilities (for CI pipelines)"
            echo "  --fix  Attempt npm audit fix for auto-resolvable issues"
            exit 0
            ;;
        *) echo "Unknown argument: $arg"; exit 1 ;;
    esac
done

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

# ── Counters ──────────────────────────────────────────────────────────────
TOTAL_CRITICAL=0
TOTAL_HIGH=0
TOTAL_MODERATE=0
TOTAL_LOW=0

# ── Initialize AUDIT.txt ─────────────────────────────────────────────────
init_audit_file() {
    cat > "$AUDIT_FILE" <<EOF
================================================================================
COHEARA SECURITY AUDIT REPORT
================================================================================
Date:    $(date -u '+%Y-%m-%d %H:%M:%S UTC')
Host:    $(hostname)
Platform: $(uname -s) $(uname -m)
================================================================================

EOF
}

append_audit() {
    echo "$1" >> "$AUDIT_FILE"
}

append_section() {
    {
        echo ""
        echo "────────────────────────────────────────────────────────────────────────────────"
        echo "$1"
        echo "────────────────────────────────────────────────────────────────────────────────"
        echo ""
    } >> "$AUDIT_FILE"
}

# ── Rust Audit ────────────────────────────────────────────────────────────
audit_rust() {
    log_step "Auditing Rust dependencies (cargo audit)"
    append_section "RUST DEPENDENCIES (cargo audit)"

    if [[ -z "$CARGO" || ! -x "$CARGO" ]]; then
        log_warn "Cargo not found — skipping Rust audit"
        append_audit "SKIPPED: Cargo not found at $CARGO"
        return
    fi

    # Install cargo-audit if missing
    if ! command -v cargo-audit >/dev/null 2>&1; then
        log_info "Installing cargo-audit..."
        "$CARGO" install cargo-audit --quiet 2>&1
        if ! command -v cargo-audit >/dev/null 2>&1; then
            log_error "Failed to install cargo-audit"
            append_audit "SKIPPED: Failed to install cargo-audit"
            return
        fi
    fi

    local audit_output
    local audit_exit=0

    audit_output=$(cd "$TAURI_DIR" && cargo audit 2>&1) || audit_exit=$?

    append_audit "$audit_output"
    append_audit ""

    if [[ $audit_exit -eq 0 ]]; then
        log_ok "Rust: No known vulnerabilities found"
        append_audit "RESULT: CLEAN — No known vulnerabilities"
    else
        # Parse severity from cargo audit output
        local crit_count high_count
        crit_count=$(echo "$audit_output" | grep -ci "critical" || true)
        high_count=$(echo "$audit_output" | grep -ci "high" || true)
        local vuln_count
        vuln_count=$(echo "$audit_output" | grep -c "^RUSTSEC-" || true)

        TOTAL_CRITICAL=$((TOTAL_CRITICAL + crit_count))
        TOTAL_HIGH=$((TOTAL_HIGH + high_count))

        if [[ $crit_count -gt 0 ]]; then
            log_error "Rust: $vuln_count advisories found ($crit_count critical)"
        elif [[ $high_count -gt 0 ]]; then
            log_warn "Rust: $vuln_count advisories found ($high_count high)"
        else
            log_warn "Rust: $vuln_count advisories found"
        fi
        append_audit "RESULT: $vuln_count advisories (critical=$crit_count, high=$high_count)"
    fi
}

# ── npm Audit (generic) ──────────────────────────────────────────────────
audit_npm() {
    local label="$1"
    local dir="$2"

    log_step "Auditing $label npm dependencies"
    append_section "$label NPM DEPENDENCIES (npm audit)"

    if [[ ! -d "$dir" ]]; then
        log_warn "$label: Directory not found ($dir) — skipping"
        append_audit "SKIPPED: Directory not found ($dir)"
        return
    fi

    if [[ ! -f "$dir/package-lock.json" && ! -f "$dir/package.json" ]]; then
        log_warn "$label: No package.json found — skipping"
        append_audit "SKIPPED: No package.json found"
        return
    fi

    # Run npm audit in JSON mode for structured parsing
    local audit_json
    local audit_exit=0

    audit_json=$(cd "$dir" && npm audit --json 2>/dev/null) || audit_exit=$?

    # Extract severity counts from JSON
    local critical high moderate low
    critical=$(echo "$audit_json" | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    v = d.get('metadata', d).get('vulnerabilities', {})
    print(v.get('critical', 0))
except: print(0)
" 2>/dev/null || echo "0")
    high=$(echo "$audit_json" | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    v = d.get('metadata', d).get('vulnerabilities', {})
    print(v.get('high', 0))
except: print(0)
" 2>/dev/null || echo "0")
    moderate=$(echo "$audit_json" | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    v = d.get('metadata', d).get('vulnerabilities', {})
    print(v.get('moderate', 0))
except: print(0)
" 2>/dev/null || echo "0")
    low=$(echo "$audit_json" | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    v = d.get('metadata', d).get('vulnerabilities', {})
    print(v.get('low', 0))
except: print(0)
" 2>/dev/null || echo "0")

    # Write human-readable output to AUDIT.txt
    local audit_readable
    audit_readable=$(cd "$dir" && npm audit 2>&1) || true
    append_audit "$audit_readable"
    append_audit ""

    local total=$((critical + high + moderate + low))
    TOTAL_CRITICAL=$((TOTAL_CRITICAL + critical))
    TOTAL_HIGH=$((TOTAL_HIGH + high))
    TOTAL_MODERATE=$((TOTAL_MODERATE + moderate))
    TOTAL_LOW=$((TOTAL_LOW + low))

    if [[ $total -eq 0 ]]; then
        log_ok "$label: No known vulnerabilities found"
        append_audit "RESULT: CLEAN — No known vulnerabilities"
    else
        local severity_summary="critical=$critical, high=$high, moderate=$moderate, low=$low"
        if [[ $critical -gt 0 ]]; then
            log_error "$label: $total vulnerabilities ($severity_summary)"
        elif [[ $high -gt 0 ]]; then
            log_warn "$label: $total vulnerabilities ($severity_summary)"
        else
            log_info "$label: $total vulnerabilities ($severity_summary)"
        fi
        append_audit "RESULT: $total vulnerabilities ($severity_summary)"
    fi

    # Optional fix
    if [[ "$FIX_MODE" == true && $total -gt 0 ]]; then
        log_info "$label: Attempting npm audit fix..."
        append_audit ""
        append_audit "--- npm audit fix ---"
        local fix_output
        fix_output=$(cd "$dir" && npm audit fix 2>&1) || true
        append_audit "$fix_output"
        log_ok "$label: npm audit fix completed (re-run audit to verify)"
    fi
}

# ── Summary ───────────────────────────────────────────────────────────────
write_summary() {
    local total=$((TOTAL_CRITICAL + TOTAL_HIGH + TOTAL_MODERATE + TOTAL_LOW))

    append_section "SUMMARY"

    {
        echo "  Critical:  $TOTAL_CRITICAL"
        echo "  High:      $TOTAL_HIGH"
        echo "  Moderate:  $TOTAL_MODERATE"
        echo "  Low:       $TOTAL_LOW"
        echo "  ─────────────────"
        echo "  Total:     $total"
        echo ""
    } >> "$AUDIT_FILE"

    if [[ $total -eq 0 ]]; then
        append_audit "VERDICT: CLEAN — No known vulnerabilities across all components."
        log_ok "All components clean — no known vulnerabilities"
    elif [[ $TOTAL_CRITICAL -gt 0 || $TOTAL_HIGH -gt 0 ]]; then
        append_audit "VERDICT: FAIL — $TOTAL_CRITICAL critical + $TOTAL_HIGH high vulnerabilities require attention."
        log_error "FAIL: $TOTAL_CRITICAL critical + $TOTAL_HIGH high vulnerabilities found"
    else
        append_audit "VERDICT: WARN — $total vulnerabilities found (moderate/low only)."
        log_warn "WARN: $total vulnerabilities (moderate/low) — review recommended"
    fi

    append_audit ""
    append_audit "Report written: $AUDIT_FILE"

    echo ""
    log_step "Audit report written to: $AUDIT_FILE"
}

# ── Main ──────────────────────────────────────────────────────────────────
echo -e "${BOLD}Coheara Security Audit${NC}"
echo -e "Mode: ${BOLD}$(if $CI_MODE; then echo 'CI (strict)'; else echo 'Report'; fi)${NC}  Fix: ${BOLD}$FIX_MODE${NC}"
echo ""

init_audit_file
audit_rust
audit_npm "FRONTEND" "$PROJECT_ROOT"
audit_npm "MOBILE" "$MOBILE_DIR"
write_summary

# CI gate: exit 1 if critical or high vulnerabilities found
if [[ "$CI_MODE" == true && ($TOTAL_CRITICAL -gt 0 || $TOTAL_HIGH -gt 0) ]]; then
    log_error "CI gate: Failing due to critical/high vulnerabilities"
    exit 1
fi

exit 0

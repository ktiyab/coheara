#!/usr/bin/env bash
set -euo pipefail

# ── Diagnostic Sync ──────────────────────────────────────────────────────
#
# PURPOSE:
#   The Coheara document pipeline writes diagnostic dumps of every
#   intermediate artifact (rendered pages, preprocessed images, OCR prompts,
#   LLM responses, extraction results, structuring results) to inspect
#   what happens at each stage. In dev builds, these are auto-written to:
#
#       C:\Users\tkonlambigue\Coheara-dev\diagnostic\{document_id}\
#       (WSL2: /mnt/c/Users/tkonlambigue/Coheara-dev/diagnostic/)
#
#   This script copies the diagnostic folder to the project root for
#   convenience — all dumps grouped next to the code:
#
#       ./diagnostic/{document_id}/
#
# WHAT'S INSIDE A DIAGNOSTIC DUMP:
#   00-source-info.json              Format, category, file size, DPI
#   01-rendered-page-{N}.png         Raw PDF-to-PNG output (before preprocessing)
#   02-preprocessed-page-{N}.png     896x896 normalized image (sent to Ollama)
#   02-preprocessed-page-{N}.json    Dimensions, warnings, quality report
#   03-vision-ocr-prompt-page-{N}.txt  Exact system+user prompt sent to model
#   03-vision-ocr-result-page-{N}.json Raw response, confidence, errors
#   04-extraction-result.json        Full extraction (all pages, confidence)
#   05-structuring-input-page-{N}.txt  Text sent to structuring LLM
#   05-structuring-result-page-{N}.json Parsed entities, markdown, errors
#   06-final-result.json             Merged final result
#
# USAGE:
#   ./diagnostic_sync.sh           # One-shot sync
#   ./diagnostic_sync.sh --watch   # Watch mode: re-sync on changes
#   ./diagnostic_sync.sh --clean   # Empty both source and target folders
#
# ACTIVATION:
#   Diagnostic dumps are automatic in dev builds (./dev.sh).
#   In production, set COHEARA_DUMP_DIR=/path/to/folder to enable.
#
# ──────────────────────────────────────────────────────────────────────────

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SRC="/mnt/c/Users/tkonlambigue/Coheara-dev/diagnostic"
DST="$SCRIPT_DIR/diagnostic"

sync_once() {
    if [[ ! -d "$SRC" ]]; then
        echo "Source not found: $SRC"
        echo "No diagnostic dumps yet. Import a document with dev build first."
        exit 0
    fi

    local count
    count=$(find "$SRC" -mindepth 1 -maxdepth 1 -type d | wc -l)

    mkdir -p "$DST"
    rsync -a --delete "$SRC/" "$DST/"

    echo "Synced $count document(s): $SRC → $DST"
}

watch_loop() {
    echo "Watching $SRC for changes (Ctrl+C to stop)..."
    sync_once

    if command -v inotifywait >/dev/null 2>&1; then
        while inotifywait -r -q -e modify,create,delete "$SRC" >/dev/null 2>&1; do
            sync_once
        done
    else
        echo "inotifywait not found — falling back to 2s polling"
        while true; do
            sleep 2
            sync_once
        done
    fi
}

clean() {
    local cleaned=0

    if [[ -d "$SRC" ]]; then
        rm -rf "$SRC"/*
        echo "Cleaned: $SRC"
        cleaned=1
    fi

    if [[ -d "$DST" ]]; then
        rm -rf "$DST"/*
        echo "Cleaned: $DST"
        cleaned=1
    fi

    if [[ $cleaned -eq 0 ]]; then
        echo "Nothing to clean: no diagnostic folders found"
    fi
}

case "${1:-}" in
    --watch) watch_loop ;;
    --clean) clean ;;
    *)       sync_once ;;
esac

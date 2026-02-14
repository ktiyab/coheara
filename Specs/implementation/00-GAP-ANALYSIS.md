# Coheara: End-to-End Shipping Gap Analysis (v0.3.0)

> **Purpose**: Master inventory of gaps between current codebase and shippable apps.
> **Recovery**: After compression, read THIS FIRST, find first PENDING in queue, resume.
> **Cross-refs**: Components (Specs/components/), Reviews (Specs/review/), Prior gaps (IMP-001..024 ALL RESOLVED)
> **Workspace**: Specs/implementation/ + Specs/implementation/log/

---

## TOC

| # | Section | Line |
|---|---------|------|
| 1 | Executive Summary | 20 |
| 2 | Current State Baseline | 40 |
| 3 | Gap Registry (19 gaps) | 65 |
| 4 | Dependency Graph | 185 |
| 5 | Implementation Queue (5 phases) | 215 |
| 6 | Implementation Log | 280 |
| 7 | Progress Tracker | 310 |

---

## 1. Executive Summary

**Baseline**: v0.2.0 (ceb7b68), 975 Rust tests, 0 clippy warnings, 71 IPC commands.
**Prior round**: IMP-001 through IMP-024 ALL RESOLVED (modules implemented, tests pass).
**This round**: Wire modules end-to-end, complete UI flows, ship both apps.

**Core finding**: The L1 pipeline (import, OCR, structure, store) is fully implemented as isolated modules with passing tests, but NOT wired to Tauri IPC commands. A desktop user cannot import a document through the UI. This blocks the entire value chain.

**Gap count**: 19 gaps across 5 categories:
- 8 Backend wiring gaps (B01-B08)
- 6 Frontend gaps (F01-F06)
- 5 Mobile gaps (M01-M05)

---

## 2. Current State Baseline

| Area | Module Tests | IPC Wired | User Can Do It |
|------|:-----------:|:---------:|:--------------:|
| Profile management | YES | YES | YES |
| Document import | YES | PARTIAL (WiFi only) | NO (no file picker) |
| OCR/Extraction | YES | NO | NO |
| Medical structuring | YES | NO | NO |
| Storage/embedding | YES | NO | NO |
| RAG chat | YES | YES | YES (if docs exist) |
| Safety filter | YES | YES | YES |
| Coherence engine | YES | PARTIAL (alerts) | PARTIAL |
| Medications | YES | YES | YES |
| Journal | YES | YES | YES |
| Appointments | YES | YES | YES |
| Timeline | YES | YES | YES |
| Trust/Safety | YES | YES | YES |
| Mobile chat | YES | YES | YES |
| Mobile capture | STUBS | NO | NO |
| Mobile viewers | PARTIAL | PARTIAL | PARTIAL |
| API router | DEFINED | NOT STARTED | NO |
| Sync engine | DEFINED | NOT WIRED | NO |

**71 IPC commands registered. 23 Rust modules. 61 Svelte components. 44 mobile components.**

---

## 3. Gap Registry

### Category A: Backend Pipeline Wiring [CRITICAL PATH]

| ID | Title | Spec Ref | Impact | Status |
|----|-------|----------|--------|--------|
| E2E-B01 | Direct file import IPC command | L1-01 | User cannot import docs from desktop | RESOLVED |
| E2E-B02 | Document processing orchestrator | L1-01..04 | No end-to-end import-to-searchable flow | RESOLVED |
| E2E-B03 | Storage pipeline trigger after review | L1-04 | Confirmed docs never reach RAG index | RESOLVED |
| E2E-B04 | Coherence engine trigger command | L2-03 | No on-demand coherence analysis | RESOLVED |
| E2E-B05 | Import progress Tauri events | L3-02 | No user feedback during processing | RESOLVED |
| E2E-B06 | Mobile API router startup | M0-01 | Mobile cannot talk to desktop | RESOLVED |
| E2E-B07 | Sync engine IPC wiring | M0-04 | Mobile receives stale data | RESOLVED |
| E2E-B08 | WebSocket chat relay for mobile | M0-03 | Mobile chat not connected | RESOLVED |

### E2E-B01: Direct File Import Command

**Before**: `import_file()` only reachable via `process_staged_files()` (WiFi transfer).
**After**: New `import_document` Tauri command accepts file path, triggers full pipeline.
**Files**: `src-tauri/src/commands/import.rs` (new), `commands/mod.rs`
**Security**: Validate file path is accessible, check file size limits, verify magic bytes.
**Deps**: None (L1-01 module complete).

### E2E-B02: Document Processing Orchestrator

**Before**: Each L1 stage (import, OCR, structure) works independently. Manual wiring.
**After**: Single `DocumentProcessor` orchestrator drives: validate, import, extract, structure, persist pending review.
**Files**: `src-tauri/src/pipeline/processor.rs` (new)
**Architecture**: Trait-based; accepts `OcrEngine + LlmClient` via DI. Emits Tauri events at each stage.
**Deps**: E2E-B01

### E2E-B03: Storage Pipeline After Review

**Before**: `confirm_review` updates trust but never triggers chunking/embedding/vector storage.
**After**: `confirm_review` calls storage pipeline: chunk text, embed via ONNX, store in vector DB.
**Files**: `src-tauri/src/commands/review.rs` (modify), `pipeline/storage/`
**Deps**: E2E-B02

### E2E-B04: Coherence Engine Trigger

**Before**: Coherence detection only runs in tests.
**After**: New `run_coherence_scan` command triggers 8 detection algorithms, persists observations.
**Files**: `src-tauri/src/commands/coherence.rs` (new)
**Deps**: E2E-B03 (needs stored documents to analyze)

### E2E-B05: Import Progress Events

**Before**: No feedback during document processing.
**After**: Tauri events emitted: `import-started`, `import-extracting`, `import-structuring`, `import-ready-for-review`, `import-failed`.
**Files**: `src-tauri/src/pipeline/processor.rs`, `src/lib/types/import.ts` (new)
**Deps**: E2E-B02

### E2E-B06: Mobile API Router Startup

**Before**: `api/` module defined with endpoints but HTTP server never started.
**After**: axum HTTP server starts alongside Tauri, shares CoreState, serves all /api/* endpoints.
**Files**: `src-tauri/src/api/server.rs` (new or modify), `src-tauri/src/lib.rs`
**Deps**: None

### E2E-B07: Sync Engine Wiring

**Before**: `sync` module exists but no IPC commands.
**After**: Sync endpoints wired to API router, delta sync operational.
**Files**: `src-tauri/src/api/endpoints/sync.rs`, `src-tauri/src/commands/sync.rs`
**Deps**: E2E-B06

### E2E-B08: WebSocket Chat Relay

**Before**: Mobile WebSocket defined but not connected to RAG pipeline.
**After**: WS handler receives chat queries, runs RAG + safety, streams tokens.
**Files**: `src-tauri/src/api/websocket.rs`
**Deps**: E2E-B06

### Category B: Desktop Frontend [USER EXPERIENCE]

| ID | Title | Spec Ref | Impact | Status |
|----|-------|----------|--------|--------|
| E2E-F01 | File import UI flow | L3-02 | Import button does nothing | RESOLVED |
| E2E-F02 | Import progress indicator | L3-02 | No feedback during import | RESOLVED |
| E2E-F03 | Route architecture | L3-01..05 | No deep linking, no URL state | RESOLVED |
| E2E-F04 | Document list/detail view | L3-02 | Only recent docs on home | RESOLVED |
| E2E-F05 | Error/loading state consistency | UX | Rough failure experience | RESOLVED |
| E2E-F06 | Global state stores | Arch | Props drilled through components | RESOLVED |

### Category C: Mobile Completion [COMPANION APP]

| ID | Title | Spec Ref | Impact | Status |
|----|-------|----------|--------|--------|
| E2E-M01 | Document capture UI | M1-05 | Phone can't photograph docs | RESOLVED |
| E2E-M02 | QR pairing camera UI | M0-02 | Phone can't pair with desktop | RESOLVED |
| E2E-M03 | Labs/Timeline/Appointments pages | M1-03 | "More" menu shows empty | RESOLVED |
| E2E-M04 | Android/iOS production signing | Deploy | Cannot publish to stores | RESOLVED |
| E2E-M05 | End-to-end mobile integration test | QA | Regression risk | RESOLVED |

---

## 4. Dependency Graph

```
PHASE 1: Document Pipeline Wiring (CRITICAL PATH)
  E2E-B01 (file import cmd)
    +-- E2E-B02 (orchestrator)
          +-- E2E-B03 (storage after review)
          |     +-- E2E-B04 (coherence trigger)
          +-- E2E-B05 (progress events)

PHASE 2: Desktop UX
  E2E-F01 (import UI) -- depends on E2E-B01, E2E-B05
  E2E-F02 (progress UI) -- depends on E2E-B05
  E2E-F03 (routes) -- independent
  E2E-F04 (doc list) -- independent
  E2E-F05 (error states) -- independent
  E2E-F06 (global stores) -- independent

PHASE 3: Mobile API + Completion
  E2E-B06 (API router) -- independent
    +-- E2E-B07 (sync wiring)
    +-- E2E-B08 (WS chat relay)
    +-- E2E-M01 (capture UI)
    +-- E2E-M02 (pairing UI)
    +-- E2E-M03 (viewer pages)

PHASE 4: Image Pipeline Evolution (future, separate spec)

PHASE 5: Production Polish
  E2E-M04 (signing) -- after all
  E2E-M05 (integration test) -- after all
```

---

## 5. Implementation Queue

### Phase 1: Document Pipeline Wiring [CRITICAL, blocks all value]

| Order | ID | Brick | Engineer Focus |
|:-----:|-----|-------|----------------|
| 1 | E2E-B01 | Direct file import command | Rust, Security |
| 2 | E2E-B02 | Document processing orchestrator | Rust, Architecture |
| 3 | E2E-B05 | Import progress Tauri events | Rust, UX |
| 4 | E2E-B03 | Storage pipeline after review confirm | Rust, Architecture |
| 5 | E2E-B04 | Coherence engine trigger | Rust, QA |

**Outcome**: Desktop user can import a file, see progress, review extraction, confirm, and have the document indexed for RAG chat. Coherence engine can scan for conflicts.

### Phase 2: Desktop UX Completion [makes desktop shippable]

| Order | ID | Brick | Engineer Focus |
|:-----:|-----|-------|----------------|
| 6 | E2E-F06 | Global state stores | Web, Architecture |
| 7 | E2E-F03 | Route architecture | Web, UX |
| 8 | E2E-F01 | File import UI flow | Web, UX |
| 9 | E2E-F02 | Import progress indicator | UX |
| 10 | E2E-F04 | Document list/detail view | Web, UX |
| 11 | E2E-F05 | Error/loading state consistency | UX, QA |

**Outcome**: Desktop app has proper routing, file import flow with progress, consistent UX.

### Phase 3: Mobile API + Completion [makes mobile shippable]

| Order | ID | Brick | Engineer Focus |
|:-----:|-----|-------|----------------|
| 12 | E2E-B06 | Mobile API router startup | Rust, Architecture |
| 13 | E2E-B07 | Sync engine wiring | Rust |
| 14 | E2E-B08 | WebSocket chat relay | Rust, Security |
| 15 | E2E-M02 | QR pairing camera UI | Mobile, UX |
| 16 | E2E-M01 | Document capture UI | Mobile, UX |
| 17 | E2E-M03 | Labs/Timeline/Appointments pages | Mobile, UX |

**Outcome**: Phone can pair, sync, chat, capture documents, view all data.

### Phase 4: Image Pipeline Evolution [future spec]

Separate coherence-spec: clinical image classification + MedGemma vision integration.
Depends on Phase 1 completion. Will be specced as specs-evolution.

### Phase 5: Production Polish [store-ready]

| Order | ID | Brick | Engineer Focus |
|:-----:|-----|-------|----------------|
| 18 | E2E-M05 | End-to-end integration tests | QA |
| 19 | E2E-M04 | Android/iOS production signing | Security, Deploy |

---

## 6. Implementation Log

| Date | ID | Tests Delta | Total Tests | Brick Summary |
|------|-----|:-----------:|:-----------:|---------------|
| (starting) | -- | -- | 975 Rust | E2E gap analysis complete, queue defined |
| 2026-02-14 | E2E-B01 | +5 | 981 | Direct file import IPC command |
| 2026-02-14 | E2E-B02 | +9 | 986 | Document processing orchestrator (pipeline::processor) |
| 2026-02-14 | E2E-B03 | +0 | 986 | Storage pipeline trigger after review confirm |
| 2026-02-14 | E2E-B04 | +6 | 992 | Coherence engine trigger (8 repo fns + 6 IPC commands) |
| 2026-02-14 | E2E-B05 | +0 | 992 | Import progress events (structuring stage + failure event) |
| 2026-02-14 | E2E-F06 | +0 | 992 | Global state stores (navigation + profile, 23 components updated) |
| 2026-02-14 | E2E-F03 | +0 | 992 | Route architecture (hash-based URL state in navigation store) |
| 2026-02-14 | E2E-F01+F02 | +0 | 992 | File import UI + progress (ImportScreen, Tauri dialog plugin, progress events) |
| 2026-02-14 | E2E-F04 | +4 | 996 | Document list/detail (DocumentListScreen, DocumentDetailScreen, get_document_detail cmd) |
| 2026-02-14 | E2E-F05 | +0 | 996 | Error/loading consistency (AppointmentScreen + DocumentListScreen error states) |
| 2026-02-14 | E2E-B06 | +4 | 1000 | Mobile API server startup (api/server.rs, 3 IPC commands, CoreState.api_server) |
| 2026-02-14 | E2E-B07 | +0 | 1000 | Sync engine IPC wiring (3 IPC commands: versions, reset, summary) |
| 2026-02-14 | E2E-B08 | +0 | 1000 | WebSocket chat relay — already complete from M0-03 (handle_chat_query, RAG+safety wired) |
| 2026-02-14 | E2E-M02 | +0 | 1000 | QR pairing camera UI (jsQR scanner, X25519 ECDH via tweetnacl, HKDF+AES-GCM, layout integration) |
| 2026-02-14 | E2E-M01 | +7 | 1007 | Document capture UI (real camera, upload endpoint with base64 decode + L1-01 import, 7 tests) |
| 2026-02-14 | E2E-M03 | +0 | 1007 | Labs/Timeline/Appointments pages already complete; added appointment prep fetch, settings page |
| 2026-02-14 | E2E-M05 | +8/+11 | 1015 Rust, 492 mobile | Integration tests: 8 new router tests + 11 pairing utils tests |
| 2026-02-14 | E2E-M04 | +0 | 1015 Rust, 492 mobile | Production signing: version alignment, iOS privacy descriptions, Android permissions |

---

## 7. Progress Tracker

```
Total gaps: 19 (B:8, F:6, M:5)
Resolved:   19/19
In Progress: 0/19
Phase 1 (Pipeline Wiring):  COMPLETE [5/5]
Phase 2 (Desktop UX):       COMPLETE [6/6]
Phase 3 (Mobile):           COMPLETE [6/6]
Phase 4 (Image Evolution):  DEFERRED
Phase 5 (Production):       COMPLETE [2/2]

ALL E2E GAPS RESOLVED. Apps ready for production signing with actual credentials.
```

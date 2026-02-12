# Coheara — Component Index & Build Order

<!--
=============================================================================
QUICK NAVIGATION — Read TOC first, then jump to needed section
=============================================================================
-->

## Table of Contents

| Section | Lines | Offset |
|---------|-------|--------|
| [CI-01] Dependency Graph | 30-75 | `offset=25 limit=55` |
| [CI-02] Build Order (Strict) | 77-110 | `offset=72 limit=43` |
| [CI-03] Component Registry | 112-200 | `offset=107 limit=93` |
| [CI-04] Layer 0: Foundation | 202-230 | `offset=197 limit=33` |
| [CI-05] Layer 1: Pipeline | 232-265 | `offset=227 limit=38` |
| [CI-06] Layer 2: Intelligence | 267-295 | `offset=262 limit=33` |
| [CI-07] Layer 3: Interface | 297-335 | `offset=292 limit=43` |
| [CI-08] Layer 4: Value | 337-370 | `offset=332 limit=38` |
| [CI-09] Layer 5: Trust | 372-395 | `offset=367 limit=28` |
| [CI-10] Session Trigger Template | 397-440 | `offset=392 limit=48` |
| [CI-11] Status Dashboard | 442-480 | `offset=437 limit=43` |

---

## [CI-01] Dependency Graph

```
LAYER 0 — FOUNDATION (sequential, no parallelism)
═══════════════════════════════════════════════════

  L0-01 PROJECT SCAFFOLD
    │
    ▼
  L0-02 DATA MODEL
    │
    ▼
  L0-03 ENCRYPTION LAYER


LAYER 1 — PIPELINE (sequential pipeline, starts after L0-03)
═══════════════════════════════════════════════════════════════

  L0-03 ──► L1-01 DOCUMENT IMPORT
                │
                ▼
             L1-02 OCR & EXTRACTION
                │
                ▼
             L1-03 MEDICAL STRUCTURING (MedGemma)
                │
                ▼
             L1-04 STORAGE PIPELINE (chunk → embed → store)


LAYER 2 — INTELLIGENCE (partially parallel, after L1-04)
════════════════════════════════════════════════════════════

  L1-04 ──┬──► L2-01 RAG PIPELINE
           │      │
           │      ▼
           │   L2-02 SAFETY FILTER
           │
           └──► L2-03 COHERENCE ENGINE (parallel with L2-01)


LAYER 3 — INTERFACE (mostly parallel, after L2)
════════════════════════════════════════════════════

  L0-03 ──────► L3-01 PROFILE MANAGEMENT
  L1-04 ──────► L3-02 HOME & DOCUMENT FEED
  L2-02 ──────► L3-03 CHAT INTERFACE
  L1-03 ──────► L3-04 REVIEW SCREEN
  L1-04 ──────► L3-05 MEDICATION LIST

  L3-01 through L3-05 are PARALLEL (independent UI components)


LAYER 4 — VALUE (parallel, after L3 core exists)
══════════════════════════════════════════════════

  L3 ──┬──► L4-01 SYMPTOM JOURNAL
       ├──► L4-02 APPOINTMENT PREP
       ├──► L4-03 WIFI TRANSFER
       └──► L4-04 TIMELINE VIEW

  All L4 components are PARALLEL


LAYER 5 — TRUST (cross-cutting, after L2 + L3)
══════════════════════════════════════════════════

  L2-03 + L3 ──► L5-01 TRUST & SAFETY (emergency, plausibility, backup)
```

---

## [CI-02] Build Order (Strict Sequence)

**Phase A — Bootstrap (Layers 0)**
```
Session 1:  L0-01 Project Scaffold
Session 2:  L0-02 Data Model
Session 3:  L0-03 Encryption Layer
GATE: Project compiles, DB initializes, encryption round-trips
```

**Phase B — Data Flow (Layer 1)**
```
Session 4:  L1-01 Document Import
Session 5:  L1-02 OCR & Extraction
Session 6:  L1-03 Medical Structuring
Session 7:  L1-04 Storage Pipeline
GATE: Load a document photo → OCR → structure → store in both DBs
```

**Phase C — Brain (Layer 2)**
```
Session 8:  L2-01 RAG Pipeline
Session 9:  L2-02 Safety Filter
Session 10: L2-03 Coherence Engine
GATE: Ask a question → get grounded, safe, cited answer
```

**Phase D — Surface (Layer 3)**
```
Session 11: L3-01 Profile Management
Session 12: L3-02 Home & Document Feed
Session 13: L3-03 Chat Interface
Session 14: L3-04 Review Screen
Session 15: L3-05 Medication List
GATE: Marie's 5-minute walkthrough works end-to-end
```

**Phase E — Depth (Layer 4)**
```
Session 16: L4-01 Symptom Journal
Session 17: L4-02 Appointment Prep
Session 18: L4-03 WiFi Transfer
Session 19: L4-04 Timeline View
GATE: Full alpha feature set functional
```

**Phase F — Hardening (Layer 5)**
```
Session 20: L5-01 Trust & Safety
GATE: Emergency protocol, backup, plausibility all tested
```

**Sessions 21+: Integration testing, installer packaging, polish**

---

## [CI-03] Component Registry

### Layer 0 — Foundation

| ID | Component | Spec File | Dependencies | Complexity | Status |
|----|-----------|-----------|-------------|------------|--------|
| L0-01 | Project Scaffold | `L0-01-PROJECT-SCAFFOLD.md` | None | Medium | PENDING |
| L0-02 | Data Model | `L0-02-DATA-MODEL.md` | L0-01 | High | PENDING |
| L0-03 | Encryption Layer | `L0-03-ENCRYPTION-LAYER.md` | L0-01, L0-02 | High | PENDING |

### Layer 1 — Pipeline

| ID | Component | Spec File | Dependencies | Complexity | Status |
|----|-----------|-----------|-------------|------------|--------|
| L1-01 | Document Import | `L1-01-DOCUMENT-IMPORT.md` | L0-03 | Medium | PENDING |
| L1-02 | OCR & Extraction | `L1-02-OCR-EXTRACTION.md` | L1-01 | High | PENDING |
| L1-03 | Medical Structuring | `L1-03-MEDICAL-STRUCTURING.md` | L1-02 | High | PENDING |
| L1-04 | Storage Pipeline | `L1-04-STORAGE-PIPELINE.md` | L1-03 | High | PENDING |

### Layer 2 — Intelligence

| ID | Component | Spec File | Dependencies | Complexity | Status |
|----|-----------|-----------|-------------|------------|--------|
| L2-01 | RAG Pipeline | `L2-01-RAG-PIPELINE.md` | L1-04 | Very High | PENDING |
| L2-02 | Safety Filter | `L2-02-SAFETY-FILTER.md` | L2-01 | High | PENDING |
| L2-03 | Coherence Engine | `L2-03-COHERENCE-ENGINE.md` | L1-04 | Very High | PENDING |

### Layer 3 — Interface

| ID | Component | Spec File | Dependencies | Complexity | Status |
|----|-----------|-----------|-------------|------------|--------|
| L3-01 | Profile Management | `L3-01-PROFILE-MANAGEMENT.md` | L0-03 | Medium | PENDING |
| L3-02 | Home & Document Feed | `L3-02-HOME-DOCUMENT-FEED.md` | L1-04 | Medium | PENDING |
| L3-03 | Chat Interface | `L3-03-CHAT-INTERFACE.md` | L2-02 | High | PENDING |
| L3-04 | Review Screen | `L3-04-REVIEW-SCREEN.md` | L1-03 | Medium | PENDING |
| L3-05 | Medication List | `L3-05-MEDICATION-LIST.md` | L1-04 | Medium | PENDING |

### Layer 4 — Value

| ID | Component | Spec File | Dependencies | Complexity | Status |
|----|-----------|-----------|-------------|------------|--------|
| L4-01 | Symptom Journal | `L4-01-SYMPTOM-JOURNAL.md` | L3 core | Medium | PENDING |
| L4-02 | Appointment Prep | `L4-02-APPOINTMENT-PREP.md` | L3 core | High | PENDING |
| L4-03 | WiFi Transfer | `L4-03-WIFI-TRANSFER.md` | L3 core | Medium | PENDING |
| L4-04 | Timeline View | `L4-04-TIMELINE-VIEW.md` | L3 core | High | PENDING |

### Layer 5 — Trust

| ID | Component | Spec File | Dependencies | Complexity | Status |
|----|-----------|-----------|-------------|------------|--------|
| L5-01 | Trust & Safety | `L5-01-TRUST-SAFETY.md` | L2 + L3 | High | PENDING |

---

## [CI-04] Layer 0 Detail — Foundation

**L0-01 Project Scaffold** — Creates the entire Tauri 2.x + Svelte 5 + Rust project structure. After this session: `cargo tauri dev` works, shows a blank Svelte page.

**L0-02 Data Model** — Implements ALL SQLite tables (18 tables from tech spec v1.1) + LanceDB vector table setup. Rust structs for every entity. CRUD traits. Migration system. After this session: database initializes, all tables exist, can insert/query test data.

**L0-03 Encryption Layer** — Per-profile AES-256-GCM encryption. PBKDF2 key derivation. Encrypted SQLite (via sqlcipher or application-level encryption). Secure memory (zeroize). Recovery phrase generation. After this session: can create encrypted profile, write/read encrypted data, derive keys from passwords, generate/verify recovery phrases.

**GATE TEST:** Run `cargo test` → all foundation tests pass. Create a profile, insert test data, encrypt, close, reopen with password, verify data intact.

---

## [CI-05] Layer 1 Detail — Pipeline

**L1-01 Document Import** — File picker (native Tauri dialog), drag-and-drop handler, format detection (PDF, image, text), file staging area, perceptual hash for duplicate detection. After this session: user can select/drag files, app detects format, checks for duplicates.

**L1-02 OCR & Extraction** — Tesseract integration for images, PDF text extraction for digital PDFs, scanned PDF detection (image-based → OCR path), confidence scoring per extraction. After this session: any image or PDF → raw text with confidence score.

**L1-03 Medical Structuring** — Ollama/MedGemma integration. Structuring prompt. Raw text → structured Markdown. Entity extraction (medications, labs, diagnoses, professionals, dates). After this session: raw text → clean .md + extracted entities as JSON.

**L1-04 Storage Pipeline** — Markdown chunking strategy. Embedding generation (MiniLM). LanceDB vector writes. SQLite structured writes (medications, labs, diagnoses, etc.). Bidirectional linking (document_id on every entity). After this session: structured .md → chunks in LanceDB + entities in SQLite, queryable from both layers.

**GATE TEST:** Load a real prescription photo → OCR → MedGemma structure → stored in both DBs → query medication list from SQLite → query "metformin" from LanceDB → both return correct results.

---

## [CI-06] Layer 2 Detail — Intelligence

**L2-01 RAG Pipeline** — Query classification (factual/exploratory/symptom/timeline). Parallel retrieval (LanceDB semantic + SQLite structured). Context assembly with token budgeting. MedGemma generation via Ollama (streaming). Source citation injection. Conversation memory model.

**L2-02 Safety Filter** — 3-layer system: structured prompt enforcement (boundary check), regex keyword scan (diagnostic/prescriptive/alarm), reporting vs stating distinction. Input sanitization (prompt injection defense). Post-generation validation.

**L2-03 Coherence Engine** — CONFLICT detection (medication mismatches). DUPLICATE detection (alias table). GAP detection (diagnosis without treatment). DRIFT detection (unexplained changes). TEMPORAL correlation (symptom near medication change). ALLERGY cross-check. DOSE plausibility. CRITICAL value protocol. Alert lifecycle (store → surface → dismiss).

**GATE TEST:** Load 3 documents with a deliberate conflict → coherence engine detects it. Ask "why am I taking metformin?" → RAG returns grounded, cited, safe response. Inject diagnostic language → safety filter catches it.

---

## [CI-07] Layer 3 Detail — Interface

**L3-01 Profile Management** — Profile picker (first screen). Create/switch/lock profiles. Password entry + PBKDF2 derivation. Caregiver attribution (managed_by field). Profile list (names visible, data encrypted).

**L3-02 Home & Document Feed** — Recent documents grid. Quick action buttons (load document, ask question, journal). Onboarding milestones. Empty states. Coherence observation summary.

**L3-03 Chat Interface** — Message list with streaming responses. Source citation chips (tappable → shows source document). Confidence indicator. "Was this helpful?" feedback. Conversation persistence. Patient context preamble injection.

**L3-04 Review Screen** — Side-by-side: original image/PDF + extracted Markdown. Key field highlighting (medications, doses, dates, names). Confidence visual flags (< 0.70). Correction interface. Confirm/reject. Dose plausibility warnings.

**L3-05 Medication List** — Current medications (active, structured). Medication history (timeline per drug). OTC medication manual entry form. Brand/generic display. Dose change history. Filter by prescriber.

**GATE TEST:** Marie's 5-minute walkthrough runs end-to-end: open app → create profile → load document (drag) → review screen (confirm) → chat "why am I taking this?" → get grounded answer → see medication in list.

---

## [CI-08] Layer 4 Detail — Value

**L4-01 Symptom Journal** — OLDCARTS-adapted guided recording. Category → specific → severity (face scale) → date. Expanded: body map (SVG), duration, character, aggravating/relieving, timing. Check-in nudges. Post-medication-change proactive prompt. Symptom history view.

**L4-02 Appointment Prep** — Professional selector. Date picker. Dual artifact generation: patient copy (plain language questions) + professional copy (structured summary). PDF export. Post-appointment note capture. Trust calibration metric.

**L4-03 WiFi Transfer** — Local HTTP server (Rust). QR code generation with URL. Mandatory 6-digit PIN. Mobile upload page (responsive HTML). File type validation. Auto-timeout. Session lifecycle.

**L4-04 Timeline View** — SVG timeline component. Color-coded event types. Zoom (day/week/month/year). Filter by type/professional. "Since last visit" mode. Correlation lines (symptom ↔ medication change). Tap-to-detail cards.

**GATE TEST:** Record a symptom → see it on timeline. Prepare for appointment → get PDF. Transfer photo from phone via QR → appears in app. Timeline shows medication changes and symptoms with correlation lines.

---

## [CI-09] Layer 5 Detail — Trust

**L5-01 Trust & Safety** — Emergency protocol (critical lab values). Dose plausibility checking with reference DB. Backup/restore (encrypted .coheara-backup). Cryptographic erasure. Privacy verification screen (data location, airplane mode test documentation). Error recovery flows per catalog.

**GATE TEST:** Load a lab result with critical value → emergency banner appears. Backup to USB → restore on fresh install → all data intact. Delete profile → verify cryptographic erasure.

---

## [CI-10] Session Trigger Template

```markdown
# SESSION: [Component ID] — [Component Name]
# Date: [YYYY-MM-DD]
# Phase: [A/B/C/D/E/F]

## 1. ORIENT
Read: Specs/components/00-DEV-CONTINUITY.md (Process State table)
Read: Specs/components/00-COMPONENT-INDEX.md (CI-11 Status Dashboard)

## 2. RECALL
Read: Specs/components/[LX-XX-COMPONENT].md (FULL — this is your build spec)
Read: Specs/components/00-ENGINEERING-CONVENTIONS.md (relevant sections)
Read: Specs/brainstorm/05-TECHNICAL-SPEC.md (Section [N] — master spec context)

## 3. BUILD
Implement per component spec sections 3-6 (Interfaces, Data, Logic, Error Handling)
Follow engineering conventions strictly
Run tests after each logical unit

## 4. TEST
Execute acceptance criteria from component spec Section 8
Run: cargo test (unit + integration)
Run: cargo clippy (lint)
Verify security requirements from component spec Section 7

## 5. CAPTURE
Update component spec if reality diverged from plan (Section 10 → resolved)
Update 00-DEV-CONTINUITY.md:
  - Session log entry
  - Decision log (if implementation choices were made)
  - Discovery log (if spec assumptions were wrong)
Update 00-COMPONENT-INDEX.md status: PENDING → COMPLETE

## 6. VERIFY
Does the code match the component spec intent?
Does it follow engineering conventions?
Does it integrate with previously built components?
Are all tests passing?
```

---

## [CI-11] Status Dashboard

| Component | Status | Session | Date | Notes |
|-----------|--------|---------|------|-------|
| L0-01 Project Scaffold | PENDING | - | - | - |
| L0-02 Data Model | PENDING | - | - | - |
| L0-03 Encryption Layer | PENDING | - | - | - |
| L1-01 Document Import | PENDING | - | - | - |
| L1-02 OCR & Extraction | PENDING | - | - | - |
| L1-03 Medical Structuring | PENDING | - | - | - |
| L1-04 Storage Pipeline | PENDING | - | - | - |
| L2-01 RAG Pipeline | PENDING | - | - | - |
| L2-02 Safety Filter | PENDING | - | - | - |
| L2-03 Coherence Engine | PENDING | - | - | - |
| L3-01 Profile Management | PENDING | - | - | - |
| L3-02 Home & Doc Feed | PENDING | - | - | - |
| L3-03 Chat Interface | PENDING | - | - | - |
| L3-04 Review Screen | PENDING | - | - | - |
| L3-05 Medication List | PENDING | - | - | - |
| L4-01 Symptom Journal | PENDING | - | - | - |
| L4-02 Appointment Prep | PENDING | - | - | - |
| L4-03 WiFi Transfer | PENDING | - | - | - |
| L4-04 Timeline View | PENDING | - | - | - |
| L5-01 Trust & Safety | PENDING | - | - | - |
| **GATES** | | | | |
| Gate A: Foundation | PENDING | - | - | Project compiles, DB init, encryption works |
| Gate B: Pipeline | PENDING | - | - | Photo → OCR → structure → stored |
| Gate C: Brain | PENDING | - | - | Question → grounded safe answer |
| Gate D: Surface | PENDING | - | - | Marie's 5-min walkthrough |
| Gate E: Depth | PENDING | - | - | Full alpha features |
| Gate F: Hardened | PENDING | - | - | Trust & safety verified |

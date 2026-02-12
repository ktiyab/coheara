# Coheara — Development Continuity Plan

<!--
=============================================================================
COMPRESSION-PROOF ANCHOR FOR DEVELOPMENT PHASE
Read this file FIRST after any context compression during coding.
=============================================================================
-->

## Table of Contents

| Section | Lines | Offset |
|---------|-------|--------|
| [DC-01] Recovery Protocol | 20-45 | `offset=15 limit=30` |
| [DC-02] Process State | 47-75 | `offset=42 limit=33` |
| [DC-03] What's Built | 77-110 | `offset=72 limit=38` |
| [DC-04] What's Next | 112-135 | `offset=107 limit=28` |
| [DC-05] Session Log | 137-200 | `offset=132 limit=68` |
| [DC-06] Decision Log | 202-250 | `offset=197 limit=53` |
| [DC-07] Discovery Log | 252-300 | `offset=247 limit=53` |
| [DC-08] Spec Drift Register | 302-330 | `offset=297 limit=33` |

---

## [DC-01] Recovery Protocol — After Context Compression

**You lost context. You know nothing. Follow this sequence:**

```
1. Read THIS FILE (00-DEV-CONTINUITY.md)
   → Sections DC-02 through DC-04 tell you where we are

2. Read 00-COMPONENT-INDEX.md Section [CI-11]
   → Status Dashboard tells you what's built and what's pending

3. Identify the CURRENT TASK:
   → Check DC-04 "What's Next"
   → Read that component's spec file (Specs/components/LX-XX-*.md)

4. Read 00-ENGINEERING-CONVENTIONS.md
   → Sections relevant to current component

5. If coding is in progress, check:
   → git status (what files are modified)
   → cargo check (does it compile)
   → cargo test (what passes, what fails)

6. Resume from where you are.
```

**NEVER start coding without this recovery. You WILL contradict prior work.**

---

## [DC-02] Process State

| Phase | Status | Gate Status |
|-------|--------|------------|
| Phase A: Foundation (L0) | NOT STARTED | Gate A: PENDING |
| Phase B: Pipeline (L1) | NOT STARTED | Gate B: PENDING |
| Phase C: Brain (L2) | NOT STARTED | Gate C: PENDING |
| Phase D: Surface (L3) | NOT STARTED | Gate D: PENDING |
| Phase E: Depth (L4) | NOT STARTED | Gate E: PENDING |
| Phase F: Hardening (L5) | NOT STARTED | Gate F: PENDING |

**Current phase:** Phase A — Foundation
**Current component:** L0-01 Project Scaffold
**Blocking issues:** None

---

## [DC-03] What's Built (Cumulative)

| Component | Built | Tests Pass | Integrated | Notes |
|-----------|-------|-----------|------------|-------|
| (nothing yet — development not started) | | | | |

**Last successful build:** N/A
**Last test run:** N/A
**Last gate passed:** N/A

---

## [DC-04] What's Next

**Immediate next session:** L0-01 Project Scaffold
**Read before starting:** `Specs/components/L0-01-PROJECT-SCAFFOLD.md`
**Dependencies satisfied:** Yes (no dependencies)
**Expected outcome:** `cargo tauri dev` launches app, shows blank Svelte page

---

## [DC-05] Session Log

Each coding session records its outcome here. Append-only. Most recent at top.

```
(No sessions yet — development not started)

FORMAT for each entry:
---
### Session [N]: [Component ID] — [Date]
**Objective:** [What was attempted]
**Outcome:** [COMPLETE / PARTIAL / BLOCKED]
**Code changes:** [Files created/modified]
**Tests:** [N passing, M failing]
**Discoveries:** [Anything that contradicted the spec]
**Decisions:** [Implementation choices made]
**Next:** [What the next session should do]
---
```

---

## [DC-06] Decision Log

Implementation decisions made during coding that aren't in the spec. These are the decisions a future session needs to know about.

| # | Decision | Rationale | Component | Date |
|---|---------|-----------|-----------|------|
| (none yet) | | | | |

**Format:**
```
D-001: "Used rusqlite bundled feature instead of system libsqlite3"
  Rationale: Ensures consistent SQLite version across platforms.
  Bundled adds ~2MB to binary. Acceptable tradeoff.
  Component: L0-02
```

---

## [DC-07] Discovery Log

Things we learned during coding that the spec didn't anticipate. These feed back into spec updates.

| # | Discovery | Impact | Spec Section Affected | Resolved |
|---|----------|--------|----------------------|----------|
| (none yet) | | | | |

**Format:**
```
DISC-001: "MedGemma Q4 quantization doesn't support multimodal input"
  Impact: Can't use image-to-text on 8GB machines with quantized model.
  Spec affected: Tech Spec Section 3 (Technology Stack), Section 12 (Hardware Detection)
  Resolution: [PENDING / RESOLVED: description]
```

---

## [DC-08] Spec Drift Register

Tracks intentional deviations between spec and implementation. Prevents future sessions from "fixing" intentional differences.

| # | Spec Says | Code Does | Why | Approved |
|---|----------|----------|-----|----------|
| (none yet) | | | | |

**Format:**
```
DRIFT-001: Spec says "AES-256-GCM" for SQLite encryption
  Code uses: SQLCipher (which uses AES-256-CBC internally)
  Why: SQLCipher is battle-tested, handles page-level encryption
       automatically. Application-level GCM would require encrypting
       every query result manually.
  Approved: Yes (E-SC reviewed)
```

---

## Reference Links

| Document | Purpose | When to Read |
|----------|---------|-------------|
| `00-COMPONENT-INDEX.md` | Build order, dependency graph, status | Every session start |
| `00-ENGINEERING-CONVENTIONS.md` | Coding standards | Every session start |
| `05-TECHNICAL-SPEC.md` | Master specification | When component spec references it |
| `00-CONTINUITY-PLAN.md` | Brainstorm-phase continuity | If you need product context |
| `LX-XX-*.md` | Individual component specs | When building that component |

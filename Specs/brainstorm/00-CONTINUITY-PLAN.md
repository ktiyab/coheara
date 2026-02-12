# Coheara Technical Spec — Continuity Plan

## Purpose of This Document

This is the **compression-proof anchor** for the Coheara technical specification brainstorming process. Any AI session that loses context must read this file FIRST to rebuild the full picture.

**Read order after compression:**
1. THIS FILE (00-CONTINUITY-PLAN.md) — where we are, what's decided
2. 02-COHEARA-FULL-DEFINITION.md — the product definition (input to this process)
3. 03-PERSONAS.md — who is brainstorming
4. 04-BRAINSTORM-LOG.md — the collaborative brainstorming turns
5. 05-TECHNICAL-SPEC.md — the converging specification (output)

---

## What We're Building

**Coheara** — a patient's personal MedAI. Local, offline, installable desktop app. Non-technical users load medical documents (photos, PDFs, scans), and Coheara helps them understand their care through grounded conversation.

**The hard constraint:** A person who has never opened a terminal must be able to install this app, load a document, and get value within 5 minutes.

---

## Process State

| Phase | Status | Document |
|-------|--------|----------|
| Product definition | COMPLETE | `02-COHEARA-FULL-DEFINITION.md` |
| Persona definition | COMPLETE | `03-PERSONAS.md` |
| Collaborative brainstorm (Rounds 1-4) | COMPLETE | `04-BRAINSTORM-LOG.md` |
| Technical spec draft v1.0 | COMPLETE | `05-TECHNICAL-SPEC.md` |
| Stress testing rounds (Rounds 5-6) | COMPLETE | `04-BRAINSTORM-LOG.md` |
| Technical spec v1.1 (iterated) | COMPLETE | `05-TECHNICAL-SPEC.md` (22 sections) |
| Final user validation | PENDING | Awaiting user review of v1.1 |

---

## Personas (Summary)

### Patient-Side Personas
| ID | Name | Represents | Key Constraint |
|----|------|-----------|----------------|
| P-PAT-01 | Marie, 72 | Elderly non-technical patient | Zero tech literacy, poor eyesight, multiple chronic conditions |
| P-PAT-02 | Karim, 34 | Tech-savvy patient | Wants control, API access, data export — but recognizes he's not the primary user |
| P-PAT-03 | Sophie, 45 | Caregiver (manages parent's care) | Manages SOMEONE ELSE's documents, multiple patients possible |
| P-PAT-04 | David, 58 | Privacy-anxious patient | Won't use anything cloud-based, wants to verify data never leaves device |

### Health Professional Personas
| ID | Name | Represents | Key Constraint |
|----|------|-----------|----------------|
| P-PRO-01 | Dr. Chen | General Practitioner | Sees 30+ patients/day, 7min/visit, already drowning in documentation |
| P-PRO-02 | Nurse Adama | Hospital Nurse | Shift-based, handoff-focused, observes what doctors miss |
| P-PRO-03 | Pharmacist Dubois | Community Pharmacist | Last checkpoint before patient takes medication, sees ALL prescriptions |
| P-PRO-04 | Dr. Moreau | Cardiologist (Specialist) | Sees patient 2x/year, needs full picture fast, documents don't talk to each other |

### Builder Personas
| ID | Name | Represents | Key Constraint |
|----|------|-----------|----------------|
| P-BLD-01 | Lena | UX/Product Lead | Ruthless simplicity, if grandma can't use it, it's wrong |
| P-BLD-02 | Marcus | Senior Engineer | Technical grounding, what's actually buildable locally today |

---

## Decided Constraints (Locked)

These come from `02-COHEARA-FULL-DEFINITION.md` and are non-negotiable:

1. **Local only** — no cloud, no external API calls, no data leaves the device
2. **No clinical advice** — understanding, awareness, preparation only
3. **Zero professional workflow change** — works with documents that already exist
4. **MedGemma 1.5 4B** via Ollama as the local SLM
5. **Semantic database** for meaning by projection (embeddings)
6. **Markdown** as universal intermediate format
7. **Patient-centered** — the patient is the primary user

---

## Open Decisions (To Be Resolved by Brainstorm)

| # | Decision Needed | Status |
|---|----------------|--------|
| OD-01 | Application packaging format (Electron, Tauri, etc.) | OPEN |
| OD-02 | Semantic database choice (ChromaDB, LanceDB, FAISS, Qdrant) | OPEN |
| OD-03 | OCR engine for document ingestion | OPEN |
| OD-04 | Embedding model (MedGemma or separate) | OPEN |
| OD-05 | Installer technology (one-click, bundled Ollama?) | OPEN |
| OD-06 | UI framework | OPEN |
| OD-07 | How Ollama gets installed transparently | OPEN |
| OD-08 | Minimum hardware requirements | OPEN |
| OD-09 | Document ingestion UX (drag-drop, camera, file picker) | OPEN |
| OD-10 | Conversation UX (chat, structured Q&A, guided) | OPEN |
| OD-11 | Multi-language support | OPEN |
| OD-12 | Data backup/recovery model | OPEN |
| OD-13 | Patient feelings/symptoms recording UX | OPEN |
| OD-14 | Summary export for doctor visits | OPEN |
| OD-15 | Caregiver multi-patient support | OPEN |

---

## Key Insight Log

Insights that emerged during brainstorming that must not be lost:

| # | Insight | Source | Round |
|---|---------|--------|-------|
| I-01 | The app needs dual-layer data: semantic (LanceDB) + structured (SQLite). Embeddings alone can't answer "list all medications." Structured alone can't answer "why am I taking this?" | Karim, Pharmacist Dubois, Dr. Moreau | R3 |
| I-02 | Local WiFi transfer via QR code solves the hardest UX problem (phone photo → desktop) without violating privacy. Approved by David with conditions. | Sophie (proposed), David (validated) | R2 |
| I-03 | Post-ingestion review screen is non-negotiable. OCR errors on medication names can be medically dangerous. Patient confirms before data enters constellation. | Marie, Sophie, Dr. Chen | R3 |
| I-04 | Professional dismissal handling: when patient says "doctor addressed this," Coheara records it and stops re-alerting. Prevents AI-vs-doctor dynamic. | Dr. Chen | R1 |
| I-05 | Calm design language is a DESIGN CONSTRAINT, not a preference. All professionals independently demanded it. Red alerts and alarm language erode both professional trust and patient wellbeing. | All professionals | R1 |
| I-06 | Appointment summary needs two artifacts: patient copy (plain language questions) and professional copy (structured, sourced, one-page). | Sophie, Dr. Chen, Dr. Moreau | R2-R3 |
| I-07 | 8GB RAM machines can be supported via Q4 quantized model with honest quality messaging. Auto-detection at first launch. | Karim (proposed), Marcus (validated) | R2 |
| I-08 | Medication name aliasing (brand→generic) is essential for duplicate therapy detection. Phase 1: curated JSON for top 500 meds. Phase 2: full DB. | Pharmacist Dubois, Marcus | R2-R3 |
| I-09 | Guided symptom recording (not blank text box) with severity tracking creates clinically useful temporal data that correlates with medication changes. | Nurse Adama, Lena | R2-R3 |
| I-10 | Tauri over Electron: ~10MB vs ~150MB shell. Rust backend gives performance headroom for running alongside MedGemma. | Marcus, Karim | R1 |
| I-11 | Allergies table is CRITICAL safety data missing from v1.0. Allergy-to-compound-medication cross-checking (Augmentin → penicillin family) must be Phase 1. | Karim, Pharmacist Dubois | R5 |
| I-12 | Multi-profile must be Phase 1, not Phase 2. Sophie (caregiver) IS the primary use case. Profile picker is the first screen. | Sophie | R5 |
| I-13 | Safety filter must be 3 layers: structured prompt enforcement + regex scan + reporting/stating distinction. Single-layer proved insufficient by Dr. Chen's false-positive/negative analysis. | Dr. Chen | R5 |
| I-14 | Emergency protocol for critical lab values: calm but NOT dismissive. "Promptly" not "urgently." Cannot be suppressed by normal alert dismissal. | Dr. Chen | R5 |
| I-15 | Prompt injection defense is a real attack vector (malicious text in OCR'd documents). Input sanitization + document sandboxing in LLM prompt mandatory. | David | R5 |
| I-16 | Split installer solves the 5GB download problem: 150MB launcher gives instant app, model downloads in background. Full offline bundle as alternative. | Joint patients/Marcus | R6 |
| I-17 | Symptom recording adapted from clinical OLDCARTS framework but made patient-friendly. Progressive: severity always, details on demand. Body map for location. | Nurse Adama | R5 |
| I-18 | Compound medications (Augmentin = amoxicillin + clavulanic acid) need ingredient-level mapping for allergy cross-checking. New compound_ingredients table. | Pharmacist Dubois | R5 |
| I-19 | Password recovery impossible with proper encryption. Solution: 12-word recovery phrase generated at setup, written down physically by patient. | Marie, David, Marcus | R5 |
| I-20 | Dose plausibility checking catches OCR errors that patients miss (5000mg vs 500mg). Cross-check against bundled reference ranges. Informational, not blocking. | Marie, Dr. Chen, Marcus | R5 |

---

## Open Decisions (Updated)

| # | Decision | Status | Resolution |
|---|---------|--------|------------|
| OD-01 | MVP target OS | PROPOSED → Windows first | |
| OD-02 | Embedding model | PROPOSED → MiniLM, benchmark medical variant | |
| OD-03 | Frontend framework | PROPOSED → Svelte 5 | |
| OD-04 | Medication alias DB source | PROPOSED → Manual curation top 500 | |
| OD-05 | PDF generation | PROPOSED → Rust-native | |
| OD-06 | Accessibility target | PROPOSED → WCAG AA Phase 1, AAA Phase 3 | |
| OD-07 | Update mechanism | PROPOSED → Manual re-download Phase 1 | |
| OD-08 | Conversation history | PROPOSED → Keep all, user can clear | |
| OD-09 | Context window budget | RESOLVED | 500 system + 500 preamble + 3000 RAG + 2500 history + 1500 generation |
| OD-10 | Multi-profile timing | RESOLVED → Phase 1 | Sophie's caregiver case is primary |
| OD-11 | Safety filter depth | RESOLVED → 3 layers | Dr. Chen's analysis proved necessity |
| OD-12 | Password recovery | RESOLVED → 12-word mnemonic | Only viable approach with proper encryption |

---

## How to Resume After Compression

```
1. Read 00-CONTINUITY-PLAN.md        → This file. Rebuild full picture.
2. Check "Process State" table        → Know where we stopped.
3. Check "Open Decisions" table       → Know what's unresolved.
4. Read the latest section of 04-BRAINSTORM-LOG.md → Pick up the conversation.
5. Continue from where we left off.
```

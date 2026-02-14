# Coheara: Comprehensive Overview

**Version:** 1.0
**Date:** 2026-02-12
**Status:** Reference document, grounded in specifications and implemented code

---

## Table of Contents

| # | Section | Description |
|---|---------|-------------|
| 1 | [What Is Coheara](#1-what-is-coheara) | Identity, positioning, one-paragraph definition |
| 2 | [The Problem](#2-the-problem) | Healthcare information fragmentation, quantified |
| 3 | [The Collaborative Triangle](#3-the-collaborative-triangle) | Patient + Professional + AI relationship model |
| 4 | [Who Uses Coheara](#4-who-uses-coheara) | Detailed personas: patients, caregivers, professionals, builders |
| 5 | [Design Principles](#5-design-principles) | 10 non-negotiable constraints and UX mandates |
| 6 | [What Coheara Does](#6-what-coheara-does) | Complete feature inventory with implementation details |
| 7 | [What Coheara Never Does](#7-what-coheara-never-does) | The hard boundary: comprehension vs clinical authority |
| 8 | [Architecture](#8-architecture) | System topology, layer diagram, data flow |
| 9 | [AI System](#9-ai-system) | MedGemma, RAG pipeline, safety filter, coherence engine |
| 10 | [Security Model](#10-security-model) | Encryption, key lifecycle, device pairing, threat model |
| 11 | [Data Model](#11-data-model) | 18 tables, 16 enums, entity relationships |
| 12 | [Mobile Companion](#12-mobile-companion) | Phone app: sync, capabilities, native features, PWA |
| 13 | [App Distribution Server](#13-app-distribution-server) | Direct install over WiFi, no app store |
| 14 | [Implementation Status](#14-implementation-status) | Component completion, test counts, what's built |
| 15 | [Engineering Conventions](#15-engineering-conventions) | Rust, Svelte, patterns, dependency policy |

---

## 1. What Is Coheara

Coheara is a patient's personal MedAI: a local, offline, encrypted desktop application that ingests the medical documents a patient already receives from their health professionals, builds structured and semantic understanding from them, and helps the patient understand their care through grounded conversation, inconsistency detection, and appointment preparation.

The desktop application is the brain. It runs AI inference (MedGemma 1.5 4B via Ollama), OCR (Tesseract), semantic search (all-MiniLM-L6-v2 via ONNX), and encrypted storage (AES-256-GCM over SQLite) entirely on the patient's machine. A smartphone companion (Capacitor or PWA) puts the patient's structured health data in their pocket, synchronized over local WiFi. No cloud. No accounts. No internet required after installation.

Coheara has **comprehension authority**, not clinical authority. It helps patients understand their documents, spot inconsistencies across providers, and prepare the right questions for their next appointment. It never diagnoses, prescribes, or gives medical advice.

---

## 2. The Problem

Healthcare suffers from a single architectural failure: information moves forward but meaning does not come back. This manifests across every level of the system:

| Symptom | Scale |
|---------|-------|
| Physician-hours lost to documentation per year | 125 million |
| Serious adverse events involving handoff miscommunication | 80% |
| Medical coding error rates | 22% |
| Clinical alert override rates | 90-96% |
| Prior authorization friction (annual cost) | $35 billion |
| Adults below proficient health literacy | 88% |
| Physician burnout rate | 62.8% |

Each is a symptom of an open-loop system where no coherence observation exists between what was meant and what was understood.

The patient sits at the center of this failure. They receive documents from multiple professionals who do not see each other's work. Prescriptions from three specialists may conflict. Lab results may contradict a diagnosis. A dosage change may happen without explanation. No single person, not even the patient's primary doctor, sees the complete picture.

Coheara closes the loop at the patient level, where meaning matters most. It works with documents that already exist, loaded by the patient, requiring zero professional workflow change.

---

## 3. The Collaborative Triangle

Coheara does not position the patient as a passive recipient of AI-generated information. It enables a collaborative relationship between three participants:

```
                 Health Professional
                  clinical judgment
                 ▲                 ▲
                /                   \
        better /                     \ better
     encounter/                       \encounter
              /                         \
         You ◄───────────────────────────► Coheara
    your questions            comprehension + preparation
              \                         /
               \       personal        /
                └──────► Vault ◄──────┘
```

**The professional** brings clinical judgment, years of training, and the authority to diagnose and treat.

**The patient** brings lived experience, continuity across all providers, and the ability to ask the right questions when properly informed.

**Coheara** brings comprehension: it reads the documents, structures them, detects inconsistencies, and prepares the patient with grounded questions.

The result: the quality of the clinical encounter improves for both sides. The patient arrives understanding their documents, aware of potential concerns, and prepared with clear questions. The professional gets a patient who can engage meaningfully, saving time and improving care.

---

## 4. Who Uses Coheara

### 4.1 Patient Profiles

#### Marie, 72, Retired Teacher (low-tech patient)

**Situation:** Hypertension, type 2 diabetes, early-stage arthritis. Takes 7 medications from 3 different doctors. Forgets medication rationale by the time she gets home.

**Friction:**
- Health literacy: low-moderate (understands "blood pressure is high" but not ACE inhibitor mechanisms)
- Tech literacy: minimal (can take photos, send email, cannot navigate file systems)
- Afraid of "breaking" things on her computer
- Needs to prepare questions for her doctor but does not know what to ask

**What Coheara gives her:** Take a photo of a prescription and ask "what does this mean?" in her own words. Large text, simple language, one primary action per screen. Progressive disclosure: simple by default, detail hidden unless requested.

**Design constraints from Marie:** 5-minute time-to-value, no jargon anywhere in the UI, camera/photo as primary input, guided symptom recording (not a blank text box), print-friendly outputs.

#### Karim, 34, Software Developer (tech-savvy patient)

**Situation:** Recently diagnosed with Crohn's disease. Technically sophisticated. Already maintains spreadsheets to track symptoms, medications, and lab results. Reads PubMed papers.

**Friction:**
- Consumer health apps lack depth and data export
- Already caught 2 prescribing errors his doctors missed
- Wants timeline visualization of condition progression
- Cares about data portability

**What Coheara gives him:** Full medication timeline, lab value tracking with trends (CRP, calprotectin), search across all documents, data export. Detail available when he wants it (not forced as default).

**Design constraints from Karim:** Actual data visibility (not just summaries), export capabilities, power user features accessible on demand.

#### David, 58, Retired Military, Privacy Advocate

**Situation:** Prostate cancer survivor. Experienced a data breach in 2019 involving his medical records. Deeply distrustful of cloud services. Will not use any health app that connects to the internet.

**Friction:**
- Refuses cloud-based health tools
- Wants to verify that zero network calls occur
- Needs to see exactly where data is stored and what the AI "knows"
- Backup must be local (USB, external HDD), never cloud

**What Coheara gives him:** Complete local privacy. Single-file installer. Everything works offline. Per-profile encryption. Cryptographic erasure supported. 24-word recovery phrase for forgotten passwords. Privacy verification showing concrete architecture facts.

**Design constraints from David:** Zero telemetry, no analytics, no "phone home," verifiable offline operation, data location transparency.

### 4.2 Caregiver Profile

#### Sophie, 45, Caregiver (managing her mother's care)

**Situation:** Works full-time while managing medical care for her 78-year-old mother who has dementia, heart failure, and osteoporosis. Coordinates between 4 specialists, a GP, home nurse visits, and a pharmacy. Her mother cannot manage her own documents.

**Friction:**
- Mother had an ER visit caused by conflicting prescriptions from two doctors who did not know about each other
- Cannot track which doctor changed what medication and when
- Time-poor: every interaction with the system must be efficient
- Needs printable appointment summaries
- May eventually need to manage her own health in a separate profile

**What Coheara gives her:** Multi-profile support (one for her mother, potentially one for herself). Medication conflict detection across doctors. Printable appointment summaries. Visible professional identity and timing on every piece of data. Phone companion for access during appointments with her mother.

**Design constraints from Sophie:** Support for managing another person's documents, per-profile isolation, efficient workflows, mobile access during appointments.

### 4.3 Professional Beneficiaries

Coheara requires zero workflow change from health professionals. They continue producing the documents they already produce. The benefit flows back to them through better-prepared patients.

#### Dr. Chen, General Practitioner

**Situation:** 15 years in practice, sees 30-35 patients per day. Average consultation: 7 minutes. Spends 2+ hours per day on documentation after clinic hours.

**How Coheara helps her:** A patient who arrives with an organized list of questions sorted by topic saves time. A patient with AI-generated anxiety about every minor inconsistency costs time. Coheara's calm design language (no red alerts, no alarm wording) and preparatory framing ("ask your doctor about this") prevents panic while enabling productive conversation.

**Her constraints on design:** Summaries must be concise (1 page max). Must clearly distinguish "your documents say" from "this might mean." Never frame observations as diagnoses. Indicate which documents generated each observation (traceability).

#### Nurse Adama, Hospital Nurse

**Situation:** 8 years in a hospital ward, works 12-hour shifts. Handles shift handoffs where information routinely gets lost. Often first to notice changes in patient condition.

**How Coheara helps her:** If a patient can say "my app noticed blood pressure trending up since the medication change," that is clinically useful. Patient self-reported symptoms between visits, captured and correlated with medication changes, provide a picture that no professional document alone contains.

**Her constraints on design:** Temporal correlation is critical (symptom X after event Y). Patient self-reports must be clearly labeled "patient-reported" vs "clinically documented."

#### Pharmacist Dubois, Community Pharmacist

**Situation:** 12 years in community pharmacy. The last checkpoint before the patient takes a medication. Sees ALL prescriptions from ALL doctors: the only professional with a complete medication picture.

**How Coheara helps him:** If a patient shows a complete, organized medication list from Coheara, that is already useful for cross-checking. Generic vs brand name confusion (a real patient safety issue) is addressed by Coheara's medication alias system.

**His constraints on design:** Correctly parse medication names, doses, frequencies, routes. Brand/generic name mapping required. Detect duplicate therapies (same drug, different names, different prescribers). Do NOT cause patient panic about interactions: framing is critical. Handle compound medication mapping (e.g., Augmentin = amoxicillin + clavulanate).

#### Dr. Moreau, Cardiologist

**Situation:** Sees patients 1-4 times per year. Must rapidly understand what happened since the last visit across ALL other care the patient received.

**How Coheara helps him:** A timeline of medications, symptoms, and labs since the last specialist visit, organized with source document references, is worth 10 minutes of chart review he does not have. Referral letters tell him what the GP wants him to know; the full document set tells him what he actually needs to know.

**His constraints on design:** Summary export must include source document references. Timeline view of events between specialist visits is critical. "Since last visit" focused view. Clearly separate what different professionals said.

### 4.4 Builder Personas

#### Lena, UX/Product Lead

**Golden rule:** "If my grandmother can't figure it out in 2 minutes, the design is wrong."

**Design mandates:**
1. 5-minute time-to-value: install, load first document, get first useful response
2. No jargon anywhere in UI (no "embeddings," "semantic," "OCR," "vector database")
3. Camera/photo first: Marie's primary input method
4. Guided symptom recording (not a blank text box)
5. Print-friendly outputs
6. Calm design language: no red alerts, no exclamation marks, gentle blues and warm neutrals, "something to discuss" not "WARNING"

**Principles:** Progressive disclosure (simple by default, detail on demand). One primary action per screen. Errors must be recoverable and non-frightening. Accessibility is non-negotiable (contrast, font size, screen reader, keyboard navigation).

#### Marcus, Senior Engineer

**Reality check:** Stress-tests every feature request against: Does this run on a 2020 laptop with 8GB RAM? Single installer? Works offline?

**Principles:** Local-first means the entire stack works without internet after install. MedGemma 1.5 4B (~4-8GB) sets the minimum RAM floor. Every dependency is a risk. Data format must be future-proof. Cross-platform matters but MVP targets one OS first.

---

## 5. Design Principles

### 5.1 Non-Negotiable Constraints

These are inherited from the product definition and confirmed by all personas. They govern every technical decision.

| # | Constraint | Source |
|---|-----------|--------|
| NC-01 | All processing is local. No data leaves the device. No network calls after installation. | Product def, David |
| NC-02 | No clinical advice. Output is understanding, awareness, preparation only. | Product def, all professionals |
| NC-03 | Zero professional workflow change. Works with documents that already exist. | Product def |
| NC-04 | Single-file installer. No internet required after download. No accounts. No telemetry. | David, Marie |
| NC-05 | 5-minute time-to-value: install, load first document, receive first useful response. | Lena, Marie |
| NC-06 | All observations trace to source documents. No ungrounded statements. | Dr. Chen, Dr. Moreau |
| NC-07 | Calm design language. No alarm wording. No red alerts. Preparatory framing. | All professionals |
| NC-08 | Patient-reported data always distinguished from professionally-documented data. | Nurse Adama, all professionals |
| NC-09 | Per-profile encryption (AES-256-GCM). Cryptographic erasure supported. | David, Sophie |
| NC-10 | Progressive disclosure: simple by default, detail on demand. | Lena, Marie, Karim |

### 5.2 UX Mandates

| Principle | Implementation |
|-----------|---------------|
| Camera-first input | Photo capture as primary document input, not file picker |
| No jargon | UI never shows "embeddings," "vectors," "OCR," "tokens," "inference" |
| One action per screen | Each screen has a single primary affordance |
| Recoverable errors | Errors are non-frightening and always offer a next step |
| Print-friendly | Appointment summaries, medication lists export to clean PDF |
| Calm palette | Gentle blues, warm neutrals. No red. No exclamation marks. |
| Accessibility | WCAG contrast ratios, keyboard navigation, screen reader support |
| Progressive disclosure | Simple view by default. Detail available on request. Never forced. |

---

## 6. What Coheara Does

### 6.1 Document Import

**What it handles:** Prescriptions, lab reports, clinical notes, discharge summaries, radiology reports, pharmacy records, and other medical documents in PDF, JPEG, PNG, TIFF, and WebP formats.

**How it works:**
- Magic byte validation (not file extension) to detect true format
- SHA-256 content hashing for deduplication (same document not imported twice)
- Perceptual hashing (pHash) for near-duplicate image detection
- Encrypted staging area: documents are encrypted before touching disk
- Original files preserved in `originals/` directory within the profile

**Implementation:** `pipeline::import` module. 30 tests.

### 6.2 OCR and Text Extraction

**For scanned/photo documents:**
- Tesseract 5 OCR engine (bundled, runs locally)
- Image preprocessing: deskew, contrast normalization, noise reduction
- Confidence scoring per page

**For digital PDFs:**
- Direct text extraction via pdf-extract (no OCR needed)
- Falls back to OCR for scanned PDF pages (hybrid documents handled)

**For all documents:**
- MedGemma structures messy OCR output into clean, labeled Markdown
- Output: one `.md` file per document with structured sections

**Implementation:** `pipeline::ocr` and `pipeline::structuring` modules.

### 6.3 Medical Structuring (AI)

MedGemma 1.5 4B (via Ollama) converts raw text into structured medical data:

| Extracted Entity | Fields |
|-----------------|--------|
| Medications | Name, dose, frequency, route, status, prescriber, start/end dates, instructions |
| Lab results | Test name, value, unit, reference range, abnormal flag, date |
| Diagnoses | Name, ICD code (when present), status (active/resolved/monitoring), diagnosing professional |
| Professionals | Name, specialty, institution, contact information |
| Allergies | Allergen, reaction, severity, source (document-extracted vs patient-reported) |
| Procedures | Name, date, outcome, performing professional |

The structuring prompt constrains MedGemma to extract only what is present in the document, never infer or fabricate. Output is validated JSON parsed into typed Rust structs.

**Implementation:** `pipeline::structuring` module.

### 6.4 Storage Pipeline

Documents go through a dual storage path:

**Semantic storage (vector search):**
- Markdown chunked into segments (by section, paragraph, or semantic boundary)
- Each chunk embedded via all-MiniLM-L6-v2 (384-dimensional vectors, ONNX Runtime)
- Stored in SQLite-backed vector store with cosine similarity search
- Metadata preserved: source document, date, professional, document type

**Structured storage (relational):**
- Extracted entities (medications, labs, diagnoses, etc.) stored in typed SQLite tables
- Foreign key relationships: medication belongs to document and prescribing professional
- Full-text search across structured fields

**Implementation:** `pipeline::storage` module (chunking, embedding, vector store, entity store).

### 6.5 RAG Chat (Conversational AI)

The patient asks questions in their own words. Coheara answers grounded in their documents.

**Pipeline (13 stages):**

| Stage | What Happens |
|-------|-------------|
| 1. Classify query | Determine query type: Medication, Symptom, Lab, Timeline, General |
| 2. Retrieval strategy | Select search parameters based on query type (k, filters) |
| 3. Retrieve context | Semantic search (vector similarity) + structured data (SQLite) |
| 4. Check coverage | Verify sufficient context exists to answer |
| 5. Assemble context | Combine retrieved chunks within token budget |
| 6. Load history | Get prior conversation messages for continuity |
| 7. Build prompt | Construct full prompt with context, history, and system instructions |
| 8. Generate | MedGemma produces a response via Ollama |
| 9. Parse boundary | Extract boundary check: Understanding, Awareness, Preparation, or OutOfBounds |
| 10. Extract citations | Identify source document references in the response |
| 11. Validate citations | Verify cited documents exist in the database |
| 12. Calculate confidence | Score based on boundary check, citation count, and chunk coverage |
| 13. Build response | Assemble final RagResponse with text, citations, confidence, and boundary check |

**Key properties:**
- Every answer cites specific source documents
- Responses that fail the boundary check (OutOfBounds) are blocked
- Confidence score (0.0-1.0) reflects grounding quality
- Conversation history maintained across sessions

**Implementation:** `pipeline::rag` module. 48 tests.

### 6.6 Safety Filter (3 Layers)

Every AI response passes through three sequential safety layers before reaching the patient:

| Layer | Name | Type | What It Catches |
|-------|------|------|----------------|
| 1 | BoundaryCheck | Blocking | Response falls outside Understanding/Awareness/Preparation. Blocked immediately, no rephrase. |
| 2 | KeywordScan | Rephrasable | Diagnostic language ("you have X"), prescriptive language ("you should take Y"), alarm language ("WARNING," "DANGER"). Up to 3 violations can be rephrased. |
| 3 | GroundingCheck | Rephrasable | Ungrounded claims: statements without document attribution ("you have diabetes" instead of "your lab report from Dr. Chen indicates..."). Up to 3 violations can be rephrased. |

**Outcomes:**
- **Passed**: no violations detected
- **Rephrased**: violations found and successfully rewritten in safe framing
- **Blocked**: violations unresolvable (too many, or non-rephrasable type)

**Input sanitization:** Before any query reaches the LLM, patient input is sanitized to remove prompt injection attempts, excessive length, and non-medical content.

**Implementation:** `pipeline::safety` module. 51 tests.

### 6.7 Coherence Engine (8 Detection Algorithms)

The coherence engine continuously monitors the patient's medical data for inconsistencies, risks, and gaps. It runs on every new document import and can be triggered for a full analysis.

| # | Algorithm | What It Detects |
|---|-----------|----------------|
| 1 | CONFLICT | Same medication, different dose/frequency/route from different prescribers |
| 2 | DUPLICATE | Same medication under different brand names (generic equivalents) |
| 3 | GAP | Missing required medications or unmatched conditions |
| 4 | DRIFT | Dose values drifting beyond safe ranges over time |
| 5 | TEMPORAL | Medications prescribed within suspicious temporal correlation window (14 days) |
| 6 | ALLERGY | Medication prescribed despite documented allergy or cross-sensitivity |
| 7 | DOSE | Absolute dose violations and daily accumulation exceeding safe limits |
| 8 | CRITICAL | Lab values outside critical ranges (e.g., potassium < 3.0 or > 6.0) |

**Alert lifecycle:**
- Alerts are generated with severity levels: Info, Standard, Critical
- Each alert includes: patient-facing message, affected entity IDs, source document IDs, detection timestamp
- Alerts can be surfaced (shown to patient) or remain pending
- Patients can dismiss alerts with a reason (or mark as professional-reviewed)
- Dismissed alerts are tracked for audit

**Emergency protocol:** Critical alerts (life-threatening lab values, severe allergy conflicts) trigger an elevated workflow with immediate surfacing and cannot be silently dismissed.

**Architecture pattern:** Detection functions are pure: they take a `RepositorySnapshot` (pre-fetched data) instead of live database connections. This keeps all 8 algorithms testable without database setup.

**Implementation:** `intelligence` module (detection.rs, engine.rs, types.rs, helpers.rs, messages.rs, reference.rs, store.rs, emergency.rs). 55 tests.

### 6.8 Medication Tracking

**Current medications:** Name, dose, frequency, route, prescriber, start date, status (active/stopped/paused).

**Historical tracking:** Dose changes recorded as audit trail with date, old value, new value, and reason when available.

**Compound medications:** Multi-ingredient medications (e.g., Augmentin = amoxicillin + clavulanate) tracked with individual ingredients for allergy cross-checking.

**Tapering schedules:** Step-by-step dose reduction plans with dates and target doses.

**Medication aliases:** Bundled database of brand-to-generic name mappings (top 500+ medications). User can add custom aliases.

### 6.9 Symptom Journal

**OLDCARTS-guided recording:** Structured symptom entry following the clinical framework:
- **O**nset: when it started
- **L**ocation: where on the body
- **D**uration: how long it lasts
- **C**haracter: what it feels like
- **A**ggravating factors: what makes it worse
- **R**elieving factors: what makes it better
- **T**emporal pattern: when it occurs
- **S**everity: how bad (scale)

**Temporal correlation:** Symptoms are automatically correlated with medication changes, lab results, and appointments by date proximity. "I started feeling dizzy" correlated with "dose increased 3 days ago" is surfaced as awareness.

**Source labeling:** All symptom entries are permanently marked as "patient-reported" to distinguish from clinically documented observations (NC-08).

### 6.10 Appointment Preparation

**Auto-generated summaries:** Before an appointment, Coheara assembles:
- Current medication list with recent changes
- Open coherence alerts (conflicts, gaps, questions)
- Recent symptoms correlated with medications or labs
- Questions the patient may want to ask, grounded in document analysis

**PDF export:** Summaries generate as clean, print-friendly PDFs the patient can bring to the appointment. Concise (1 page target, per Dr. Chen's constraint). Source document references included for traceability.

### 6.11 Timeline

**Interactive SVG timeline** spanning all health events: document imports, medication changes, lab results, diagnoses, symptoms, appointments. Visual display of the patient's medical history across time with event type color coding and zoom/scroll navigation.

### 6.12 Profile Management

**Multi-profile support:** Each profile is a fully isolated, independently encrypted container. Sophie can manage her mother's health in one profile and her own in another.

**Profile lifecycle:**
- Create: name, password, generates 24-word BIP39 recovery phrase
- Unlock: password derives encryption key via PBKDF2
- Lock: key material zeroed from memory
- Delete: cryptographic erasure (key destroyed, data unrecoverable)

### 6.13 Backup and Recovery

**Encrypted backup:** Full profile exported as a single `.coheara-backup` file. Encrypted with the profile's key. Includes database, originals, markdown, and metadata.

**Recovery phrase:** 24-word BIP39 mnemonic generated at profile creation. Allows password reset if forgotten. The mnemonic is shown once and never stored digitally.

**Cryptographic erasure:** Deleting a profile destroys the encryption key. Without the key (or recovery phrase), the data is unrecoverable by design.

---

## 7. What Coheara Never Does

This boundary is non-negotiable. It governs every design decision, every prompt, every safety filter rule.

| Coheara DOES | Coheara DOES NOT |
|-------------|-----------------|
| Explain what a document says, in the patient's language | Diagnose conditions |
| Surface inconsistencies across documents from different professionals | Prescribe treatments |
| Flag what the patient should ask about at their next appointment | Give clinical advice |
| Help the patient understand their medications, labs, and care plan | Override or contradict professional judgment |
| Detect patterns across time that the patient can bring to their doctor | Make treatment recommendations |
| Prepare the patient to be an informed participant in their care | Replace any clinical encounter |

**Coheara's output is always:**
- **Understanding**: "Here's what your documents say, in your words"
- **Awareness**: "Here's something that looks inconsistent across your documents"
- **Preparation**: "Here are questions you should bring to your next appointment"

**Coheara's output is never:**
- Diagnosis: "You have X condition"
- Prescription: "You should take Y"
- Clinical judgment: "Your doctor is wrong about Z"
- Treatment advice: "Stop taking this medication"

When Coheara detects an inconsistency (two prescriptions that may interact, a lab result that does not align with a diagnosis) it does not tell the patient what to do. It says: **"Ask your doctor about this at your next appointment."**

The professional has **clinical authority**. Coheara has **comprehension authority**. They do not overlap.

---

## 8. Architecture

### 8.1 System Topology

```
┌──────────────────────────┐         WiFi         ┌──────────────────────────┐
│   Desktop (Tauri 2.10)   │◄═══════════════════►  │   Phone (PWA/Capacitor)  │
│                          │   encrypted sync      │                          │
│  Import documents        │                       │  View medications        │
│  OCR + AI structuring    │   REST + WebSocket    │  Check lab results       │
│  RAG chat (MedGemma)     │   X25519 key exchange │  Read alerts             │
│  Coherence detection     │   Token rotation      │  Log symptoms            │
│  Encrypted SQLite store  │                       │  Prepare for appointments│
│  Vector search (ONNX)    │   ┌──────────────┐    │  Capture documents       │
│  Distribution server ────│──►│ Install page  │    │                          │
└──────────────────────────┘   │ QR code scan  │    └──────────────────────────┘
     Everything computed       │ APK / PWA     │       Reads cached data
     and stored here           └──────────────┘        Works offline too
```

### 8.2 Desktop Stack

| Layer | Technology | Purpose |
|-------|-----------|---------|
| App shell | Tauri 2.10 (Rust + WebView) | ~10MB shell (vs Electron 150MB), cross-platform, native performance |
| Frontend | Svelte 5, SvelteKit 2, TailwindCSS 4 | Lightweight reactive framework, fast compilation, small bundle |
| Backend | Rust 1.80+ | Memory safety, performance, zero-cost abstractions |
| Database | SQLite (rusqlite, WAL mode) | Zero-config embedded, stores structured medical data |
| Vectors | SQLite-backed cosine similarity | Semantic search over document embeddings |
| Encryption | AES-256-GCM (aes-gcm crate) | Per-profile encryption at application level |
| AI inference | MedGemma 1.5 4B via Ollama | Local model serving, medical domain fine-tuned |
| Embeddings | all-MiniLM-L6-v2 via ONNX Runtime | 384-dim vectors, fast inference, good retrieval quality |
| OCR | Tesseract 5 (bundled) | Open-source, handles standard printed text |
| Phone API | axum REST + WebSocket | Local WiFi server for phone sync |
| Distribution | HTTP server (tower-http) | Serves companion app install over WiFi |

### 8.3 Data Flow

```
Document In (photo, PDF, scan)
      │
      ▼
┌─────────────────┐
│  Import          │  Magic byte validation, SHA-256 hash, encrypted staging
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  OCR / Extract   │  Tesseract for images, pdf-extract for PDFs
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  Structure       │  MedGemma converts raw text to clean labeled Markdown
└────────┬────────┘
         │
         ├──────────────────────┐
         ▼                      ▼
┌─────────────────┐    ┌─────────────────┐
│  Chunk + Embed   │    │  Entity Extract  │
│  MiniLM vectors  │    │  Meds, labs, dx  │
│  → Vector Store  │    │  → SQLite tables │
└─────────────────┘    └─────────────────┘
         │                      │
         └──────────┬───────────┘
                    ▼
         ┌─────────────────┐
         │  Coherence Check │  8 detection algorithms run on new data
         └─────────────────┘
```

### 8.4 Per-Profile Isolation

Each profile is a fully independent encrypted directory:

```
~/Coheara/profiles/<uuid>/
├── database/coheara.db     SQLite (encrypted at application level)
├── originals/              Source document files (encrypted)
├── markdown/               Structured .md files (encrypted)
└── verification.enc        Password verification token
```

Profiles share no data. Switching profiles requires unlocking with that profile's password. Deleting a profile destroys its encryption key.

---

## 9. AI System

### 9.1 MedGemma 1.5 4B

**What it is:** A medical domain fine-tuned large language model from Google, running locally via Ollama. 4 billion parameters. Fits on consumer hardware with 8GB+ RAM.

**What it does in Coheara:**
- Converts raw OCR output into structured medical Markdown
- Extracts entities (medications, labs, diagnoses, professionals) from documents
- Generates conversational responses grounded in patient documents
- Produces plain-language explanations of clinical text

**What it does not do:** Diagnose, prescribe, give clinical advice, or make recommendations. These boundaries are enforced by the safety filter, not by relying on the model's behavior.

### 9.2 Embedding Model

**all-MiniLM-L6-v2** runs via ONNX Runtime (not Ollama). 384-dimensional vectors. Fast inference. Runs alongside MedGemma without competing for GPU/CPU resources.

Used for: document chunk embedding, query embedding, semantic similarity search.

### 9.3 RAG Pipeline

Retrieval-Augmented Generation ensures every AI response is grounded in the patient's actual documents. See Section 6.5 for the 13-stage pipeline.

Key architectural decisions:
- Dual retrieval: semantic (vector similarity) + structured (SQLite queries)
- Token budget management: context assembled within model limits
- Citation validation: every cited document verified against database
- Boundary check parsed from every response: Understanding, Awareness, Preparation, or OutOfBounds

### 9.4 Safety Filter

Three-layer validation on every AI output. See Section 6.6 for details.

Key architectural decisions:
- Layer 1 (BoundaryCheck) is non-rephrasable: violations are blocked immediately
- Layers 2-3 attempt rephrase for up to 3 violations
- Input sanitization runs before the query reaches the LLM
- The safety filter is a trait (`SafetyFilter`) for testability and pluggability

### 9.5 Coherence Engine

Eight detection algorithms running on structured data. See Section 6.7 for details.

Key architectural decisions:
- Pure functions operating on `RepositorySnapshot` (pre-fetched data), not live DB connections
- Alert lifecycle with surfacing, dismissal, and audit trail
- Emergency protocol for critical findings
- Drug family matching and dose normalization helpers
- Patient-facing message templates (calm language, preparatory framing)

---

## 10. Security Model

### 10.1 Encryption at Rest

| Property | Implementation |
|----------|---------------|
| Algorithm | AES-256-GCM with random 12-byte nonces per operation |
| Key derivation | PBKDF2 with 600,000 iterations (SHA-256) |
| Key storage | Never written to disk. Derived from password on each unlock. |
| Memory safety | `Zeroize` + `ZeroizeOnDrop` on all key material |
| Scope | Per-profile. Each profile has its own derived key. |

### 10.2 Key Lifecycle

```
Password entered
      │
      ▼
PBKDF2 (600K iterations, random salt)
      │
      ▼
256-bit encryption key (in memory only)
      │
      ├──► Encrypt/decrypt database operations
      ├──► Encrypt/decrypt document files
      │
      ▼
Profile locked → key zeroed from memory (ZeroizeOnDrop)
```

### 10.3 Recovery

**BIP39 24-word mnemonic** generated at profile creation from cryptographic entropy. Allows password reset if the password is forgotten. The mnemonic is shown once during profile creation and never stored digitally. The user is responsible for recording it.

**Cryptographic erasure:** Deleting a profile destroys the encryption key and salt. Without the key or recovery phrase, the encrypted data is computationally unrecoverable.

### 10.4 Device Pairing

| Property | Implementation |
|----------|---------------|
| Key exchange | X25519 ECDH (Curve25519 elliptic curve) |
| Session tickets | One-time WebSocket tickets with 30-second TTL |
| Token rotation | Periodic token refresh with 30-second grace period |
| Pairing flow | QR code scanned by phone, contains connection details |
| Revocation | Revoking a device clears all cached data on the phone |

### 10.5 Phone Security

| Property | Implementation |
|----------|---------------|
| Authentication | Face ID / fingerprint via NativeBiometric |
| Screenshot prevention | FLAG_SECURE (Android) / view hiding (iOS) on sensitive screens |
| Session timeout | 5-minute inactivity auto-lock |
| Root/jailbreak detection | Warning displayed (not blocking) |
| Data scope | Read-only cache of active profile. No AI inference on phone. |

### 10.6 Network Security

| Property | Implementation |
|----------|---------------|
| Internet access | Zero. No network calls to external servers. Ever. |
| Phone sync | Local WiFi only (REST + WebSocket on the local network) |
| Transport | WPA2/WPA3 WiFi encryption at the network layer |
| Distribution server | HTTP on ephemeral port. Serves public install artifacts only, never patient data. Per-IP rate limiting. |

### 10.7 Threat Model

**Protected against:**
- Data at rest: AES-256-GCM encryption per profile
- Lost/stolen device: encryption key not on disk, requires password
- Network sniffing: local WiFi only, no internet traffic to intercept
- Cloud breach: no cloud. Data never leaves the device.
- Abandoned device: cryptographic erasure destroys access permanently
- Prompt injection: input sanitization before LLM, safety filter on output

**Out of scope:**
- Physical access to an unlocked, running application (desktop security is the user's responsibility)
- Keylogger or malware on the user's device (OS-level security)
- Compromise of the Ollama process (trusted local service)

---

## 11. Data Model

### 11.1 Tables (18 core + supporting)

**Core entity tables:**

| Table | Purpose | Key Fields |
|-------|---------|------------|
| `documents` | Source medical documents | type, title, date, professional_id, OCR confidence, perceptual hash |
| `medications` | Medication records | name, dose, frequency, route, status, prescriber, start/end dates |
| `compound_ingredients` | Multi-ingredient medications | medication_id, ingredient_name, dose |
| `tapering_schedules` | Dose reduction plans | medication_id, step_number, dose, start_date |
| `medication_instructions` | Special medication instructions | medication_id, instruction_text |
| `dose_changes` | Historical dose modifications | medication_id, old_dose, new_dose, change_date, reason |
| `lab_results` | Laboratory test values | test_name, value, unit, reference_range, abnormal_flag, date |
| `diagnoses` | Medical diagnoses | name, icd_code, status, diagnosing_professional |
| `professionals` | Healthcare providers | name, specialty, institution, dates_seen |
| `allergies` | Allergy records | allergen, reaction, severity, source |
| `procedures` | Surgical/medical procedures | name, date, outcome, performing_professional |
| `symptoms` | Patient-reported symptoms (OLDCARTS) | onset, location, duration, character, severity, source |
| `appointments` | Patient appointments | type, date, professional_id, notes |
| `referrals` | Referral relationships | from_professional, to_professional, status, reason |
| `conversations` | Chat session records | title, created_at, updated_at |
| `messages` | Individual chat messages | conversation_id, role, content, feedback |
| `dismissed_alerts` | Dismissed coherence alerts | alert_type, entity_ids, dismissed_by, reason |
| `medication_aliases` | Brand/generic name mappings | brand_name, generic_name, source |

**Supporting tables:**

| Table | Purpose |
|-------|---------|
| `profile_trust` | Singleton trust metrics for the profile |
| `dose_references` | Bundled safe dosing reference data |
| `schema_version` | Migration version tracking |

### 11.2 Enums (16)

| Enum | Variants |
|------|----------|
| `DocumentType` | Prescription, LabResult, ClinicalNote, DischargeSummary, RadiologyReport, PharmacyRecord, Other |
| `FrequencyType` | Scheduled, AsNeeded, Tapering |
| `MedicationStatus` | Active, Stopped, Paused |
| `DoseType` | Fixed, SlidingScale, WeightBased, Variable |
| `AbnormalFlag` | Normal, Low, High, CriticalLow, CriticalHigh |
| `DiagnosisStatus` | Active, Resolved, Monitoring |
| `AllergySeverity` | Mild, Moderate, Severe, LifeThreatening |
| `AllergySource` | DocumentExtracted, PatientReported |
| `SymptomSource` | PatientReported, GuidedCheckin, FreeText |
| `AppointmentType` | Upcoming, Completed |
| `ReferralStatus` | Pending, Scheduled, Completed, Cancelled |
| `AlertType` | Conflict, Gap, Drift, Ambiguity, Duplicate, Allergy, Dose, Critical, Temporal |
| `DismissedBy` | Patient, ProfessionalFeedback |
| `MessageRole` | Patient, Coheara |
| `MessageFeedback` | Helpful, NotHelpful |
| `AliasSource` | Bundled, UserAdded |

All enums use the `str_enum!` macro (implements `as_str()` and `FromStr`) for serialization to/from SQLite text columns.

### 11.3 Entity Relationships

```
Professional ──┬── prescribes ──► Medication ──► DoseChange (audit trail)
               │                      │
               │                      ├──► CompoundIngredient
               │                      ├──► TaperingStep
               │                      └──► MedicationInstruction
               │
               ├── produces ──► Document ──► (chunks → vector store)
               │
               ├── diagnoses ──► Diagnosis
               │
               ├── orders ──► LabResult
               │
               └── refers ──► Referral ──► Professional (target)

Patient ──┬── reports ──► Symptom (OLDCARTS, source: patient-reported)
          ├── records ──► Allergy
          ├── has ──► Appointment
          ├── converses ──► Conversation ──► Message
          └── dismisses ──► DismissedAlert
```

---

## 12. Mobile Companion

### 12.1 What It Does

The phone companion is a read-only window into the patient's personal Vault. It does not process documents or run AI inference. It provides immediate access to structured health data when the patient is away from their desktop.

| Feature | Description |
|---------|-------------|
| View medications | Current and historical medications with dose, frequency, prescriber |
| Check lab results | Lab values with abnormal flags and reference ranges |
| Read alerts | Coherence alerts surfaced by the desktop's detection engine |
| Log symptoms | OLDCARTS-guided symptom entry, synced back to desktop |
| Capture documents | Photograph medical documents with phone camera, send to desktop for processing |
| Prepare for appointments | View auto-generated appointment summaries and questions |
| Browse timeline | Health events across time |

### 12.2 What It Does Not Do

- No AI inference (MedGemma runs on desktop only)
- No document processing (OCR and structuring happen on desktop)
- No direct database access (reads a cached snapshot)
- No independent operation without initial pairing to a desktop

### 12.3 Sync Model

**Version-based delta sync:** The phone requests changes since its last known version. The desktop responds with only the new or modified records. Full sync is not required after initial pairing.

**Read-only cache:** The phone stores a snapshot of the active profile's structured data. Changes made on the phone (symptom entries, document captures) are sent to the desktop and synced back after processing.

**Conflict resolution:** Desktop is authoritative. Phone never overwrites desktop data.

### 12.4 Native Capabilities

| Capability | Technology | Purpose |
|-----------|-----------|---------|
| Biometric auth | NativeBiometric (Face ID / fingerprint) | Gate access to health data |
| Camera | Capacitor Camera | Capture medical documents |
| Secure storage | Capacitor Preferences (Keychain / Keystore) | Store session tokens and sync state |
| Privacy screen | PrivacyScreen plugin | FLAG_SECURE / view hiding on sensitive screens |
| Root detection | Integrity check | Warning on rooted/jailbroken devices |

### 12.5 PWA Support

For users who prefer not to install a native app (or on iOS where sideloading is not possible), Coheara's mobile companion is also available as a Progressive Web App:

- **Service worker:** Cache-first for app shell (JS, CSS, HTML), network-first for API calls
- **Offline support:** Full app shell works offline; data requires WiFi sync
- **Manifest:** Installable to home screen with standalone display mode
- **Icons:** 192px and 512px maskable icons

### 12.6 Mobile Stack

| Layer | Technology |
|-------|-----------|
| Shell | Capacitor 8 (iOS + Android) or PWA (any browser) |
| Frontend | Svelte 5, SvelteKit 2, TailwindCSS 4 |
| Auth | Face ID / fingerprint via NativeBiometric |
| Storage | Capacitor Preferences (Keychain / Keystore) |
| Camera | Capacitor Camera (document capture) |
| Privacy | PrivacyScreen (FLAG_SECURE / view hiding) |
| Integrity | Root/jailbreak detection (warning, not blocking) |
| Sync | REST + WebSocket over local WiFi |
| PWA | Service worker, offline cache, manifest |

---

## 13. App Distribution Server

### 13.1 Purpose

The App Distribution Server (ADS) allows the desktop application to serve the mobile companion directly to phones over local WiFi. No app store account, no internet, no sideloading instructions needed.

### 13.2 How It Works

1. User opens Settings > Companion Setup in the desktop app
2. Taps "Start Distribution Server"
3. Desktop binds an HTTP server on an ephemeral port on the local network
4. A QR code is displayed containing the server URL
5. User scans the QR code with their phone's camera
6. Phone opens the install landing page in its browser
7. Page detects the platform (Android or iOS) and routes accordingly:
   - **Android:** Downloads and installs the APK directly
   - **iOS:** Opens the PWA, which can be added to the home screen

### 13.3 Endpoints

| Path | Purpose |
|------|---------|
| `/install` | Landing page with platform detection |
| `/install/android` | Android-specific install instructions + download link |
| `/install/android/download` | APK binary with SHA-256 integrity hash |
| `/app` | PWA index.html |
| `/app/*` | PWA static assets with SPA fallback |
| `/update` | Version check JSON |
| `/health` | Server health check |

### 13.4 Security

- HTTP (not HTTPS): distribution payloads are public install artifacts, never patient data
- Local WiFi with WPA2/WPA3 provides transport encryption
- Per-IP rate limiting (60 requests per minute)
- Path traversal prevention (canonicalize + starts_with check)
- Ephemeral port: closed when the user stops the server

### 13.5 Asset Resolution

In production builds, the mobile PWA is bundled as a Tauri resource inside the desktop installer. The server checks:
1. Bundled resources (production): `{resource_dir}/resources/mobile-pwa/`
2. User data fallback (development): `~/Coheara/mobile-pwa/`

APK files are resolved from `~/Coheara/mobile-apk/coheara.apk`.

---

## 14. Implementation Status

### 14.1 Completed Components

| Component | ID | Tests | Description |
|-----------|----|-------|-------------|
| Project Scaffold | L0-01 | 5 | Tauri 2.10 + Svelte 5 + SvelteKit 2 + TailwindCSS 4 |
| Data Model | L0-02 | 19 | 18 tables, 16 enums, repository functions |
| Encryption Layer | L0-03 | 32 | AES-256-GCM, PBKDF2 600K, BIP39 recovery, Zeroize |
| Document Import | L1-01 | 30 | Magic bytes, hashing, deduplication, encrypted staging |
| OCR and Extraction | L1-02 | - | Tesseract, pdf-extract, preprocessing |
| Medical Structuring | L1-03 | - | Ollama/MedGemma, LLM prompting, JSON parsing |
| Storage Pipeline | L1-04 | - | Chunking, embedding, vector store, entity store |
| RAG Pipeline | L2-01 | 48 | 13-stage pipeline: classify, retrieve, assemble, cite, converse |
| Safety Filter | L2-02 | 51 | 3-layer validation, rephrase, sanitize |
| Coherence Engine | L2-03 | 55 | 8 detection algorithms, alert lifecycle, emergency protocol |
| Profile Management UI | L3-01 | - | Create, unlock, lock, delete, switch profiles |
| App Distribution Server | ADS | 22 | HTTP server, QR code, APK/PWA serving, rate limiting |

### 14.2 Test Summary

| Suite | Tests | Scope |
|-------|-------|-------|
| Desktop (Rust) | 975 | All backend components |
| Mobile (Vitest) | 481 | Stores, API clients, biometric, lifecycle, sync, accessibility |
| **Total** | **1,456** | |

Clippy: 0 warnings (enforced in CI with `-D warnings`).

### 14.3 CI/CD

**On every push/PR:** Frontend type check, mobile type check + 481 tests, Rust clippy + 975 tests, PWA build verification.

**On version tag:** Builds 4 desktop platforms (Windows NSIS/MSI, macOS ARM/Intel DMG, Linux deb/AppImage). Mobile PWA built and bundled as a Tauri resource. Draft GitHub Release created with all installers.

---

## 15. Engineering Conventions

### 15.1 Rust

| Rule | Detail |
|------|--------|
| Error handling | `thiserror` for error enums. No `unwrap()` in production code. |
| Logging | `tracing` crate for structured logging. No `println!`. |
| Dependency injection | Trait-based DI everywhere: `OcrEngine`, `LlmClient`, `EmbeddingModel`, `VectorStore`, `SafetyFilter` |
| Repository pattern | Free functions taking `&Connection`, not methods on a repository struct |
| Enums | `str_enum!` macro for string serialization via `FromStr` trait |
| Memory safety | `Zeroize` + `ZeroizeOnDrop` on all cryptographic material |
| MSRV | 1.80 (for `std::sync::LazyLock`) |

### 15.2 Svelte 5

| Rule | Detail |
|------|--------|
| Reactivity | `$props()`, `$state()`, `$derived()`. Not legacy `export let`. |
| Components | Single responsibility. One primary action per component. |
| Styling | TailwindCSS 4 utility classes. No component-scoped `<style>` blocks. |
| Type safety | TypeScript throughout. Shared types in `lib/types/`. |

### 15.3 General

| Rule | Detail |
|------|--------|
| Single responsibility | One function = one task. One class = one purpose. |
| Naming | Short, clear, self-explanatory names. Consistent across the codebase. |
| Coupling | Low coupling, high cohesion. Related code together, dependencies minimal. |
| Testing | Test-driven development. Encode purpose into validated tests. |
| Data model first | Always design and validate the data model before coding features. |
| No over-engineering | Only build what is needed now. No hypothetical future requirements. |

---

*The patient is not the subject of healthcare. The patient is the reason healthcare exists. Build the system around them.*

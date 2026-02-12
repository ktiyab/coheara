# Coheara: Full Definition
## Version 2.0 — Patient-Centered Architecture

**Date:** 2026-02-11
**Status:** Brainstorm (pending validation)
**Supersedes:** v1.0, 01-PATIENT-MEDAI-DEFINITION.md

---

## Coheara

**A patient's personal MedAI** — an AI-powered platform that encodes intent preservation as operating logic, deploys domain-specific language models at clinical speed, and maintains meaning coherence across every professional, every shift, and every system that touches the patient's care; so that what the clinician means, what the nurse observes, what the pharmacist verifies, and what the patient experiences all survive the journey intact.

---

## The Hard Boundary: Coheara Does Not Replace Professionals

**This is non-negotiable. It governs every design decision.**

Coheara helps the patient **understand** their care. It does not **provide** care.

| Coheara DOES | Coheara DOES NOT |
|-------------|-----------------|
| Explain what a document says, in the patient's language | Diagnose conditions |
| Surface inconsistencies across documents from different professionals | Prescribe treatments |
| Flag what the patient should ask about at their next appointment | Give clinical advice |
| Help the patient understand their medications, labs, and care plan | Override or contradict professional judgment |
| Detect patterns across time that the patient can bring to their doctor | Make treatment recommendations |
| Prepare the patient to be an informed participant in their care | Replace any clinical encounter |

**Coheara's output is always:**
- **Understanding** — "Here's what your documents say, in your words"
- **Awareness** — "Here's something that looks inconsistent across your documents"
- **Preparation** — "Here are questions you should bring to your next appointment"

**Coheara's output is never:**
- ~~Diagnosis~~ — "You have X condition"
- ~~Prescription~~ — "You should take Y"
- ~~Clinical judgment~~ — "Your doctor is wrong about Z"
- ~~Treatment advice~~ — "Stop taking this medication"

When Coheara detects an inconsistency — two prescriptions that may interact, a lab result that doesn't align with a diagnosis — it does not tell the patient what to do. It tells the patient: **"Ask your doctor about this at your next appointment."**

The professional has **clinical authority**. Coheara has **comprehension authority**. They don't overlap. This is P3 (Expertise-Based Authority) applied to the patient-AI boundary.

**The result:** The patient arrives at their appointment understanding their documents, aware of potential concerns, and prepared with the right questions. The quality of the clinical encounter improves because the patient is informed — not because the AI replaced the professional.

---

## What It Is

Coheara is a personal, local, secure medical AI for each patient. It does three things:

1. **Ingests** the medical documents the patient already receives from their health professionals
2. **Builds meaning** by projecting those documents into a semantic space where relationships emerge automatically
3. **Helps the patient understand** their care through grounded conversation — answering questions, surfacing inconsistencies, preparing them for their next appointment

Health professionals change nothing. They keep producing the documents they already produce — prescriptions, clinical notes, lab results, discharge summaries, radiology reports, pharmacy instructions. Those documents already exist. **The patient loads them into Coheara.** Coheara does the rest.

No professional workflow change. No institutional integration. No external connection. Just the patient and their MedAI, running locally on their device.

---

## Why It Exists

Healthcare suffers from a single architectural failure: information moves forward but meaning doesn't come back. This manifests as:

- 125 million physician-hours/year lost to documentation
- 80% of serious adverse events involving handoff miscommunication
- 22% coding error rates
- 90-96% clinical alert override rates
- $35 billion in prior authorization friction
- 88% of adults below proficient health literacy
- 62.8% physician burnout

Each is a symptom of an open-loop system where no coherence observation exists between what was meant and what was understood. Coheara closes the loop — not at the institutional level, but at the patient level, where meaning matters most.

And critically: **adding another system that requires professionals to input data would add burden on top of burden** — the exact problem Coheara exists to solve. Instead, Coheara works with documents that already exist, loaded by the patient, requiring zero professional workflow change.

---

## How It Works

### The Document Ingestion Pipeline

Every medical document the patient loads — prescription scan, lab PDF, discharge summary photo, clinical note printout, radiology report — goes through one pipeline:

```
DOCUMENT IN (any format: photo, PDF, scan, typed text)
    │
    ▼
┌─────────────────────┐
│ CONVERT TO MARKDOWN  │  Photo → OCR → .md
│                      │  PDF → extract → .md
│                      │  Typed text → .md
│                      │  Scanned note → OCR → .md
│                      │  MedGemma structures messy
│                      │  text into clean labeled .md
└──────────┬──────────┘
           │
           ▼
┌─────────────────────┐
│ INGEST INTO          │  Parse .md into chunks
│ SEMANTIC DATABASE    │  Generate embeddings
│                      │  Store with metadata:
│                      │    - source document
│                      │    - date
│                      │    - professional who produced it
│                      │    - document type
│                      │    - patient context tags
└──────────┬──────────┘
           │
           ▼
┌─────────────────────┐
│ MEANING BY           │  Embeddings project into
│ PROJECTION           │  semantic space where
│                      │  proximity = relatedness
│                      │
│                      │  Meaning is not declared.
│                      │  Meaning EMERGES from
│                      │  proximity in the space.
└─────────────────────┘
```

### Why Markdown

Markdown is the universal intermediate format:

| Reason | Detail |
|--------|--------|
| **Structure without complexity** | Headers, lists, tables — enough to preserve document hierarchy |
| **Human readable** | The patient can read the .md directly |
| **Machine parseable** | Clean chunking for embedding — sections, paragraphs, key-value pairs naturally delimited |
| **Lossless meaning** | A prescription in .md retains: drug name, dose, frequency, prescriber, date, instructions |
| **Uniform input** | Every document, regardless of source format, becomes the same input type |
| **Versionable** | .md files diff cleanly — changes are visible |
| **Lightweight** | No binary dependencies, no rendering engine, no format lock-in |

### Why Semantic Database (Meaning by Projection)

The artifact constellation is not a relational database with tables and foreign keys. It is a **semantic space** where meaning is built by projection — embeddings place every piece of medical information in a high-dimensional space where **proximity equals relatedness**.

| Traditional DB | Semantic DB |
|---------------|-------------|
| Relationships declared by schema | Relationships **emerge** from embedding proximity |
| Query by exact match or SQL | Query by semantic similarity |
| Adding a new document requires manual linking | Adding a new document **automatically** relates it to everything already there |
| Inconsistency requires explicit rules | Inconsistency **appears** as unexpected distance or conflicting proximity |
| Meaning is in the structure | Meaning is in the **projection** |

This is P2 (Emergent Ontology) encoded as architecture. Shared vocabulary isn't declared top-down — it emerges from how documents embed relative to each other.

### How Coherence Works in the Semantic Space

**Inconsistency detection:** Two documents that should be close in the space but carry conflicting content. A prescription for a drug and a lab result showing contraindication — they embed near each other because they share medical concepts, but their content conflicts. MedGemma retrieves both, detects the conflict, surfaces it to the patient as a question to ask their doctor.

**Gap detection:** A region of the space that should have coverage but doesn't. A diagnosis exists but no corresponding treatment plan embeds near it. Coheara notices the gap, surfaces it as something to discuss.

**Drift detection:** Embeddings that shift over time. Early documents cluster around one treatment approach; recent documents drift toward another without explicit rationale. Coheara detects the trajectory, helps the patient understand what changed and why (or asks if they know why).

**Question answering:** Patient asks "why am I taking metformin?" → Coheara queries the semantic space for embeddings near "metformin" → retrieves the prescription, the diagnosis (diabetes), the lab results (HbA1c) → synthesizes a grounded, plain-language answer from the patient's own documents. Not generic medical advice — their specific care, their specific documents.

---

## Architecture

```
┌──────────────────────────────────────────────────────────┐
│                     THE PATIENT                           │
│                                                          │
│  Loads documents:       Talks to Coheara:                │
│  - Prescription photos  - "I feel dizzy since the change"│
│  - Lab result PDFs      - "Why am I taking this?"        │
│  - Discharge summaries  - "What should I ask my doctor?" │
│  - Clinical notes       - "Is this normal?"              │
│  - Radiology reports    - "I've had headaches for 3 days"│
│  - Pharmacy printouts                                    │
└────────────┬─────────────────────┬───────────────────────┘
             │                     │
             ▼                     ▼
┌────────────────────┐  ┌─────────────────────────────────┐
│ DOCUMENT PIPELINE  │  │ CONVERSATION                     │
│                    │  │                                   │
│ Any format         │  │ Patient input → .md chunk         │
│   → OCR if needed  │  │   → embed into semantic DB        │
│   → MedGemma       │  │   → query related embeddings      │
│     structures     │  │   → MedGemma generates response   │
│   → Clean .md      │  │     grounded in patient's own     │
│   → Chunk          │  │     documents                     │
│   → Embed          │  │                                   │
│   → Store in       │  │ OUTPUT IS ALWAYS:                 │
│     semantic DB    │  │   Understanding, Awareness,       │
│                    │  │   Preparation                     │
│                    │  │ OUTPUT IS NEVER:                   │
│                    │  │   Diagnosis, Prescription,         │
│                    │  │   Clinical Advice                  │
└────────────────────┘  └─────────────────────────────────┘
             │                     │
             └──────────┬──────────┘
                        ▼
┌──────────────────────────────────────────────────────────┐
│              SEMANTIC DATABASE                            │
│              (The Artifact Constellation)                 │
│                                                          │
│  Every .md chunk embedded in semantic space               │
│  Meaning emerges from proximity                          │
│  No manual linking required                              │
│                                                          │
│  ┌─────┐  ┌─────┐  ┌─────┐  ┌─────┐  ┌─────┐          │
│  │ Rx  │  │ Lab │  │ Dx  │  │Nurse│  │Patient│         │
│  │     │  │     │  │     │  │ obs │  │report │         │
│  └──┬──┘  └──┬──┘  └──┬──┘  └──┬──┘  └──┬───┘         │
│     └────────┼────────┼────────┼────────┘               │
│              ▼        ▼        ▼                         │
│         PROXIMITY = RELATEDNESS                          │
│         DISTANCE  = POTENTIAL INCONSISTENCY              │
│         ABSENCE   = GAP                                  │
│         MOVEMENT  = DRIFT                                │
└──────────────────────┬───────────────────────────────────┘
                       │
                       ▼
┌──────────────────────────────────────────────────────────┐
│              COHEARA (MedGemma 1.5 4B)                   │
│                                                          │
│  Queries semantic DB for every patient interaction        │
│  Retrieves relevant chunks by proximity                  │
│  Generates responses grounded in patient's documents     │
│  Runs coherence observation across the space             │
│  Surfaces: understanding, awareness, preparation         │
│  Never: diagnosis, prescription, clinical advice         │
│                                                          │
│  All local. No external connection. Patient's device.    │
└──────────────────────────────────────────────────────────┘
```

### Where Documents Come From (Zero Professional Burden)

Professionals already produce these. The patient already receives them. Nothing changes for professionals.

| Professional | Documents They Already Produce | How Patient Gets Them |
|-------------|-------------------------------|----------------------|
| **Doctor** | Clinical notes, prescriptions, referral letters, treatment plans | Patient portal, printout, copy request |
| **Nurse** | Vitals records, care notes, observation sheets | Discharge paperwork, patient portal |
| **Pharmacist** | Medication labels, interaction warnings, dispensing records | Printed with medication, patient portal |
| **Lab** | Lab results, pathology reports | Patient portal, mailed, printed |
| **Radiologist** | Imaging reports, scan results | Patient portal, mailed, printed |
| **Hospital** | Discharge summaries, procedure records, billing summaries | Printed at discharge, patient portal |
| **Specialist** | Consultation notes, specialist recommendations | Mailed, patient portal, given to patient |

The patient loads these into Coheara: photo, scan, PDF, typed text. Coheara converts to .md, embeds into the semantic database, and the constellation grows.

---

## What Coheara Does

### 1. Ingests and Structures Documents

The patient loads a document. Coheara:
- Converts it to clean Markdown (OCR if photo/scan, extraction if PDF, structuring if messy text)
- Identifies document type (prescription, lab result, clinical note, etc.)
- Extracts key fields (medication names, doses, diagnoses, values, dates, professional names)
- Chunks the .md into meaningful segments
- Generates embeddings for each chunk
- Stores in the semantic database with metadata

Every new document automatically relates to everything already in the constellation through embedding proximity.

### 2. Builds the Artifact Constellation

The semantic database holds the patient's complete medical meaning:

| What's in the Space | How It Got There |
|--------------------|-----------------|
| Diagnoses | Extracted from clinical notes, discharge summaries |
| Medications | Extracted from prescriptions, pharmacy records |
| Lab results | Extracted from lab reports |
| Allergies & contraindications | Extracted from clinical notes, pharmacy records |
| Treatment goals | Extracted from clinical notes, treatment plans |
| Professional involvement | Extracted from document signatures, letterheads |
| Patient's own reports | From patient conversation input |
| Temporal information | From document dates, sequence of events |

Relationships are not manually declared. They emerge from semantic proximity. A prescription for lisinopril embeds near the hypertension diagnosis, near the blood pressure lab results, near the patient's report of dizziness — because they share medical meaning.

### 3. Helps the Patient Understand

The patient talks to Coheara. Coheara responds grounded in their documents:

**Understanding:**
- "What does this lab result mean?" → Coheara retrieves the lab report and related diagnoses from the constellation, explains in plain language what the values indicate relative to the patient's conditions
- "Why am I taking metformin?" → Coheara traces: metformin prescription → diabetes diagnosis → HbA1c lab results → explains the chain in the patient's words
- "What did the doctor say at my last visit?" → Coheara retrieves the clinical note, presents the key decisions in plain language

**Awareness:**
- "Your cardiologist prescribed medication X, but your endocrinologist's notes mention condition Y — these may interact. Ask your doctor about this at your next appointment."
- "Your last three lab results show a trend in value Z. You may want to discuss this with your doctor."
- "Your discharge summary mentions follow-up in 2 weeks, but no follow-up appointment appears in your recent documents."

**Preparation:**
- "Based on your documents, here are questions you might want to ask at your next cardiology appointment: [list grounded in constellation gaps and signals]"
- "Your medication list has changed three times in the past month. You may want to ask your doctor to review the current list with you."

### 4. Guards Coherence

The Coherence Engine observes the semantic space continuously across five dimensions:

| Dimension | What It Observes | Weight |
|-----------|-----------------|--------|
| **Purpose alignment** | Do treatments documented serve the stated care goals? | 0.30 |
| **Boundary compliance** | Are documented constraints honored across all documents? | 0.25 |
| **Rationale consistency** | Are decisions across documents consistent with each other? | 0.20 |
| **Agent agreement** | Do documents from different professionals align? | 0.15 |
| **Temporal stability** | Has the documented care direction shifted without explanation? | 0.10 |

When Coheara detects a signal, the output is always framed as preparation for the professional encounter — never as clinical advice.

### 5. Detects Inconsistencies

| Signal Type | Example | Patient Output |
|-------------|---------|---------------|
| **CONFLICT** | Prescription from doctor A conflicts with documented allergy from doctor B | "Your records show an allergy to X, but this prescription contains a related ingredient. Ask your doctor about this." |
| **GAP** | Diagnosis exists but no corresponding treatment documented | "Your records mention condition X, but I don't see a treatment plan for it in your documents. You may want to ask about this." |
| **DRIFT** | Treatment approach changed without documented rationale | "Your medication for X was changed from Y to Z. I don't see a note explaining why. You might want to ask at your next visit." |
| **AMBIGUITY** | Two specialists give different guidance | "Dr. A's notes say X, but Dr. B's notes say Y. You may want to clarify which guidance to follow." |

### 6. Persists Memory (Lifetime)

Coheara never forgets. Every document ingested is permanently embedded. The constellation grows over time. When the patient sees a new specialist, they can share their Coheara-generated summary — a coherent, structured overview of their complete medical history, grounded in actual documents.

### 7. Gives the Patient a Voice

The patient's input is an artifact too:
- "I've been having headaches since the medication change" → embedded into the constellation, correlated with medication timeline
- "I feel better since starting the new treatment" → embedded as coherence evidence, linked to treatment artifacts
- "I'm worried about side effects" → captured as a patient concern, surfaced when relevant documents are discussed

The patient is the first sensor of their own body. Their reports enrich the constellation with information no professional document contains.

---

## Core Design Decisions

| Decision | Rationale |
|----------|-----------|
| **Coheara does not replace professionals** | Clinical authority belongs to professionals. Coheara provides comprehension, awareness, and preparation. These don't overlap. |
| **Patient loads documents; professionals change nothing** | Adding professional burden contradicts the purpose. Documents already exist. The patient is already the collector. |
| **All documents convert to Markdown** | Universal intermediate format. Human readable, machine parseable, structured without complexity, versionable. |
| **Semantic database with meaning by projection** | Relationships emerge from embedding proximity. No manual linking. No schema declaration. New documents automatically relate to the constellation. |
| **Patient-centered, not institution-centered** | The patient is the only entity that persists across their entire care. |
| **Local, secure, no external connection** | The patient's medical data never leaves their device. No cloud. No API calls. Privacy by architecture. |
| **Framework encoded as logic, not documentation** | The Intent Preservation Framework is operational firmware, not a reference document. |
| **AI operates as middleware, not endpoint** | MedGemma observes, translates, detects, surfaces. It does not decide. |
| **Single local SLM (MedGemma 1.5 4B)** | One model. Runs on Ollama. On-device. Differentiated by prompts and context. |
| **Patient as active agent** | The patient is both the organizer (loads documents) and a participant (talks to Coheara). Their input has equal standing in the constellation. |

---

## The Eleven Principles Encoded

| Principle | How Coheara Embodies It |
|-----------|------------------------|
| **P1: Bootstrap from Need** | Every Coheara instance starts from the patient's need. The first document loaded is the first artifact. |
| **P2: Emergent Ontology** | When different professionals use different terms, semantic proximity detects the mapping. Vocabulary alignment emerges from the embedding space. |
| **P3: Expertise-Based Authority** | Professionals have clinical authority. Coheara has comprehension authority. They don't overlap. Coheara never advises — it explains, surfaces, and prepares. |
| **P4: Layered Validation** | The patient brings Coheara's observations to professionals. Professionals validate. The patient closes the loop. |
| **P5: Adaptive Cycle Rate** | More documents ingested in a short period (acute care) → more coherence checks. Stable periods → less activity. |
| **P6: Empirical Disagreement** | When documents from different professionals conflict, Coheara surfaces both and lets the patient bring the question to the appropriate professional. |
| **P7: Evidence-Weighted Memory** | Documents with outcomes (lab improvements, resolved symptoms) strengthen nearby artifacts. Documents contradicted by outcomes are flagged. |
| **P8: Failure Metabolism** | If Coheara surfaces a concern that the professional dismisses with rationale, that rationale enriches the constellation. |
| **P9: Dual-Test Truth** | Understanding requires both the document (evidence) and the patient's experience (lived consensus). |
| **P10: Meta-Principles** | When documents conflict and no resolution exists, Coheara holds the ambiguity and helps the patient ask the right questions. |
| **P11: Cross-Domain Learning** | Documents from different specialties that conflict reveal where one domain's assumptions break against another's. Coheara makes these visible. |

---

## The SLM Layer

MedGemma 1.5 4B powers Coheara with bounded capabilities:

| Role | Task | Phase |
|------|------|-------|
| **Document conversion** | OCR + structuring into clean .md | 1 (Alpha) |
| **Embedding generation** | Create semantic vectors for each .md chunk | 1 (Alpha) |
| **Coherence checking** | Compare new embeddings against constellation | 1 (Alpha) |
| **Patient conversation** | Answer questions grounded in patient's documents | 1 (Alpha) |
| **Plain language** | Translate clinical text for the patient | 1 (Alpha) |
| **Inconsistency detection** | Surface conflicts, gaps, drift across documents | 1 (Alpha) |
| **Appointment preparation** | Generate questions for the patient's next visit | 1 (Alpha) |
| **Medical imaging** | Interpret images loaded by patient (X-rays, etc.) | 2 (Future) |
| **Trend analysis** | Temporal patterns across document timeline | 2 (Future) |

All outputs are understanding, awareness, and preparation. None are clinical advice.

---

## What the Alpha Proves

A patient's personal MedAI, running locally on their device with no external connections, can:

1. Ingest medical documents the patient already has (photos, PDFs, scans)
2. Convert them to structured Markdown and embed them in a semantic database
3. Build meaning automatically through embedding projection
4. Let the patient converse about their care, grounded in their actual documents
5. Detect inconsistencies across documents from different professionals
6. Prepare the patient for their next appointment with the right questions
7. Do all of this without replacing any professional and without any professional changing their workflow

If this works — the architecture holds. Everything else is scale.

---

## Open Questions

| Question | Why It Matters |
|----------|---------------|
| Which semantic database? (ChromaDB, FAISS, Qdrant, LanceDB?) | Local deployment, performance, persistence |
| MedGemma for embeddings or a separate embedding model? | Quality of semantic space, resource usage |
| How does OCR handle poor-quality photos of handwritten prescriptions? | Real-world document quality varies wildly |
| What's the onboarding experience? First document → first value? | Patient must see value immediately or they won't continue loading documents |
| How does the patient correct OCR/extraction errors? | Documents will be misread; the patient must be able to fix this |
| What device does it run on? (Laptop? Tablet? Phone?) | MedGemma 1.5 4B needs ~8GB; limits mobile deployment |
| How does the patient share Coheara summaries with their doctor? | The bridge back to the professional encounter |
| How does Coheara handle documents in multiple languages? | Patients may receive documents in different languages |
| What's the data backup/recovery model? | Patient's medical history must not be lost to device failure |

---

*The patient is not the subject of healthcare. The patient is the reason healthcare exists. Build the system around them. Professionals already produce the documents. The patient already collects them. Coheara turns that collection into understanding — and sends the patient to their next appointment prepared, not lost.*

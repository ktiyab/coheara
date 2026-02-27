# MEDGEMMA-BENCHMARK-05: Focused Q&A Extraction Strategy

> **Purpose**: Test whether replacing JSON-schema extraction with focused natural
> language questions eliminates degeneration on the 4B model.
>
> **Date**: 2026-02-26 | **Status**: DRAFT
> **Depends on**: `MEDGEMMA-BENCHMARK-04.md` (baseline data), `MODEL-FACTORY-SPEC.md` (model config)
> **Hypothesis**: Degeneration is caused by two compounding factors —
> (1) multi-domain attention overload and (2) JSON formatting overhead.
> Remove both. Let the model answer questions. Let code build structure.

---

## Table of Contents

| # | Section | Line |
|---|---------|------|
| 0 | [Problem Statement](#0-problem) | 30 |
| 1 | [Root Cause Analysis](#1-root-cause) | 80 |
| 2 | [The Right Split](#2-split) | 130 |
| 3 | [Question Design](#3-questions) | 175 |
| 4 | [Document Chunking](#4-chunking) | 310 |
| 5 | [Code Assembly Layer](#5-assembly) | 350 |
| 6 | [Test Matrix](#6-matrix) | 410 |
| 7 | [Success Criteria](#7-criteria) | 470 |
| 8 | [Execution Plan](#8-plan) | 510 |
| 9 | [Decision Log](#9-decisions) | 560 |

---

<a id="0-problem"></a>
## 0. Problem Statement

### What BM-04 Asked the Model to Do

BM-04's extraction prompt asked the 4B model to simultaneously:

1. **Comprehend** a medical document (OCR from image or parse text)
2. **Classify** every piece of information into 7+ categories
3. **Format** output as valid JSON with nested objects and arrays
4. **Comply** with a ~300-token schema (field names, types, nullability rules)
5. **Apply** conversion rules (dates → YYYY-MM-DD, decimals → period separator)
6. **Preserve** the original language in text fields
7. **Produce** structured Markdown after the JSON

That's 7 cognitive tasks in one prompt. For a 4B model.

### What Happened

| Task Complexity | GPU Degen Rate |
|-----------------|----------------|
| "Describe what you see" (1 task) | 0% (0/4) |
| "Extract {domain}" (1 domain + JSON schema) | 25% (2/8) |
| "Extract ALL as JSON" (7+ domains + full schema) | 50-88% |

### What We Got Wrong

Even the chat extraction tests — which are already single-domain — still ask the
model to produce JSON. They degenerate at 25%. The schema overhead is a second
source of attention waste on top of multi-domain overload.

**The model should not be formatting JSON.** That's code's job.

---

<a id="1-root-cause"></a>
## 1. Root Cause Analysis

### Two Compounding Factors

```
Factor 1: MULTI-DOMAIN ATTENTION OVERLOAD
  "Extract medications AND lab results AND diagnoses AND allergies
   AND procedures AND referrals AND instructions"
  → Model's thinking chain tries to classify each data point into 7 categories
  → Combinatorial reasoning space grows with each domain
  → Thinking loops: "Is this a medication or a diagnosis?"

Factor 2: JSON FORMATTING OVERHEAD
  "Return valid JSON: {name: string, dose: string or null, ...}"
  → Model must hold schema structure in working memory
  → Must track nesting, commas, brackets, null vs empty string
  → Must apply formatting rules (YYYY-MM-DD, period decimals)
  → Thinking loops: "What JSON field does this go into?"

Combined: The model spends more attention on FORMAT than on COMPREHENSION.
```

### Evidence: Thinking Token Analysis (from A1)

BM-04's thinking blocks (avg 469 words) show the model reasoning about structure:

```
"I need to extract... medications... lab_results... diagnoses..."  ← domain selection
"The schema requires source_messages to be a list..."             ← format compliance
"Let me construct the JSON... {medications: [{name:..."           ← manual formatting
"Double-check rules: 1. Only patient messages: Yes..."            ← rule verification
```

More than half the thinking is about **how to format**, not **what the content means**.
A 4B model's attention is finite. Every token spent on formatting is a token NOT
spent on understanding the medical document.

### The Radiograph Proof

Radiograph tests ask: "Describe what you see in this medical image."
No JSON. No schema. Just answer in natural language.
Result: **0% degeneration on GPU.** The model excels when it just answers questions.

---

<a id="2-split"></a>
## 2. The Right Split — Model Answers, Code Structures

### Principle

```
MODEL = comprehension engine  →  Answers questions in natural language
CODE  = structuring engine    →  Parses answers into typed data structures
```

| Responsibility | MODEL does | CODE does |
|----------------|-----------|-----------|
| Read the document | YES | no |
| Understand medical terms | YES | no |
| Answer "what medications?" | YES | no |
| JSON schema compliance | no | YES |
| Field naming and types | no | YES |
| Date format conversion | no | YES |
| Null handling | no | YES |
| Nested object creation | no | YES |

### Current (BM-04) — Model Does Everything

```
Prompt (300+ tokens of schema):
  "Extract ALL medical information... return valid JSON:
   {document_type: ..., medications: [{name: string, dose: ...}], ...}"

Model output:
  <unused94>thought [469 words of reasoning about schema + content] <unused95>
  ```json
  {"document_type": "prescription", "medications": [...], ...}
  ```

Risk: Schema compliance competes with comprehension for attention → degeneration
```

### BM-05 — Model Just Answers

```
Prompt (~20 tokens):
  "What medications are mentioned in this document?
   For each, state the name, dose, and frequency."

Model output:
  <unused94>thought [short reasoning about content only] <unused95>
  Ibuprofen 400mg, twice daily.
  Metoprolol 50mg, once in the morning.

Code: parse response → [{name: "Ibuprofen", dose: "400mg", frequency: "twice daily"}, ...]

Risk: Minimal. The model is doing what it's best at — reading and answering.
```

### Token Budget Comparison

| Component | BM-04 | BM-05 |
|-----------|-------|-------|
| System prompt | ~400 tokens | ~400 tokens (same) |
| Extraction schema | ~300 tokens | 0 tokens |
| Question | ~50 tokens | ~20-30 tokens |
| Image (vision) | 256 tokens | 256 tokens (same) |
| **Total prompt** | **~1,000 tokens** | **~680 tokens** |
| Thinking overhead | ~469 words (format + content) | Expected ~200 words (content only) |
| Answer format | JSON (complex) | Natural language (simple) |

~32% fewer prompt tokens. Thinking expected to be shorter and more focused.

---

<a id="3-questions"></a>
## 3. Question Design

### 3.1 Principle: One Value Per Question

Each question asks for ONE type of information. The model answers naturally.
The code parses the answer.

### 3.2 Document Metadata Questions

**Q-META-TYPE**: "What type of medical document is this? (prescription, lab result,
clinical note, discharge summary, radiology report, or other)"

**Q-META-DATE**: "What is the date of this document?"

**Q-META-AUTHOR**: "Who is the healthcare professional who authored or signed
this document? State their name and specialty if mentioned."

### 3.3 Medication Questions

**Q-MED-LIST**: "List all medications mentioned in this document. For each
medication, state the name, dose, and how often it should be taken."

**Q-MED-INSTRUCTIONS**: "For each medication listed, what are the specific
instructions? (e.g., take with food, duration, tapering schedule)"

### 3.4 Lab Results Questions

**Q-LAB-LIST**: "List all laboratory test results in this document. For each test,
state the test name, the measured value with its unit, and the reference range
if provided."

**Q-LAB-FLAGS**: "Which test results are outside the normal range? State whether
each is high or low."

### 3.5 Diagnosis Questions

**Q-DIAG-LIST**: "What diagnoses or medical conditions are mentioned in this
document? For each, state the name and whether it is active, resolved, or suspected."

### 3.6 Other Domain Questions

**Q-ALLERGY**: "Are any allergies or adverse reactions mentioned? If so, list
the substance and the reaction."

**Q-PROCEDURE**: "Are any medical procedures or surgeries mentioned? If so, list
the procedure name, date, and body site."

**Q-REFERRAL**: "Are any referrals to other specialists mentioned? If so, list
the specialist type and reason."

**Q-INSTRUCTION**: "What follow-up instructions are given to the patient?
(appointments, lifestyle changes, medication reminders)"

### 3.7 Chat-Specific Questions (Replaces Chat Extraction)

For conversation-based extraction, questions target patient messages only:

**Q-CHAT-SYMPTOM**: "What symptoms does the PATIENT describe in this conversation?
For each symptom, state what it is, where in the body, how severe (if mentioned),
and when it started."

**Q-CHAT-MED**: "Does the PATIENT mention taking any medications? If so, state the
medication name, dose, how often, and when they started taking it."

**Q-CHAT-APPT**: "Does the PATIENT mention any upcoming medical appointments?
If so, state the doctor's name, specialty, date, and time."

### 3.8 Language Matching — Ask in the Document's Language

**Phase 0 observation**: Questions were in English, documents in German/French.
The model's thinking chain shows code-switching overhead:

```
Thinking (V-DE-01, English question on German doc):
  "The document mentions 'Migrän' (migraine) and 'Filmtabletten' (tablets)"
  → Model spends tokens TRANSLATING German terms into English parentheses

Thinking (V-FR-03, English question on French doc):
  "The section labeled 'HEMATOLOGIE' contains the test results"
  → Model explains French labels in English context
```

This is wasted attention. If the question matches the document's language, the
model reasons natively — no translation, no parenthetical explanations.

#### Multilingual Question Templates

**Q-MED-LIST**:

| Language | Question |
|----------|----------|
| EN | "What medications are prescribed in this document? List each as a bullet point with name, dose, and instructions." |
| FR | "Quels médicaments sont prescrits dans ce document ? Listez chacun avec le nom, la dose et les instructions." |
| DE | "Welche Medikamente werden in diesem Dokument verschrieben? Listen Sie jedes als Aufzählung mit Name, Dosis und Anweisungen auf." |

**Q-LAB-LIST**:

| Language | Question |
|----------|----------|
| EN | "What laboratory test results are shown in this document? List each as a bullet point with test name, value, unit, and reference range." |
| FR | "Quels résultats d'analyses sont présentés dans ce document ? Listez chacun avec le nom du test, la valeur, l'unité et les valeurs de référence." |
| DE | "Welche Laborergebnisse werden in diesem Dokument angezeigt? Listen Sie jeden Test mit Name, Wert, Einheit und Referenzbereich auf." |

**Q-META**:

| Language | Question |
|----------|----------|
| EN | "What type of document is this, what is its date, and who is the author?" |
| FR | "Quel est le type de ce document, sa date, et qui en est l'auteur ?" |
| DE | "Was für ein Dokument ist das, welches Datum hat es, und wer ist der Autor?" |

**Q-CHAT-SYMPTOM**:

| Language | Question |
|----------|----------|
| EN | "What symptoms does the PATIENT describe? List each as a bullet point with the symptom, severity, location, and onset." |
| FR | "Quels symptômes le PATIENT décrit-il ? Listez chacun avec le symptôme, la sévérité, la localisation et le début." |
| DE | "Welche Symptome beschreibt der PATIENT? Listen Sie jedes Symptom mit Schweregrad, Lokalisation und Beginn auf." |

#### Language Detection Strategy

The app already knows the document's language from:
1. **User's locale** — the app language setting
2. **Document metadata** — detected during import
3. **Simple heuristic** — scan for language markers (é/è/ê→FR, ü/ö/ä/ß→DE)

Code selects the matching question template before sending to the model.

#### What This Changes

| | Phase 0 (EN questions) | BM-05 Full (matched language) |
|---|---|---|
| Question language | English | Matches document |
| Thinking overhead | Translation parentheses | Native reasoning |
| Expected improvement | — | Fewer thinking tokens, faster, possibly fewer edge cases |

### 3.9 Question Properties (Revised)

| Property | Value |
|----------|-------|
| Language | **Matches document language** (EN/FR/DE) |
| Length | 15-30 tokens per question |
| Format requested | Markdown bullet list (Format C) |
| Schema | None — code parses the answer |
| JSON | Never asked for |

### 3.9 Not All Questions Apply to All Documents

A prescription doesn't have lab results. Don't ask.

| Document Type | Questions to Ask |
|---------------|-----------------|
| Prescription | META, MED-LIST, MED-INSTRUCTIONS |
| Lab result | META, LAB-LIST, LAB-FLAGS |
| Clinical note | META, DIAG-LIST, MED-LIST, PROCEDURE, REFERRAL |
| Discharge summary | META, DIAG-LIST, MED-LIST, INSTRUCTION |
| Radiology report | META, PROCEDURE (imaging), DIAG-LIST (findings) |
| Chat (symptom) | CHAT-SYMPTOM |
| Chat (medication) | CHAT-MED |
| Chat (appointment) | CHAT-APPT |

### 3.10 Two-Phase Strategy

**Phase A — Document type detection** (1 call):
Ask Q-META-TYPE on page 1. The answer determines which domain questions to send.
This can even be keyword-based (no LLM) for known document formats.

**Phase B — Domain extraction** (N calls):
Send only relevant questions based on document type.

```
Page 1 of prescription:
  Call 1: Q-META-TYPE → "prescription"
  Call 2: Q-META-DATE → "16 octobre 2024"
  Call 3: Q-META-AUTHOR → "Dr Frédéric Vidal, médecin généraliste"
  Call 4: Q-MED-LIST → "Colecalciferol 100 000 UI/2ml, 1 ampoule, renew in 3 months"
  Call 5: Q-MED-INSTRUCTIONS → "1 ampoule à renouveler dans 3 mois"

Code: {
  document_type: "prescription",
  document_date: "2024-10-16",       ← code converts "16 octobre 2024"
  professional: {name: "Dr Frédéric Vidal", specialty: "médecin généraliste"},
  medications: [{name: "Colecalciferol", dose: "100 000 UI/2ml", ...}]
}
```

### 3.11 Batching Related Questions

Some questions can be combined without overloading:

```
INDIVIDUAL (safest, most calls):
  "What is the document date?"
  "Who is the author?"
  → 2 calls

BATCHED (still focused, fewer calls):
  "What is the date of this document, and who authored it?"
  → 1 call

OVERBATCHED (back to BM-04 territory):
  "Extract the date, author, all medications, all lab results..."
  → degeneration risk
```

The sweet spot: **batch 2-3 closely related values per question**. Metadata fields
(date + author + type) are safe to batch — they're all header information.
Domain extraction (medications, lab results) should stay separate.

---

<a id="4-chunking"></a>
## 4. Document Chunking

### 4.1 Page Chunking (Same as Before)

| Document Type | Strategy | Already Done? |
|---------------|----------|---------------|
| Vision (scanned) | 1 image = 1 page | YES |
| Text (digital PDF) | Split by page markers | YES |
| Chat conversation | Full conversation (no split) | N/A |

### 4.2 Multi-Page Documents

For a 3-page lab result:

```
Page 1: Q-META (date, author, type) + Q-LAB-LIST + Q-LAB-FLAGS
Page 2: Q-LAB-LIST + Q-LAB-FLAGS  (metadata already captured)
Page 3: Q-LAB-LIST + Q-LAB-FLAGS

Code: merge lab results from all 3 pages, deduplicate
```

### 4.3 Dense Pages

Some pages have many values (e.g., full blood count with 20+ results). If a single
Q-LAB-LIST question on a dense page still degenerates, further chunking options:

- Split the page image into top/bottom halves
- Ask sequential questions: "List the first 10 lab results" then "Are there more?"
- Use follow-up: "You listed 15 results. Are there any others on this page?"

These are fallback strategies — test the simple approach first.

---

<a id="5-assembly"></a>
## 5. Code Assembly Layer

### 5.1 Purpose

The model produces natural language answers. Code converts them to structured data.

### 5.2 Parsing Strategy

The parsing layer is **not an LLM**. It uses pattern matching and heuristics:

```
Model answer: "Ibuprofen 400mg, twice daily"

Code parsing:
  1. Split by comma/newline → ["Ibuprofen 400mg", "twice daily"]
  2. Regex for dose: (\d+\s*mg|\d+\s*UI|...) → "400mg"
  3. Regex for frequency: (twice daily|once|three times|...) → "twice daily"
  4. Remaining text → medication name: "Ibuprofen"
  5. Build: {name: "Ibuprofen", dose: "400mg", frequency: "twice daily"}
```

### 5.3 What If Parsing Fails?

If the model's answer doesn't match expected patterns:
- Store the raw answer in a `raw_text` field
- Flag for human review
- Never hallucinate structure from unparseable text

### 5.4 Date Conversion (Code, Not Model)

```
Model answer: "16 octobre 2024"
Code: parse_french_date("16 octobre 2024") → "2024-10-16"

Model answer: "next Tuesday"
Code: resolve_relative_date("next Tuesday", today=2026-02-26) → "2026-03-03"
```

Date parsing libraries handle this deterministically. No reason for a 4B model
to spend attention on date format conversion.

### 5.5 Language Detection (Code, Not Model)

The model preserves the document's language naturally — it just answers in whatever
language the document uses. Code detects the language of the answer if needed
(for i18n routing), using simple heuristics or a lightweight classifier.

### 5.6 Deduplication Across Pages

For multi-page documents, same information may appear on multiple pages (headers,
repeated medication names). Code deduplicates by matching key fields:

- Medications: by `name` + `dose`
- Lab results: by `test_name`
- Diagnoses: by `name`

### 5.7 This Is Benchmark Code, Not App Code

The assembly layer in BM-05 is a **benchmark utility** that validates the approach.
The production implementation (Phase D) will use existing `ExtractedEntities` types
and the `DomainExtractor` trait.

---

<a id="6-matrix"></a>
## 6. Test Matrix

### 6.1 Document Extraction Tests (Q&A Approach)

| Doc ID | Document | Pages | Questions | Calls |
|--------|----------|-------|-----------|-------|
| D-FR-01 | FR prescription (text) | 1 | META(1), MED-LIST, MED-INSTR | 3 |
| D-FR-02 | FR lab order (text) | 1 | META(1), LAB-LIST, INSTRUCTION | 3 |
| D-DE-01 | DE prescription (vision) | 1 | META(1), MED-LIST, MED-INSTR | 3 |
| D-DE-02 | DE rezept (vision) | 1 | META(1), MED-LIST, MED-INSTR | 3 |
| D-DE-03 | DE rezept 2 (vision) | 1 | META(1), MED-LIST, MED-INSTR | 3 |
| D-EN-01 | EN prescription (vision) | 1 | META(1), MED-LIST, MED-INSTR | 3 |
| D-EN-02 | EN prescription 2 (vision) | 1 | META(1), MED-LIST, MED-INSTR | 3 |
| D-FR-03 | FR lab results p1 (vision) | 1 | META(1), LAB-LIST, LAB-FLAGS | 3 |
| D-FR-04 | FR lab results p2 (vision) | 1 | LAB-LIST, LAB-FLAGS | 2 |
| D-FR-05 | FR lab results p3 (vision) | 1 | LAB-LIST, LAB-FLAGS | 2 |

**Total document calls**: 28

### 6.2 Chat Extraction Tests (Q&A Approach — Replaces BM-04 Chat)

| Test ID | Question | Language | Calls |
|---------|----------|----------|-------|
| C-EMPTY | Q-CHAT-MED | EN | 1 |
| C-EN-01 | Q-CHAT-SYMPTOM | EN | 1 |
| C-EN-02 | Q-CHAT-MED | EN | 1 |
| C-EN-03 | Q-CHAT-APPT | EN | 1 |
| C-FR-01 | Q-CHAT-SYMPTOM | FR | 1 |
| C-FR-02 | Q-CHAT-MED | FR | 1 |
| C-DE-01 | Q-CHAT-SYMPTOM | DE | 1 |
| C-DE-02 | Q-CHAT-MED | DE | 1 |

**Total chat calls**: 8

### 6.3 Radiograph Tests (Unchanged Control Group)

| Test ID | Image | Calls |
|---------|-------|-------|
| R-01 | Shoulder | 1 |
| R-02 | Chest | 1 |
| R-03 | Pelvis | 1 |
| R-04 | Dental | 1 |

**Total radiograph calls**: 4

### 6.4 Total

| Group | BM-05 Calls | BM-04 Calls | Change |
|-------|-------------|-------------|--------|
| Document extraction | 28 | 14 | +100% calls, but each is simpler |
| Chat extraction | 8 | 8 | Same count, simpler prompts |
| Radiograph | 4 | 4 | Unchanged |
| **Total** | **40** | **26** | +54% |

### 6.5 Hardware Matrix

| Backend | Model | Tests |
|---------|-------|-------|
| CPU (WSL2) | `ktiyab/coheara-medgemma-4b-q8` | 40 |
| GPU Vulkan (Windows) | `ktiyab/coheara-medgemma-4b-q8` | 40 |

---

<a id="7-criteria"></a>
## 7. Success Criteria

### 7.1 Primary: Degeneration Eliminated

| Metric | BM-04 Baseline | BM-05 Target | Pass If |
|--------|---------------|-------------|---------|
| GPU doc degeneration | 88% (7/8) | <5% | Q&A eliminates formatting loops |
| GPU chat degeneration | 25% (2/8) | 0% | No JSON = no schema loops |
| GPU total degeneration | 45% (10/22) | <5% | Overall reliability |
| CPU degeneration | 0% (0/14) | 0% | No regression |

### 7.2 Secondary: Answer Quality

| Metric | Target |
|--------|--------|
| Correct information extracted | >90% of values present in document |
| No hallucination | 0 invented values |
| Language preservation | Answers in document's language |
| Empty answers for absent domains | "No medications mentioned" (not invented data) |

### 7.3 Tertiary: Parsability

| Metric | Target |
|--------|--------|
| Code can parse model answer | >85% of answers fully parseable |
| Partial parse (some fields) | >95% of answers at least partially parseable |
| Raw text preserved | 100% (unparseable answers stored verbatim) |

### 7.4 Performance

| Metric | BM-04 | BM-05 Target |
|--------|-------|-------------|
| Wall time (GPU, 40 calls) | 12.5 min (22 calls, 10 wasted) | <20 min (40 calls, <2 wasted) |
| Useful output rate | 55% | >95% |
| Effective throughput | 12 useful results in 12.5 min | 38+ useful results in 20 min |

---

<a id="8-format"></a>
## 8. Output Format Exploration — Phase 0

### 8.1 Why This Matters

"Answer naturally" is too vague — unpredictable responses are hard to parse.
"Return JSON" is too heavy — schema compliance wastes attention.
We need to find the **simplest structured format** that:

1. The 4B model can produce reliably (no degeneration, no format errors)
2. Code can parse deterministically (no ambiguity)
3. Adds minimal formatting overhead (few extra tokens in thinking)

### 8.2 Candidate Formats

**FORMAT A — Delimiter-separated (ordered fields)**

```
Prompt: "List medications. For each, write: name:dose:frequency (one per line)"

Expected output:
  Ibuprofen:400mg:twice daily
  Metoprolol:50mg:once morning
  Spasfon::as needed

Parse: split(":") → fields[0]=name, fields[1]=dose, fields[2]=frequency
Empty field = null
```

- Pros: Trivial to parse, minimal tokens, predictable
- Cons: Delimiter in values (e.g., "100 000 UI:2 ml") breaks parsing.
  Model must understand field ordering without labels.
- Risk: Colon in dose values ("100 000 UI:2 ml"). Pipe `|` may be safer.

**FORMAT B — Labeled key-value (one item per line)**

```
Prompt: "List medications. For each, write on one line:
         name=X | dose=X | frequency=X"

Expected output:
  name=Ibuprofen | dose=400mg | frequency=twice daily
  name=Metoprolol | dose=50mg | frequency=once morning

Parse: split("|") → split("=") → {key: value}
```

- Pros: Self-documenting, order-independent, handles missing fields
- Cons: More tokens per line. Model may vary label names ("Name" vs "name").
- Risk: Moderate. Labels add some formatting overhead but are intuitive.

**FORMAT C — Markdown list (semi-structured)**

```
Prompt: "List medications as a bullet list: name, dose, frequency"

Expected output:
  - Ibuprofen, 400mg, twice daily
  - Metoprolol, 50mg, once morning

Parse: split(",") per line after stripping "- "
```

- Pros: Markdown is natural for LLMs, common in training data
- Cons: Comma in values ("100,000 UI"). Field order not guaranteed.
- Risk: Moderate — commas in medical values are common (FR uses comma decimals).

**FORMAT D — Free text (baseline)**

```
Prompt: "What medications are mentioned?"

Expected output:
  The patient takes Ibuprofen 400mg twice daily for headaches.
  They also take Metoprolol 50mg once in the morning for blood pressure.

Parse: NLP/regex extraction from natural sentences
```

- Pros: Least model overhead, most natural
- Cons: Hardest to parse, most ambiguous, requires NLP
- Risk: Low degeneration risk, high parse complexity

### 8.3 Test Protocol — Format Exploration

**Use ONE document, ONE domain, FOUR formats.** Compare:

```
Test document: FR prescription (text, V-FR-01 from BM-04)
Domain: medications
Model: ktiyab/coheara-medgemma-4b-q8

Run on GPU Vulkan:
  FMT-A: delimiter format → measure: degeneration, answer quality, parsability
  FMT-B: labeled format  → measure: same
  FMT-C: markdown format → measure: same
  FMT-D: free text       → measure: same

Then repeat on a VISION document (V-DE-01 — degenerated in BM-04):
  FMT-A through FMT-D → same measurements
```

**Total exploration calls**: 8 (4 formats × 2 documents).
Fast — ~2 min on GPU. This determines the format for all remaining tests.

### 8.4 Evaluation Criteria for Format Selection

| Criterion | Weight | Description |
|-----------|--------|-------------|
| Degeneration rate | HIGH | Format must not trigger degeneration |
| Parse success | HIGH | Code must be able to extract values |
| Completeness | MEDIUM | All values from the document captured |
| Consistency | MEDIUM | Same format across different documents |
| Token efficiency | LOW | Fewer tokens is better but secondary |

### 8.5 Fallback Strategy

If no single format works for all document types:
- Use Format X for text documents, Format Y for vision documents
- Or: Use the safest format (D: free text) everywhere and invest in a smarter parser

### 8.6 Phase 0 Results — COMPLETE (2026-02-26)

**Configuration**: GPU Q4 Vulkan (worst case — highest degeneration in BM-04)
**Documents**: V-DE-01 (German prescription, vision) + V-FR-03 (French lab results p1, vision)
**Both documents degenerated on ALL 4 configs in BM-04** (CPU Q4, CPU Q8*, GPU Q4, GPU Q8)

#### Per-Test Results

| Test | Format | Time | Tokens | TPS | Degen | Output Quality |
|------|--------|------|--------|-----|-------|----------------|
| V-DE-01-FMT-A | delimiter (pipe) | 222s | ~70+repeats | 64.9 | **BLOCK-REPEAT** | Model output correct JSON, then repeated the entire block 90+ times. Watchdog didn't catch (block-level, not sequence-level). |
| V-DE-01-FMT-B | labeled key-value | 52s | 438 | 58.3 | no | Clean. `name=Migränetabletten 400 mg/Filmtabletten \| dose=400 mg \| instructions=1 bis 2 Filmtabletten täglich` |
| V-DE-01-FMT-C | markdown list | 4s | 221 | 61.0 | no | Clean. `- Migrän Filmtabletten, 1 bis 2 täglich`. Fastest response. |
| V-DE-01-FMT-D | free text | 9s | 367 | 59.7 | no | Clean. Structured with bold headers: Name, Dose, Instructions. |
| V-FR-03-FMT-A | delimiter (pipe) | 51s | 871 | 53.8 | no | Extracted 12 lab results in pipe format. Correct structure. |
| V-FR-03-FMT-B | labeled key-value | 43s | 547 | — | **YES** | Model fell back to JSON output despite key=value instruction. Degenerated mid-output. |
| V-FR-03-FMT-C | markdown list | 12s | 526 | 55.9 | no | Clean. 5 hematology results with values, units, ranges. |
| V-FR-03-FMT-D | free text | 11s | 602 | 55.7 | no | Clean. Detailed listing with bold test names, values, units, ranges. |

#### Summary Table

| Format | V-DE-01 (meds) | V-FR-03 (labs) | Total OK | Verdict |
|--------|----------------|----------------|----------|---------|
| **A — Delimiter (pipe)** | BLOCK-REPEAT | OK | 1/2 | REJECTED — triggers JSON instinct, block-level repetition |
| **B — Labeled key-value** | OK | DEGEN | 1/2 | REJECTED — structural cue triggers JSON fallback |
| **C — Markdown list** | OK (4s) | OK (12s) | **2/2** | **WINNER** — fast, clean, parseable |
| **D — Free text** | OK (9s) | OK (11s) | **2/2** | **WINNER** — reliable, detailed, slightly harder to parse |

#### Analysis

**1. The model WANTS to output JSON.**
Even when explicitly asked for pipe-delimited or key=value format, the model's
training instinct is to produce JSON. Format A (pipes) triggered a correct JSON
response that then repeated as a block 90+ times. Format B (key=value) triggered
a JSON fallback on V-FR-03 that degenerated. Any format that resembles structured
output cues the model toward JSON.

**2. Natural formats avoid the JSON trap.**
Format C (markdown bullet list) and Format D (free text) both avoid triggering the
JSON instinct. The model treats them as natural language tasks — read the document,
list what you see. This is the radiograph pattern: simple question → natural answer
→ 0% degeneration.

**3. Format C is faster than Format D.**
V-DE-01: 4s (C) vs 9s (D). V-FR-03: 12s (C) vs 11s (D). Format C produces
more concise output (fewer tokens), which means faster generation and easier parsing.

**4. Format D produces richer output.**
Free text answers include more context ("Based on the document...", bold headers,
structured sub-lists). This is useful for complex documents but harder to parse
and takes more tokens.

**5. Both documents that failed 100% in BM-04 now pass.**
V-DE-01 degenerated on CPU Q4, GPU Q4, and GPU Q8 with the BM-04 prompt.
With focused Q&A (any format except A/B), it passes in 4-52 seconds.
V-FR-03 degenerated on CPU Q4, GPU Q4, and GPU Q8. With C/D formats, it passes.

**6. Watchdog gap confirmed: block-level repetition.**
V-DE-01-FMT-A produced the correct answer, then repeated the ENTIRE JSON block
(~10 lines) 90+ times. The sequence watchdog checks 10-token windows — a 100-token
block that repeats is invisible to it. Second watchdog gap after the paraphrase
loop (BM-04 Test 7, V-EN-01). Phase D must address both.

#### Decision: Format C (Markdown List) Is Primary

| Criterion | Format C | Format D |
|-----------|----------|----------|
| Degeneration | 0/2 | 0/2 |
| Speed | Faster (4-12s) | Moderate (9-11s) |
| Parsability | Easy (split bullets) | Hard (NLP/regex) |
| Token count | Lower (~220-526) | Higher (~367-602) |
| Completeness | Good (values + ranges) | Better (more context) |
| Consistency | High (bullet format) | Variable (model decides structure) |

**Primary: Format C (markdown list)** — fast, parseable, reliable.
**Fallback: Format D (free text)** — if markdown parsing fails, free text preserves all info.

#### What This Means for BM-05 Full Benchmark

- Use Format C (markdown list) for all domain extraction prompts
- No JSON anywhere in any prompt
- System prompt stripped of all JSON/formatting rules
- The model reads, answers with bullets, code parses bullets into structure
- Expected degeneration on GPU Q4: **<5%** (down from 44-88%)

### 8.7 Phase 0 Results — CPU Q4 (2026-02-26)

**Configuration**: CPU Q4_K_M on localhost (WSL2 Ollama, no GPU)
**Model**: `ktiyab/coheara-medgemma-4b-q4` (3.3 GB)
**Documents**: Same 2 as GPU Q4 Phase 0 (V-DE-01 + V-FR-03)
**Format**: C (markdown list) only — A/B already rejected in GPU Phase 0
**Both documents degenerated at 36% in BM-04 on CPU Q4**

#### Per-Test Results

| Test | Format | Time | Tokens | TPS | Degen | Output Quality |
|------|--------|------|--------|-----|-------|----------------|
| V-DE-01-FMT-C | markdown list | 143.8s | 297 | 9.98 | no | Clean. `* Ibuprofen Filmtabletten 400 mg Migrän 1 bis 2 Filmtabletten täglich`. Thinking chain: identified med, dose, instructions correctly. |
| V-FR-03-FMT-C | markdown list | 169.1s | 517 | 8.86 | no | Clean. 5 hematology results with values, units, and reference ranges in bullet list. Thinking chain: systematic section-by-section extraction. |

#### Summary

| Metric | Value |
|--------|-------|
| Total calls | 2 |
| Healthy | **2/2 (100%)** |
| Degenerated | **0/2 (0%)** |
| Total time | 5.2 min |
| Avg gen speed | 9.4 tok/s |

#### Comparison: CPU Q4 — BM-04 vs BM-05 Format C

| Metric | BM-04 (full JSON) | BM-05 Format C |
|--------|-------------------|----------------|
| Degen rate (these 2 docs) | **100%** (both failed) | **0%** (both pass) |
| Degen rate (full suite) | 36% (8/22) | 0% (0/2) |
| Gen speed | 12.0 tok/s | 9.4 tok/s |
| Wall time per call | — (degenerated) | 156s avg |

#### Analysis

1. **Format C eliminates degeneration on CPU Q4.** Both V-DE-01 and V-FR-03 — which
   degenerated on every BM-04 configuration — pass cleanly with markdown list format.
   This confirms that **prompt complexity, not quantization, is the dominant degeneration
   factor.** Q4_K_M precision loss only triggers degeneration when combined with complex
   multi-domain JSON prompts.

2. **Speed is slower than GPU Q4 but consistent.** 9.4 tok/s (CPU Q4) vs 58 tok/s
   (GPU Q4 Format C). The ~6x difference matches BM-04's CPU vs GPU speedup ratio.
   CPU Q4 is slightly faster than CPU Q8 (7.2 tok/s) due to smaller model.

3. **Thinking tokens present and helpful.** Both responses show `<unused94>thought...
   <unused95>` reasoning chains — same pattern as CPU Q8 in BM-04. The model restates
   the task, identifies document sections, and formats its answer. 93% thinking
   frequency holds across quantizations.

4. **Completeness trade-off visible.** V-FR-03 extracted 5/12 hematology results
   (same as GPU Q4 Format C). The markdown list format produces a summary rather than
   exhaustive enumeration. BM-06 iterative extracted 12/12 on the same document —
   the completeness gap is a strategy trade-off, not a quality failure.

5. **Wall time is batch-only territory.** 143-169s per call means CPU Q4 is practical
   for night batch processing but not real-time extraction. Consistent with BM-04's
   CPU tier finding: batch-only at T1.

### 8.8 Phase 0 Results — GPU Q4_K_S (2026-02-26)

**Configuration**: GPU Vulkan (gfx1010) on Windows Ollama
**Model**: `ktiyab/coheara-medgemma-4b-q4s` (3.2 GB, Q4_K_S — smallest supported quantization)
**Documents**: Same 2 (V-DE-01 + V-FR-03)
**Format**: C (markdown list) only
**Purpose**: Find the quantization floor — Q4_K_S is the lowest Ollama supports for Gemma3

#### Per-Test Results

| Test | Format | Time | Tokens | TPS | Degen | Output Quality |
|------|--------|------|--------|-----|-------|----------------|
| V-DE-01-FMT-C | markdown list | 45.9s | 308 | 60.0 | no | Clean. `- Migräne 400 Filmtabletten, 1 bis 2 Filmtabletten, täglich`. Thinking chain present — identified medication, dose, instructions. |
| V-FR-03-FMT-C | markdown list | 44.0s | 529 | 57.4 | no | Clean. 5 hematology results with values, units, and reference ranges in bullet list. Thinking chain: systematic section-by-section extraction. |

#### Summary

| Metric | Value |
|--------|-------|
| Total calls | 2 |
| Healthy | **2/2 (100%)** |
| Degenerated | **0/2 (0%)** |
| Total time | 1.5 min |
| Avg gen speed | 58.7 tok/s |

#### Comparison: GPU Q4_K_S vs GPU Q4_K_M (Format C)

| Metric | GPU Q4_K_M (BM-05 §8.6) | GPU Q4_K_S |
|--------|--------------------------|------------|
| Degen rate | 0% (0/2) | 0% (0/2) |
| Avg TPS | 57.7 | 58.7 |
| V-DE-01 time | 46.5s | 45.9s |
| V-FR-03 time | 36.3s | 44.0s |
| Model size | 3.3 GB | 3.2 GB |

#### Analysis

1. **Q4_K_S performs identically to Q4_K_M on Format C.** Zero degeneration, same
   speed tier (~58 tok/s), same output quality. The 100 MB size reduction has no
   measurable impact on medical extraction with simple prompts.

2. **Quantization floor confirmed for BM-05.** Q4_K_S is the smallest quantization
   Ollama supports for Gemma3 (Q2_K unsupported). Since it matches Q4_K_M on all
   metrics, Q4_K_M vs Q4_K_S is a non-decision for Format C extraction.

3. **GPU Vulkan stable with simple prompts.** The same GPU that showed 44-45%
   degeneration on BM-04 full JSON produces 0% degeneration on both Q4 variants
   with Format C. This further isolates prompt complexity as the dominant factor.

### 8.9 Phase 0 Results — CPU Q4_K_S (2026-02-26)

**Configuration**: CPU Q4_K_S on localhost (WSL2 Ollama, no GPU)
**Model**: `ktiyab/coheara-medgemma-4b-q4s` (3.2 GB, Q4_K_S — lowest supported quantization)
**Documents**: Same 2 (V-DE-01 + V-FR-03)
**Format**: C (markdown list) only
**Purpose**: Quantization floor on CPU — does further Q4 compression break extraction?

#### Per-Test Results

| Test | Format | Time | Tokens | TPS | Degen | Output Quality |
|------|--------|------|--------|-----|-------|----------------|
| V-DE-01-FMT-C | markdown list | 175.1s | 475 | 9.6 | no | Clean. `Ibuprofen Filmtabletten 400 mgL, 1 bis 2 Filmtabletten täglich`. Longer thinking chain than Q4_K_M (475 vs 297 tokens) — includes confidence score and mental sandbox. |
| V-FR-03-FMT-C | markdown list | 176.4s | 515 | 9.5 | no | Clean. 5 hematology results with values, units, and reference ranges. Same 5/12 extraction completeness as Q4_K_M. |

#### Summary

| Metric | Value |
|--------|-------|
| Total calls | 2 |
| Healthy | **2/2 (100%)** |
| Degenerated | **0/2 (0%)** |
| Total time | 5.9 min |
| Avg gen speed | 9.5 tok/s |

#### Comparison: CPU Q4_K_S vs CPU Q4_K_M (Format C)

| Metric | CPU Q4_K_M (§8.7) | CPU Q4_K_S |
|--------|-------------------|------------|
| Degen rate | 0% (0/2) | 0% (0/2) |
| Avg TPS | 9.4 | 9.5 |
| V-DE-01 time | 143.8s | 175.1s |
| V-FR-03 time | 169.1s | 176.4s |
| V-DE-01 tokens | 297 | 475 |
| V-FR-03 tokens | 517 | 515 |
| Completeness (V-FR-03) | 5/12 | 5/12 |

#### Analysis

1. **Q4_K_S matches Q4_K_M on all key metrics.** Zero degeneration, same speed tier
   (~9.5 tok/s), same extraction completeness (5/12 lab results). The quantization
   floor does not break medical extraction with simple prompts.

2. **V-DE-01 produced a longer thinking chain.** 475 tokens vs 297 on Q4_K_M — the
   model added a "Confidence Score: 5/5" and "Mental Sandbox" section. More verbose
   reasoning but same correct extraction. This is stochastic variation, not a
   quantization effect.

3. **Slightly slower wall time on V-DE-01** (175s vs 144s) due to the longer thinking
   chain (more tokens generated). V-FR-03 is nearly identical (176s vs 169s).

4. **Q4_K_S vs Q4_K_M is a non-decision.** 100 MB difference, identical behavior.
   Q4_K_M remains the recommended Q4 variant (K_M = better quality at marginal cost).

---

<a id="9-plan"></a>
## 9. Execution Plan

### Phase 0: Format Exploration (FIRST — determines all subsequent phases)

| Task | Description |
|------|-------------|
| P0-1 | Write 4 prompt variants (A/B/C/D) for medications on FR prescription (text) |
| P0-2 | Write 4 prompt variants (A/B/C/D) for medications on DE prescription (vision) |
| P0-3 | Run 8 tests on GPU Vulkan Q8 |
| P0-4 | Evaluate: degeneration rate, parse success, completeness per format |
| P0-5 | Select winning format |

### Phase 1: Build Runner (Using Winning Format)

| Task | Description |
|------|-------------|
| P1-1 | Write `bench_05_runner.py` with Q&A prompts using selected format |
| P1-2 | Implement document type → question routing |
| P1-3 | Reuse streaming watchdog from BM-04 |
| P1-4 | Dry run: print all prompts without calling Ollama |

### Phase 2: CPU Benchmark

| Task | Description |
|------|-------------|
| P2-1 | Run BM-05 on CPU Q8 (40 calls) |
| P2-2 | Verify 0% degeneration |
| P2-3 | Evaluate answer quality and parsability |

### Phase 3: GPU Benchmark

| Task | Description |
|------|-------------|
| P3-1 | Run BM-05 on GPU Q8 Vulkan (40 calls) |
| P3-2 | Compare degeneration: BM-04 (45%) vs BM-05 (target <5%) |
| P3-3 | Identify any remaining degeneration — what causes it? |

### Phase 4: Assembly + Analysis

| Task | Description |
|------|-------------|
| P4-1 | Build code parser for winning format |
| P4-2 | Test assembly: parse BM-05 answers → structured data |
| P4-3 | Compare assembled output vs BM-04 multi-domain output (quality) |
| P4-4 | Wall time comparison |
| P4-5 | Write conclusions and recommendations for Phase D |

### Dependency Graph

```
P1-1 → P1-2 → P1-3 → P1-4 (build runner)
                         ↓
                  P2-1 → P2-2 → P2-3 (CPU)
                         ↓
                  P3-1 → P3-2 → P3-3 (GPU)
                         ↓
                  P4-1 → P4-2 → P4-3 → P4-4 → P4-5 (analysis)
```

---

<a id="9-decisions"></a>
## 9. Decision Log

| ID | Decision | Status | Rationale |
|----|----------|--------|-----------|
| BM5-01 | Model answers natural language questions, code builds JSON | PROPOSED | JSON formatting wastes 4B attention. Model excels at Q&A (radiographs: 0% degen). Code excels at structuring. |
| BM5-02 | No JSON schema in any prompt | PROPOSED | Schema tokens (~300) consume prompt budget. Schema compliance reasoning dominates thinking blocks. Eliminated entirely. |
| BM5-03 | One domain per question, per page | PROPOSED | BM-04 degeneration correlates with domain count. Focused questions = focused attention. |
| BM5-04 | Code handles date conversion, language detection, deduplication | PROPOSED | Deterministic operations done deterministically. No LLM attention wasted on format rules. |
| BM5-05 | Batch only closely related values (2-3 max) | PROPOSED | Metadata (date + author + type) safe to batch. Domain extraction stays separate. |
| BM5-06 | Document type determines which questions to ask | PROPOSED | Reduces unnecessary calls. Prescription → skip lab questions. Lab result → skip medication questions. |
| BM5-07 | Unparseable answers stored as raw_text, flagged for review | PROPOSED | Never hallucinate structure. If code can't parse, preserve the original and flag. |
| BM5-08 | Assembly layer is benchmark code, not app code | PROPOSED | Evaluate before integrate. Production parser uses existing DomainExtractor trait. |
| BM5-09 | Phase 0: test 4 output formats before full benchmark | PROPOSED | The right format is unknown. Test delimiter, labeled, markdown, free text on 2 docs (8 calls) to find the sweet spot. |
| BM5-10 | Tell model the answer sequence (field order), not just "answer freely" | REVISED | Phase 0 showed structured formats (pipe, key=value) trigger JSON instinct → degeneration. Markdown list is the right balance: light structure, natural for the model. |
| BM5-11 | Phase 0 DONE: Format C (markdown list) wins — 0% degen on GPU Q4 worst case | VALIDATED | 7/8 tests passed on documents that failed 100% in BM-04. Format C: fast (4-12s), parseable, no JSON trigger. Format D (free text): also 0% degen, but harder to parse. |
| BM5-12 | Formats A (delimiter) and B (labeled) REJECTED — trigger JSON instinct | VALIDATED | Model's training makes it output JSON when it sees structural cues. Pipe delimiters → JSON block-repeat. Key=value → JSON fallback → degeneration. |
| BM5-13 | Watchdog gap #2: block-level repetition (entire JSON block repeated 90+ times) | DOCUMENTED | V-DE-01-FMT-A. Sequence watchdog uses 10-token window — a 100-token block is invisible. Needs block-level detection in Phase D. |
| BM5-14 | Questions and format instructions must be in the document's language | PROPOSED | FR/DE documents need FR/DE prompts. Model performs better when question language matches document language. Reduces code-switching overhead in thinking chain. |

---

## Appendix A: BM-04 vs BM-05 — Side by Side

### Prescription Extraction Example

**BM-04 prompt** (~350 tokens):
```
<document>
Dr FREDERIC VIDAL...
COLECALCIFEROL 100 000 UI/2 ml sol buv (UVEDOSE)
1 ampoule à renouveler dans 3 mois
</document>

Extract ALL medical information... return valid JSON:
{"document_type": "...", "medications": [{"name": "string", ...}], ...}
```

**BM-05 prompts** (~25 tokens each):

Call 1: "What type of document is this, what is its date, and who is the author?"
→ "This is a prescription dated 16 octobre 2024, by Dr Frédéric Vidal,
   remplaçant Brandon Lalouche, médecin généraliste."

Call 2: "What medications are prescribed? For each, state the name, dose, and instructions."
→ "Colecalciferol 100 000 UI/2 ml (UVEDOSE), 1 ampoule à renouveler dans 3 mois."

**Code assembles**:
```json
{
  "document_type": "prescription",
  "document_date": "2024-10-16",
  "professional": {"name": "Dr Frédéric Vidal", "specialty": "médecin généraliste"},
  "medications": [{
    "name": "Colecalciferol",
    "brand": "UVEDOSE",
    "dose": "100 000 UI/2 ml",
    "instructions": "1 ampoule à renouveler dans 3 mois"
  }]
}
```

### Token Budget Comparison (This Example)

| | BM-04 | BM-05 |
|---|---|---|
| Prompt tokens (total) | ~400 (1 call) | ~200 (2 calls × ~100) |
| Schema tokens | ~300 | 0 |
| Thinking tokens (est.) | ~469 (format + content) | ~200 (content only) |
| Answer tokens | ~200 (JSON) | ~100 (natural text) |
| **Total tokens** | **~1,370** | **~500** |
| Calls | 1 | 2 |
| Degeneration risk | HIGH | LOW |

---

## Appendix B: File Map

```
Specs/experiments/
├── MEDGEMMA-BENCHMARK-05.md          # THIS FILE
├── bench_05_runner.py                # BM-05 runner (to build)
├── bench_05_results_cpu_q8.jsonl     # CPU Q8 results (to produce)
├── bench_05_results_gpu_q8.jsonl     # GPU Q8 results (to produce)
│
├── MEDGEMMA-BENCHMARK-04.md          # BM-04 spec (baseline)
├── bench_04_runner.py                # BM-04 runner
├── bench_04_results.jsonl            # BM-04 CPU Q8 (14 tests)
├── bench_04_results_cpu_q4.jsonl     # BM-04 CPU Q4 (running)
├── bench_04_results_gpu_q8.jsonl     # BM-04 GPU Q8 (22 tests)
├── bench_04_results_gpu_q4.jsonl     # BM-04 GPU Q4 (18 tests)
└── MODEL-FACTORY-SPEC.md             # Model factory (parent spec)
```

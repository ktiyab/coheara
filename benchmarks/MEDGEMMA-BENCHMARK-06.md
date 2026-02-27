# MEDGEMMA-BENCHMARK-06: Iterative Single-Value Extraction

> **Purpose**: Test whether iterative single-value questions (one answer per call)
> eliminate degeneration entirely and map the 4B model's minimum attention unit.
>
> **Date**: 2026-02-26 | **Status**: DRAFT
> **Depends on**: `MEDGEMMA-BENCHMARK-04.md` (baseline), `MEDGEMMA-BENCHMARK-05.md` (format exploration)
> **Hypothesis**: The simplest possible question produces the most reliable answer.
> One value per call = zero formatting overhead = zero degeneration.

---

## Table of Contents

| # | Section | Line |
|---|---------|------|
| 0 | [Position in the Granularity Spectrum](#0-spectrum) | 25 |
| 1 | [Strategy — Iterative Drilling](#1-strategy) | 80 |
| 2 | [Question Chains by Domain](#2-chains) | 140 |
| 3 | [Call Flow Examples](#3-examples) | 270 |
| 4 | [Trade-offs vs BM-05](#4-tradeoffs) | 370 |
| 5 | [Test Matrix](#5-matrix) | 420 |
| 6 | [Success Criteria](#6-criteria) | 490 |
| 7 | [Execution Plan](#7-plan) | 530 |
| 8 | [Decision Log](#8-decisions) | 580 |

---

<a id="0-spectrum"></a>
## 0. Position in the Granularity Spectrum

### The Attention Capacity Curve

BM-04, BM-05, and BM-06 test three points on the same axis — prompt complexity:

```
COMPLEXITY ──────────────────────────────────────────────► HIGH

BM-06              BM-05                 BM-04
ONE VALUE           ONE DOMAIN            ALL DOMAINS
per call            per call              per call
│                   │                     │
│  "What is the     │  "List meds:        │  "Extract all as JSON:
│   dose of          │   name:dose:freq"   │   {medications: [...],
│   Ibuprofen?"      │                     │    lab_results: [...], ...}"
│                   │                     │
│  Simplest         │  Balanced           │  Most complex
│  Most calls       │  Moderate calls     │  Fewest calls
│  Zero format      │  Minimal format     │  Heavy format (JSON)
│  Expected: 0%     │  Expected: <5%      │  Measured: 45% degen
│  degeneration     │  degeneration       │  on GPU
▼                   ▼                     ▼
```

### Why We Need All Three

Testing only BM-05 tells us if single-domain prompts work. But if BM-05 still
degenerates on some inputs, we don't know if the issue is:
- (a) Multiple values per answer (BM-06 would fix this)
- (b) The document itself (nothing fixes this — model limitation)

BM-06 establishes the **floor** — the absolute minimum complexity. If even
single-value questions degenerate, the problem is in the model or compute path,
not in prompt design.

### Combined Insight

```
If BM-06 = 0% degen AND BM-05 = 0% degen  → Use BM-05 (fewer calls, same reliability)
If BM-06 = 0% degen AND BM-05 > 0% degen  → Multi-value is still too much, use BM-06
If BM-06 > 0% degen                        → Problem is deeper than prompt design
```

---

<a id="1-strategy"></a>
## 1. Strategy — Iterative Drilling

### Core Idea

Split extraction into two phases:

```
PHASE A — ENUMERATE (one call):
  "What medication names are mentioned in this document?"
  → Model lists names only: "Ibuprofen, Metoprolol, Spasfon"

PHASE B — DRILL (one call per name per field):
  "What is the dose of Ibuprofen in this document?"    → "400mg"
  "How often should Ibuprofen be taken?"               → "twice daily"
  "What is the dose of Metoprolol in this document?"   → "50mg"
  "How often should Metoprolol be taken?"              → "once in the morning"
  ...
```

### Why This Is Different From BM-05

| | BM-05 | BM-06 |
|---|---|---|
| Per call | "List meds with name, dose, freq" | "What is the dose of X?" |
| Model must | List N items × M fields | Answer 1 question about 1 item |
| Attention | Spread across N items | Focused on 1 item |
| Output | Multi-line structured | Single value (word or phrase) |
| Parse | Split lines, split fields | Direct — the answer IS the value |
| Calls | 1 per domain | 1 + (N items × M fields) per domain |

### The Parse Problem Disappears

In BM-05, the code must parse "Ibuprofen:400mg:twice daily" — handle delimiters,
empty fields, values containing delimiters.

In BM-06, the model answers "400mg" to "what is the dose?" There's nothing to parse.
The answer IS the value. Code just stores it.

### When Iteration Helps Most

- **Dense documents** (lab results with 20+ values) — BM-05 might ask for all at
  once and degenerate. BM-06 asks one at a time.
- **Vision documents** — The model must OCR + extract. Doing both for one value is
  simpler than doing both for many values.
- **Non-English** — The thinking chain in FR/DE is already longer. Adding multi-value
  formatting makes it worse.

---

<a id="2-chains"></a>
## 2. Question Chains by Domain

### 2.1 Medications

```
ENUMERATE:
  Q: "What medication names are mentioned in this document? List only the names."
  A: "Ibuprofen, Metoprolol, Spasfon"

DRILL per medication:
  Q: "What is the prescribed dose of {name}?"        → "400mg"
  Q: "How often should {name} be taken?"              → "twice daily"
  Q: "What is the route of administration for {name}?" → "oral"
  Q: "Why was {name} prescribed?"                     → "headaches"
  Q: "Are there special instructions for {name}?"     → "take with food"
```

**Calls**: 1 + (N × 5) where N = number of medications
**Example**: 3 medications → 1 + 15 = 16 calls

### 2.2 Lab Results

```
ENUMERATE:
  Q: "What laboratory tests are listed in this document? List only the test names."
  A: "NFS, Glycémie à jeun, HDL, LDL, Triglycerides, HbA1c"

DRILL per test:
  Q: "What is the measured value for {test}?"        → "5.2 mmol/L"
  Q: "What is the reference range for {test}?"       → "3.9-6.1 mmol/L"
  Q: "Is the result for {test} normal, high, or low?" → "normal"
```

**Calls**: 1 + (N × 3)
**Example**: 15 lab tests → 1 + 45 = 46 calls

### 2.3 Document Metadata

```
  Q: "What type of medical document is this?"                    → "prescription"
  Q: "What is the date of this document?"                        → "16 octobre 2024"
  Q: "Who is the healthcare professional who signed this?"       → "Dr Frédéric Vidal"
  Q: "What is their medical specialty?"                          → "médecin généraliste"
  Q: "What institution or practice is this from?"                → "3 Avenue d'Argenteuil, Asnières"
```

**Calls**: 5 (fixed, no enumeration needed)

### 2.4 Chat — Symptoms

```
ENUMERATE:
  Q: "What symptoms does the PATIENT describe? List only the symptom names."
  A: "headache, dizziness"

DRILL per symptom:
  Q: "How severe is the {symptom} the patient describes?"        → "6 out of 10"
  Q: "When did the {symptom} start?"                             → "3 days ago"
  Q: "Where in the body is the {symptom}?"                       → "right side of the head"
  Q: "What makes the {symptom} worse?"                           → "looking at screens"
  Q: "What relieves the {symptom}?"                              → "not mentioned"
```

**Calls**: 1 + (N × 5)
**Example**: 2 symptoms → 1 + 10 = 11 calls

### 2.5 Chat — Medications

```
ENUMERATE:
  Q: "Does the PATIENT mention taking any medications? List only the names."
  A: "Ibuprofen"

DRILL per medication:
  Q: "What dose of {name} does the patient take?"               → "400mg"
  Q: "How often does the patient take {name}?"                   → "twice a day"
  Q: "When did the patient start taking {name}?"                 → "yesterday"
```

**Calls**: 1 + (N × 3)

### 2.6 Chat — Appointments

```
ENUMERATE:
  Q: "Does the PATIENT mention any upcoming appointments? List the doctor names."
  A: "Dr. Martin"

DRILL per appointment:
  Q: "What is Dr. Martin's specialty?"                           → "neurologist"
  Q: "What date is the appointment with Dr. Martin?"             → "next Tuesday"
  Q: "What time is the appointment?"                             → "2pm"
```

**Calls**: 1 + (N × 3)

### 2.7 Optional Shortcut: Skip Drill When Enumerate Is Sufficient

If the enumerate answer already contains all needed info, skip drilling:

```
Q: "What medication names are mentioned?"
A: "Colecalciferol 100 000 UI/2 ml (UVEDOSE), 1 ampoule à renouveler dans 3 mois"

The model volunteered dose and instructions in the enumerate step.
Code can attempt to parse this. If successful, skip drill for this item.
If parsing fails, drill for specific values.
```

This hybrid approach reduces call count when the model is forthcoming.

---

<a id="3-examples"></a>
## 3. Call Flow Examples

### 3.1 French Prescription (Text, 1 Page, 1 Medication)

**Document**: Dr Vidal, Colecalciferol 100 000 UI/2 ml (UVEDOSE)

```
Call 1: "What type of medical document is this?"
  → "prescription"

Call 2: "What is the date of this document?"
  → "16 octobre 2024"

Call 3: "Who signed this document and what is their specialty?"
  → "Dr Frédéric Vidal, remplaçant Brandon Lalouche, médecin généraliste"

Call 4: "What medication names are mentioned?"
  → "Colecalciferol (UVEDOSE)"

Call 5: "What is the prescribed dose of Colecalciferol?"
  → "100 000 UI/2 ml"

Call 6: "How often should Colecalciferol be taken?"
  → "1 ampoule à renouveler dans 3 mois"

Total: 6 calls (BM-04: 1 call that may degenerate)
```

### 3.2 German Prescription (Vision, 1 Page, 3 Medications)

**Document**: Scanned DE prescription with 3 medications

```
Call 1: "What type of medical document is this?"          → "prescription"
Call 2: "What is the date?"                                → "15. Januar 2024"
Call 3: "Who is the prescribing doctor?"                   → "Dr. Schmidt, Hausarzt"
Call 4: "What medication names are prescribed?"            → "Amoxicillin, Ibuprofen, Pantoprazol"
Call 5: "What is the dose of Amoxicillin?"                 → "1000mg"
Call 6: "How often should Amoxicillin be taken?"           → "dreimal täglich"
Call 7: "What is the dose of Ibuprofen?"                   → "400mg"
Call 8: "How often should Ibuprofen be taken?"             → "bei Bedarf"
Call 9: "What is the dose of Pantoprazol?"                 → "20mg"
Call 10: "How often should Pantoprazol be taken?"          → "einmal morgens"

Total: 10 calls (BM-04: 1 call that degenerated)
```

### 3.3 French Lab Results (Vision, 3 Pages, ~15 Tests)

**Document**: 3-page blood test results

```
Page 1:
  Call 1: "What type of document?"                         → "lab result"
  Call 2: "Date?"                                          → "16 mai 2024"
  Call 3: "Author?"                                        → "Laboratoire BioAnalyse"
  Call 4: "What lab tests are on this page?"               → "Hématies, Hémoglobine, Hématocrite, VGM, CCMH, Leucocytes"
  Call 5: "Value of Hématies?"                             → "4.82 T/L"
  Call 6: "Reference range for Hématies?"                  → "4.0-5.5 T/L"
  Call 7: "Is Hématies normal, high, or low?"              → "normal"
  ... (×6 tests × 3 questions = 18 calls)

Page 2:
  Call 22: "What lab tests are on this page?"              → "Cholestérol, HDL, LDL, Triglycerides"
  ... (×4 tests × 3 questions = 12 calls)

Page 3:
  Call 35: "What lab tests are on this page?"              → "Glycémie, HbA1c, Créatinine"
  ... (×3 tests × 3 questions = 9 calls)

Total: ~43 calls (BM-04: 3 calls, all degenerated)
```

### 3.4 Call Count Comparison

| Document | BM-04 Calls | BM-05 Calls | BM-06 Calls |
|----------|------------|------------|------------|
| FR prescription (1 med) | 1 | 3 | 6 |
| DE prescription (3 meds) | 1 | 3 | 10 |
| FR lab results (3p, ~15 tests) | 3 | 6 | ~43 |
| EN chat (3 domains) | 3 | 3 | ~18 |
| Radiograph | 1 | 1 | 1 |

BM-06 trades call count for **guaranteed simplicity**. Each call is trivial for
the model — but there are many of them.

---

<a id="4-tradeoffs"></a>
## 4. Trade-offs vs BM-05

| Factor | BM-05 | BM-06 |
|--------|-------|-------|
| **Calls per document** | 2-6 | 6-50+ |
| **Complexity per call** | Low (1 domain, simple format) | Minimal (1 value) |
| **Degeneration risk** | Low (expected <5%) | Near-zero (simplest possible) |
| **Parse complexity** | Moderate (split format) | None (answer = value) |
| **Wall time** | Moderate (more calls than BM-04) | High (many calls, includes model load per call) |
| **Context window** | Document sent per call | Document sent MANY times |
| **Vision overhead** | 256 tok/image × N calls | 256 tok/image × MANY calls |
| **Lab results (15 tests)** | 2 calls | ~43 calls |
| **Ollama concurrency** | Ollama serves 1 request at a time | Same — sequential |

### The Vision Tax

For vision documents, each call sends the image (256 tokens via SigLIP). BM-06
sends the same image 10-40 times. This is a significant overhead:

```
BM-05: 3 calls × 256 image tokens = 768 image tokens total
BM-06: 43 calls × 256 image tokens = 11,008 image tokens total
```

This may not cause degeneration, but it wastes prompt processing time (though
Ollama caches the KV state for the same image if the model is warm).

### When BM-06 Makes Sense

- **If BM-05 still degenerates** on some inputs → BM-06 is the fallback
- **For the hardest cases** (dense lab results, complex FR/DE vision docs)
- **As a verification tool** — if BM-06 extracts a value that BM-05 missed

### When BM-05 Is Better

- **If BM-05 achieves <5% degeneration** → BM-06's extra calls are waste
- **For simple documents** (1-2 medications, short prescriptions)
- **For production use** — fewer calls = faster user experience

### Recommended Approach

Run BM-05 first. If it achieves the target (<5% degeneration), BM-06 is
informational — it maps the floor but isn't needed in production. If BM-05
still struggles on specific document types, use BM-06 selectively for those.

---

<a id="5-matrix"></a>
## 5. Test Matrix

### 5.1 Subset Testing (Not Full Matrix)

BM-06 has high call counts. Running the full document set would be 100+ calls.
Instead, test the **BM-04 failure cases** — the documents that degenerated.

| Doc ID | Document | BM-04 GPU Result | BM-06 Calls (est.) |
|--------|----------|-------------------|---------------------|
| D-FR-01 | FR prescription (text) | DEGEN | ~6 |
| D-DE-01 | DE prescription (vision) | DEGEN | ~10 |
| D-DE-03 | DE rezept 2 (vision) | DEGEN | ~10 |
| D-FR-03 | FR lab results p1 (vision) | DEGEN | ~15 |
| D-EN-01 | EN prescription (vision) | DEGEN/STALLED | ~10 |
| C-FR-01 | FR symptom chat | DEGEN | ~11 |
| C-DE-01 | DE symptom chat | DEGEN | ~11 |

**Total BM-06 calls**: ~73

### 5.2 Control Group

Include 2-3 documents that passed in BM-04 (to verify no regression):

| Doc ID | Document | BM-04 GPU Result | BM-06 Calls (est.) |
|--------|----------|-------------------|---------------------|
| D-FR-02 | FR lab order (text) | OK | ~6 |
| C-EN-01 | EN symptom chat | OK | ~11 |
| R-01 | Shoulder radiograph | OK | 1 |

**Total with control**: ~91 calls

### 5.3 Hardware

| Backend | Model | Tests |
|---------|-------|-------|
| GPU Vulkan (Windows) | `ktiyab/coheara-medgemma-4b-q8` | ~91 |

CPU only if GPU shows interesting results worth comparing.

---

<a id="6-criteria"></a>
## 6. Success Criteria

### 6.1 Degeneration

| Metric | BM-04 (these docs) | BM-06 Target |
|--------|-------------------|-------------|
| Degeneration rate | 100% (7/7 selected are BM-04 failures) | 0% |
| Stalled (watchdog gap) | 1 (V-EN-01) | 0 |

### 6.2 Answer Quality

| Metric | Target |
|--------|--------|
| Enumerate completeness | >95% of items listed |
| Drill accuracy | >90% of values match document |
| "Not mentioned" for absent values | 100% (no hallucination) |
| Language preservation | Answers in document's language |

### 6.3 What This Tells Us

| BM-06 Result | Conclusion |
|-------------|------------|
| 0% degen, good quality | Single-value is the floor. Compare with BM-05 to find optimal granularity. |
| 0% degen, poor quality | Model can answer but lacks comprehension at this scale. Document is too hard. |
| >0% degen | Problem is deeper than prompt design — Vulkan or model limitation. |

---

<a id="7-plan"></a>
## 7. Execution Plan

### Phase 0 Evaluation Protocol

All three extraction strategies are tested on the **same 2 documents** that
degenerated on every BM-04 configuration, using the **same worst-case backend**
(GPU Q4 Vulkan). Then the winners are validated on CPU Q8 (best-case).

```
PHASE 0: Compare all 3 strategies on same data, same hardware
═══════════════════════════════════════════════════════════════

Step 1: GPU Q4 (worst case — highest degeneration)
  ├── BM-04: Already done → 100% degeneration on these docs    ✓ DONE
  ├── BM-05: Format exploration (4 formats × 2 docs = 8 calls) ✓ DONE
  │     Result: Format C (markdown) + D (free text) = 0% degen
  │     Result: Format A (pipe) + B (key=value) = REJECTED
  └── BM-06: Iterative (enumerate + drill × 2 docs = ~15 calls) ← NOW
        Test: Does one-value-per-question = 0% degen?

Step 2: CPU Q8 (best case — control, confirm no regression)
  ├── BM-05 Format C: 2 calls (1 per doc)
  └── BM-06 Iterative: ~15 calls (same chain)
  → Validates that winning strategies work on CPU too

Step 3: COMPARE all results
  ├── Degeneration: BM-04 vs BM-05-C vs BM-06
  ├── Quality: completeness, accuracy
  ├── Speed: wall time, tokens
  └── DECIDE: which strategy for production

═══════════════════════════════════════════════════════════════
Only AFTER Phase 0 do we build the full benchmark runner.
```

### Test Documents (Same for All 3 Strategies)

| Document | Language | Type | BM-04 Result |
|----------|----------|------|-------------|
| V-DE-01 (Prescription_DE.jpg) | German | Vision, medications | DEGEN on CPU Q4, GPU Q4, GPU Q8 |
| V-FR-03 (resultats-d-analyse_P1_FR.png) | French | Vision, lab results | DEGEN on CPU Q4, GPU Q4, GPU Q8 |

### Why These Documents

They are the **hardest cases** — they degenerated on every configuration in BM-04.
If a strategy rescues these, it will work on everything else.

### Why GPU Q4 First

GPU Q4 is the worst-case configuration (44% degeneration in BM-04, three-factor
compounding: prompt complexity + Q4 quantization + Vulkan compute). If a strategy
works here, it works everywhere.

### Language Matching

BM-05 Phase 0 used English questions on DE/FR documents. BM-06 uses **language-matched
questions** — German questions for DE docs, French questions for FR docs. This
eliminates code-switching overhead observed in BM-05 thinking chains.

### Phases (Revised)

| Phase | Task | Description |
|-------|------|-------------|
| P0-1 | BM-05 Phase 0 on GPU Q4 | ✓ DONE — Format C wins, 7/8 pass |
| P0-2 | BM-06 Phase 0 on GPU Q4 | ← NOW — Iterative on same 2 docs |
| P0-3 | BM-05 C + BM-06 on CPU Q8 | Validate winners on best-case backend |
| P0-4 | Three-way comparison | BM-04 vs BM-05-C vs BM-06 |
| P0-5 | Strategy decision | Which approach for production |
| P1+ | Full benchmark with winner | 40+ calls, all documents, all backends |

### Phase 0 Results — BM-06 on GPU Q4 (2026-02-26)

**Script**: `bench_06_format_explore.py`
**Config**: `ktiyab/coheara-medgemma-4b-q4`, GPU Vulkan, RX 5700 XT
**Output**: `bench_06_format_explore_ktiyab_coheara_medgemma_4b_q4.jsonl`

#### Summary

| Metric | Value |
|--------|-------|
| Total calls | 28 (2 enumerate + 26 drill) |
| Healthy | 27/28 (96%) |
| Degenerated | 1/28 (4%) |
| Total time | 2.9 min |

#### Document 1: V-DE-01 (German Prescription, Vision)

| Call | Step | Question (DE) | Time | Tokens | Degen | Answer |
|------|------|---------------|------|--------|-------|--------|
| ENUM-MEDS | enumerate | "Welche Medikamentennamen werden genannt?" | 35.0s | 263 | no | "Ibuprofen Filmtabletten" |
| DRILL-dose | drill | "Welche Dosis von Ibuprofen Filmtabletten?" | 3.8s | 223 | no | "400 mg" |
| DRILL-instr | drill | "Welche Einnahmeanweisungen für Ibuprofen?" | 3.9s | 229 | **YES** | Degenerated — model looped trying to interpret "L" on prescription |

**Result**: 2/3 healthy. Enumerate and dose OK. Instructions drill degenerated on
ambiguous character in the scanned image.

#### Document 2: V-FR-03 (French Lab Results Page 1, Vision)

| Call | Step | Time | Tokens | Degen | Answer (truncated) |
|------|------|------|--------|-------|--------------------|
| ENUM-LABS | enumerate | 33.5s | 223 | no | 12 tests: Hématies, Hémoglobine, Hématocrite, TGMH, CCMH, IDR, Leucocytes, Neutrophiles, Éosinophiles, Basophiles, Lymphocytes, Monocytes |
| DRILL-HEMATIES-val | drill | 4.1s | 243 | no | 4.88 T/I |
| DRILL-HEMATIES-ref | drill | 2.7s | 161 | no | (13.4-16.7) T/I |
| DRILL-HÉMOGLOBINE-val | drill | 2.9s | 169 | no | 4.88 g/dL |
| DRILL-HÉMOGLOBINE-ref | drill | 5.7s | 339 | no | (4.2-6.00) |
| DRILL-HÉMATOCRITE-val | drill | 2.6s | 148 | no | 41.0 % |
| DRILL-HÉMATOCRITE-ref | drill | 3.3s | 187 | no | 4.88 T/I (13.4-16.7)* |
| DRILL-TGMH-val | drill | 5.6s | 165 | no | 27.3 mg/dL |
| DRILL-TGMH-ref | drill | 2.8s | 154 | no | (26.0-34.0) |
| DRILL-CCMH-val | drill | 2.8s | 158 | no | 32.5 % |
| DRILL-CCMH-ref | drill | 4.8s | 285 | no | (12.3-17.0) |
| DRILL-IDR-val | drill | 4.5s | 275 | no | 13.6 % |
| DRILL-IDR-ref | drill | 3.5s | 202 | no | (12.3-17.0) % |
| DRILL-LEUCOCYTES-val | drill | 2.7s | 149 | no | 6 100 mm3 |
| DRILL-LEUCOCYTES-ref | drill | 2.9s | 160 | no | (4000-6900) /mm3 |
| DRILL-Neutrophiles-val | drill | 2.9s | 165 | no | 62.9 % |
| DRILL-Neutrophiles-ref | drill | 8.0s | 309 | no | 62.9 / (4.00-6.90) |
| DRILL-Éosinophiles-val | drill | 4.6s | 278 | no | 62.9 /mm3* |
| DRILL-Éosinophiles-ref | drill | 4.7s | 273 | no | 6.9 % (<1000) |
| DRILL-Basophiles-val | drill | 4.7s | 278 | no | 0.5 % |
| DRILL-Basophiles-ref | drill | 2.9s | 166 | no | (<110) |
| DRILL-Lymphocytes-val | drill | 2.7s | 153 | no | 441 mm3 |
| DRILL-Lymphocytes-ref | drill | 4.4s | 262 | no | (1000-4800) |
| DRILL-Monocytes-val | drill | 2.5s | 135 | no | 690 mm3 |
| DRILL-Monocytes-ref | drill | 8.8s | 351 | no | (180-1000) |

**Result**: 25/25 healthy (100%). All 12 lab tests extracted with values and ranges.

*Some value errors (e.g., Hématocrite reference returned Hématies data) —
accuracy issues, not degeneration. The model sometimes confuses adjacent rows
in the lab report image when asked about a specific test.

#### Observations

1. **96% healthy on GPU Q4** — nearly eliminates degeneration. But not 100%.
2. **The 1 degeneration was on a vision ambiguity** — the letter "L" on the German
   prescription triggered a reasoning loop about whether it means "Lösung" (solution).
   This is a document-specific edge case, not a format issue.
3. **Lab results: 100% healthy, 12/12 tests extracted** — thorough coverage.
   The enumerate step found all 12 tests, and all 24 drill calls succeeded.
4. **Accuracy is imperfect** — some drill answers confuse adjacent rows in the image.
   The model reads "Hématocrite reference?" but returns the Hématies reference.
   This is an OCR accuracy issue, not an extraction strategy issue.
5. **Language matching worked** — DE questions for DE doc, FR for FR doc. Clean
   thinking chains with no translation overhead.
6. **28 calls but only 2.9 min** — each drill averages 3-5s. Fast individual calls.

---

<a id="8-decisions"></a>
## 8. Decision Log

| ID | Decision | Status | Rationale |
|----|----------|--------|-----------|
| BM6-01 | One value per question, iterative drilling | PROPOSED | Establishes the minimum complexity floor. If this degenerates, nothing will work. |
| BM6-02 | Enumerate first, then drill per item | PROPOSED | Enumerate gives the model one task (list names). Drill gives one task (answer about one item). Two simple phases. |
| BM6-03 | Test only BM-04 failure cases + controls | PROPOSED | BM-06 has high call count. Testing every document is impractical. Focus on the hard cases. |
| BM6-04 | Run after BM-05, not instead of | PROPOSED | BM-05 may be sufficient. BM-06 is the fallback to map the floor. |
| BM6-05 | Skip drill if enumerate already contains full info | PROPOSED | If model volunteers "Ibuprofen 400mg twice daily" in enumerate step, code can parse it directly — no need to drill. Reduces calls. |
| BM6-06 | No JSON anywhere in BM-06 prompts | PROPOSED | Same as BM-05. The model answers in natural language. Code builds structure. |
| BM6-07 | Phase 0 GPU Q4: 96% healthy (27/28), 1 drill degeneration | VALIDATED | V-FR-03 100% (25/25). V-DE-01 67% (2/3) — instructions drill hit vision ambiguity. Near-zero degeneration but not absolute zero. |
| BM6-08 | Iterative approach is more thorough but slower than single-domain markdown | VALIDATED | BM-06 extracted 12/12 lab tests (130s). BM-05 Format C extracted 5/12 (12s). Trade-off: completeness vs speed. |
| BM6-09 | Language-matched questions work — DE for DE docs, FR for FR docs | VALIDATED | Clean thinking chains, no translation overhead. Model reasons natively in document language. |
| BM6-10 | Phase 0 CPU Q4: 100% healthy (24/24), zero degeneration | VALIDATED | V-DE-01 5/5 (including instructions drill that failed on GPU Q4). V-FR-03 19/19 (9 tests × 2 drills + 1 enum). CPU Q4 eliminates the Vulkan ambiguity — confirms prompt complexity is the dominant factor. 15.8 min total, ~10 tok/s avg. |
| BM6-11 | V-DE-01 instructions drill passes on CPU Q4 but fails on GPU Q4 | VALIDATED | The "L" character vision ambiguity that caused degeneration on GPU Q4 Vulkan does not trigger degeneration on CPU Q4. Vulkan floating-point divergence amplifies borderline inputs. |
| BM6-12 | Enumerate completeness varies by backend: CPU Q4 found 9 tests, GPU Q4 found 12 | DOCUMENTED | V-FR-03 enumerate: GPU Q4 found 12 lab tests, CPU Q4 found 9 (missed Hématies, Hémoglobine, V.G.M). Accuracy/completeness trade-off, not degeneration. |
| BM6-13 | CPU Q4_K_S: V-FR-03 enumerate degenerates (1/4 calls, 25%) — quantization floor found | VALIDATED | V-DE-01: 3/3 healthy. V-FR-03 enumerate: sequence_repeat in thinking chain (model cycled through test name refinements). Q4_K_S crosses a threshold where complex enumerate on vision docs triggers repetition. Q4_K_M passes same test (0/24 degen). |
| BM6-14 | GPU Q4_K_S: 26/26 healthy, 0% degeneration — GPU masks Q4_K_S enumerate weakness | VALIDATED | V-DE-01 5/5 (enumerate found 2 meds, drilled 4). V-FR-03 21/21 (enumerate found 10 tests, drilled 20). The GPU's faster inference somehow avoids the thinking-chain loop that traps CPU Q4_K_S. |
| BM6-15 | Q4_K_S is the quantization floor for BM-06 on CPU — Q4_K_M is the safe minimum | VALIDATED | CPU Q4_K_M: 0/24 degen. CPU Q4_K_S: 1/4 degen (25%). The 100 MB difference crosses a reliability threshold for iterative enumerate on complex vision documents. Q4_K_M remains the recommended minimum. |

### Phase 0 Results — CPU Q4 (2026-02-26)

**Configuration**: CPU Q4_K_M on localhost (WSL2 Ollama, no GPU)
**Model**: `ktiyab/coheara-medgemma-4b-q4` (3.3 GB)
**Documents**: Same 2 as GPU Q4 Phase 0 (V-DE-01 + V-FR-03)

#### Summary

| Metric | Value |
|--------|-------|
| Total calls | 24 (2 enumerate + 22 drill) |
| Healthy | **24/24 (100%)** |
| Degenerated | **0/24 (0%)** |
| Total time | 15.8 min |

#### Document 1: V-DE-01 (German Prescription, Vision)

| Call | Step | Time | Tokens | Degen | Answer |
|------|------|------|--------|-------|--------|
| ENUM-MEDS | enumerate | 184.3s | 634 | no | Found 2 items: "Ibuprofen Filmtabletten", "Migrânea"* |
| DRILL-Ibuprofen-dose | drill | 40.5s | 345 | no | "400 mg" |
| DRILL-Ibuprofen-instr | drill | 42.0s | 361 | no | "1 OP ... bis 2 Filmtabletten, maximal 6 täglich" |
| DRILL-Migrânea-dose | drill | 48.8s | 414 | no | "400 mg" (conflated with Ibuprofen) |
| DRILL-Migrânea-instr | drill | 84.8s | 719 | no | "1 OP ... bis 2 Filmtabletten, maximal 6 täglich" |

*Enumerate parsed "Migrän" (migraine — the condition) as a second medication name. This is an accuracy issue, not degeneration. The document has ONE medication (Ibuprofen for Migrän).

**Key finding**: The instructions drill that **degenerated on GPU Q4** (vision ambiguity with "L" character) **passes on CPU Q4**. This confirms the GPU degeneration was Vulkan-amplified, not Q4-inherent.

**Result**: 5/5 healthy (100%).

#### Document 2: V-FR-03 (French Lab Results Page 1, Vision)

| Call | Step | Time | Tokens | Degen | Answer |
|------|------|------|--------|-------|--------|
| ENUM-LABS | enumerate | 149.6s | 409 | no | 9 tests: Hématocrite, T.G.M.H, C.C.M.H, IDR, Polynucl. neutro/éosino/baso, Lymphocytes, Monocytes |
| DRILL-Hématocrite-val | drill | 16.2s | 158 | no | 4.88 g/dL |
| DRILL-Hématocrite-ref | drill | 29.1s | 287 | no | (13.4-16.7) |
| DRILL-T.G.M.H-val | drill | 16.8s | 165 | no | 27.3 mg/dL |
| DRILL-T.G.M.H-ref | drill | 15.6s | 154 | no | (26.0-34.0) |
| DRILL-C.C.M.H-val | drill | 16.3s | 158 | no | 32.5 % |
| DRILL-C.C.M.H-ref | drill | 28.9s | 285 | no | (12.3-17.0) |
| DRILL-IDR-val | drill | 28.2s | 281 | no | 13.6 % |
| DRILL-IDR-ref | drill | 30.9s | 303 | no | (12.3-17.0) |
| DRILL-Neutrophiles-val | drill | 27.9s | 275 | no | 62.9 /mm3 |
| DRILL-Neutrophiles-ref | drill | 34.0s | 337 | no | (1800-6900) |
| DRILL-Éosinophiles-val | drill | 15.7s | 150 | no | 6.9 % |
| DRILL-Éosinophiles-ref | drill | 15.5s | 152 | no | (1800-6900)* |
| DRILL-Basophiles-val | drill | 14.4s | 138 | no | 0.9 % |
| DRILL-Basophiles-ref | drill | 14.4s | 138 | no | (<110) |
| DRILL-Lymphocytes-val | drill | 16.6s | 163 | no | 4.41 /% |
| DRILL-Lymphocytes-ref | drill | 33.6s | 335 | no | (1000-4800) |
| DRILL-Monocytes-val | drill | 16.6s | 162 | no | 690 mm3 |
| DRILL-Monocytes-ref | drill | 24.6s | 245 | no | (180-1000) |

*Éosinophiles reference "(1800-6900)" appears to be a cross-row confusion (that's the neutrophiles range). Accuracy issue, not degeneration.

**Result**: 19/19 healthy (100%). 9 tests extracted (vs 12 on GPU Q4 enumerate).

#### Comparison: CPU Q4 vs GPU Q4 — BM-06

| Metric | GPU Q4 | CPU Q4 |
|--------|--------|--------|
| Total calls | 28 | 24 |
| Healthy rate | 96% (27/28) | **100% (24/24)** |
| Degenerated | 1 (V-DE-01 instr drill) | **0** |
| V-FR-03 tests enumerated | 12 | 9 |
| Total time | 2.9 min | 15.8 min |
| Avg drill time | 3-5s | 15-35s |

#### Analysis

1. **Zero degeneration on CPU Q4.** 24/24 calls healthy. The iterative strategy
   completely eliminates degeneration even on Q4 quantization. This is significant
   because BM-04 CPU Q4 had 36% degeneration — the same model, same hardware, but
   with complex JSON prompts.

2. **The 1 GPU Q4 degeneration was Vulkan-specific, not Q4-specific.** The V-DE-01
   instructions drill that degenerated on GPU Q4 (vision ambiguity with "L" character)
   passes cleanly on CPU Q4. This confirms: Vulkan floating-point divergence on
   gfx1010 amplifies borderline inputs past the degeneration threshold.

3. **Enumerate completeness varies.** CPU Q4 found 9/12 lab tests (missed Hématies,
   Hémoglobine, V.G.M), while GPU Q4 found 12/12. Different compute paths produce
   different attention patterns in the vision encoder. Completeness is an accuracy
   metric — addressable with follow-up questions ("Are there any other tests?").

4. **Speed: 5x slower than GPU Q4.** 15.8 min (CPU) vs 2.9 min (GPU) for similar
   call counts. Individual drills: 15-35s (CPU) vs 3-5s (GPU). Consistent with the
   6-7x CPU/GPU speed ratio from BM-04.

5. **All thinking tokens present.** Every response includes `<unused94>thought...
   <unused95>` reasoning chains. The 93% pattern from BM-04 holds — iterative prompts
   do not suppress thinking behavior.

### Phase 0 Results — CPU Q4_K_S (2026-02-26)

**Configuration**: CPU Q4_K_S on localhost (WSL2 Ollama, no GPU)
**Model**: `ktiyab/coheara-medgemma-4b-q4s` (3.2 GB, Q4_K_S — lowest supported quantization)
**Documents**: Same 2 (V-DE-01 + V-FR-03)
**Purpose**: Quantization floor for iterative strategy

#### Summary

| Metric | Value |
|--------|-------|
| Total calls | 4 (2 enumerate + 2 drill) |
| Healthy | **3/4 (75%)** |
| Degenerated | **1/4 (25%) — V-FR-03 enumerate** |
| Total time | 7.1 min |

#### Document 1: V-DE-01 (German Prescription, Vision) — 3/3 healthy

| Call | Step | Time | Tokens | Degen | Answer |
|------|------|------|--------|-------|--------|
| ENUM-MEDS | enumerate | 150.8s | 274 | no | Found 1 item: "Ibuprofen Filmtabletten" |
| DRILL-Ibuprofen-dose | drill | 24.3s | 221 | no | "400 mg" (correctly: "400 mgL" in doc) |
| DRILL-Ibuprofen-instr | drill | 87.4s | 803 | no | Long reasoning chain, concluded "nicht explizit angegeben" (not explicitly stated) — incorrect, doc says "1 bis 2 Filmtabletten täglich" |

**Note**: V-DE-01 instructions drill produced a correct non-degenerate response but the 803-token thinking chain led to an incorrect conclusion. This is an accuracy issue, not a degeneration issue.

#### Document 2: V-FR-03 (French Lab Results Page 1, Vision) — DEGENERATED on enumerate

| Call | Step | Time | Tokens | Degen | Answer |
|------|------|------|--------|-------|--------|
| ENUM-LABS | enumerate | 163.2s | 394 | **YES** | sequence_repeat — model cycled through test name refinements (HEMATOLES → HEMATOCRYTE → ...) |

Enumerate degenerated → all drills skipped (no items to drill).

#### Analysis

1. **Q4_K_S crosses the quantization floor for BM-06 enumerate on CPU.** The same
   V-FR-03 enumerate that passed on CPU Q4_K_M (19/19 healthy) degenerates on CPU
   Q4_K_S. The enumerate step asks the model to list ALL test names from a complex
   vision document — this requires enough precision to maintain a coherent list
   without looping.

2. **Q4_K_M vs Q4_K_S: 100 MB matters for complex enumeration.** Q4_K_S uses smaller
   quantization groups (K_S vs K_M), reducing precision in attention computations.
   For simple single-value drills, this doesn't matter. For enumerate (which requires
   multi-item tracking), it crosses a threshold.

3. **V-DE-01 simpler document passes fine.** The German prescription has 1 medication —
   enumerate is trivial. The 803-token instructions drill shows verbose reasoning
   but no repetition loop.

### Phase 0 Results — GPU Q4_K_S (2026-02-26)

**Configuration**: GPU Vulkan (gfx1010) on Windows Ollama
**Model**: `ktiyab/coheara-medgemma-4b-q4s` (3.2 GB, Q4_K_S)
**Documents**: Same 2 (V-DE-01 + V-FR-03)

#### Summary

| Metric | Value |
|--------|-------|
| Total calls | 26 (2 enumerate + 24 drill) |
| Healthy | **26/26 (100%)** |
| Degenerated | **0/26 (0%)** |
| Total time | 4.4 min |

#### Document 1: V-DE-01 — 5/5 healthy

| Call | Step | Time | Tokens | Degen | Answer |
|------|------|------|--------|-------|--------|
| ENUM-MEDS | enumerate | 39.5s | 188 | no | Found 2 items: "Ibuprofen Filmtabletten", "Migrânea Filmtabletten"* |
| DRILL-Ibuprofen-dose | drill | 4.4s | 224 | no | "400 mg" |
| DRILL-Ibuprofen-instr | drill | 4.9s | 261 | no | Correctly extracted |
| DRILL-Migrânea-dose | drill | 5.6s | 292 | no | "400 mg" (conflated) |
| DRILL-Migrânea-instr | drill | 3.0s | 151 | no | Extracted |

*Same accuracy issue as previous runs: "Migrän" (migraine condition) parsed as medication.

#### Document 2: V-FR-03 — 21/21 healthy

| Call | Step | Time | Tokens | Degen | Answer |
|------|------|------|--------|-------|--------|
| ENUM-LABS | enumerate | 48.6s | 858 | no | 10 tests found |
| (20 drills) | drill | 2.6-39.1s | 132-438 | no | All values and reference ranges extracted |

Tests enumerated: HEMATOLOGIES, HémoGLOBINE, Hématocrite, T.G.M.H, C.C.M.H, IDR, LEUCOCYTES, Polycuédaires neutro/éosino/baso, Lymphocytes, Monocytes.

#### Comparison: Q4_K_S — CPU vs GPU

| Metric | CPU Q4_K_S | GPU Q4_K_S |
|--------|-----------|-----------|
| V-DE-01 | 3/3 healthy | 5/5 healthy |
| V-FR-03 | **1/1 DEGEN** (enumerate) | 21/21 healthy |
| Total healthy | 75% (3/4) | **100% (26/26)** |
| Total time | 7.1 min | 4.4 min |

**Key finding**: GPU Q4_K_S passes where CPU Q4_K_S fails on enumerate. The GPU's
faster inference and different floating-point behavior avoids the thinking-chain
repetition loop that traps the CPU path. This is the opposite of BM-06 on Q4_K_M
where GPU had 1 degeneration and CPU had 0 — suggesting the degeneration triggers
are different between compute paths.

---

## Appendix A: Complete Test Matrix — All Combinations Run (2026-02-26)

### Three Extraction Strategies

| Strategy | Description | Prompt to Model | Code Responsibility |
|----------|-------------|-----------------|---------------------|
| **All-at-once JSON** | One call extracts 7+ domains as JSON | "Extract ALL medical info as JSON: {schema...}" (~300 tok schema) | Parse JSON |
| **Single-domain markdown** | One call per domain, markdown list output | "What medications are prescribed? List as bullet points." (~25 tok) | Split bullets, build JSON |
| **Iterative single-value** | Enumerate items, then one question per item per field | "What is the dose of Ibuprofen?" (~15 tok) | Store answer directly |

### Four Hardware Configurations

| Config | Backend | Quantization | Speed | Description |
|--------|---------|-------------|-------|-------------|
| **CPU Q8** | WSL2 Ollama (CPU-only) | Q8_0 (5.0 GB) | 7.2 tok/s | Best-case: highest precision, deterministic compute |
| **CPU Q4** | WSL2 Ollama (CPU-only) | Q4_K_M (3.3 GB) | 12.0 tok/s | Lower precision on reliable compute |
| **CPU Q4s** | WSL2 Ollama (CPU-only) | Q4_K_S (3.2 GB) | 9.5 tok/s | Quantization floor — smallest supported |
| **GPU Q8** | Windows Ollama, Vulkan | Q8_0 (5.0 GB) | 48.8 tok/s | Good precision on variable compute (gfx1010) |
| **GPU Q4** | Windows Ollama, Vulkan | Q4_K_M (3.3 GB) | 51.6 tok/s | Lowest K_M precision + variable compute |
| **GPU Q4s** | Windows Ollama, Vulkan | Q4_K_S (3.2 GB) | 58.7 tok/s | Quantization floor + variable compute |

### What Has Been Tested

```
                              CPU Q8    CPU Q4    CPU Q4s   GPU Q8    GPU Q4    GPU Q4s
                              ──────    ──────    ──────    ──────    ──────    ──────
All-at-once JSON (BM-04)
  Full suite (22 tests)        14/14*    22/22      —       22/22     18/22**    —
  V-DE-01 result                 —       DEGEN      —       DEGEN     DEGEN      —
  V-FR-03 result                 —       DEGEN      —       DEGEN     DEGEN      —

Single-domain markdown (BM-05 Phase 0)
  Format A (pipe delimited)     —         —         —        —       BLOCK-RPT / OK  —
  Format B (labeled key=val)    —         —         —        —       OK / DEGEN      —
  Format C (markdown list)      —       OK / OK ✓ OK / OK ✓  —       OK / OK ✓  OK / OK ✓
  Format D (free text)          —         —         —        —       OK / OK ✓       —
  V-DE-01 with Format C         —       OK (144s)  OK (175s)  —      OK (4s)   OK (46s) ✓
  V-FR-03 with Format C         —       OK (169s)  OK (176s)  —      OK (12s)  OK (44s) ✓

Iterative single-value (BM-06 Phase 0)
  V-DE-01 (enumerate+drill)     —       5/5 OK ✓  3/3 OK ✓   —      2/3 OK    5/5 OK ✓
  V-FR-03 (enumerate+drill)     —       19/19 OK ✓ DEGEN ✗    —      25/25 OK ✓ 21/21 OK ✓

*  CPU Q8 ran 14/22 tests (vision not all tested)
** GPU Q4 ran 18/22 (stalled on V-EN-01, radiographs not reached)
```

### Results Summary: Degeneration Rate

| Strategy × Config | V-DE-01 | V-FR-03 | Full Suite Degen Rate |
|-------------------|---------|---------|----------------------|
| All-at-once JSON + CPU Q8 | — | — | 0% (0/14) |
| All-at-once JSON + CPU Q4 | DEGEN | DEGEN | 36% (8/22) |
| All-at-once JSON + GPU Q8 | DEGEN | DEGEN | 45% (10/22) |
| All-at-once JSON + GPU Q4 | DEGEN | DEGEN | 44% (8/18) |
| Single-domain markdown + CPU Q4 | **OK (144s)** | **OK (169s)** | **0% (0/2)** |
| Single-domain markdown + CPU Q4s | **OK (175s)** | **OK (176s)** | **0% (0/2)** |
| Single-domain markdown + GPU Q4 | **OK (4s)** | **OK (12s)** | **0% (0/2)** |
| Single-domain markdown + GPU Q4s | **OK (46s)** | **OK (44s)** | **0% (0/2)** |
| Iterative single-value + CPU Q4 | **5/5 OK** | **19/19 OK** | **0% (0/24)** |
| Iterative single-value + CPU Q4s | 3/3 OK | **DEGEN** | **25% (1/4)** |
| Iterative single-value + GPU Q4 | 2/3 OK | **25/25 OK** | 4% (1/28) |
| Iterative single-value + GPU Q4s | **5/5 OK** | **21/21 OK** | **0% (0/26)** |

### Results Summary: Completeness (V-FR-03 Lab Results)

| Strategy | Lab Tests Found | Values Extracted | Time |
|----------|----------------|-----------------|------|
| All-at-once JSON + GPU Q4 | DEGEN (0 usable) | 0 | ~45s (wasted) |
| Single-domain markdown + CPU Q4 | 5/12 | 5 values + ranges | 169s |
| Single-domain markdown + GPU Q4 | 5/12 | 5 values + ranges | 12s |
| Iterative single-value + CPU Q4 | 9/12 | 9 values + 9 ranges | ~550s |
| Iterative single-value + GPU Q4 | **12/12** | 12 values + 12 ranges | 130s |

### What Has NOT Been Tested Yet

```
                              CPU Q8    CPU Q4    GPU Q8    GPU Q4
                              ──────    ──────    ──────    ──────
Single-domain markdown          ✗       ✓ done      ✗       ✓ done
Iterative single-value          ✗       ✓ done      ✗       ✓ done
```

**GPU Q8 BLOCKED**: Ollama stalling on block-repeat makes GPU Q8 testing unreliable.
Multiple restart cycles, no data produced. Since GPU Q4 is the worst case and both
strategies passed there, GPU Q8 results are **extrapolated** (equal or better than Q4).

**Remaining**: Both winning strategies need CPU Q8 validation (control — confirm
no regression on the best-case backend).

### Three-Factor Degeneration Model (Updated)

```
FACTOR 1: EXTRACTION STRATEGY (dominant)
  All-at-once JSON:        36-45% degen (attention overload + JSON formatting)
  Single-domain markdown:  0% degen on Phase 0 (focused attention, natural format)
  Iterative single-value:  4% degen on Phase 0 (near-zero, 1 edge case)

FACTOR 2: QUANTIZATION
  Q8_0:  0% degen with all-at-once on CPU (enough precision)
  Q4_K_M: 36% degen with all-at-once on CPU (precision loss)
  → With focused strategies, quantization impact expected to be minimal

FACTOR 3: COMPUTE PATH
  CPU:   Deterministic floating-point
  GPU Vulkan (gfx1010): Variable floating-point, adds ~8-9% degen on top
  → With focused strategies, compute path impact expected to be minimal
```

**The extraction strategy is the dominant factor.** Changing from all-at-once JSON
to focused Q&A reduced degeneration from 100% to 0-4% on the worst-case hardware
config (GPU Q4). Quantization and compute path become secondary when the prompt
is simple enough.

---

## Appendix B: The Granularity Spectrum — All Three Benchmarks

```
┌─────────────────────────────────────────────────────────────────────────┐
│                     EXTRACTION GRANULARITY SPECTRUM                     │
├──────────┬──────────────────┬──────────────────┬───────────────────────┤
│          │ BM-06            │ BM-05            │ BM-04                 │
│          │ SINGLE VALUE     │ SINGLE DOMAIN    │ ALL DOMAINS           │
├──────────┼──────────────────┼──────────────────┼───────────────────────┤
│ Per call │ 1 value          │ N values, 1 type │ N values, 7+ types    │
│ Format   │ Natural answer   │ Simple template  │ Full JSON schema      │
│ Calls    │ Many (6-50+)     │ Moderate (2-6)   │ Few (1)               │
│ Parse    │ None (answer=val)│ Split/regex      │ JSON parse            │
│ Degen    │ Expected: 0%     │ Expected: <5%    │ Measured: 45%         │
│ Speed    │ Slowest          │ Moderate         │ Fastest (when works)  │
├──────────┼──────────────────┼──────────────────┼───────────────────────┤
│ Best for │ Hardest docs     │ Most documents   │ Simple docs on CPU    │
│          │ Verification     │ Production use   │ (if reliable)         │
│          │ Fallback         │                  │                       │
└──────────┴──────────────────┴──────────────────┴───────────────────────┘

PRODUCTION STRATEGY (expected):
  1. Try BM-05 (single domain, simple format) — fast, usually works
  2. If degeneration detected → fall back to BM-06 (iterative) for that document
  3. Never use BM-04 approach (all-at-once JSON) — unreliable on 4B model
```

---

## Appendix: File Map

```
Specs/experiments/
├── MEDGEMMA-BENCHMARK-06.md          # THIS FILE
├── bench_06_runner.py                # BM-06 runner (to build)
├── bench_06_results_gpu_q8.jsonl     # GPU Q8 results (to produce)
│
├── MEDGEMMA-BENCHMARK-05.md          # BM-05 (format exploration + single domain)
├── MEDGEMMA-BENCHMARK-04.md          # BM-04 (baseline — all-at-once JSON)
├── MODEL-FACTORY-SPEC.md             # Model factory (parent spec)
│
├── bench_04_results.jsonl            # BM-04 CPU Q8
├── bench_04_results_cpu_q4.jsonl     # BM-04 CPU Q4 (running)
├── bench_04_results_gpu_q8.jsonl     # BM-04 GPU Q8
└── bench_04_results_gpu_q4.jsonl     # BM-04 GPU Q4
```

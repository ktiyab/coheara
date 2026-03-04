# Medical Invariants in Coheara

> *Medical knowledge is a lookup table, not a generation task. The SLM articulates; invariants ground.*

---

## What Is an Invariant?

In medicine, some facts don't change with the patient, the doctor, the language, or the day. Blood pressure above 140/90 mmHg is hypertension, whether the patient is in Paris, Berlin, or Nairobi. Warfarin combined with aspirin creates a high bleeding risk, regardless of who prescribed them. A penicillin allergy means amoxicillin is contraindicated, no matter how the allergy was documented.

These are **medical invariants**: universal clinical truths that hold across all patients, sourced from international guidelines, and computable without judgment. They have no opinion. They are deterministic.

In Coheara, invariants are the foundational layer of medical intelligence. They encode the pattern recognition a generalist doctor builds over 30 years of practice (which lab values are dangerous, which drug combinations are risky, which monitoring intervals are overdue) as structured, testable, reproducible computations.

### Invariant vs. Data

Not everything in a patient's record is an invariant. A doctor's note about seasonal allergies is **data**. The fact that a penicillin allergy cross-reacts with cephalosporins at a 16.5% rate is an **invariant**.

| | Patient Data | Medical Invariant |
|---|---|---|
| Changes per patient | Yes | Rules are universal |
| Requires tracking | Sometimes | Always |
| Has a lifecycle | Sometimes | Always (monitoring interval, expiry) |
| Connects to other items | Accidentally (string matching) | By medical knowledge (drug families, cross-reactivity) |
| Failing to track causes harm | Sometimes | Always |

A patient's HbA1c of 7.2% is data. The fact that 7.2% crosses the diabetes threshold (IDF 2025) and that it means their metformin monitoring is overdue (ADA/KDIGO 2022): that's invariant knowledge applied to data.

---

## Why Invariants Exist in Coheara

### The Problem Without Them

Without invariants, Coheara's pipeline looks like this:

```
Patient asks a question → retrieve similar text from documents → throw at SLM → hope it's correct
```

The SLM receives raw data (lab values, medication names, vital signs) and must:
1. Know that BP 145/92 is Grade 1 Hypertension (ISH 2020)
2. Know that Warfarin + Aspirin is a HIGH bleeding risk
3. Know that a penicillin allergy means amoxicillin is contraindicated
4. Know that metformin requires HbA1c monitoring every 90 days
5. Know that eGFR 28 means KDIGO Stage G4 kidney disease

A 4-billion parameter model **cannot reliably know any of this**. It hallucinates classification thresholds, misses drug interactions, invents monitoring intervals. Medical knowledge is not a language generation task. It requires deterministic accuracy that no statistical model can guarantee.

Without invariants, Coheara is a document search engine with an LLM wrapper.

### The Solution With Them

With invariants, the pipeline becomes:

```
Patient asks a question → retrieve data → ENRICH with invariants → pre-computed insights → SLM articulates
```

The SLM no longer needs to know medicine. It receives pre-computed clinical insights:
- "BP 145/92 = ISH Grade 1 Hypertension (source: ISH 2020)"
- "Warfarin + Aspirin: HIGH bleeding risk (source: WHO EML)"
- "Penicillin allergy → Amoxicillin CONTRAINDICATED (source: EAACI 2020)"
- "On Metformin, no HbA1c in 120 days: overdue monitoring (source: ADA/KDIGO 2022)"

The model's job becomes **articulation**: turning structured insights into natural language the patient can understand. It doesn't diagnose. It doesn't classify. It communicates what the invariant engine has already determined.

With invariants, Coheara is a medical knowledge engine.

---

## Where Invariants Come From

Every threshold, classification, interaction rule, and monitoring interval in Coheara traces to a published international clinical guideline. Nothing is invented. Nothing is approximated. The curation follows a strict principle: **if a generalist doctor would look this up in a reference, Coheara encodes that reference as computable data**.

### Source Guidelines

#### Vital Signs (I-VIT)

| Domain | Guideline | Citation | What It Provides |
|---|---|---|---|
| Blood Pressure | ISH 2020 | International Society of Hypertension Global Practice Guidelines | 4-tier classification: Normal, High-Normal, Grade 1, Grade 2 |
| Heart Rate | ESC 2021/2019 | European Society of Cardiology | 6-tier: Severe Bradycardia through Severe Tachycardia |
| Oxygen Saturation | BTS 2017 | British Thoracic Society | 5-tier: Normal through Severe Hypoxemia |
| BMI | WHO TRS 894 | World Health Organization Technical Report | 6-tier: Underweight through Obese Class III |
| Fasting Glucose | WHO 2006 | WHO/IDF Diagnostic Criteria | 3-tier: Normal, Impaired Fasting Glucose, Diabetes |
| Temperature | Clinical consensus | Standard clinical ranges | 7-tier: Hypothermia through Hyperthermia |

#### Laboratory Tests (I-LAB)

| Domain | Guideline | What It Provides |
|---|---|---|
| Kidney Function (eGFR) | KDIGO 2024 | 5-stage CKD classification (G1-G5) |
| Diabetes (HbA1c) | IDF 2025 | 3-tier: Normal, Pre-diabetes, Diabetes |
| Cholesterol (LDL) | ESC/EAS 2019 | 4-tier risk-stratified targets |
| Potassium (K+) | Clinical consensus | Hypokalemia through Severe Hyperkalemia |
| Sodium (Na+) | Clinical consensus | Hyponatremia through Hypernatremia |
| Liver Function (ALT) | EASL | 3×/5× ULN (Upper Limit of Normal) thresholds |
| Hemoglobin | WHO | Anemia classification (sex-differentiated) |
| Thyroid (TSH) | ETA | Hypo/hyperthyroidism ranges |
| Kidney Damage (uACR) | KDIGO | Albuminuria staging (A1-A3) |
| Vitamin D | Endocrine Society / IOF | Deficiency, Insufficiency, Sufficiency |

Each lab test includes **aliases** for multilingual matching. 88 aliases across 10 tests ensure that "Hémoglobine glyquée" (French), "HbA1c", and "Glycated hemoglobin" all resolve to the same threshold. This is critical for a trilingual app (EN/FR/DE) processing documents from different healthcare systems.

#### Drug Safety (I-MED, I-ALG)

| Domain | Guideline | What It Provides |
|---|---|---|
| Drug Families | WHO Essential Medicines List | 20 families (statins, NSAIDs, penicillins, ACE inhibitors, SSRIs, DOACs, opioids...) with all member drugs |
| Drug Interactions | WHO EML, ESC, KDIGO | 20+ clinically significant pairs (Warfarin+NSAID, ACEi+K-sparing, Metformin+contrast...) |
| Drug Cross-Reactivity | EAACI 2020, WAO 2024 | 10 allergen cross-reactivity chains (penicillin-cephalosporin, NSAID phenotypes...) |
| Allergen Cross-Reactivity | WAO 2024, JCM 2024 | OAS (birch-apple), food-food (peanut-tree nut), insect venom, latex-fruit |
| Canonical Allergens | FDA FALCPA, EU 1169/2011, AAAAI 2022, WAO/ARIA 2024 | 46 allergen classes across 5 categories (food, drug, environmental, insect, other) |
| Allergen Aliases | Clinical consensus | 88 common name-to-canonical mappings for multilingual resolution |
| Monitoring Schedules | ADA/KDIGO, ESC/EAS, EHRA, STOPP/START v3 | 24 drug-to-lab monitoring rules (Metformin-HbA1c/90d, Statin-ALT/365d, Warfarin-INR/30d...) |

#### Blood Types (I-BT)

| Domain | Guideline | What It Provides |
|---|---|---|
| ABO/Rh System | ISBT 2023, AABB Technical Manual 21st Ed. | 8 blood types with antigens, antibodies, global frequencies |
| Transfusion Compatibility | AABB Table 14-1 | RBC compatibility matrix (donor-recipient pairs) |
| Pregnancy Awareness | ACOG Practice Bulletin 181, RCOG Green-top Guideline 65 | Rh-negative awareness for childbearing-age females |

#### Screening and Vaccines (I-SCR)

| Domain | Guideline | What It Provides |
|---|---|---|
| Cancer Screening | IARC 2019/2024, WHO 2021, EAU 2024, ESC 2024, IOF 2024 | 6 age+sex-gated screening schedules |
| Vaccine Schedules | WHO 2024, WHO SAGE 2024 | 8 adult vaccine schedules with dose series and validity windows |

### Curation Principles

1. **International over national**: WHO, ESC, KDIGO over country-specific bodies, since Coheara serves users globally
2. **Conservative thresholds**: When guidelines differ, the more cautious threshold is chosen (e.g., female hemoglobin threshold at 12.0 g/dL)
3. **Traceable sourcing**: Every single threshold carries a `source` field linking to the originating guideline and year
4. **No interpolation**: If a guideline doesn't define a threshold, it's not encoded. No "reasonable estimates"
5. **Updateable**: Bundled-tier data (JSON files) can be updated without recompiling the app; const-tier data (Rust arrays) is reserved for stable, rarely-changing thresholds

---

## How Invariants Are Stored

### Two-Tier Architecture

The invariant system uses two storage tiers, chosen for different update frequencies:

```
┌───────────────────────────────────────────────────────────┐
│                   InvariantRegistry                       │
│               (single access point, on CoreState)         │
├────────────────────────┬──────────────────────────────────┤
│     CONST TIER         │        BUNDLED TIER              │
│  (compiled into binary)│     (JSON at startup)            │
├────────────────────────┼──────────────────────────────────┤
│ Vital sign thresholds  │ Drug families                    │
│  - Blood pressure (4)  │  - 20 families, 200+ members    │
│  - Heart rate (6)      │                                  │
│  - SpO2 (5)            │ Interaction pairs                │
│  - BMI (6)             │  - 20+ clinically significant    │
│  - Glucose (3)         │                                  │
│  - Temperature (7)     │ Cross-reactivity chains          │
│                        │  - 10 allergen family chains     │
│ Lab thresholds         │                                  │
│  - 10 tests            │ Allergen cross-reactivity        │
│  - 47 classification   │  - OAS, food-food, insect, latex │
│    tiers               │                                  │
│  - 88 multilingual     │ Allergen aliases                 │
│    aliases             │  - 88 common name → canonical    │
│                        │                                  │
│ Canonical allergens    │ Monitoring schedules             │
│  - 46 allergen classes │  - 24 drug-to-lab rules          │
│  - 5 categories        │                                  │
│                        │ Location:                        │
│ Blood types            │  resources/invariants/*.json     │
│  - 8 ABO/Rh types      │                                  │
│  - Compatibility matrix │ Update: edit JSON, restart app  │
│                        │                                  │
│ Screening schedules    │                                  │
│  - 6 cancer screenings │                                  │
│  - 8 vaccine schedules │                                  │
│                        │                                  │
│ Location:              │                                  │
│  invariants/vitals.rs  │                                  │
│  invariants/labs.rs    │                                  │
│  invariants/allergens.rs│                                 │
│  invariants/blood_types.rs│                               │
│  invariants/screening.rs│                                 │
│                        │                                  │
│ Update: recompile      │                                  │
└────────────────────────┴──────────────────────────────────┘
```

**Why two tiers?** Vital sign and lab thresholds change on the timescale of decades (ISH revises BP guidelines every 5-10 years). They benefit from compile-time type safety and zero I/O overhead. Drug families and interactions change more frequently as new medications enter the market, so JSON files can be updated without rebuilding the app.

### Trilingual Labels

Every classification carries an `InvariantLabel` with translations:

```rust
InvariantLabel {
    key: "bp_grade_1_htn",                              // Machine key (language-independent)
    en: "Grade 1 Hypertension (140-159/90-99 mmHg)",    // English
    fr: "Hypertension de grade 1 (140-159/90-99 mmHg)", // French
    de: "Hypertonie Grad 1 (140-159/90-99 mmHg)",       // German
}
```

Computation always uses the `key`. Display resolves through `label.get(lang)` based on the user's language. The SLM context uses the detected document language for insight descriptions.

---

## How Invariants Intervene in the Pipeline

### The Enrichment Step

When a patient asks a question in chat, the RAG pipeline runs:

```
1. CLASSIFY    What kind of question is this? (factual, exploratory, timeline...)
      │
      ▼
2. RETRIEVE    Fetch the patient's medical data from the database:
      │         medications, lab results, allergies, vital signs, document chunks
      │
      ▼
3. ENRICH      ← THIS IS WHERE INVARIANTS INTERVENE
      │
      │         enrich(medications, labs, allergies, vitals, registry, today, demographics)
      │
      │         Pure function. No LLM. No database. No network. No async.
      │         Takes patient data + invariant registry + demographics → returns clinical insights.
      │         ME-04: Demographics enable sex-aware, ethnicity-aware, and age-gated insights.
      │
      ▼
4. ASSEMBLE    Build the SLM's context window:
      │         - Patient's blood type (Priority 0.5)        ← Identity-level context
      │         - Patient's allergies (Priority 1)
      │         - CLINICAL INSIGHTS section (Priority 1.5)   ← Insights injected here
      │         - Relevant document excerpts (Priority 2)
      │         - Medications, labs, symptoms, vitals (Priority 3-6)
      │
      ▼
5. GENERATE    SLM reads pre-computed insights as grounded facts
      │         and generates a natural-language response
      │
      ▼
6. CITE        Dual citations attached to the response:
               - [Doc: uuid] (from patient's documents)
               - [Guideline: ISH 2020] (from invariant sources, deterministic)
```

### The Ten Detection Algorithms

The `enrich()` function runs ten deterministic sub-algorithms:

#### 1. Classify Vital Signs
Matches each vital sign against const-tier thresholds.

```
Input:  BP 145/92 mmHg
Lookup: ISH 2020 classifications → systolic 140-159, diastolic 90-99
Output: [WARNING] Grade 1 Hypertension (source: ISH 2020)
```

Covers: blood pressure, heart rate, SpO2, temperature, fasting glucose, BMI (computed from weight + height).

#### 2. Classify Lab Results
Resolves lab test names through 88 multilingual aliases, then matches against thresholds.

```
Input:  "Hémoglobine glyquée" = 7.2%
Alias:  "Hémoglobine glyquée" → test_key "hba1c"
Lookup: IDF 2025 thresholds → ≥6.5% = Diabetes
Output: [CRITICAL] HbA1c 7.2%: Diabetes (source: IDF 2025)
```

#### 3. Detect Drug Interactions
Checks all pairs of active medications against the interaction database. Medications are matched by both direct name and drug family key.

```
Input:  Active: [Warfarin, Ibuprofen]
Match:  Warfarin + NSAID pair (ibuprofen is in the NSAID family)
Output: [CRITICAL] Warfarin + Ibuprofen: HIGH bleeding risk (source: WHO EML)
```

#### 4. Detect Cross-Family Reactivity
Checks allergen-medication pairs against cross-reactivity chains.

```
Input:  Allergy: Penicillin | Active med: Cephalexin
Chain:  Aminopenicillin → Aminocephalosporin (~16.5% cross-reactivity)
Output: [CRITICAL] Penicillin allergy → Cephalexin cross-reactivity (source: EAACI 2020)
```

#### 5. Detect Same-Family Allergy
Catches when an allergen and an active medication belong to the same drug family, a contraindication the cross-family chains don't cover.

```
Input:  Allergy: Penicillin | Active med: Amoxicillin
Family: Both in "penicillin" family (WHO EML)
Output: [CRITICAL] Penicillin allergy → Amoxicillin (same Penicillin family)
```

#### 6. Detect Missing Monitoring
For each active medication, checks if required lab tests exist and are within their monitoring interval. Resolves through drug families: "lisinopril" finds "ace_inhibitor" monitoring schedules.

```
Input:  Active: Metformin | Last HbA1c: 120 days ago | Required interval: 90 days
Output: [WARNING] Metformin: HbA1c overdue (last: 2025-11-01, interval: 90 days)
```

#### 7. Detect Screening Due (ME-04)
When patient demographics (sex, age) are available, checks against 6 evidence-based preventive screening schedules.

```
Input:  Sex = Female, Age = 55
Lookup: IARC/WHO 2024 mammography (Female, 50-74) → eligible
        WHO 2021 cervical (Female, 25-65) → eligible
        IARC 2019 colorectal (Both, 50-75) → eligible
Output: [INFO] Mammography screening recommended (source: IARC/WHO 2024)
        [INFO] Cervical cancer screening recommended (source: WHO 2021)
        [INFO] Colorectal cancer screening recommended (source: IARC 2019)
```

Covers: mammography, cervical (Pap/HPV), prostate (PSA), colorectal (FIT), AAA ultrasound, osteoporosis (DXA), plus 8 vaccine schedules. All severity = Info (reminders, not alarms). Graceful degradation: no demographics = no screening insights. Record-aware: suppresses reminders when screening records show the patient is up to date.

#### 8. Detect Vital Trends
Compares multiple readings of the same vital type over time to detect clinically significant trends.

```
Input:  BP readings: 125/80 (Jan), 135/85 (Apr), 148/92 (Jul)
Window: 6-12 months, >= +20 mmHg systolic (ISH 2020)
Output: [WARNING] Blood pressure trending upward (source: ISH 2020)
```

Covers: BP trend (systolic/diastolic), weight loss (moderate >5%, severe >10% in 6 months), weight gain (>=5% in 6 months). Sources: ISH 2020 (BP), GLIM 2019 (weight).

#### 9. Detect Food/Environmental Cross-Reactivity
Checks allergen records against allergen-specific cross-reactivity chains (OAS, food-food, insect venom, latex-fruit).

```
Input:  Allergy: Birch pollen | Registry: OAS cross-reactivity
Chain:  Birch pollen → Apple, pear, cherry, hazelnut (30-70%)
Output: [WARNING] Birch pollen allergy: oral allergy syndrome risk with apple, pear (source: WAO 2024)
```

Uses the bundled allergen cross-reactivity JSON (distinct from drug cross-reactivity). Resolution through canonical allergen keys and aliases.

#### 10. Detect Rh-Negative Awareness (BT-01)
When patient blood type is Rh-negative and the patient is a female of childbearing age, produces an awareness insight about anti-D prophylaxis.

```
Input:  Blood type: O- | Sex: Female | Age: 28
Guards: Rh-negative = true, Female = true, age 15-50 = true
Output: [WARNING] Rh-negative blood type - discuss anti-D prophylaxis
         with your doctor if pregnancy is relevant (source: ACOG PB 181, RCOG GTG 65)
```

Guards: blood type must be set and Rh-negative, sex must be Female, age must be 15-50 (ACOG childbearing range). If any guard fails, no insight is produced.

### What the SLM Sees

After enrichment, the assembled context contains a `<CLINICAL INSIGHTS>` section:

```xml
<PATIENT BLOOD TYPE>
Blood type: O- (O Rh-negative, ISBT 2023 / AABB 21st Ed.)
</PATIENT BLOOD TYPE>

<CLINICAL INSIGHTS>
[Notable] eGFR 28 mL/min: KDIGO Stage G4, Severely decreased kidney function (source: KDIGO 2024)
[Notable] Penicillin allergy - Amoxicillin (same Penicillin family) (source: WHO EML)
[Guideline note] BP 145/92 mmHg: Grade 1 Hypertension (source: ISH 2020)
[Guideline note] Metformin: HbA1c overdue (last: 2025-11-01, interval: 90 days) (source: ADA/KDIGO 2022)
[Guideline note] Rh-negative blood type - discuss anti-D prophylaxis if pregnancy is relevant (source: ACOG PB 181)
[For reference] BMI 27.8 kg/m2: Overweight (source: WHO TRS 894)
</CLINICAL INSIGHTS>
```

Blood type is injected at Priority 0.5 (before allergies) as identity-level context (~10 tokens). Insights are sorted by severity (Notable first) and use regulatory-compliant labels per REG-01 (Notable/Guideline note/For reference instead of Critical/Warning/Info). The SLM reads these as established facts and weaves them into its response. It doesn't need to classify BP, detect interactions, or compute monitoring gaps. That work is already done, deterministically, with full traceability to published guidelines.

---

## Where Invariants Also Operate

The invariant registry is not limited to the chat pipeline. It feeds four systems:

### 1. Chat Pipeline (RAG Enrichment)
As described above. Every chat response benefits from pre-computed clinical insights. The SLM articulates; invariants ground.

### 2. Me Screen (Health Center)
The Me Screen calls `enrich()` with all patient data on every load, displaying clinical insights as severity-coded cards (InsightCard). It also shows screening/vaccine schedules via `build_screening_info()` with record-aware status (due, up-to-date, expired), blood type as a profile badge, and demographic-aware insight filtering. The Me Screen is the primary surface where patients see their invariant-derived health overview.

### 3. Coherence Engine (Drug Family Detection)
The intelligence module uses the registry for allergy-medication conflict detection during document review. When a new prescription is imported, the coherence engine checks: does this medication conflict with a known allergy? It resolves through the same drug family registry.

### 4. Guideline Citations
Clinical insight sources (ISH 2020, KDIGO 2024, etc.) are extracted as `GuidelineCitation` objects and streamed to the frontend alongside document citations. These are deterministic: they come from the invariant registry, never from LLM output.

---

## The Design Philosophy

### Separate What Must Be Correct From What Can Be Approximate

An SLM is good at language: understanding questions, generating natural responses, explaining complex topics simply. An SLM is unreliable at medical classification: threshold comparisons, drug interaction detection, monitoring interval arithmetic. The invariant system draws a hard line:

| Responsibility | Owner | Why |
|---|---|---|
| "Is BP 145/92 hypertension?" | **Invariant engine** | Deterministic lookup against ISH 2020. Must be correct. |
| "Explain to the patient what Grade 1 Hypertension means" | **SLM** | Language task. Approximation is acceptable. |
| "Do Warfarin and Ibuprofen interact?" | **Invariant engine** | Safety-critical. Binary answer from curated data. |
| "Explain the bleeding risk in simple terms" | **SLM** | Communication task. Tone and clarity matter more than precision. |

This separation means:
- **Medical accuracy doesn't depend on model size, prompt engineering, or temperature settings**
- **Insights are reproducible**: the same data always produces the same insights
- **Insights are testable**: 300+ unit tests verify every classification, interaction, detection, screening, and blood type algorithm
- **Insights are auditable**: every single output traces to a published guideline and year

### Pure Functions, No Side Effects

The entire enrichment engine is a pure function:

```
enrich(medications, labs, allergies, vitals, registry, today, demographics) → Vec<ClinicalInsight>
```

No database queries. No network calls. No async. No LLM. No file I/O. This is deliberate:

- **Testable in isolation**: Pass in mock data, verify the output. No test fixtures, no mocking frameworks.
- **Deterministic**: Same input always produces same output. No randomness, no model drift.
- **Fast**: Microseconds, not seconds. Runs synchronously in the pipeline without blocking.
- **Safe**: Cannot corrupt state, leak data, or fail due to external dependencies.

### Graceful Degradation

If the JSON resource files are missing (first run, corrupted install, CI environment), the registry falls back to `InvariantRegistry::empty()`. The const tier (vital signs, lab thresholds) is always available because it's compiled into the binary. The bundled tier (drug families, interactions) degrades to empty. The pipeline continues without crashing; it simply produces fewer insights.

This means the app never fails because of the invariant system. It only gets smarter when the data is loaded.

---

## File Map

```
src-tauri/src/invariants/
├── mod.rs            InvariantRegistry: single access point, load(), lookup helpers
├── types.rs          ClinicalInsight, InsightKind, InsightSeverity, InvariantLabel, MeaningFactors
├── vitals.rs         31 vital sign tiers (BP, HR, SpO2, BMI, Glucose, Temperature)
├── labs.rs           10 lab tests, 47 tiers, 88 multilingual aliases
├── loader.rs         JSON deserializer for bundled tier (DrugFamily, InteractionPair, etc.)
├── enrich.rs         10 sub-algorithms: classify, detect, match, screen, trend, cross-react (the enrichment engine)
├── demographics.rs   Male hemoglobin tiers (WHO 2024), Asian BMI thresholds (WHO 2004)
├── screening.rs      14 schedules: 6 cancer screenings + 8 vaccine schedules (ME-04/ME-06)
├── allergens.rs      46 canonical allergen classes with mechanism + category (ALLERGY-01)
└── blood_types.rs    8 ABO/Rh blood types with compatibility matrix (BT-01)

src-tauri/resources/invariants/
├── drug_families.json              20 drug families with member medications
├── interaction_pairs.json          20+ clinically significant drug-drug interactions
├── cross_reactivity.json           10 allergen cross-reactivity chains
├── monitoring_schedules.json       24 drug-to-lab monitoring rules
├── allergen_cross_reactivity.json  OAS, food-food, insect venom, latex-fruit chains
└── allergen_aliases.json           88 common name → canonical allergen key mappings
```

---

## Verification

The invariant system is covered by **300+ unit tests** across the module, including:

- Threshold classification accuracy for every vital sign and lab test
- Boundary condition testing (exact threshold values)
- Drug interaction detection via direct name and family key
- Cross-reactivity chain matching with family resolution
- Same-family allergy detection
- Monitoring schedule lookup with family-based resolution
- Multilingual alias matching (French, German lab names)
- Empty registry graceful degradation
- Full patient scenario integration tests (multiple concurrent insights)
- Sex-aware hemoglobin classification (male vs female thresholds)
- Ethnicity-aware BMI classification (Asian vs standard thresholds)
- Age+sex-gated screening schedule eligibility (6 cancer + 8 vaccine)
- Demographics backward compatibility (None = universal defaults)
- Canonical allergen lookup (46 allergens, exact key + fuzzy label + alias match)
- Allergen classification and cross-reactivity (OAS, food-food, drug families)
- Blood type lookup, compatibility matrix (8 types, universal donor/recipient)
- Rh-negative awareness detection (sex+age guards)
- Screening record awareness (due, up-to-date, expired status)
- Vital sign trend detection (BP trending, weight loss/gain)

All tests run deterministically with no external dependencies: no database, no network, no model.

---

## Invariant Class Status

All 7 planned invariant classes are implemented, plus one additional class (I-BT) added in BT-01:

| Class | Status | What It Does |
|---|---|---|
| **I-VIT** Vital Signs | Implemented | 6 vital types, 31 classification tiers, trend detection. ME-04: Ethnicity-aware BMI (Asian thresholds) |
| **I-LAB** Laboratory | Implemented | 10 tests, 47 tiers, 88 aliases. ME-04: Sex-aware hemoglobin (Male 13.0, Female 12.0 g/dL) |
| **I-MED** Medications | Implemented | 20 families, 20+ interactions, 24 monitoring schedules |
| **I-ALG** Allergies | Implemented | 46 canonical allergens, 10 drug cross-reactivity chains, allergen cross-reactivity (OAS, food-food, latex-fruit), 88 aliases, auto-classification |
| **I-SCR** Screening | Implemented (ME-04/ME-06) | 14 schedules: 6 cancer screenings + 8 vaccine schedules. Record-aware (due/up-to-date/expired) |
| **I-BT** Blood Type | **Implemented (BT-01)** | 8 ABO/Rh types, compatibility matrix, Rh-negative pregnancy awareness |
| **I-FAM** Family Risk | Future | Family history risk modifiers for screening intervals |

The architecture is designed so that adding new invariant classes follows the same pattern: curate from guidelines, encode as const or JSON, add a detection algorithm to `enrich()`, write tests.

### Demographic-Aware Thresholds (ME-04)

The enrichment engine accepts an optional `PatientDemographics` parameter containing biological sex, ethnicity blend (1-3 populations), and age. When available, thresholds are personalized:

**Sex-Aware Hemoglobin (WHO 2024)**: Male normal threshold = 13.0 g/dL; Female = 12.0 g/dL. When sex is unknown, the conservative female threshold is used.

**Ethnicity-Aware BMI (WHO Expert Consultation 2004)**: South Asian, East Asian, and Pacific Islander populations use lower thresholds: overweight at 23.0, obese at 27.5 (vs standard 25.0, 30.0). If any ethnicity in the blend is Asian, Asian thresholds apply.

**Screening Schedules**: 6 age+sex-gated preventive screenings. See I-SCR section below.

### Where Demographics Come From: The Onboarding Connection

The demographics that personalize invariant thresholds originate from the **profile creation wizard** (UX-04). During onboarding, the user provides:

| Source | Field | Invariant Use |
|---|---|---|
| Profile Wizard Step 1 | Date of birth | Computed into `age_years`, which gates 14 preventive screening/vaccine schedules |
| Profile Wizard Step 2 | Biological sex | Selects sex-specific hemoglobin thresholds; gates sex-specific screenings (mammography, prostate); Rh-negative pregnancy awareness |
| Profile Wizard Step 2 | Ethnicity (up to 3) | Selects Asian BMI thresholds when South Asian, East Asian, or Pacific Islander is present |
| Me Screen Edit | Blood type (BT-01) | Rh-negative pregnancy awareness (enrich sub-algo 10); RAG identity context (Priority 0.5); compatibility matrix lookup |

The data chain flows through five components:

```
Profile Wizard / Me Screen Edit (Svelte)
    → ProfileInfo (stored on disk, encrypted)
        → CoreState.get_patient_demographics()
            → PatientDemographics { sex, ethnicities, age_context, age_years, blood_type }
                → enrich(..., demographics) in RAG pipeline
                → assemble_context(..., demographics) for blood type in RAG context
```

**Key design decisions**:

- **Country and address** (Step 3: Location) are stored in the profile but **not transferred** to `PatientDemographics`. They have no current invariant use and are reserved for future specialist locator features.
- **Name** is for identification only. It never reaches the enrichment pipeline or the SLM prompt.
- **Demographics are never sent to the SLM**. The model sees only the *derived clinical insights* (e.g., "Mild anemia, Hemoglobin 12.5 g/dL"), not the raw sex or ethnicity. This preserves privacy while enabling personalization.
- **Graceful degradation**: When demographics are absent (user skipped the health step), the engine uses conservative defaults (female hemoglobin threshold, standard BMI) and suppresses screening insights entirely. The system never fails; it only becomes more precise with more information.

---

## Complete Invariant Reference

For full transparency, every single invariant encoded in the system is listed below with its exact threshold, classification, and source guideline.

---

### I-VIT: Vital Sign Thresholds (Const Tier)

#### Blood Pressure (ISH 2020 Global Hypertension Practice Guidelines)

| Classification | Systolic (mmHg) | Diastolic (mmHg) | Significance | Source |
|---|---|---|---|---|
| Normal | < 130 | < 85 | 0.2 | ISH 2020 |
| High-Normal | 130–139 | 85–89 | 0.4 | ISH 2020 |
| Grade 1 Hypertension | 140–159 | 90–99 | 0.6 | ISH 2020 |
| Grade 2 Hypertension | >= 160 | >= 100 | 0.9 | ISH 2020 |

**Classification rule** (ISH 2020): When systolic and diastolic fall in different tiers, the **higher category** is used.

#### Heart Rate (ESC 2021 Pacing + ESC 2019 SVT Guidelines)

| Classification | Range (bpm) | Significance | Source |
|---|---|---|---|
| Severe Bradycardia | < 40 | 1.8 | ESC 2021 Pacing |
| Bradycardia | 40–49 | 1.2 | ESC 2021 Pacing |
| Low-Normal | 50–59 | 0.3 | ESC 2021 Pacing |
| Normal | 60–100 | 0.2 | ESC 2021 Pacing |
| Tachycardia | 101–150 | 1.0 | ESC 2019 SVT |
| Severe Tachycardia | > 150 | 1.8 | ESC 2019 SVT |

#### Oxygen Saturation / SpO2 (BTS 2017 Guideline for Oxygen Use)

| Classification | Range (%) | Significance | Source |
|---|---|---|---|
| Hypoxemia: supplemental oxygen indicated | < 90 | 1.8 | BTS 2017, WHO |
| COPD target range | 88–92 | 0.8 | BTS 2017 |
| Below action threshold, investigate | 90–93 | 1.2 | BTS 2017 |
| Lower limit of normal | 94 | 0.5 | BTS 2017 |
| Normal | >= 95 | 0.2 | BTS 2017 |

**Note**: COPD patients use a different target range (88–92%). The general population classifier treats < 94% as actionable.

#### Body Mass Index / BMI (WHO Technical Report Series 894)

| Classification | Range (kg/m²) | Significance | Source |
|---|---|---|---|
| Underweight | < 18.5 | 1.2 | WHO TRS 894 |
| Normal weight | 18.5–24.9 | 0.2 | WHO TRS 894 |
| Overweight (pre-obese) | 25.0–29.9 | 0.5 | WHO TRS 894 |
| Obese Class I | 30.0–34.9 | 0.8 | WHO TRS 894 |
| Obese Class II | 35.0–39.9 | 1.2 | WHO TRS 894 |
| Obese Class III | >= 40.0 | 1.5 | WHO TRS 894 |

**Asian thresholds** (WHO Expert Consultation, Lancet 2004): Overweight >= 23.0 kg/m², Obese >= 27.5 kg/m². Defined as constants, applied at enrichment layer when ethnicity is known.

#### Fasting Glucose (WHO 2006 Diagnostic Criteria)

| Classification | Range (mmol/L) | Range (mg/dL) | Significance | Source |
|---|---|---|---|---|
| Normal | < 6.1 | < 110 | 0.2 | WHO 2006 |
| Impaired Fasting Glucose (pre-diabetes) | 6.1–6.9 | 110–125 | 0.8 | WHO 2006 |
| Diabetes range | >= 7.0 | >= 126 | 1.5 | WHO 2006 |

**Conversion**: mmol/L x 18.0 = mg/dL

#### Body Temperature (WHO / Clinical Standard)

| Classification | Range (°C) | Significance | Source |
|---|---|---|---|
| Hypothermia | < 35.0 | 1.5 | WHO |
| Below normal | 35.0–36.0 | 0.5 | Clinical standard |
| Normal | 36.1–37.1 | 0.2 | Clinical standard |
| Low-grade fever | 37.2–37.9 | 0.5 | Clinical standard |
| Fever | 38.0–38.9 | 1.0 | WHO |
| High fever | 39.0–40.9 | 1.5 | WHO |
| Hyperthermia (emergency) | >= 41.0 | 2.0 | WHO |

#### Trend Thresholds

| Metric | Threshold | Timeframe | Source |
|---|---|---|---|
| Systolic BP trend | >= +20 mmHg | 6–12 months | ISH 2020 |
| Diastolic BP trend | >= +10 mmHg | 6–12 months | ISH 2020 |
| Orthostatic hypotension (SBP drop) | >= 20 mmHg | Within 3 min of standing | ESC 2018 Syncope |
| Orthostatic hypotension (DBP drop) | >= 10 mmHg | Within 3 min of standing | ESC 2018 Syncope |
| Orthostatic SBP floor | < 90 mmHg | Absolute | ESC 2018 Syncope |
| Weight loss (moderate) | > 5% | <= 6 months | GLIM 2019 |
| Weight loss (severe) | > 10% | <= 6 months | GLIM 2019 |
| Weight gain | >= 5% | 6 months | GLIM 2019 |

---

### I-LAB: Laboratory Thresholds (Const Tier)

Each lab test includes multilingual **aliases** for name normalization. 88 aliases across 10 tests ensure that French, German, and English lab names all resolve to the correct threshold.

#### 1. eGFR: Kidney Function (KDIGO 2024)

**Unit**: mL/min/1.73m² | **Trend threshold**: > 20% change between consecutive tests

| Stage | Range | Classification | Significance |
|---|---|---|---|
| G5 | < 15 | Kidney failure | 2.0 |
| G4 | 15–29 | Severely decreased | 1.8 |
| G3b | 30–44 | Moderately to severely decreased | 1.4 |
| G3a | 45–59 | Mildly to moderately decreased | 1.0 |
| G2 | 60–89 | Mildly decreased | 0.4 |
| G1 | >= 90 | Normal or high | 0.2 |

**Aliases** (10): eGFR, GFR, estimated GFR, glomerular filtration rate, DFG, DFGe, débit de filtration glomérulaire, GFR geschätzt, glomeruläre Filtrationsrate

#### 2. HbA1c: Diabetes (IDF 2025, WHO)

**Unit**: %  | **Trend threshold**: > 0.5% increase over 6 months

| Classification | Range (%) | Significance |
|---|---|---|
| Normal | < 5.7 | 0.2 |
| Pre-diabetes | 5.7–6.4 | 0.8 |
| Diabetes | >= 6.5 | 1.5 |

**Aliases** (9): HbA1c, A1c, glycated hemoglobin, glycosylated hemoglobin, hémoglobine glyquée, hémoglobine A1c, glykiertes Hämoglobin, Glykohämoglobin

#### 3. LDL Cholesterol (ESC/EAS 2019/2025 Risk-Stratified Targets)

**Unit**: mmol/L

| Classification | Range (mmol/L) | Significance |
|---|---|---|
| Below extreme-risk target | < 1.0 | 0.2 |
| Very-high-risk range | 1.0–1.3 | 0.3 |
| High-risk range | 1.4–1.7 | 0.5 |
| Moderate-risk range | 1.8–2.5 | 0.6 |
| Low-risk range | 2.6–2.9 | 0.4 |
| Elevated | >= 3.0 | 0.8 |

**Note**: Classification depends on patient cardiovascular risk category. These tiers provide context; clinician assessment determines target.

**Aliases** (8): LDL, LDL-C, LDL cholesterol, low-density lipoprotein, cholestérol LDL, LDL-cholestérol, LDL-Cholesterin, LDL Cholesterin

#### 4. Potassium / K+ (KDIGO 2024)

**Unit**: mmol/L

| Classification | Range (mmol/L) | Significance |
|---|---|---|
| Severe hypokalemia | < 3.0 | 2.0 |
| Hypokalemia | 3.0–3.4 | 1.3 |
| Normal | 3.5–4.9 | 0.2 |
| Mild hyperkalemia | 5.0–5.4 | 0.8 |
| Hyperkalemia, review medications | 5.5–5.9 | 1.5 |
| Severe hyperkalemia (emergency) | >= 6.0 | 2.0 |

**Aliases** (8): K, K+, potassium, serum potassium, kaliémie, potassium sérique, Kalium, Serum-Kalium

#### 5. Sodium / Na+ (Clinical Standard)

**Unit**: mmol/L

| Classification | Range (mmol/L) | Significance |
|---|---|---|
| Severe hyponatremia | < 125 | 2.0 |
| Hyponatremia | 125–134 | 1.3 |
| Normal | 135–144 | 0.2 |
| Hypernatremia | >= 145 | 1.3 |

**Aliases** (8): Na, Na+, sodium, serum sodium, natrémie, sodium sérique, Natrium, Serum-Natrium

#### 6. ALT / Liver Function (EASL DILI Guidelines)

**Unit**: U/L | **ULN** (Upper Limit of Normal) = 40 U/L

| Classification | Range (U/L) | Significance |
|---|---|---|
| Normal | < 40 (< 1x ULN) | 0.2 |
| Mildly elevated | 40–119 (1–3x ULN) | 0.6 |
| Hepatocellular injury | 120–199 (3–5x ULN) | 1.5 |
| Severe liver injury | >= 200 (> 5x ULN) | 2.0 |

**Aliases** (11): ALT, SGPT, ALAT, alanine aminotransferase, alanine transaminase, alanine aminotransférase, transaminase ALAT, Alanin-Aminotransferase, GPT

#### 7. Hemoglobin (WHO)

**Unit**: g/dL | **Trend threshold**: > 2 g/dL drop over 3 months → urgent investigation

| Classification | Range (g/dL) | Significance |
|---|---|---|
| Severe anemia | < 8.0 | 2.0 |
| Moderate anemia | 8.0–10.9 | 1.3 |
| Mild anemia | 11.0–11.9 | 0.6 |
| Normal | 12.0–17.4 | 0.2 |
| Polycythemia | >= 17.5 | 1.0 |

**Note**: Uses female threshold (12.0 g/dL) as conservative default. WHO gender-specific: Female 12.0, Male 13.0. Sex-specific correction applied at enrichment layer when patient sex is known.

**Aliases** (8): Hb, Hgb, hemoglobin, haemoglobin, hémoglobine, Hämoglobin

#### 8. TSH / Thyroid (European Thyroid Association, ETA)

**Unit**: mU/L

| Classification | Range (mU/L) | Significance |
|---|---|---|
| Suppressed (hyperthyroidism) | < 0.4 | 1.3 |
| Normal | 0.4–3.9 | 0.2 |
| Subclinical hypothyroidism | 4.0–9.9 | 0.8 |
| Overt hypothyroidism | >= 10.0 | 1.5 |

**Aliases** (7): TSH, thyroid stimulating hormone, thyrotropin, thyréostimuline, Thyreotropin

#### 9. uACR / Kidney Damage (KDIGO 2024)

**Unit**: mg/g

| Stage | Range (mg/g) | Classification | Significance |
|---|---|---|---|
| A1 | < 30 | Normal to mildly increased | 0.2 |
| A2 | 30–299 | Moderately increased (microalbuminuria) | 1.0 |
| A3 | >= 300 | Severely increased (macroalbuminuria) | 1.8 |

**Aliases** (8): uACR, ACR, albumin-creatinine ratio, urine albumin creatinine ratio, RAC, rapport albumine/créatinine urinaire, Albumin-Kreatinin-Verhältnis

#### 10. Vitamin D (Endocrine Society / IOF)

**Unit**: ng/mL

| Classification | Range (ng/mL) | Significance |
|---|---|---|
| Deficiency | < 20 | 1.0 |
| Insufficiency | 20–29 | 0.5 |
| Sufficient | 30–99 | 0.2 |
| Excess (toxicity risk) | >= 100 | 1.3 |

**Aliases** (10): vitamin D, 25-OH vitamin D, 25-hydroxyvitamin D, calcidiol, vitamine D, 25-OH vitamine D, Vitamin D, 25-OH-Vitamin-D, Calcidiol

---

### I-MED: Drug Families (Bundled Tier, JSON)

20 drug families, 125 member medications. Source guidelines listed per family.

| # | Family Key | Family Name | Members | Source |
|---|---|---|---|---|
| 1 | `penicillin` | Penicillins | penicillin, amoxicillin, ampicillin, piperacillin, oxacillin, nafcillin, dicloxacillin, flucloxacillin | WHO EML |
| 2 | `cephalosporin` | Cephalosporins | cephalexin, cefazolin, ceftriaxone, cefuroxime, cefixime, cefpodoxime, ceftazidime, cefadroxil, cefprozil, cefaclor | WHO EML |
| 3 | `sulfonamide` | Sulfonamides | sulfamethoxazole, sulfasalazine, sulfadiazine, trimethoprim-sulfamethoxazole, sulfisoxazole | WHO EML |
| 4 | `nsaid` | NSAIDs | ibuprofen, naproxen, diclofenac, indomethacin, piroxicam, meloxicam, celecoxib, aspirin, ketorolac, etoricoxib | WHO EML, STOPP/START v3 |
| 5 | `statin` | Statins | atorvastatin, rosuvastatin, simvastatin, pravastatin, lovastatin, fluvastatin, pitavastatin | ESC/EAS 2019, WHO EML |
| 6 | `ace_inhibitor` | ACE Inhibitors | lisinopril, enalapril, ramipril, captopril, benazepril, fosinopril, quinapril, perindopril | KDIGO 2024, WHO EML |
| 7 | `arb` | Angiotensin II Receptor Blockers | losartan, valsartan, irbesartan, candesartan, telmisartan, olmesartan | KDIGO 2024, WHO EML |
| 8 | `opioid` | Opioids | morphine, codeine, hydrocodone, oxycodone, tramadol, fentanyl, methadone, hydromorphone, meperidine, buprenorphine | WHO Pain Ladder, STOPP/START v3 |
| 9 | `fluoroquinolone` | Fluoroquinolones | ciprofloxacin, levofloxacin, moxifloxacin, norfloxacin, ofloxacin | WHO EML |
| 10 | `macrolide` | Macrolides | azithromycin, clarithromycin, erythromycin | WHO EML |
| 11 | `tetracycline` | Tetracyclines | tetracycline, doxycycline, minocycline | WHO EML |
| 12 | `beta_blocker` | Beta-Blockers | metoprolol, atenolol, bisoprolol, propranolol, carvedilol, nebivolol | ESC 2023, WHO EML |
| 13 | `doac` | Direct Oral Anticoagulants | apixaban, rivaroxaban, dabigatran, edoxaban | EHRA 2021, ESC 2024 |
| 14 | `ssri` | SSRIs | sertraline, escitalopram, fluoxetine, citalopram, paroxetine, fluvoxamine | WHO EML, STOPP/START v3 |
| 15 | `ppi` | Proton Pump Inhibitors | omeprazole, pantoprazole, esomeprazole, lansoprazole, rabeprazole | STOPP/START v3, WHO EML |
| 16 | `anticonvulsant` | Anticonvulsants | valproate, carbamazepine, phenytoin, lamotrigine, levetiracetam, topiramate | ILAE, WHO EML |
| 17 | `sulfonylurea` | Sulfonylureas | gliclazide, glimepiride, glipizide, glyburide | WHO EML, KDIGO |
| 18 | `benzodiazepine` | Benzodiazepines | diazepam, lorazepam, alprazolam, clonazepam, midazolam, oxazepam | WHO EML, STOPP/START v3 |
| 19 | `thiazide` | Thiazide Diuretics | hydrochlorothiazide, chlorthalidone, indapamide, metolazone | ESC 2023, WHO EML |
| 20 | `k_sparing_diuretic` | Potassium-Sparing Diuretics | spironolactone, eplerenone, amiloride, triamterene | KDIGO, STOPP/START v3 |

---

### I-MED: Drug-Drug Interactions (Bundled Tier, JSON)

16 clinically significant interaction pairs. Severity: **Critical** (potentially fatal) > **High** (serious harm risk) > **Moderate** (requires monitoring).

| # | Drug A | Drug B | Severity | Clinical Description | Source |
|---|---|---|---|---|---|
| 1 | SSRI | MAOI | **Critical** | Serotonin accumulation leading to serotonin syndrome (potentially fatal). Minimum 14-day washout required (5 weeks for fluoxetine). | WHO EML, EMA, STOPP/START v3 |
| 2 | Alcohol | Benzodiazepine | **Critical** | Additive CNS depression causing respiratory depression, potentially fatal. | WHO, STOPP/START v3 |
| 3 | Alcohol | Opioid | **Critical** | Additive CNS depression causing respiratory depression, potentially fatal. | WHO, STOPP/START v3 |
| 4 | Lithium | NSAID | **Critical** | NSAIDs reduce lithium renal clearance by 25–40%, causing lithium toxicity (narrow therapeutic index). | WHO EML, BNF |
| 5 | SSRI | Tramadol | High | Serotonergic synergy increasing risk of serotonin syndrome. | FAERS 2025, EMA |
| 6 | Warfarin | Fluoroquinolone | High | CYP1A2 inhibition and vitamin K disruption causing excessive bleeding risk. | EMA, MHRA |
| 7 | Warfarin | NSAID | High | Antiplatelet and anticoagulant synergy. GI bleeding risk doubles. | STOPP/START v3, ESC |
| 8 | ACE Inhibitor | K-Sparing Diuretic | High | Reduced potassium excretion leading to hyperkalemia and cardiac arrhythmia risk. | KDIGO, STOPP/START v3 |
| 9 | Statin | Clarithromycin | High | CYP3A4 inhibition reduces statin metabolism. Risk of rhabdomyolysis (muscle breakdown leading to renal failure). | ESC/EAS 2019, EMA |
| 10 | Beta-Blocker | Verapamil | High | Additive AV node depression causing severe bradycardia or heart block. | STOPP/START v3, ESC |
| 11 | Beta-Blocker | Diltiazem | High | Additive AV node depression causing severe bradycardia or heart block. | STOPP/START v3, ESC |
| 12 | Lithium | Thiazide | High | Thiazides increase lithium reabsorption. Lithium levels rise 25–40%, risking toxicity. | WHO EML, Australian Prescriber |
| 13 | Digoxin | Amiodarone | High | Amiodarone inhibits P-glycoprotein and renal clearance, doubling digoxin levels. Mandatory 50% dose reduction. | ESC, WHO EML |
| 14 | Digoxin | Verapamil | High | Reduced renal and hepatic digoxin clearance leading to digoxin toxicity. | ESC |
| 15 | Digoxin | K-Depleting Diuretic | High | Hypokalemia potentiates digoxin toxicity, causing cardiac arrhythmia at otherwise therapeutic levels. | ESC, WHO EML |
| 16 | Valproate | Carbamazepine | High | Valproate inhibits epoxide hydrolase, causing toxic carbamazepine-10,11-epoxide accumulation. Neurotoxicity despite normal carbamazepine levels. | ILAE, WHO EML |
| 17 | Metformin | Iodinated Contrast | Moderate | Contrast-induced nephropathy may reduce metformin clearance, risking lactic acidosis. Hold if eGFR < 30 or AKI risk. | ESUR 2025 |

**Matching logic**: Interactions match by direct drug name **and** by drug family key. If a patient takes ibuprofen (NSAID family) and warfarin, the Warfarin + NSAID interaction is detected.

---

### I-ALG: Cross-Reactivity Chains (Bundled Tier, JSON)

10 allergen cross-reactivity chains with evidence-based rates and clinical action guidance.

| # | Primary Allergen | Cross-Reactive With | Rate | Action | Source |
|---|---|---|---|---|---|
| 1 | Aminopenicillin | Aminocephalosporin | ~16.5% skin test positive, ~1.9% clinical | Skin test before use. Higher risk than dissimilar-chain cephalosporins. | EAACI 2020 |
| 2 | Penicillin | Cephalosporin (dissimilar R1) | ~2.1% skin test positive | Generally safe; assess prior reaction severity. | EAACI 2020 |
| 3 | Penicillin | 3rd/4th gen Cephalosporin | < 1% (0.3% in challenge studies) | Generally safe. | Meta-analysis PMC7822086 |
| 4 | Penicillin | Carbapenem | < 1% (0.87%, 95% CI 0.32–2.32) | Safe even with anaphylactic penicillin history (conditional recommendation). | Picard et al. 2019 |
| 5 | Penicillin | Monobactam (aztreonam) | Negligible | Safe to use. Exception: aztreonam shares R1 with ceftazidime. | ICON 2014 |
| 6 | Sulfonamide antibiotic | Non-antibiotic sulfonamide (furosemide, thiazide, celecoxib) | No immunological cross-reactivity | Safe: different antigenic determinant (arylamine group absent). | ICON 2014 |
| 7 | Latex | Banana, avocado, kiwi, chestnut | 30–50% (up to 70%) | Food allergy counseling; latex-fruit syndrome. | WAO, JCM 2024 |
| 8 | Fluoroquinolone | Other fluoroquinolone | 2–5% (dual IgE + MRGPRX2) | Levofloxacin safest alternative. Skin test + challenge for others. | EAACI, OFID 2022 |
| 9 | Morphine | Codeine | MRGPRX2-mediated pseudoallergy (not IgE) | Synthetic opioids (fentanyl, tramadol) rarely cross-react. Meperidine safe alternative. | EAACI 2024 |
| 10 | PEG | Polysorbate 80 | Suggested but not consistently demonstrated | Skin testing recommended for mRNA vaccine allergy evaluation. | EAACI/ENDA 2022 |

**Same-family detection**: In addition to cross-family chains, the system detects when an allergen and an active medication belong to the **same drug family** (e.g., penicillin allergy + amoxicillin prescription). This uses the drug family registry above.

**Allergen-specific cross-reactivity** (ALLERGY-01): A separate set of chains covers non-drug allergen relationships: oral allergy syndrome (OAS, e.g., birch pollen - apple), food-food (e.g., peanut - tree nut), insect venom (e.g., bee - wasp), and extended latex-fruit (latex - banana, avocado, kiwi, chestnut). These are stored in `allergen_cross_reactivity.json` and searched via `find_all_cross_reactivity()` which combines both drug and allergen chains.

**Canonical allergen classification** (ALLERGY-01): 46 canonical allergens in const tier (`allergens.rs`) enable auto-classification of free-text allergen entries. Resolution runs: exact key -> alias match (88 aliases from JSON) -> fuzzy label match (substring on EN/FR/DE). See I-ALG section in the Complete Invariant Reference below.

---

### I-MED: Monitoring Schedules (Bundled Tier, JSON)

24 drug-to-lab monitoring rules. When a patient takes a medication and the required lab test is overdue, the system generates a "Missing Monitoring" insight.

| # | Drug / Family | Lab Test | Interval (days) | Clinical Context | Source |
|---|---|---|---|---|---|
| 1 | Metformin | HbA1c | 90 | Quarterly when not at glycemic target | ADA/KDIGO 2022 |
| 2 | Metformin | eGFR | 365 | Annually; every 90–180 days if eGFR 30–60. Contraindicated if eGFR < 30 | KDIGO 2024 |
| 3 | Warfarin | INR | 30 | Monthly when stable; weekly during initiation. Target INR 2.0–3.0 | ESC 2024, ISTH |
| 4 | Statin (family) | ALT | 365 | Baseline + repeat if symptoms. Discontinue if ALT > 3x ULN | ESC/EAS 2019 |
| 5 | Statin (family) | LDL Cholesterol | 365 | 4–12 weeks after start, then annually | ESC/EAS 2019 |
| 6 | ACE Inhibitor (family) | Potassium | 90 | 7–14 days after start/change, then every 3–6 months. Alert if K > 5.5 | KDIGO 2024 |
| 7 | ACE Inhibitor (family) | eGFR | 90 | 7–14 days after start/change, then every 3–6 months. Investigate if creatinine rise > 30% | KDIGO 2024 |
| 8 | ARB (family) | Potassium | 90 | Same monitoring as ACE inhibitors. Alert if K > 5.5 | KDIGO 2024 |
| 9 | ARB (family) | eGFR | 90 | Same monitoring as ACE inhibitors | KDIGO 2024 |
| 10 | Lithium | eGFR | 180 | Every 6 months. Narrow therapeutic index 0.6–1.2 mEq/L | WHO EML, NICE |
| 11 | Lithium | TSH | 180 | Every 6 months; thyroid dysfunction in significant proportion of patients | WHO EML, NICE |
| 12 | Amiodarone | TSH | 180 | Every 6 months; thyroid dysfunction in 4–30% of patients. Half-life ~40–55 days | ESC, EMA |
| 13 | Amiodarone | ALT | 180 | Every 6 months; hepatotoxicity monitoring | ESC, EMA |
| 14 | Levothyroxine | TSH | 365 | 4–8 weeks after dose change; annually when stable | ETA, WHO EML |
| 15 | DOAC (family) | eGFR | 180 | Every 6–12 months (EHRA rule: CrCl/10 = months). Dabigatran contraindicated if CrCl < 30 | EHRA 2021, ESC 2024 |
| 16 | DOAC (family) | Hemoglobin | 365 | Annual CBC; monitor for occult bleeding | EHRA 2021 |
| 17 | Gliclazide | HbA1c | 90 | Quarterly; hypoglycemia monitoring. Alert if glucose < 3.9 mmol/L | WHO EML, KDIGO |
| 18 | Glimepiride | HbA1c | 90 | Quarterly; hypoglycemia monitoring | WHO EML, KDIGO |
| 19 | Valproate | ALT | 180 | Every 6 months, especially first 6 months. Discontinue if ALT > 3x ULN | ILAE, WHO EML |
| 20 | Digoxin | Potassium | 180 | Every 6–12 months. Hypokalemia potentiates digoxin toxicity | WHO EML, ESC |
| 21 | Digoxin | eGFR | 180 | Every 6–12 months; renal function affects digoxin clearance | WHO EML, ESC |
| 22 | NSAID (family) | eGFR | 180 | Periodic, especially in elderly or CKD. Discontinue if eGFR declines | STOPP/START v3, WHO EML |
| 23 | SSRI (family) | Sodium | 180 | Baseline + periodic in elderly; hyponatremia risk. Alert if Na < 130 | STOPP/START v3, EMA |
| 24 | K-Sparing Diuretic (family) | Potassium | 90 | Every 3–6 months; hyperkalemia risk. Alert if K > 5.5 | KDIGO, STOPP/START v3 |

**Family-based resolution**: When a monitoring schedule is keyed by a family name (e.g., "statin"), any member drug (atorvastatin, rosuvastatin, simvastatin...) will match. The system resolves "lisinopril" → ACE Inhibitor family → finds ACE Inhibitor monitoring schedules for K+ and eGFR.

---

### I-SCR: Screening and Vaccine Schedules (ME-04/ME-06)

14 evidence-based schedules (6 cancer screenings + 8 vaccine schedules), age+sex-gated. All produce `InsightKind::ScreeningDue` with `InsightSeverity::Info`. Record-aware: screening records (migration 021) track completion dates and suppress reminders when up to date.

#### Cancer Screenings (6)

| # | Screening | Sex | Age Range | Interval | Source |
|---|---|---|---|---|---|
| 1 | Mammography | Female | 50-74 | 24 months | IARC/WHO 2024 |
| 2 | Cervical (Pap/HPV) | Female | 25-65 | 36 months | WHO 2021 |
| 3 | Prostate (PSA) | Male | 50-70 | 12 months | EAU 2024 |
| 4 | Colorectal (FIT/colonoscopy) | Both | 50-75 | 24 months | IARC 2019 |
| 5 | AAA Ultrasound | Male | 65-75 | One-time | ESC 2024 |
| 6 | Osteoporosis (DXA) | Female | 65+ | 24 months | IOF 2024 |

#### Vaccine Schedules (8)

| # | Vaccine | Sex | Age Range | Interval / Doses | Validity | Source |
|---|---|---|---|---|---|---|
| 7 | Influenza (seasonal) | Both | 18+ | 12 months (recurring) | 12 months | WHO 2024 |
| 8 | Tdap (Tetanus-Diphtheria-Pertussis) | Both | 18+ | 120 months (recurring) | 120 months | WHO 2024 |
| 9 | Pneumococcal (PCV/PPSV) | Both | 65+ | 1 dose | Lifetime | WHO 2024 |
| 10 | Shingles (Herpes Zoster) | Both | 50+ | 2-dose series | Lifetime | WHO 2024 |
| 11 | Hepatitis B | Both | 18+ | 3-dose series | Lifetime | WHO 2024 |
| 12 | HPV | Both | 18-26 | 3-dose series | Lifetime | WHO 2024 |
| 13 | MMR | Both | 18+ | 2-dose series | Lifetime | WHO 2024 |
| 14 | COVID-19 | Both | 18+ | 12 months (recurring) | 12 months | WHO SAGE 2024 |

**Record awareness (ME-06)**: Each schedule is checked against screening records stored in `screening_records` (SQLite table). Status per schedule: **due** (no record or interval expired), **up-to-date** (record within interval or series complete), **expired** (validity window elapsed). The `build_screening_info()` function in `me.rs` computes status; `enrich()` stays pure (no DB access).

---

### I-ALG: Canonical Allergens (Const Tier, ALLERGY-01)

46 canonical allergen classes compiled into binary. Each entry carries a machine key, category, immune mechanism, trilingual label, and clinical guideline source. Used for auto-classification of free-text allergen entries and cross-reactivity resolution.

#### Categories (5)

| Category | Count | Examples | Source |
|---|---|---|---|
| Food | 15 | Peanut, tree nuts, milk, egg, shellfish, wheat, soy, sesame, fish, mustard, celery, lupin, mollusks, buckwheat, corn | FDA FALCPA 2004, FASTER Act 2021, EU 1169/2011 |
| Drug | 13 | Beta-lactam, NSAID, sulfonamide, fluoroquinolone, opioid, local anesthetic, contrast media, anticonvulsant, statin, ACE inhibitor, PEG, polysorbate | AAAAI 2022, EAACI/ENDA 2022 |
| Environmental | 10 | Dust mite, cat, dog, tree pollen (birch, cedar, oak), grass pollen, ragweed, mold (Alternaria, Aspergillus) | WAO/ARIA 2024 |
| Insect | 4 | Honeybee, wasp, fire ant, mosquito | AAAAI 2016 |
| Other | 4 | Latex, nickel, formaldehyde, sunscreen (benzophenone) | EAACI/ENDA 2022, ESCD 2020 |

#### Resolution Pipeline

1. **Exact key match**: `"food_peanut"` -> food_peanut
2. **Alias match** (88 aliases from bundled JSON): `"arachide"` -> food_peanut, `"penicillin"` -> drug_beta_lactam
3. **Fuzzy label match**: `"peanut"` -> substring match on EN/FR/DE labels -> food_peanut

The registry's `classify_allergen()` method runs all three steps in order, returning the first match.

---

### I-BT: Blood Types (Const Tier, BT-01)

8 ABO/Rh blood types compiled into binary with full transfusion compatibility matrix.

#### Blood Type Catalog

| Key | Display | ABO Group | Rh Factor | Global Frequency | Source |
|---|---|---|---|---|---|
| `o_positive` | O+ | O | + | ~38% | ISBT 2023, AABB 21st Ed. |
| `o_negative` | O- | O | - | ~7% | ISBT 2023, AABB 21st Ed. |
| `a_positive` | A+ | A | + | ~27% | ISBT 2023, AABB 21st Ed. |
| `a_negative` | A- | A | - | ~6% | ISBT 2023, AABB 21st Ed. |
| `b_positive` | B+ | B | + | ~22% | ISBT 2023, AABB 21st Ed. |
| `b_negative` | B- | B | - | ~2% | ISBT 2023, AABB 21st Ed. |
| `ab_positive` | AB+ | AB | + | ~5% | ISBT 2023, AABB 21st Ed. |
| `ab_negative` | AB- | AB | - | ~1% | ISBT 2023, AABB 21st Ed. |

#### RBC Transfusion Compatibility Matrix

| Recipient | Can Receive RBC From |
|---|---|
| O+ | O+, O- |
| O- | O- (universal donor) |
| A+ | A+, A-, O+, O- |
| A- | A-, O- |
| B+ | B+, B-, O+, O- |
| B- | B-, O- |
| AB+ | All 8 types (universal recipient) |
| AB- | AB-, A-, B-, O- |

#### Clinical Significance

- **Universal donor**: O- can donate RBCs to all types (emergency transfusion)
- **Universal recipient**: AB+ can receive RBCs from all types
- **Rh sensitization**: Rh-negative individuals risk anti-D antibodies if exposed to Rh-positive blood
- **Pregnancy awareness**: Rh-negative mother may need anti-D prophylaxis (ACOG PB 181, RCOG GTG 65)

#### Storage and Display

Blood type is stored on `ProfileInfo` (encrypted, not SQLite) alongside sex and ethnicities. Displayed as a compact badge on the Me Screen ProfileCard ("O+", "AB-"). Editable via 4x2 grid selector in EditDemographicsModal. Injected into RAG context at Priority 0.5 as identity-level information.

---

### Source Guideline Index

All guidelines referenced in the invariant system, alphabetically:

| Abbreviation | Full Name | Year |
|---|---|---|
| AABB | American Association of Blood Banks, Technical Manual 21st Ed. | 2023 |
| AAAAI | American Academy of Allergy, Asthma and Immunology | 2016/2022 |
| ACOG | American College of Obstetricians and Gynecologists, Practice Bulletin 181 | 2017 |
| ADA/KDIGO | American Diabetes Association / Kidney Disease: Improving Global Outcomes | 2022 |
| BNF | British National Formulary | - |
| BTS | British Thoracic Society: Guideline for Oxygen Use | 2017 |
| CDC ACIP | Centers for Disease Control, Advisory Committee on Immunization Practices | 2024 |
| EAU | European Association of Urology | 2024 |
| EAACI | European Academy of Allergy and Clinical Immunology | 2020/2024 |
| EAACI/ENDA | EAACI / European Network for Drug Allergy | 2022 |
| EASL | European Association for the Study of the Liver, DILI Guidelines | - |
| EHRA | European Heart Rhythm Association | 2021 |
| EMA | European Medicines Agency | - |
| ESC | European Society of Cardiology | 2018/2019/2021/2023/2024 |
| ESC/EAS | ESC / European Atherosclerosis Society | 2019/2025 |
| ESCD | European Society of Contact Dermatitis | 2020 |
| ESUR | European Society of Urogenital Radiology | 2025 |
| ETA | European Thyroid Association | - |
| EU 1169/2011 | EU Food Information to Consumers Regulation | 2011 |
| FAERS | FDA Adverse Event Reporting System | 2025 |
| FDA FALCPA | Food Allergen Labeling and Consumer Protection Act | 2004 |
| FASTER Act | Food Allergy Safety, Treatment, Education, and Research Act | 2021 |
| GLIM | Global Leadership Initiative on Malnutrition | 2019 |
| IARC | International Agency for Research on Cancer | 2019/2024 |
| ICON | International Consensus on Drug Allergy | 2014 |
| IDF | International Diabetes Federation | 2025 |
| ILAE | International League Against Epilepsy | - |
| IOF | International Osteoporosis Foundation | 2024 |
| ISH | International Society of Hypertension: Global Practice Guidelines | 2020 |
| ISBT | International Society of Blood Transfusion | 2023 |
| ISTH | International Society on Thrombosis and Haemostasis | - |
| KDIGO | Kidney Disease: Improving Global Outcomes | 2024 |
| MHRA | Medicines and Healthcare products Regulatory Agency (UK) | - |
| NICE | National Institute for Health and Care Excellence (UK) | - |
| RCOG | Royal College of Obstetricians and Gynaecologists, Green-top Guideline 65 | 2017 |
| STOPP/START v3 | Screening Tool of Older Persons' Prescriptions / Screening Tool to Alert to Right Treatment, v3 | - |
| WAO | World Allergy Organization | 2024 |
| WAO/ARIA | World Allergy Organization / Allergic Rhinitis and its Impact on Asthma | 2024 |
| WHO | World Health Organization | Various |
| WHO EML | WHO Essential Medicines List | - |
| WHO SAGE | WHO Strategic Advisory Group of Experts on Immunization | 2024 |
| WHO TRS 894 | WHO Technical Report Series 894: Obesity | 2000 |

---

## Summary

Invariants in Coheara are not a feature. They are the reason Coheara can be trusted with medical data.

A model that guesses whether blood pressure is dangerous is a liability. A system that looks it up in ISH 2020 and tells the model the answer is a safety architecture. The invariant engine ensures that the hardest, most safety-critical medical reasoning (classification, interaction detection, allergy contraindication, blood type compatibility, monitoring compliance, screening schedules) is never delegated to a component that can be wrong.

The SLM speaks. The invariants know.

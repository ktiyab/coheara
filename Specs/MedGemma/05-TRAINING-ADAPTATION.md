# Spec-MG-05: Training Data & Adaptation Methods
## MedGemma 1.5 4B — Coheara SLM Reference

**Version:** 1.0
**Last Updated:** 2026-02-11
**Parent:** 00-MEDGEMMA-INDEX.md
**Source:** Google HuggingFace model card, Google Health AI Developer Foundations

---

## PURPOSE

This document catalogs MedGemma's training data (what the model already knows) and adaptation methods (how to extend it for Coheara-specific tasks). Understanding what data shaped the model informs where it is strong, where it has gaps, and what fine-tuning targets will yield the highest return.

---

## TRAINING DATA

### Public Datasets

| Dataset | Domain | Content | Coheara Relevance |
|---------|--------|---------|-------------------|
| **MIMIC-CXR** | Radiology | 377K chest X-rays with free-text radiology reports | CXR report generation, SLM-03 handoff context |
| **ChestImaGenome** | Radiology | Bounding boxes linking radiographic findings to anatomical structures | Anatomical localization (1.5 feature) |
| **SLAKE** | Multimodal VQA | Medical images with question-answer pairs (radiology, pathology, etc.) | Visual QA capabilities |
| **PAD-UFES-20** | Dermatology | Skin lesion images (6 categories) | Dermatology classification |
| **SCIN** | Dermatology | Dermatology images from diverse skin tones | Bias mitigation for skin tone diversity |
| **TCGA** | Pathology/Genomics | Cancer genomics with histopathology images | Histopathology analysis |
| **CAMELYON** | Pathology | Lymph node histopathology (metastasis detection) | Pathology classification |
| **PMC-OA** | Biomedical literature | Open-access PubMed Central articles with images | Medical knowledge, text comprehension |
| **Mendeley Digital Knee X-Ray** | Orthopedics | Knee X-ray images | Orthopedic imaging |
| **MedQA** | Medical QA | USMLE-style questions (4-option) | Clinical reasoning, medical knowledge |
| **MedMCQA** | Medical QA | Indian medical entrance exam questions | Broad medical knowledge |
| **PubMedQA** | Biomedical QA | Research-based questions from PubMed abstracts | Evidence assessment |
| **VQA-RAD** | Radiology VQA | Radiology visual question answering | Image-based clinical QA |
| **AfriMed-QA** | Medical QA | African medical context questions | Regional medical knowledge diversity |
| **MedXpertQA** | Expert medical QA | Expert-level medical questions | Upper-bound clinical reasoning |

### Proprietary / Licensed Datasets

| Dataset | Domain | Content | Significance |
|---------|--------|---------|-------------|
| **De-identified radiology CT studies** | Radiology | Cross-sectional imaging (CT scans) | Enables CT interpretation (1.5 feature) |
| **De-identified ophthalmology (EyePACS)** | Ophthalmology | Fundus photography for diabetic retinopathy | Fundus image analysis |
| **De-identified dermatology (Colombia, Australia)** | Dermatology | Diverse geographic skin lesion data | Geographic diversity in training |
| **De-identified pathology (colon, prostate, lymph nodes)** | Pathology | Histopathology tissue samples | Multi-organ pathology analysis |
| **Synthea (Synthetic FHIR EHR)** | EHR | Synthetic electronic health records in FHIR format | EHR understanding, structured data extraction |

### Training Data Implications for Coheara

| Coheara Need | Training Data Coverage | Gap |
|-------------|----------------------|-----|
| Clinical note patterns | MIMIC-CXR reports, PMC-OA | Limited to radiology note style; general clinical notes underrepresented |
| ICD-10/CPT coding | None directly | **Major gap.** No coding-specific training data. Fine-tuning required. |
| Handoff templates (SBAR, etc.) | None directly | **Gap.** Synthea EHR provides structure but not handoff-specific format. |
| Alert contextualization | None directly | **Gap.** No CDS alert training data. |
| Plain language (reading levels) | None directly | **Gap.** No health literacy-targeted training data. |
| PA form/criteria | None directly | **Gap.** No payer criteria or PA form training data. |
| EHR field extraction | Synthea FHIR | Moderate coverage via synthetic EHR. Real institutional EHR patterns may differ. |

---

## ADAPTATION METHODS

Google documents three adaptation approaches, in order of increasing investment:

### Method 1: Prompt Engineering

**Investment:** Low
**When to use:** Task can be adequately specified through instructions and examples.
**Technique:** System prompts with role definition, output format specification, constraints, and few-shot examples.

```
EFFORT:     Hours
RISK:       Low (no model modification)
DURABILITY: Fragile (prompt-sensitive model; small phrasing changes affect output)
BEST FOR:   SLM-01 (notes), SLM-03 (handoffs) — tasks closest to pre-trained capabilities
```

**Prompt Engineering Patterns for MedGemma:**

1. **Role specification:** Always start system prompt with explicit role ("You are an expert [specialty]")
2. **Output format:** Explicitly state expected structure (SOAP, SBAR, JSON, etc.)
3. **Boundary enforcement:** State what NOT to do ("Do not interpret findings. Do not recommend treatments.")
4. **Gap flagging:** Instruct to flag missing information ("If any required field cannot be determined from the input, state 'INSUFFICIENT DATA: [field name]'")
5. **Traceability instruction:** "For each output element, cite the specific input element it derives from"

### Method 2: Fine-Tuning (LoRA)

**Investment:** Medium
**When to use:** Prompt engineering is insufficient; task requires specialized behavior not in pre-training.
**Technique:** Low-Rank Adaptation — trains small adapter layers while freezing most model parameters.

```
EFFORT:     Days to weeks (data preparation + training + evaluation)
RISK:       Medium (can degrade general capabilities if poorly executed)
DURABILITY: Strong (behavior becomes part of the model)
BEST FOR:   SLM-02 (coding), SLM-05 (plain language), SLM-06 (PA) — tasks with training data gaps
```

**LoRA Configuration (from Google's fine-tuning notebook):**

```python
from peft import LoraConfig, get_peft_model

lora_config = LoraConfig(
    r=16,                    # Rank (lower = fewer params, less expressive)
    lora_alpha=32,           # Scaling factor
    target_modules=[         # Which layers to adapt
        "q_proj", "k_proj", "v_proj", "o_proj",
        "gate_proj", "up_proj", "down_proj"
    ],
    lora_dropout=0.05,
    bias="none",
    task_type="CAUSAL_LM"
)
```

**Fine-tuning targets by SLM role:**

| SLM Role | What to Fine-Tune | Training Data Needed |
|----------|-------------------|---------------------|
| SLM-02 (Coding) | Language decoder | ICD-10/CPT coding examples with clinical narratives and rationale |
| SLM-04 (Alerts) | Language decoder | CDS alert + patient context → contextualized assessment pairs |
| SLM-05 (Plain Language) | Language decoder | Clinical text → plain language pairs at verified reading levels |
| SLM-06 (PA) | Language decoder | Clinical documentation + payer criteria → PA request drafts |
| Imaging tasks | Image encoder + language decoder | Domain-specific image-report pairs |

**What can be fine-tuned:**
- Image encoder only (for new imaging modalities)
- Language decoder only (for new text tasks)
- Both encoder and decoder (full adaptation)

### Method 3: Agentic Orchestration

**Investment:** Medium-High
**When to use:** Task requires external knowledge, tools, or multi-step reasoning beyond model capacity.
**Technique:** Wrap MedGemma in an agent framework with access to tools (FHIR API, coding databases, payer criteria databases, web search).

```
EFFORT:     Weeks (system design + integration + testing)
RISK:       Medium (complexity, latency, failure modes multiply)
DURABILITY: Strong (model + tools + framework)
BEST FOR:   SLM-02 (with coding DB), SLM-04 (with alert rules DB), SLM-06 (with payer criteria DB)
```

**Agentic Pattern for Coheara:**

```
Clinical Event
    → Context Assembly (Artifact Manager gathers relevant artifacts)
    → Prompt Construction (system prompt + artifacts + clinical data)
    → MedGemma Call (bounded SLM task)
    → Output Validation (schema check, completeness check)
    → Coherence Check (Coherence Engine compares against purpose)
    → Signal Generation (if tension detected)
    → Delivery or Escalation
```

This is exactly the cycle described in Spec-04 (Living Cycle Engine), with MedGemma as the execution substrate at Stage 4 (Reification).

---

## ADAPTATION STRATEGY FOR COHEARA

| SLM Role | Primary Method | Secondary Method | Priority |
|----------|---------------|-----------------|----------|
| SLM-01: Note Generation | Prompt engineering | LoRA fine-tuning (if needed) | Phase 1 |
| SLM-02: Code Suggestion | LoRA fine-tuning | Agentic (coding DB lookup) | Phase 2 |
| SLM-03: Handoff Generation | Prompt engineering | LoRA fine-tuning (template conformance) | Phase 1 |
| SLM-04: Alert Filtering | Agentic (alert rules DB) | LoRA fine-tuning | Phase 2 |
| SLM-05: Plain Language | LoRA fine-tuning | Prompt engineering (reading level) | Phase 2 |
| SLM-06: PA Drafting | LoRA fine-tuning + Agentic | — | Phase 3 |
| Medical Imaging | Prompt engineering | LoRA (for specific modalities) | Phase 1 |

**Phase 1:** Tasks where MedGemma is strong out-of-box (notes, handoffs, imaging)
**Phase 2:** Tasks requiring moderate adaptation (coding, alerts, plain language)
**Phase 3:** Tasks requiring heavy adaptation (PA drafting)

---

*Adaptation is not about making the model smarter. It is about making the model's intelligence serve a specific purpose within specific boundaries. Prompt engineering shapes the conversation. Fine-tuning shapes the model. Agentic orchestration shapes the workflow. Coheara uses all three, matched to each task's requirements.*

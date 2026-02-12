# Spec-MG-02: Capabilities & Supported Tasks
## MedGemma 1.5 4B — Coheara SLM Reference

**Version:** 1.0
**Last Updated:** 2026-02-11
**Parent:** 00-MEDGEMMA-INDEX.md
**Source:** Google HuggingFace model card, Google Health AI Developer Foundations

---

## PURPOSE

This document catalogs every validated capability of MedGemma 1.5 4B, organized by modality and medical domain. Each capability is assessed for strength, validation status, and relevance to Coheara's SLM roles.

---

## CAPABILITY MAP

```
┌─────────────────────────────────────────────────────────────────┐
│                  MedGemma 1.5 4B CAPABILITIES                   │
│                                                                 │
│  TEXT-ONLY                        MULTIMODAL (Image + Text)     │
│  ─────────                        ─────────────────────────     │
│  ● Medical QA                     ● CXR classification          │
│  ● Clinical reasoning             ● CXR report generation       │
│  ● Medical knowledge retrieval    ● Dermatology classification  │
│  ● EHR understanding              ● Ophthalmology (fundus)      │
│  ● Structured data extraction     ● Histopathology analysis     │
│  ● Lab report parsing             ● CT interpretation (1.5)     │
│  ● Clinical summarization         ● MRI interpretation (1.5)    │
│  ● Medical text comprehension     ● Whole-slide pathology (1.5) │
│                                   ● Longitudinal CXR (1.5)     │
│                                   ● Anatomical localization(1.5)│
│                                   ● Visual QA (medical)         │
│                                   ● Medical document OCR        │
└─────────────────────────────────────────────────────────────────┘
```

---

## TEXT-ONLY CAPABILITIES

### 1. Medical Question Answering

**What it does:** Answers medical knowledge questions across clinical domains.

**Validated on:** MedQA (USMLE-style, 4-option), MedMCQA (Indian medical entrance), PubMedQA (biomedical research), MMLU Medical subset, AfriMed-QA (African medical context), MedXpertQA.

**Strength:** Moderate-Strong. MedQA 64.4% (vs 50.7% base Gemma 3). Consistent improvement across all medical QA benchmarks.

**Coheara relevance:** Foundation for clinical reasoning in all SLM roles. Not a standalone capability but underpins every task that requires medical knowledge.

### 2. Clinical Reasoning

**What it does:** Performs multi-step clinical reasoning — differential diagnosis, treatment rationale, contraindication identification, clinical decision support.

**Validated on:** Implicit in MedQA/MedMCQA scores (questions require reasoning, not just recall). Patient triaging and clinical decision support listed as supported use cases by Google.

**Strength:** Moderate. 4B model has reasoning limits vs 27B variant. Sufficient for bounded tasks; insufficient for complex multi-factor clinical decisions.

**Coheara relevance:** Core to SLM-04 (alert contextualization requires reasoning about patient-specific risk).

### 3. EHR Understanding & Structured Data Extraction

**What it does:** Parses unstructured medical text (lab reports, clinical notes, EHR fields) and extracts structured data.

**Validated on:** Enhanced in 1.5 specifically. Trained on Synthea synthetic FHIR-based EHR records. Google documentation highlights "extraction of structured data from unstructured medical lab reports and EHR understanding" as a 1.5 improvement.

**Strength:** Strong (1.5 enhancement). Core design intent of the 1.5 update.

**Coheara relevance:** Critical for SLM-01 (note generation from EHR data), SLM-03 (handoff generation from EHR state), and the Artifact Manager (structured artifact population).

### 4. Clinical Summarization

**What it does:** Summarizes clinical information — encounter data, patient history, lab results, medication lists.

**Strength:** Strong (implicit from EHR understanding + text generation quality).

**Coheara relevance:** Direct foundation for SLM-03 (handoff summary generation).

### 5. Medical Text Comprehension

**What it does:** Reads and understands biomedical literature, clinical guidelines, coding guidelines, payer criteria documents.

**Validated on:** PubMedQA 73.4%. Trained on PMC-OA (biomedical literature with images).

**Strength:** Moderate-Strong.

**Coheara relevance:** Supports SLM-02 (coding guideline comprehension), SLM-06 (payer criteria matching).

---

## MULTIMODAL CAPABILITIES (Image + Text)

### 6. Chest X-Ray Analysis

**What it does:** Classifies findings, generates radiology reports, detects conditions from chest radiographs.

**Validated on:** MIMIC-CXR (F1 88.9), CheXpert (F1 48.1), CXR14 (F1 50.1). Report generation RadGraph F1 30.3 (tuned).

**Strength:** Very Strong. Best-performing capability. MIMIC-CXR F1 88.9 is near clinical utility.

**Coheara relevance:** Direct medical imaging capability. Supports coherence checks on radiology documentation.

**1.5 addition:** Longitudinal CXR assessment — can track temporal changes across time-stamped radiographs when prompted with comparison context.

### 7. Dermatology Image Analysis

**What it does:** Classifies skin lesions, describes dermatological findings.

**Validated on:** US-DermMCQA (accuracy 71.8%). Trained on PAD-UFES-20, SCIN, de-identified dermatology datasets.

**Strength:** Strong.

**Coheara relevance:** Supports dermatology workflow if Coheara extends to specialty imaging.

### 8. Ophthalmology (Fundus Photography)

**What it does:** Analyzes retinal fundus images for diabetic retinopathy and other conditions.

**Validated on:** EyePACS Fundus (accuracy 64.9% — massive improvement from 14.4% base Gemma 3).

**Strength:** Strong (for screening-level tasks).

**Coheara relevance:** Supports ophthalmology workflow.

### 9. Histopathology Analysis

**What it does:** Analyzes tissue samples from histopathology slides.

**Validated on:** PathMCQA (accuracy 69.8%). Trained on TCGA, CAMELYON (lymph node), de-identified colon/prostate/lymph pathology.

**Strength:** Strong.

**1.5 addition:** Whole-slide histopathology support (high-dimensional imaging).

**Coheara relevance:** Supports pathology workflow.

### 10. CT & MRI Interpretation (1.5 Only)

**What it does:** Interprets cross-sectional imaging (CT scans, MRI sequences).

**Validated on:** New in 1.5. Trained on de-identified radiology CT studies.

**Strength:** Moderate (new capability, less mature than CXR).

**Coheara relevance:** Extends imaging coherence checks to cross-sectional modalities.

### 11. Medical Visual Question Answering

**What it does:** Answers questions about medical images based on visual content.

**Validated on:** SLAKE Radiology (F1 72.3), VQA-RAD (F1 49.9).

**Strength:** Moderate-Strong (SLAKE) to Moderate (VQA-RAD).

**Coheara relevance:** Foundation for interactive image-based queries within the collaboration space.

### 12. Anatomical Feature Localization (1.5 Only)

**What it does:** Identifies and locates anatomical features in radiographs.

**Trained on:** ChestImaGenome (bounding boxes linking findings to anatomy).

**Strength:** New in 1.5, maturity developing.

**Coheara relevance:** Supports structured radiology reporting with anatomical references.

### 13. Medical Document Understanding (1.5 Enhanced)

**What it does:** Reads medical documents (lab reports, forms, scanned records) and extracts structured information.

**Strength:** Enhanced in 1.5. Core 1.5 improvement.

**Coheara relevance:** Supports artifact population from existing paper/PDF records.

---

## CAPABILITY MATRIX BY COHEARA SLM ROLE

| Capability | SLM-01 Notes | SLM-02 Codes | SLM-03 Handoff | SLM-04 Alerts | SLM-05 Plain | SLM-06 PA | Imaging |
|------------|:---:|:---:|:---:|:---:|:---:|:---:|:---:|
| Medical QA | + | ++ | + | ++ | + | + | + |
| Clinical reasoning | + | + | + | +++ | + | ++ | + |
| EHR understanding | +++ | + | +++ | ++ | - | + | - |
| Structured extraction | ++ | ++ | ++ | + | - | ++ | - |
| Summarization | ++ | - | +++ | - | ++ | - | - |
| Text comprehension | + | ++ | + | + | ++ | ++ | - |
| CXR analysis | - | - | - | - | - | - | +++ |
| Derm/Ophtho/Path | - | - | - | - | - | - | +++ |
| CT/MRI (1.5) | - | - | - | - | - | - | ++ |
| Visual QA | - | - | - | - | - | - | ++ |
| Document understanding | + | + | + | - | - | ++ | - |

Legend: `+++` primary fit, `++` strong support, `+` relevant, `-` not applicable

---

*Capabilities are not features. A capability is a validated behavior under tested conditions. The gap between capability and clinical utility is bridged by the framework: prompt engineering, coherence observation, and human governance. MedGemma provides the substrate; Coheara provides the accountability.*

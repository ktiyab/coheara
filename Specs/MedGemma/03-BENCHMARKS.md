# Spec-MG-03: Benchmarks & Performance
## MedGemma 1.5 4B — Coheara SLM Reference

**Version:** 1.0
**Last Updated:** 2026-02-11
**Parent:** 00-MEDGEMMA-INDEX.md
**Source:** Google HuggingFace model card (google/medgemma-4b-it)

---

## PURPOSE

This document records all published benchmark results for MedGemma 1.5 4B. These numbers establish the performance baseline against which Coheara's SLM outputs will be evaluated and against which fine-tuning improvements will be measured.

---

## BASELINE COMPARISON

All benchmarks compare MedGemma 4B against its base model (Gemma 3 4B) to isolate the effect of medical specialization.

---

## MEDICAL IMAGE CLASSIFICATION

| Benchmark | Domain | Metric | Gemma 3 4B | MedGemma 4B | Delta | Assessment |
|-----------|--------|--------|-----------|-------------|-------|------------|
| MIMIC-CXR (top 5 conditions) | Chest X-ray | Macro F1 | 81.2 | **88.9** | +7.7 | Near clinical utility |
| CheXpert CXR (top 5 conditions) | Chest X-ray | Macro F1 | 32.6 | **48.1** | +15.5 | Major improvement; still requires fine-tuning |
| CXR14 (3 conditions) | Chest X-ray | Macro F1 | 32.0 | **50.1** | +18.1 | Major improvement; still requires fine-tuning |
| PathMCQA | Histopathology | Accuracy | 37.1 | **69.8** | +32.7 | Transformative improvement |
| US-DermMCQA | Dermatology | Accuracy | 52.5 | **71.8** | +19.3 | Strong improvement |
| EyePACS Fundus | Ophthalmology | Accuracy | 14.4 | **64.9** | +50.5 | Transformative improvement |

**Key insight:** The largest gains are in domains where base Gemma 3 was weakest (EyePACS: +50.5, PathMCQA: +32.7). Medical pre-training has the greatest impact where general models have the least relevant training data.

---

## MEDICAL VISUAL QUESTION ANSWERING

| Benchmark | Domain | Metric | Gemma 3 4B | MedGemma 4B | Delta | Assessment |
|-----------|--------|--------|-----------|-------------|-------|------------|
| SLAKE Radiology | Radiology VQA | F1 | 40.2 | **72.3** | +32.1 | Strong; reliable for structured radiology questions |
| VQA-RAD Radiology | Radiology VQA | F1 | 33.6 | **49.9** | +16.3 | Moderate; still below 50% on harder questions |

**Key insight:** SLAKE (structured, well-defined questions) shows much stronger performance than VQA-RAD (more open-ended). This suggests MedGemma performs better on bounded, well-structured medical queries — which aligns with Coheara's bounded SLM task design.

---

## TEXT-ONLY MEDICAL BENCHMARKS

| Benchmark | Domain | Metric | Gemma 3 4B | MedGemma 4B | Delta | Assessment |
|-----------|--------|--------|-----------|-------------|-------|------------|
| MedQA (4-option) | USMLE-style | Accuracy | 50.7 | **64.4** | +13.7 | Solid; above passing threshold for some question types |
| MedMCQA | Indian medical entrance | Accuracy | 45.4 | **55.7** | +10.3 | Moderate improvement |
| PubMedQA | Biomedical research | Accuracy | 68.4 | **73.4** | +5.0 | Incremental; base was already decent |
| MMLU Medical subset | General medical knowledge | Accuracy | 67.2 | **70.0** | +2.8 | Incremental; base was already decent |
| MedXpertQA (text only) | Expert medical | Accuracy | 11.6 | **14.2** | +2.6 | Both very low; expert-level questions beyond 4B capacity |
| AfriMed-QA | African medical context | Accuracy | 48.0 | **52.0** | +4.0 | Modest improvement; regional medical context is hard |

**Key insight:** Text-only improvements are meaningful but smaller than imaging gains. The 4B model shows clear ceiling effects on expert-level questions (MedXpertQA: 14.2%). For complex clinical reasoning, the 27B variant or fine-tuning would be needed.

---

## CHEST X-RAY REPORT GENERATION

| Model | MIMIC-CXR RadGraph F1 | Notes |
|-------|----------------------|-------|
| Gemma 3 4B | 28.8 | Baseline |
| PaliGemma 2 3B (tuned) | 28.8 | Smaller model, same score |
| **MedGemma 4B (pre-trained)** | **29.5** | Out-of-box improvement |
| **MedGemma 4B (tuned for CXR)** | **30.3** | Best 4B performance with task-specific tuning |
| PaliGemma 2 10B (tuned) | 29.5 | 2.5x larger model, same as MedGemma 4B pre-trained |

**Key insight:** MedGemma 4B pre-trained matches PaliGemma 2 10B tuned (a 2.5x larger model). Fine-tuning adds another +0.8. This validates the 4B model as efficient for report generation tasks and shows that task-specific fine-tuning produces measurable gains.

---

## PERFORMANCE BY COHEARA SLM ROLE

Mapping benchmark evidence to SLM task readiness:

### SLM-01: Clinical Note Generation

| Relevant Benchmark | Score | Readiness |
|-------------------|-------|-----------|
| EHR understanding (1.5 enhancement) | Not separately scored | High (design intent) |
| CXR report generation | RadGraph F1 30.3 | Moderate (report ≠ note, but related) |
| MedQA (clinical knowledge) | 64.4% | Supports clinical accuracy |

**Readiness: HIGH** — Strongest alignment between model capabilities and task requirements.

### SLM-02: Code Suggestion

| Relevant Benchmark | Score | Readiness |
|-------------------|-------|-----------|
| MedQA (clinical knowledge for coding rationale) | 64.4% | Moderate |
| No ICD-10/CPT-specific benchmark | N/A | Gap |

**Readiness: MEDIUM** — Medical knowledge is present but coding-specific accuracy is unvalidated. Fine-tuning with coding datasets required.

### SLM-03: Handoff Summary Generation

| Relevant Benchmark | Score | Readiness |
|-------------------|-------|-----------|
| EHR understanding (1.5 enhancement) | Not separately scored | High |
| Structured extraction (1.5 enhancement) | Not separately scored | High |
| MIMIC-CXR (understands clinical data) | F1 88.9 | High |

**Readiness: HIGH** — Core model strengths (EHR parsing, summarization, structured extraction) directly serve this task.

### SLM-04: Alert Filtering

| Relevant Benchmark | Score | Readiness |
|-------------------|-------|-----------|
| MedQA (clinical reasoning) | 64.4% | Moderate |
| PubMedQA (evidence assessment) | 73.4% | Moderate-Strong |
| No alert-specific benchmark | N/A | Gap |

**Readiness: MEDIUM** — Clinical reasoning is present but real-time alert contextualization is unvalidated. Requires validation with CDS alert datasets.

### SLM-05: Plain Language Translation

| Relevant Benchmark | Score | Readiness |
|-------------------|-------|-----------|
| No readability-specific benchmark | N/A | Gap |
| MMLU Medical (knowledge for accurate simplification) | 70.0% | Moderate |

**Readiness: MEDIUM-LOW** — No validation on reading-level targeting, Flesch-Kincaid calibration, or simplification-without-accuracy-loss. Requires dedicated evaluation.

### SLM-06: PA Drafting

| Relevant Benchmark | Score | Readiness |
|-------------------|-------|-----------|
| No PA-specific benchmark | N/A | Gap |
| MedQA (clinical knowledge for evidence matching) | 64.4% | Moderate |
| Document understanding (1.5) | Not separately scored | Moderate |

**Readiness: LOW-MEDIUM** — Largest gap between model capabilities and task requirements. Payer criteria matching, form generation, and evidence-to-criterion mapping are unvalidated. Heaviest fine-tuning investment needed.

---

## BENCHMARK LIMITATIONS

1. **Benchmark ≠ Clinical performance.** All scores are on curated datasets with known answer distributions. Real-world clinical text is noisier, more ambiguous, and more variable.

2. **Data contamination risk.** MedGemma may have encountered benchmark-related content during pre-training. Google's model card explicitly warns about this. Validate on non-public institutional data.

3. **English-only evaluation.** All benchmarks are English. Multilingual performance is unknown and likely lower.

4. **Single-turn only.** All benchmarks are single-turn. Multi-turn clinical dialogue performance is uncharacterized.

5. **No Coheara-specific benchmarks exist yet.** These benchmarks measure general medical capability, not performance on Coheara's specific task definitions (note templates, handoff formats, coding guideline adherence). Coheara-specific evaluation must be built.

---

*Numbers without context are noise. Each benchmark tells us one thing: how this model performs on that specific test under those specific conditions. The gap between benchmark and bedside is bridged by validation, fine-tuning, and the coherence framework that catches what the model misses.*

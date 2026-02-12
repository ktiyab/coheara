# MedGemma 1.5 4B: Model Specification Index
## Coheara SLM Layer — Foundation Model Reference
### The Execution Substrate for Bounded Translation Tasks

---

## CONTINUITY ANCHOR

This document is the root of the MedGemma model specification for Coheara. It describes the foundation model that powers the SLM execution layer (Spec-02, Spec-05). Every architectural decision about SLM integration, task boundaries, prompt design, and evaluation traces back to the capabilities and limitations documented here.

**Source authority:** Google DeepMind, HuggingFace model card (google/medgemma-4b-it).

---

## SPECIFICATION DOCUMENTS

| Code | Document | Scope | Status |
|------|----------|-------|--------|
| **00** | Index (this document) | Anchor, model identity, Coheara fit assessment | Active |
| **01** | Model Architecture & Technical Specs | Architecture, parameters, context, image encoder, data types | Active |
| **02** | Capabilities & Supported Tasks | Medical imaging, text tasks, multimodal, what it can do | Active |
| **03** | Benchmarks & Performance | All evaluation scores, comparisons, per-domain performance | Active |
| **04** | Input/Output Protocol & Code Samples | Message format, API usage, Ollama integration, code examples | Active |
| **05** | Training Data & Adaptation | Training datasets, fine-tuning methods, LoRA, prompt engineering | Active |
| **06** | Limitations & Boundaries | Hard constraints, failure modes, what it cannot do | Active |
| **07** | Coheara Integration Mapping | MedGemma capabilities mapped to Coheara SLM roles (SLM-01 through SLM-06) | Active |

---

## MODEL IDENTITY

| Field | Value |
|-------|-------|
| **Name** | MedGemma 4B IT v1.5 |
| **Author** | Google (Health AI Developer Foundations) |
| **Base model** | Gemma 3 |
| **Architecture** | Decoder-only Transformer, Grouped-Query Attention (GQA) |
| **Parameters** | 4 billion |
| **Modality** | Multimodal (text + medical images) |
| **Image encoder** | SigLIP (pre-trained on de-identified medical imagery) |
| **Data type** | BF16 |
| **Context window** | 128K+ tokens |
| **Max output** | 8,192 tokens |
| **Image resolution** | Normalized to 896x896, encoded to 256 tokens each |
| **License** | Health AI Developer Foundations terms of use |
| **Initial release** | May 20, 2025 |
| **v1.0.1 bug fix** | July 9, 2025 (restored missing end-of-image token) |
| **v1.5 release** | January 13, 2026 |
| **Local deployment** | Ollama: `MedAIBase/MedGemma1.5:4b` (7.8 GB) |

---

## WHAT 1.5 ADDS OVER 1.0

| Capability | 1.0 | 1.5 |
|------------|-----|-----|
| 2D medical imaging (CXR, derm, fundus, pathology) | Yes | Yes |
| High-dimensional imaging (CT, MRI) | No | **Yes** |
| Whole-slide histopathology | No | **Yes** |
| Longitudinal chest X-ray assessment (temporal changes) | No | **Yes** |
| Anatomical feature localization in radiographs | No | **Yes** |
| Medical document understanding (lab reports, EHR) | Basic | **Enhanced** |
| Structured data extraction from unstructured medical text | Basic | **Enhanced** |
| Medical text reasoning | Yes | **Improved** |

---

## COHEARA FIT ASSESSMENT (Summary)

How MedGemma 1.5 4B maps to the six SLM roles defined in Spec-05:

| Coheara SLM Role | MedGemma Fit | Confidence | Gap Analysis |
|-------------------|-------------|------------|--------------|
| **SLM-01: Note Generation** | Strong | High | Text comprehension + EHR understanding align well. Requires prompt engineering for institutional note templates. |
| **SLM-02: Code Suggestion** | Moderate | Medium | Medical knowledge present (MedQA 64.4%). No ICD-10/CPT-specific training documented. Requires fine-tuning or RAG augmentation with coding guidelines. |
| **SLM-03: Handoff Generation** | Strong | High | Summarization + structured extraction are core strengths. EHR understanding enhanced in 1.5. Template conformance via prompting. |
| **SLM-04: Alert Filtering** | Moderate | Medium | Clinical reasoning present. 128K context supports patient history. Not validated for real-time alert stream processing. |
| **SLM-05: Plain Language** | Moderate | Medium | Text generation strong. Reading-level targeting not validated. No Flesch-Kincaid calibration documented. Requires fine-tuning or constrained generation. |
| **SLM-06: PA Drafting** | Weak-Moderate | Low-Medium | Clinical reasoning yes. Payer criteria matching, form generation, evidence-to-criterion mapping not validated. Heaviest fine-tuning needed. |
| **Medical Imaging** | Very Strong | Very High | Core strength. CXR, dermato, pathology, ophthalmology, CT/MRI (1.5). Direct applicability for image-based coherence checks. |

**Strategic implication:** MedGemma 1.5 4B provides a strong foundation for SLM-01, SLM-03, and medical imaging. SLM-02, SLM-04, SLM-05 require targeted prompt engineering or fine-tuning. SLM-06 requires the most adaptation work. See Spec-07 for detailed mapping.

---

## CRITICAL CONSTRAINTS (Non-Negotiable)

1. **NOT for direct clinical use.** All outputs are draft reifications requiring human review. This aligns with Coheara's architecture (Spec-02: AI as middleware, not endpoint).
2. **Single-image primary.** Multi-image comprehension not validated. Limits longitudinal comparisons to prompted single-image analysis.
3. **No multi-turn optimization.** Each request is stateless. Conversation context must be assembled externally (by the Cycle Orchestrator).
4. **English only.** Multilingual patient materials (SLM-05) require separate model or fine-tuning.
5. **Prompt sensitive.** More sensitive to prompt phrasing than base Gemma 3. Prompt templates must be validated per task.
6. **Hallucination risk.** Inaccurate outputs possible even in well-trained domains. Coherence Engine (Spec-07) is the safety net.

---

**Version:** 1.0
**Last Updated:** 2026-02-11
**Parent:** 00-MASTER-INDEX.md (app-specs)

*This model is the execution substrate. The framework governs it. Without the framework, it is a fast medical typewriter. Within the framework, it is a bounded, accountable agent that participates in a system designed to catch its own mistakes.*

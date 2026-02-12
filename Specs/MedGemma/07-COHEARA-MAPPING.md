# Spec-MG-07: Coheara Integration Mapping
## MedGemma 1.5 4B → Coheara SLM Roles

**Version:** 1.0
**Last Updated:** 2026-02-11
**Parent:** 00-MEDGEMMA-INDEX.md
**Dependencies:** Spec-05 (SLM Integration Layer), Spec-MG-02 (Capabilities), Spec-MG-03 (Benchmarks), Spec-MG-06 (Limitations)

---

## PURPOSE

This document maps MedGemma 1.5 4B capabilities to each of the six SLM roles defined in Coheara's Spec-05. For each role: what MedGemma provides out-of-box, what gaps exist, what adaptation is needed, and what the integration architecture looks like.

---

## ARCHITECTURE: ONE MODEL, SIX ROLES

Coheara's Spec-05 defines six specialized SLMs. Rather than deploying six separate models, MedGemma 1.5 4B serves as the single foundation model, differentiated by:

1. **System prompts** — Each SLM role has a distinct, versioned system prompt
2. **Context assembly** — Each role receives different artifact context
3. **Output validation** — Each role has role-specific schema validation
4. **Fine-tuned adapters** — Where needed, LoRA adapters specialize the model per role

```
┌─────────────────────────────────────────────────────────────────┐
│                    MedGemma 1.5 4B (Ollama)                     │
│                    Single model instance                        │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌──────────┐│
│  │ SLM-01  │ │ SLM-02  │ │ SLM-03  │ │ SLM-04  │ │ SLM-05   ││
│  │ Notes   │ │ Codes   │ │ Handoff │ │ Alerts  │ │ Plain    ││
│  │         │ │         │ │         │ │         │ │ Language ││
│  │ Prompt  │ │ Prompt  │ │ Prompt  │ │ Prompt  │ │ Prompt   ││
│  │ + Context│ │+Context │ │+Context │ │+Context │ │+Context  ││
│  │ + Schema│ │+Schema  │ │+Schema  │ │+Schema  │ │+Schema   ││
│  └─────────┘ └─────────┘ └─────────┘ └─────────┘ └──────────┘│
│                                                                 │
│  ┌─────────┐ ┌──────────────────────────────────────┐          │
│  │ SLM-06  │ │ IMAGING                              │          │
│  │ PA Draft│ │ CXR, Derm, Fundus, Path, CT/MRI      │          │
│  │         │ │                                       │          │
│  │ Prompt  │ │ Prompt + Image + Context              │          │
│  │ +Context│ │                                       │          │
│  │ +Schema │ │                                       │          │
│  └─────────┘ └──────────────────────────────────────┘          │
│                                                                 │
│  SHARED: Coherence check → Signal generation → Triage          │
└─────────────────────────────────────────────────────────────────┘
```

---

## SLM-01: CLINICAL NOTE GENERATION

### Fit Assessment: STRONG

| Aspect | Detail |
|--------|--------|
| **Core capability match** | EHR understanding (1.5 enhanced), structured extraction, medical text generation |
| **Relevant benchmarks** | CXR report generation RadGraph F1 30.3; MedQA 64.4% (clinical knowledge) |
| **Out-of-box readiness** | High — closest to pre-trained capabilities |

### Gap Analysis

| Gap | Severity | Mitigation |
|-----|----------|------------|
| No institution-specific note templates in training | Medium | Prompt engineering with template specification |
| SOAP/H&P format adherence not validated | Low | Few-shot examples in system prompt |
| Specialty-specific note conventions vary | Medium | Role-specific prompt variants per specialty |

### Integration Design

```
INPUT ASSEMBLY:
  Patient demographics (EHR) +
  Encounter data (chief complaint, vitals, exam, orders, labs) +
  Active Purpose Articulation artifact (treatment goals) +
  Active Boundary Conditions artifact (documentation requirements) +
  Note template (from Interpretation Interface)

SYSTEM PROMPT:
  "You are a clinical documentation specialist for [specialty].
   Generate a [note type] note from the encounter data below.
   Follow the template structure exactly.
   Include ONLY information present in the input data.
   Flag any required fields where data is insufficient as:
   'INSUFFICIENT DATA: [field name]'.
   Do not interpret diagnostic findings.
   Do not recommend treatments.
   Do not fabricate information."

OUTPUT SCHEMA:
  Structured note (SOAP, H&P, or institution-specific format) +
  Confidence per section +
  Gaps flagged +
  Traceability: each note element → input source

VALIDATION:
  - Template completeness check (all required sections present)
  - Fabrication check (no output elements without input source)
  - Boundary compliance (no interpretation, no recommendations)

ADAPTATION: Prompt engineering (Phase 1). LoRA fine-tuning if edit rate > 30%.
```

---

## SLM-02: CODE SUGGESTION

### Fit Assessment: MODERATE

| Aspect | Detail |
|--------|--------|
| **Core capability match** | Medical knowledge (MedQA 64.4%), text comprehension, clinical reasoning |
| **Relevant benchmarks** | No ICD-10/CPT-specific benchmarks |
| **Out-of-box readiness** | Medium — medical knowledge present but coding-specific behavior absent |

### Gap Analysis

| Gap | Severity | Mitigation |
|-----|----------|------------|
| No ICD-10-CM / CPT training data | High | LoRA fine-tuning with coding datasets |
| No coding guideline comprehension validation | High | RAG with coding guidelines or fine-tuning |
| No code-to-narrative traceability training | Medium | Prompt engineering for rationale linkage |
| No upcoding prevention mechanism | High | Boundary enforcement in prompt + output validation |

### Integration Design

```
INPUT ASSEMBLY:
  Clinical note (from SLM-01 or existing EHR note) +
  Active coding guidelines (from Boundary Conditions) +
  Patient code history +
  Payer-specific requirements (if applicable)

SYSTEM PROMPT:
  "You are a certified medical coder.
   Suggest ICD-10-CM and CPT codes for the clinical documentation below.
   For each suggestion, provide:
   1. Code and description
   2. Rationale: quote the specific clinical text that supports this code
   3. Confidence: high/medium/low
   Output as structured JSON.
   Do NOT suggest codes without supporting clinical evidence.
   Do NOT upcode. If documentation is ambiguous, flag the ambiguity.
   Flag documentation gaps: 'DOCUMENTATION GAP: [specific element needed for code X]'."

OUTPUT SCHEMA:
  {
    "suggested_codes": [{code, description, rationale, confidence, source_text_span}],
    "documentation_gaps": [{code_affected, missing_element, recommendation}],
    "notes": "text"
  }

VALIDATION:
  - Code existence check (validate against ICD-10/CPT database)
  - Evidence linkage check (every code has source text)
  - Upcoding pattern detection

ADAPTATION: LoRA fine-tuning (Phase 2) + Agentic (coding DB for code validation).
```

---

## SLM-03: HANDOFF SUMMARY GENERATION

### Fit Assessment: STRONG

| Aspect | Detail |
|--------|--------|
| **Core capability match** | EHR understanding (1.5), summarization, structured extraction |
| **Relevant benchmarks** | EHR understanding enhanced in 1.5; Synthea FHIR training |
| **Out-of-box readiness** | High — core strengths align directly |

### Gap Analysis

| Gap | Severity | Mitigation |
|-----|----------|------------|
| SBAR format not in training data | Low | Prompt engineering with SBAR template |
| Temporal change highlighting not validated | Medium | Prompt engineering + post-processing diff |
| "What are you worried about?" capture | Medium | Prompt for tacit knowledge solicitation |

### Integration Design

```
INPUT ASSEMBLY:
  Patient EHR state (active problems, orders, vitals trends, med changes, pending results, events) +
  Active Purpose Articulation (treatment goals) +
  Active Boundary Conditions (safety concerns, isolation, allergies) +
  Handoff template (institution-specific, from Interpretation Interface) +
  Previous shift unresolved items

SYSTEM PROMPT:
  "You are a clinical handoff specialist.
   Generate a structured handoff summary in [SBAR/institution] format.
   Highlight all changes since [timestamp of last handoff].
   List pending actions with assigned responsibility.
   Flag concerns: trends, unanswered questions, items requiring follow-up.
   For each summary element, cite its EHR data source.
   Do NOT prioritize patients by acuity.
   Do NOT omit information without flagging the omission.
   Do NOT interpret trends; present them."

OUTPUT SCHEMA:
  Structured handoff (SBAR or institution format) +
  Critical changes (highlighted) +
  Pending actions (with responsibility) +
  Flagged concerns +
  Traceability: each element → EHR source

ADAPTATION: Prompt engineering (Phase 1). LoRA if completeness < 90%.
```

---

## SLM-04: ALERT FILTERING AND CONTEXTUALIZATION

### Fit Assessment: MODERATE

| Aspect | Detail |
|--------|--------|
| **Core capability match** | Clinical reasoning (MedQA 64.4%), patient context handling (128K context) |
| **Relevant benchmarks** | No alert-specific benchmarks |
| **Out-of-box readiness** | Medium — reasoning present, alert-specific behavior absent |

### Gap Analysis

| Gap | Severity | Mitigation |
|-----|----------|------------|
| No CDS alert training data | High | Fine-tuning or agentic pattern with alert rules DB |
| No severity calibration for patient context | High | Prompt engineering + output validation |
| Real-time processing not validated | Medium | Latency testing; pre-loaded patient context |
| Critical alert suppression risk | Critical | Hard boundary: never suppress critical alerts |

### Integration Design

```
INPUT ASSEMBLY:
  Raw CDS alert (drug-drug, allergy, dose range, etc.) +
  Patient context (current meds with duration, diagnoses, labs, monitoring) +
  Active Boundary Conditions (hard safety constraints) +
  Alert configuration rules (from Interpretation Interface) +
  Historical override patterns for this patient + alert type

SYSTEM PROMPT:
  "You are a clinical pharmacology specialist.
   Assess the following CDS alert in the context of this specific patient.
   Provide:
   1. Alert severity adjusted for patient context
   2. Clinical context: 'This patient has been on [X] for [duration] with [monitoring status]'
   3. Recommendation: present to clinician / log only / escalate
   4. Rationale for your assessment
   NEVER suppress alerts classified as critical by hard boundary constraints.
   If uncertain, err toward presenting to clinician."

OUTPUT SCHEMA:
  {
    "original_alert": {type, severity, detail},
    "contextualized_severity": "informational|advisory|urgent|critical",
    "clinical_context": "text",
    "recommendation": "present|log|escalate",
    "rationale": "text",
    "hard_constraint_check": "passed|flagged"
  }

VALIDATION:
  - Critical alert preservation check (critical alerts never downgraded)
  - Hard boundary compliance
  - Rationale completeness

ADAPTATION: Agentic (Phase 2, alert rules DB) + LoRA fine-tuning with alert assessment data.
```

---

## SLM-05: PLAIN LANGUAGE TRANSLATION

### Fit Assessment: MODERATE

| Aspect | Detail |
|--------|--------|
| **Core capability match** | Text generation, medical knowledge for accuracy preservation |
| **Relevant benchmarks** | No readability-specific benchmarks |
| **Out-of-box readiness** | Medium-Low — generation strong, reading-level targeting unvalidated |

### Gap Analysis

| Gap | Severity | Mitigation |
|-----|----------|------------|
| No Flesch-Kincaid calibration | High | Post-processing readability check + re-prompting |
| No health literacy training data | High | LoRA fine-tuning with clinical→plain pairs |
| Simplification may lose clinical accuracy | High | Clinician review required; accuracy check in coherence pipeline |
| Multilingual support absent | High | Separate model or multilingual fine-tuning (future) |

### Integration Design

```
INPUT ASSEMBLY:
  Source clinical text (diagnosis, med instructions, discharge, care plan) +
  Target reading level (default: 6th grade) +
  Patient context (known conditions, current meds) +
  Active Purpose Articulation (what patient needs to understand)

SYSTEM PROMPT:
  "You are a health literacy specialist.
   Translate the following clinical text into plain language
   at a [6th grade / specified] reading level.
   Provide:
   1. Plain-language version
   2. Key terms glossary: medical term → plain explanation
   3. Flag items that may require verbal explanation
   Do NOT simplify to the point of clinical inaccuracy.
   Do NOT provide advice beyond what is in the source text.
   Do NOT assume cultural context not documented."

OUTPUT SCHEMA:
  {
    "plain_text": "translated content",
    "reading_level_estimate": "grade level",
    "glossary": [{term, plain_definition}],
    "verbal_followup_needed": ["items requiring face-to-face explanation"],
    "accuracy_flags": ["any simplifications that risk meaning loss"]
  }

VALIDATION:
  - Readability score check (Flesch-Kincaid in post-processing)
  - Clinical accuracy preservation (compare key facts source vs output)
  - Clinician approval required before distribution

ADAPTATION: LoRA fine-tuning (Phase 2) with clinical→plain language pairs at verified reading levels.
```

---

## SLM-06: PRIOR AUTHORIZATION DRAFTING

### Fit Assessment: WEAK-MODERATE

| Aspect | Detail |
|--------|--------|
| **Core capability match** | Clinical reasoning, document understanding (1.5), text generation |
| **Relevant benchmarks** | No PA-specific benchmarks |
| **Out-of-box readiness** | Low-Medium — largest gap between model capabilities and task requirements |

### Gap Analysis

| Gap | Severity | Mitigation |
|-----|----------|------------|
| No payer criteria training data | Critical | LoRA fine-tuning + RAG with payer criteria DB |
| No PA form structure training | High | Prompt engineering with form templates + fine-tuning |
| No evidence-to-criterion matching validation | High | Agentic pattern with criteria matching logic |
| No appeal drafting capability | High | Fine-tuning with appeal examples |
| Fabrication risk (stretching clinical evidence) | Critical | Hard boundary in prompt + output validation |

### Integration Design

```
INPUT ASSEMBLY:
  Clinical documentation supporting the request +
  Payer-specific PA criteria (from Boundary Conditions, source: external_payer) +
  Patient coverage information +
  Prior PA history (approvals, denials, appeals) +
  Clinical guidelines relevant to request

SYSTEM PROMPT:
  "You are a prior authorization specialist.
   Draft a PA request by matching clinical documentation to payer criteria.
   For each criterion:
   1. State the payer requirement
   2. Quote the specific clinical evidence that satisfies it
   3. If evidence is insufficient, state: 'EVIDENCE GAP: [criterion] requires [specific documentation]'
   Do NOT fabricate or stretch clinical evidence.
   Do NOT misrepresent clinical situations.
   If documentation does not support the request, say so clearly."

OUTPUT SCHEMA:
  {
    "pa_request": {
      "requested_service": "text",
      "criteria_mapping": [{criterion, clinical_evidence, source_document, status: "met|gap|partial"}],
      "evidence_gaps": [{criterion, missing_element, recommendation}],
      "supporting_summary": "text"
    },
    "appeal_draft": null | {if previously denied: appeal text with outcome data},
    "confidence": "high|medium|low"
  }

VALIDATION:
  - Evidence fabrication check (every claim traced to source document)
  - Criterion coverage completeness
  - Human review mandatory before submission

ADAPTATION: LoRA fine-tuning + Agentic (payer criteria DB) (Phase 3). Heaviest investment.
```

---

## MEDICAL IMAGING (Cross-Role Capability)

### Fit Assessment: VERY STRONG

| Aspect | Detail |
|--------|--------|
| **Core capability match** | Primary design purpose of MedGemma multimodal |
| **Relevant benchmarks** | MIMIC-CXR F1 88.9, PathMCQA 69.8%, EyePACS 64.9% |
| **Out-of-box readiness** | Very High |

### Integration Design

```
INPUT: System prompt (specialist role) + Medical image (base64) + Clinical query

MODALITIES:
  - Chest X-ray: classification, report generation, longitudinal comparison (1.5)
  - Dermatology: lesion classification, description
  - Ophthalmology: fundus analysis, diabetic retinopathy screening
  - Histopathology: tissue classification, whole-slide analysis (1.5)
  - CT/MRI: cross-sectional interpretation (1.5, newer capability)

COHEARA ROLE:
  Not mapped to a single SLM role. Medical imaging is a cross-cutting capability
  that supports coherence checks on clinical documentation (does the note match
  the image?), radiology workflow, and specialty consultation.

ADAPTATION: Prompt engineering (Phase 1). LoRA for institution-specific imaging protocols.
```

---

## IMPLEMENTATION PRIORITY

| Phase | SLM Roles | Adaptation | Rationale |
|-------|-----------|------------|-----------|
| **Phase 1** | SLM-01 (Notes), SLM-03 (Handoff), Imaging | Prompt engineering only | Strongest out-of-box fit; fastest to validate |
| **Phase 2** | SLM-02 (Codes), SLM-04 (Alerts), SLM-05 (Plain Language) | LoRA fine-tuning + Agentic | Moderate gaps; require training data collection |
| **Phase 3** | SLM-06 (PA Drafting) | LoRA + Agentic + payer DB | Largest gap; requires most investment |

---

*One model, six roles, one framework. The model provides the medical intelligence. The prompts provide the task boundaries. The framework provides the accountability. Together, they produce bounded, auditable, governable clinical AI — not a chatbot with a stethoscope.*

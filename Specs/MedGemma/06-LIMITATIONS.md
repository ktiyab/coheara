# Spec-MG-06: Limitations & Boundaries
## MedGemma 1.5 4B — Coheara SLM Reference

**Version:** 1.0
**Last Updated:** 2026-02-11
**Parent:** 00-MEDGEMMA-INDEX.md
**Source:** Google HuggingFace model card, Google Health AI Developer Foundations

---

## PURPOSE

This document catalogs every known limitation, failure mode, and hard boundary of MedGemma 1.5 4B. In healthcare AI, understanding what the model cannot do is more important than understanding what it can. These limitations directly inform Coheara's Boundary Conditions artifacts and the Coherence Engine's validation rules.

---

## HARD LIMITATIONS (Non-Negotiable)

These are stated by Google as absolute constraints. They cannot be mitigated by prompting or fine-tuning alone.

### HL-01: Not for Direct Clinical Use

**Statement:** "Model outputs are not intended to directly inform clinical diagnosis, patient management decisions, treatment recommendations, or any other direct clinical practice applications."

**Implication for Coheara:** Every MedGemma output is a draft reification (Spec-04, Stage 4). No output is delivered to a clinician without passing through the coherence pipeline and, for high-stakes outputs, human review. This aligns with Coheara's architecture: AI as middleware, not endpoint.

**Enforcement:** Hard-coded in every SLM's Interpretation Interface artifact. The system never commits an SLM output to the EHR or delivers it to a patient without human approval.

### HL-02: Single-Image Primary

**Statement:** "Primarily evaluated on single-image tasks. NOT validated for multiple-image comprehension."

**Implication for Coheara:** Longitudinal comparisons (e.g., "compare today's CXR with last week's") cannot be done by passing two images simultaneously. Workaround: analyze each image separately and compare the text outputs in the coherence pipeline.

**Enforcement:** Coheara's image processing pipeline limits to one image per SLM call. Multi-image analysis is a multi-call orchestration task.

### HL-03: No Multi-Turn Optimization

**Statement:** "Not evaluated or optimized for multi-turn conversations."

**Implication for Coheara:** Each SLM call is stateless. Conversation history must be assembled externally by the Cycle Orchestrator and passed as context in each call. There is no "memory" between calls.

**Enforcement:** The middleware assembles full context (artifacts + clinical data + history) for every call. No reliance on model-side conversation state.

### HL-04: English Only

**Statement:** "Evaluations primarily included English language prompts."

**Implication for Coheara:** SLM-05 (Plain Language Translation) cannot reliably produce multilingual patient materials without a separate model or multilingual fine-tuning. Multilingual support is a future capability, not a current one.

**Enforcement:** System flags multilingual requests as unsupported and routes to alternative pipeline (or human translator).

### HL-05: Prompt Sensitivity

**Statement:** "More sensitive to specific prompts than base Gemma 3."

**Implication for Coheara:** Small changes in prompt phrasing can significantly affect output quality and format. Prompts must be treated as code — versioned, tested, validated, and not casually modified.

**Enforcement:** Prompt templates are stored as Interpretation Interface artifacts. Changes to prompts require governance review (Stage 7). Each prompt template is tested against a validation dataset before deployment.

---

## SOFT LIMITATIONS (Mitigable)

These limitations can be partially addressed through engineering, fine-tuning, or system design.

### SL-01: Hallucination Risk

**Statement:** "Inaccurate outputs are possible even for domains with substantial training data."

**Severity:** Critical in healthcare context. A hallucinated finding in a radiology report or a fabricated ICD code can cause direct patient harm.

**Mitigation in Coheara:**
- Coherence Engine compares every output against source artifacts (purpose, boundaries, input data)
- Traceability requirement: every output element must link to an input element
- Gap flagging instruction in system prompts: model explicitly states what it cannot determine
- Human review for all high-stakes outputs (urgent/critical signal threshold)
- Pattern tracking: systematic hallucination patterns feed into SLM evaluation (Spec-05 evaluation framework)

### SL-02: Data Contamination Risk

**Statement:** "Risk of model having seen related medical info during pre-training. Validate on non-public datasets."

**Severity:** Moderate. Benchmark scores may overestimate real-world performance.

**Mitigation in Coheara:**
- Do not rely solely on public benchmark scores
- Build Coheara-specific evaluation datasets from institutional data
- Track real-world performance metrics (physician edit rate, coding accuracy vs expert, handoff error rate)

### SL-03: Bias — Demographic

**Statement:** "Developers must validate performance on representative data (age, sex, gender, condition, device, etc.)."

**Severity:** High. Under-performance on specific demographics can cause health disparities.

**Mitigation in Coheara:**
- Coherence Evidence artifacts track performance by demographic segment
- Systematic under-performance triggers governance escalation (P6: empirical disagreement)
- Dermatology training data includes diverse skin tones (SCIN dataset), but other domains may lack diversity
- Institutional validation must include demographic stratification

### SL-04: Bias — Geographic/Cultural

**Statement:** Training data is primarily from US/Western medical contexts. AfriMed-QA score (52.0%) is notably lower than US-context benchmarks.

**Severity:** Moderate-High for institutions serving diverse populations.

**Mitigation in Coheara:**
- Track performance by patient population demographics
- Fine-tune with institution-specific data that reflects served population
- Flag when model encounters terminology or clinical patterns outside training distribution

### SL-05: 4B Model Reasoning Ceiling

**Evidence:** MedXpertQA score is 14.2% — expert-level clinical reasoning is beyond the 4B model's capacity.

**Severity:** Moderate. Limits the complexity of clinical decisions the model can support.

**Mitigation in Coheara:**
- SLM tasks are bounded (Spec-05): translation, not decision-making
- Complex reasoning is a governance function (human), not an execution function (AI)
- For tasks requiring deeper reasoning, escalation to human or consideration of 27B variant

### SL-06: Output Length Constraints

**Specification:** Max output 8,192 tokens.

**Severity:** Low-Moderate. Sufficient for most clinical tasks but may limit comprehensive reports.

**Mitigation in Coheara:**
- Most SLM outputs (notes, codes, handoffs, alerts, plain language) are well within 8K tokens
- For longer outputs (comprehensive PA packages), use multi-call orchestration
- Monitor for truncated outputs in the middleware

### SL-07: Context Window Practical Limits

**Specification:** 128K+ tokens theoretically, but latency increases significantly beyond 50K tokens.

**Severity:** Moderate for EHR-heavy tasks where patient history is extensive.

**Mitigation in Coheara:**
- Context Assembly (Spec-04, Stage 3) performs intelligent context selection, not full-history dump
- Prioritize recent and relevant data; summarize older history
- Monitor latency and adjust context budget per task type

---

## FAILURE MODES

Documented and anticipated failure patterns:

| Failure Mode | Description | Detection | Response |
|-------------|-------------|-----------|----------|
| **Hallucinated finding** | Model reports finding not present in input data | Traceability check (output element without input source) | Flag as CONFLICT signal; require human review |
| **Fabricated code** | Suggests ICD/CPT code that doesn't exist or doesn't match | Code validation against reference database | Reject and re-prompt or flag as GAP |
| **Over-confident output** | Model does not flag uncertainty when it should | Confidence calibration check; compare stated confidence to actual accuracy | Governance review of confidence reporting |
| **Template non-compliance** | Output doesn't match requested format (SOAP, SBAR, JSON) | Schema validation | Retry with clarified format instruction; log failure |
| **Repetitive output** | Model repeats phrases or sentences | Repetition detection in post-processing | Apply repeat penalty; retry |
| **Refusal to answer** | Model refuses valid medical query (safety over-calibration) | Empty or meta-response detection | Re-prompt with explicit permission; log pattern |
| **Incomplete output** | Model truncates response mid-sentence | Completion detection (trailing text, missing sections) | Re-prompt for remaining content; flag as GAP |
| **Wrong patient context** | Model confuses details from different patients in long context | Patient identifier verification in output | Critical signal; block output delivery |
| **Outdated medical knowledge** | Model training data has cutoff; may miss recent guidelines | Cross-reference with guideline databases | Agentic pattern: supplement with current guideline lookup |

---

## REGULATORY CONSIDERATIONS

| Consideration | Status | Coheara Implication |
|--------------|--------|---------------------|
| **FDA clearance** | Not FDA-cleared | Cannot be marketed as a medical device. Positioned as clinical decision support tool requiring human oversight. |
| **HIPAA compliance** | Model itself is not HIPAA-covered | HIPAA compliance is a deployment concern (on-premise Ollama satisfies data residency). Model does not transmit data externally. |
| **Clinical validation** | Required per Google's terms | Each SLM task must be validated before clinical deployment. Validation evidence stored as Coherence Evidence artifacts. |
| **Audit trail** | Not built into model | Coheara provides audit trail via Artifact Engine versioning and traceability. Every SLM input/output is logged. |
| **Bias assessment** | Recommended by Google | Required by Coheara's governance framework. Demographic performance tracking is a Coherence Evidence artifact. |

---

## BOUNDARY SUMMARY TABLE

| Boundary | Type | Coheara Enforcement |
|----------|------|---------------------|
| No direct clinical decisions | Hard | SLM outputs are always drafts; human review required |
| Single image per call | Hard | Middleware enforces one-image-per-call |
| No multi-turn state | Hard | Context assembled externally per call |
| English only | Hard | Multilingual requests flagged and routed elsewhere |
| Prompt sensitivity | Hard | Prompts versioned and governance-controlled |
| Hallucination possible | Soft | Coherence Engine + traceability + human review |
| Demographic bias | Soft | Stratified performance tracking + governance escalation |
| 4B reasoning ceiling | Soft | Bounded tasks only; complex reasoning to humans |
| 8K output limit | Soft | Multi-call orchestration for longer outputs |
| No FDA clearance | Hard | Positioned as decision support, not medical device |

---

*Every limitation is a boundary condition. Every boundary condition is an artifact. Every artifact is observed by the coherence engine. The model's weaknesses are not hidden — they are declared, tracked, and governed. That is the difference between using AI carelessly and using AI within a framework.*

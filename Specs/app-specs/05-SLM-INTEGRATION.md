# Spec-05: SLM Integration Layer
## Coheara Specification

**Version:** 1.0
**Last Updated:** 2026-02-10
**Parent:** 00-MASTER-INDEX.md
**Dependencies:** Spec-01 (definitions), Spec-02 (architecture), Spec-03 (artifacts), Spec-04 (cycle)

---

## PURPOSE

This document specifies the SLM integration layer: which models serve which tasks, what their declared expertise boundaries are, how their inputs and outputs are structured, and how their performance is evaluated. The SLM layer is the execution substrate; the framework is the governance logic. This spec defines the interface between them.

---

## DESIGN PRINCIPLES

1. **SLMs are bounded agents.** Each model has a declared expertise domain, authority scope, and accountability. These are expressed as Interpretation Interface artifacts (Spec-03). An SLM that generates clinical notes does not suggest billing codes; a coding SLM does not generate patient education materials.

2. **SLMs consume artifacts, not raw data.** Every SLM receives its input as an assembled context package that includes the relevant artifacts (purpose, constraints, role boundaries) alongside the clinical data. The SLM operates within the artifact constellation, not outside it.

3. **SLMs produce reifications, not final outputs.** Every SLM output is a draft that enters the coherence check pipeline (Spec-04, Stage 5) before delivery. The SLM does not have the final word; the cycle does.

4. **SLMs are replaceable.** The artifact constellation and the cycle logic are independent of any specific model. An SLM can be upgraded, retrained, or replaced without restructuring the system. The Interpretation Interface artifact for that SLM updates; the rest of the system adapts.

---

## SLM REGISTRY

Each SLM is registered as an agent in the system with the following declaration:

```
slm_id:                 UUID
slm_name:               TEXT (human-readable identifier)
slm_version:            TEXT (model version string)
expertise_domain:       TEXT (what this model is qualified for)
authority_scope:        TEXT (what this model can produce)
boundaries:             TEXT (what this model cannot do)
accountability:         TEXT (what outcomes this model is responsible for)
input_schema:           OBJECT (expected input structure)
output_schema:          OBJECT (guaranteed output structure)
confidence_reporting:   BOOLEAN (does this model report confidence scores?)
latency_target:         INTEGER (milliseconds; max acceptable response time)
evaluation_metrics:     ARRAY of METRIC_DEFINITION
last_evaluated:         TIMESTAMP
evaluation_results:     OBJECT (latest evaluation scores)
deployment_mode:        ENUM [on_premise, institutional_cloud, edge_device]
```

---

## SLM TASK DEFINITIONS

### SLM-01: Clinical Note Generation

**Expertise domain:** Converting clinical encounter data into structured, compliant clinical notes.

**Authority scope:** Generates draft notes for physician review. Does not commit to EHR. Does not make clinical decisions.

**Input:**
- Patient demographics and active problem list (from EHR)
- Encounter data: chief complaint, vital signs, exam findings, orders placed, medications reviewed
- Active Purpose Articulation artifact (treatment goals)
- Active Boundary Conditions artifact (documentation requirements, regulatory constraints)
- Note template for encounter type (from Interpretation Interface)

**Output:**
- Structured clinical note draft (SOAP, H&P, or institution-specific format)
- Confidence score per section
- Flagged gaps: "I could not determine [X] from available data"
- Traceability metadata: which input elements contributed to which output sections

**Boundaries:** Does not interpret diagnostic findings. Does not recommend treatments. Does not assess prognosis. Does not fabricate information not present in input data.

**Evaluation metrics:** Note completeness (% of required fields populated), clinical accuracy (% of facts correctly transcribed from input), format compliance (adherence to institutional template), physician edit rate (% of generated text modified by reviewing physician), time saved (documentation time with vs. without SLM).

### SLM-02: Code Suggestion

**Expertise domain:** Mapping clinical narrative to ICD-10-CM, CPT, and HCPCS codes.

**Authority scope:** Suggests codes with rationale. Does not assign codes. Does not modify clinical documentation.

**Input:**
- Clinical note (from SLM-01 output or existing EHR note)
- Active coding guidelines (from Boundary Conditions artifact)
- Patient's existing code history
- Payer-specific coding requirements (if applicable)

**Output:**
- Ranked list of suggested codes (primary and secondary)
- For each suggestion: rationale linking code to clinical evidence in note
- Confidence score per suggestion
- Documentation gaps: "Clinical support for [X code] is insufficient; consider [specific documentation element]"
- Traceability: clinical text span → coding rule → suggested code

**Boundaries:** Does not upcode. Does not suggest codes without clinical evidence in documentation. Does not override coder judgment. Flags ambiguity rather than guessing.

**Evaluation metrics:** Code accuracy (agreement with expert coder), specificity (correct level of detail), documentation gap detection rate, false positive rate, denial rate for SLM-suggested codes vs. baseline.

### SLM-03: Handoff Summary Generation

**Expertise domain:** Generating structured clinical handoff summaries from EHR data at shift change or care transition.

**Authority scope:** Generates draft handoff summaries. Does not replace verbal handoff. Does not make triage decisions.

**Input:**
- Patient's current EHR state: active problems, recent orders, vital sign trends, medication changes, pending results, recent events
- Active Purpose Articulation (current treatment goals)
- Active Boundary Conditions (safety concerns, isolation status, allergy information)
- Handoff template (institution-specific, from Interpretation Interface)
- Previous shift's unresolved items

**Output:**
- Structured handoff summary (SBAR-aligned or institution-specific format)
- Critical changes highlighted (new since last handoff)
- Pending actions with assigned responsibility
- Flagged concerns: trends, unanswered questions, items requiring follow-up
- Traceability: each summary element linked to its EHR source data

**Boundaries:** Does not prioritize patients by acuity (that is clinical judgment). Does not omit information based on its own relevance assessment without flagging the omission. Does not interpret trends; presents them.

**Evaluation metrics:** Completeness (% of critical elements captured), accuracy (verified against EHR), handoff error rate (post-implementation vs. baseline), nursing time saved, adverse event rate at transitions.

### SLM-04: Alert Filtering and Contextualization

**Expertise domain:** Receiving raw clinical decision support alerts and producing contextualized, patient-specific signal assessments.

**Authority scope:** Filters, contextualizes, and prioritizes alerts. Does not suppress alerts autonomously. Does not make prescribing decisions.

**Input:**
- Raw CDS alert (drug-drug interaction, allergy, dose range, etc.)
- Patient context: current medications with duration, diagnoses, monitoring history, relevant lab values
- Active Boundary Conditions (hard safety constraints)
- Alert configuration rules (from Interpretation Interface)
- Historical override patterns for this patient and this alert type

**Output:**
- Classified alert: severity level adjusted for patient context
- Clinical context statement: "This patient has been on [combination] for [duration] with [monitoring status]"
- Recommendation: present to clinician / log only / escalate
- Traceability: alert rule → patient data points → contextualized assessment

**Boundaries:** Never suppresses alerts classified as critical by hard boundary constraints. Reports its filtering rationale transparently. Does not override physician judgment on alert disposition.

**Evaluation metrics:** Clinically appropriate alert rate (post-filter vs. pre-filter), override rate reduction, time clinicians spend on alerts, adverse drug event rate, alert-to-action conversion rate.

### SLM-05: Plain Language Translation

**Expertise domain:** Converting clinical text into patient-readable materials at specified reading levels.

**Authority scope:** Generates draft patient materials. Does not provide medical advice. Does not replace clinician counseling.

**Input:**
- Source clinical text (diagnosis explanation, medication instructions, discharge instructions, care plan summary)
- Target reading level (default: 6th grade; configurable)
- Target language (if multilingual; requires appropriate model variant)
- Patient context: known conditions, current medications, cultural considerations (if documented)
- Active Purpose Articulation (what the patient needs to understand)

**Output:**
- Plain-language version of clinical content
- Reading level assessment of generated text (Flesch-Kincaid or equivalent)
- Key terms glossary (clinical term → plain explanation)
- Flagged items: "This concept may require verbal explanation; written materials may be insufficient"
- Traceability: each plain-language statement linked to its clinical source

**Boundaries:** Does not simplify to the point of clinical inaccuracy. Flags when simplification risks meaning loss. Does not provide advice beyond what is in the source text. Does not assume cultural context not documented.

**Evaluation metrics:** Reading level accuracy, clinical accuracy preservation, patient comprehension (if tested), clinician approval rate, patient satisfaction with materials.

### SLM-06: Prior Authorization Drafting

**Expertise domain:** Generating PA request forms by matching clinical documentation to payer-specific criteria.

**Authority scope:** Generates draft PA requests. Does not submit without physician review and approval.

**Input:**
- Clinical documentation supporting the requested service/medication
- Payer-specific PA criteria and requirements (from Boundary Conditions, source: external_payer)
- Patient's coverage information
- Prior PA history for this patient (approvals, denials, appeal outcomes)
- Clinical guidelines relevant to the request

**Output:**
- Completed PA request draft, mapped to payer criteria
- Clinical evidence summary matched to each criterion
- Gap analysis: "Criterion [X] not fully supported; consider documenting [specific element]"
- Appeal draft (if request is for a previously denied item, with outcome data)
- Traceability: each criterion → supporting clinical evidence → source document

**Boundaries:** Does not fabricate clinical evidence. Does not misrepresent clinical situations. Flags when clinical documentation does not support the request rather than stretching the match. Does not submit without human approval.

**Evaluation metrics:** PA approval rate for SLM-drafted requests vs. baseline, staff time per PA, turnaround time, appeal success rate, documentation gap detection accuracy.

---

## INPUT/OUTPUT PROTOCOL

Every SLM interaction follows a standard protocol:

```
REQUEST:
{
  slm_id:               UUID,
  request_id:           UUID (for tracing),
  clinical_event_id:    UUID,
  patient_scope_id:     UUID,
  input_data:           OBJECT (per input schema),
  artifact_context: {
    purpose:            artifact snapshot,
    boundaries:         artifact snapshot,
    interpretation:     artifact snapshot (relevant agent declarations),
    context_rationale:  artifact snapshot (relevant decisions),
    traceability:       relevant links
  },
  requesting_agent:     agent_id,
  timestamp:            TIMESTAMP
}

RESPONSE:
{
  slm_id:               UUID,
  request_id:           UUID,
  reification: {
    content:            OBJECT (per output schema),
    confidence:         FLOAT (0.0-1.0) or per-section scores,
    gaps_flagged:       ARRAY of TEXT,
    traceability_map:   ARRAY of { output_element → input_source }
  },
  metadata: {
    processing_time_ms: INTEGER,
    model_version:      TEXT,
    artifacts_consumed: ARRAY of artifact_id
  }
}
```

Every response includes traceability. The system can answer, for any SLM output element: "Where did this come from, and what artifacts governed its production?"

---

## EVALUATION FRAMEWORK

SLM performance is evaluated on two axes:

**Axis 1: Task Quality** (is the output correct and useful?)
- Measured by task-specific metrics defined per SLM above
- Evaluated by clinical expert review (gold standard) and automated metrics
- Tracked over time for trend analysis

**Axis 2: Framework Compliance** (does the output honor the framework?)
- Boundary compliance: did the SLM stay within its declared authority?
- Traceability completeness: is every output element traceable to input?
- Confidence calibration: does the confidence score predict actual accuracy?
- Gap honesty: does the SLM flag what it does not know?

**Evaluation cadence:** Automated metrics are continuous. Clinical expert review is periodic (monthly for established SLMs; weekly for new deployments or post-update).

**Failure response:** When evaluation reveals declining performance, the system triggers a governance-level review (Spec-04, Stage 7). P8 (Failure Metabolism) activates: failure → root cause → lesson → SLM retraining or configuration update.

---

*An SLM without the framework is a fast typewriter. An SLM within the framework is a bounded, accountable, auditable agent that participates in a system designed to catch its own mistakes.*

# Spec-08: Problematics-to-Capabilities Mapping
## Coheara Specification

**Version:** 1.0
**Last Updated:** 2026-02-10
**Parent:** 00-MASTER-INDEX.md
**Dependencies:** Spec-01 through Spec-07

---

## PURPOSE

This document maps each empirically documented healthcare problematic to the specific tool capabilities that address it, the framework mechanisms that govern the response, and the measurable outcomes that validate success. This is where the abstract framework meets the concrete hospital floor.

---

## MAPPING STRUCTURE

For each problematic: what was broken (evidence), what the tool does about it (capabilities), what framework logic governs the response (principles), and how we know it is working (metrics).

---

## 1. DOCUMENTATION BURDEN

**The problem, quantified:** 1.77 hours/day after-hours documentation; 125 million physician-hours/year nationally; 37% of workday consumed by EHR; 84.7% of physicians report billing-driven documentation inflates workload.

**Root cause in framework terms:** Documentation is treated as the meaning itself rather than as an artifact that serves a purpose. No distinction between clinically valuable documentation and compliance-driven overhead. No coherence observation on whether documentation serves its declared purpose. Open-loop: notes are written, filed, and rarely evaluated for fitness.

### Tool Response

| Capability | SLM | Framework Mechanism | What Changes |
|------------|-----|---------------------|--------------|
| Auto-draft clinical notes from encounter data | SLM-01 | Reification (Stage 4); purpose-constrained generation | Physician reviews draft instead of writing from scratch |
| Separate clinical documentation from compliance documentation | Artifact Engine | Boundary Conditions (source field distinguishes internal vs. external requirements) | Each documentation element is tagged by purpose; redundancy becomes visible |
| Flag documentation that serves no active purpose | Coherence Engine | P7 (Evidence-Weighted Memory); Purpose Alignment dimension | "Note bloat" elements identified and reported to governance |
| Track documentation time as coherence metric | Coherence Evidence artifact | P9 (Dual-Test Truth); results track | Time savings measured, correlated with outcome quality |

### Success Metrics

| Metric | Baseline (Pre-2021) | Target | Measurement |
|--------|---------------------|--------|-------------|
| After-hours documentation time | 1.77 h/day | Reduce by 40%+ | Self-report + system-tracked editing time |
| EHR interaction as % of workday | 37% | Reduce to <25% | Time-motion sampling |
| Physician edit rate on SLM drafts | N/A | <30% modification | System-tracked diff |
| Documentation elements serving no clinical purpose | Unknown | Identified and reported to governance | Coherence Engine audit |

---

## 2. CLINICAL HANDOVER FAILURES

**The problem, quantified:** 80% of serious adverse events involve handoff miscommunication; 70% of deaths from medical errors linked to communication breakdown; 4,000 handovers/day in a teaching hospital; 67% of sign-out sheets contain errors.

**Root cause in framework terms:** Agent discontinuity without artifact continuity. No systematic coherence observation at the transition point. Tacit knowledge has no capture mechanism. Current handoff artifacts (verbal report, personal notes) are ephemeral, unstandardized, and not linked to the source data they summarize.

### Tool Response

| Capability | SLM | Framework Mechanism | What Changes |
|------------|-----|---------------------|--------------|
| Generate structured handoff summaries from EHR data | SLM-03 | Reification constrained by Interpretation Interface (handoff template) | Structured, complete, source-linked summary available at every transition |
| Highlight changes since last handoff | Coherence Engine | Temporal Stability dimension | Critical changes cannot be buried in noise |
| Surface unresolved items from previous shift | Artifact Manager | Traceability (pending actions linked to source decisions) | Nothing falls through the cracks between shifts |
| Prompt for tacit knowledge capture | Collaboration Space | Purpose Articulation ("What are you worried about?") | Partially articulable concerns enter the record |
| Track handoff quality as coherence metric | Coherence Evidence | P9 Dual-Test Truth | Handoff errors become visible and measurable |

### Success Metrics

| Metric | Baseline | Target | Measurement |
|--------|----------|--------|-------------|
| Handoff information completeness | 33% error-free sign-out sheets | >90% completeness | Automated audit against template |
| Adverse events at transitions | 80% involve miscommunication | Reduce adverse events at transitions by 30%+ | Incident reporting correlation |
| Time per handoff | Variable; often truncated | Standardized; no increase despite richer content | System-tracked |
| Unresolved item carry-over | Untracked | 100% tracked, <10% unaddressed by end of next shift | Queue monitoring |

---

## 3. MEDICAL CODING ERRORS

**The problem, quantified:** 82% of records differ from discharge abstract; 22% of items incorrect; primary diagnosis miscoded in 26%; physician errors account for 62% (documentation failures), coding errors 35%.

**Root cause in framework terms:** Translation without traceability. Physician and coder use different ontologies (P2 failure). No feedback loop from coding outcome to documentation quality. The translation from clinical narrative to code is opaque; neither party sees the other's reasoning.

### Tool Response

| Capability | SLM | Framework Mechanism | What Changes |
|------------|-----|---------------------|--------------|
| Suggest codes from clinical narrative with rationale | SLM-02 | Reification with traceability (text span → rule → code) | Translation becomes visible and auditable |
| Detect documentation gaps before coding | SLM-02 | Coherence Engine (GAP signal) | Physician knows what to document; coder receives complete input |
| Bridge physician-coder ontology | Artifact Engine | P2 (Emergent Ontology); terminology alignment module | Shared vocabulary emerges from use, not mandate |
| Track coding accuracy as coherence metric | Coherence Evidence | P9; cross-role monitoring | Systematic patterns surface (which note types produce which errors) |
| Feed denial data back to documentation guidance | Memory & Learning | P8 (Failure Metabolism) | Denied claims → documentation lesson → improved SLM prompts |

### Success Metrics

| Metric | Baseline | Target | Measurement |
|--------|----------|--------|-------------|
| Primary diagnosis coding accuracy | 74% correct | >92% | Audit against expert coder |
| Documentation completeness for coding | 78% of records adequate | >95% | SLM-02 gap detection rate |
| Revenue loss from coding errors | Tens of thousands per physician/year | Reduce by 50%+ | Denial and undercoding tracking |
| Coder-physician communication | Minimal; divided | Shared visibility through traceability view | Qualitative assessment + cross-role conflict rate |

---

## 4. PRIOR AUTHORIZATION FRICTION

**The problem, quantified:** 43 PAs/week per physician; 12+ staff hours/week; $35 billion system cost; 91% report care delays; 75% of patients abandon treatment due to PA obstacles.

**Root cause in framework terms:** Cross-domain conflict (P11) between clinical domain and payer domain. Misaligned coherence criteria: clinician measures patient outcome, payer measures utilization efficiency. No empirical resolution mechanism (P6): disagreements are resolved by administrative power, not by outcomes. No feedback loop from PA outcomes to clinical documentation or payer criteria.

### Tool Response

| Capability | SLM | Framework Mechanism | What Changes |
|------------|-----|---------------------|--------------|
| Auto-generate PA requests from clinical documentation | SLM-06 | Reification constrained by Boundary Conditions (external_payer) | Staff time per PA drops; physician time reclaimed |
| Match clinical evidence to payer criteria with gap analysis | SLM-06 | Traceability (criterion → evidence → source) | Transparent mapping; gaps addressed before submission |
| Draft appeals for denied requests with outcome data | SLM-06 + Memory | P8 (previous denial → lesson → improved appeal) | Appeal quality improves systematically |
| Track PA outcomes as empirical evidence | Coherence Evidence | P6 + P9 (denial outcome → hypothesis → evidence) | Political friction becomes empirical data; governance has evidence for policy advocacy |
| Surface cross-domain conflict patterns | Coherence Engine | P11 (Cross-Domain Learning) | Patterns of clinically harmful denials become visible and reportable |

### Success Metrics

| Metric | Baseline | Target | Measurement |
|--------|----------|--------|-------------|
| Staff time per PA | 12+ hours/week per physician | Reduce by 60%+ | System-tracked |
| PA approval rate | Variable | Improve by 20%+ | Outcome tracking |
| Patient treatment abandonment due to PA | 75% | Reduce by 40%+ | Patient outcome tracking |
| Time from PA submission to decision | 3+ business days for 26% | Track and report; evidence for policy advocacy | System-tracked |

---

## 5. ALERT FATIGUE

**The problem, quantified:** 2 million alerts/month from 66 ICU beds; 90-96% override rate; only 7.3% clinically appropriate; 13% of physicians provide no override reason.

**Root cause in framework terms:** Coherence observation without purpose constraint. Alerts fire based on artifact rules (drug interaction database) without passing through the cycle: no patient-specific interpretation, no purpose alignment check, no adaptive rate. The system skips Stages 3-6 and goes directly from rule to signal. The result is signal saturation; the opposite of coherent observation.

### Tool Response

| Capability | SLM | Framework Mechanism | What Changes |
|------------|-----|---------------------|--------------|
| Context-aware alert filtering | SLM-04 | Full cycle: artifact → interpretation (patient context) → reification (contextualized assessment) → coherence check → triaged signal | Alert volume drops; remaining alerts are relevant |
| Patient-specific risk assessment | SLM-04 | Boundary Conditions (patient-specific hard constraints vs. generic rules) | "On this combination for 3 years, monitored" vs. "new combination" distinction |
| Clinician interruption budget enforcement | Collaboration Space | P5 (Adaptive Cycle Rate applied to signal presentation) | Attention treated as finite resource |
| Override pattern tracking | Coherence Evidence | P7 + P8 (systematic overrides → lesson → rule refinement) | Alert rules evolve based on outcome evidence |
| Alert-to-action conversion tracking | Coherence Evidence | P9 (Dual-Test Truth) | Alerts that never produce action are candidates for removal |

### Success Metrics

| Metric | Baseline | Target | Measurement |
|--------|----------|--------|-------------|
| Clinically appropriate alert rate | 7.3% | >50% | Expert review of filtered alerts |
| Override rate | 90-96% | <40% | System-tracked |
| Alert-related adverse drug events | Baseline rate | Reduce by 25%+ | Incident correlation |
| Clinician time on alerts | Substantial (unquantified) | Reduce by 70%+ | Time-motion sampling |

---

## 6. HEALTH LITERACY GAP

**The problem, quantified:** 88% of adults below proficient health literacy; 54% read below sixth-grade level; 62-69% misunderstand medication changes at discharge; 77 million adults with limited health literacy.

**Root cause in framework terms:** Role boundary failure. Clinical text is produced by and for clinicians; it does not translate across the clinician-patient boundary. No systematic translation artifact exists. No coherence observation on whether the patient understood.

### Tool Response

| Capability | SLM | Framework Mechanism | What Changes |
|------------|-----|---------------------|--------------|
| Generate plain-language patient materials from clinical text | SLM-05 | Reification constrained by Boundary Conditions (reading level, language) | Every patient leaves with comprehensible written materials |
| Reading-level verification | SLM-05 | Coherence Evidence (Flesch-Kincaid or equivalent) | Materials verified against target literacy level |
| Key-term glossary generation | SLM-05 | P2 (cross-ontology mapping: clinical → plain) | Medical jargon translated, not eliminated |
| Clinician approval before distribution | Collaboration Space | Interpretation Interface (human review required) | Clinical accuracy preserved |
| Patient comprehension tracking (where feasible) | Coherence Evidence | P9 (results: did the patient understand?) | Feedback loop on material quality |

### Success Metrics

| Metric | Baseline | Target | Measurement |
|--------|----------|--------|-------------|
| Materials at target reading level | Most above patient comprehension | 100% at or below specified target | Automated readability scoring |
| Clinician time on patient education materials | Substantial; often skipped | Reduce preparation time by 70%+ | System-tracked |
| Patient comprehension of discharge instructions | 31-38% deficit rates | Improve by 30%+ | Teach-back assessment (where implemented) |

---

## 7. BURNOUT (COMPOSITE)

**The problem, quantified:** 62.8% physician burnout; 31% cite paperwork as leading cause; $5 billion annual turnover cost.

**Root cause in framework terms:** Cumulative consequence of all above. Each problematic extracts time, attention, and professional meaning from the clinical encounter. The documentation burden alone consumes more time than patient care. The system separates professionals from their purpose.

### Tool Response

Burnout is not addressed by a dedicated capability. It is addressed by the aggregate effect of all capabilities above. The tool's success on burnout is measured by:

| Metric | Baseline | Target | Measurement |
|--------|----------|--------|-------------|
| Time returned to direct patient care | Baseline time-motion data | Increase by 30%+ | Time-motion sampling |
| Self-reported burnout symptoms | 62.8% (2021) | Reduce by 15%+ | Validated burnout instrument (Maslach or equivalent) |
| Clinician turnover rate | Institution-specific baseline | Reduce by 10%+ | HR data |
| Professional satisfaction with documentation process | Baseline survey | Improve by 40%+ | Survey |

---

## AGGREGATE IMPACT MODEL

The seven problematics are not independent. Reducing documentation burden frees time for better handoffs. Better handoffs reduce coding errors (because source documentation improves). Better coding reduces PA friction (because clinical evidence is stronger). Reduced alert noise allows attention to real risks. Better patient materials reduce readmissions. The whole reduces burnout.

The framework's Living Cycle captures this interdependence: coherence improvements in one dimension propagate through artifact links to other dimensions. The Traceability artifact makes these connections visible. The Governance Dashboard tracks the aggregate, not just the parts.

---

*Each problematic was created by systems that demanded more of humans than humans can sustainably give. The tool does not ask clinicians to work differently. It removes the silt that buries the work they already do.*

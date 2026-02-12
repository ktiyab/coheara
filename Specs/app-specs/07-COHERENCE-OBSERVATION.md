# Spec-07: Coherence Observation System
## Coheara Specification

**Version:** 1.0
**Last Updated:** 2026-02-10
**Parent:** 00-MASTER-INDEX.md
**Dependencies:** Spec-01, Spec-02, Spec-03, Spec-04

---

## PURPOSE

This document specifies the Coherence Observation System: the mechanism that continuously compares system behavior against declared purpose. This is the framework's feedback loop; the component that closes the open-loop architecture responsible for healthcare's information failures.

---

## CORE FUNCTION

The Coherence Observation System answers one question continuously: **does what is happening match what was meant?**

It compares:
- Reifications (outputs) against Purpose Articulation artifacts (goals)
- Behavior against Boundary Conditions (constraints)
- Current state against Context & Rationale (assumptions still valid?)
- Cross-agent outputs against each other (agreement?)
- Current artifacts against previous versions (drift?)

The output is a coherence assessment: a multi-dimensional score with classified tension signals.

---

## COHERENCE DIMENSIONS

### Dimension 1: Purpose Alignment

**Question:** Does this output serve the stated clinical purpose?

**Measurement method:**
- Semantic comparison: SLM output content vs. Purpose Articulation success criteria
- Scope check: does the output address what is in scope and avoid what is out of scope?
- Assumption validation: are the assumptions listed in the Purpose Articulation still active?

**Signal types:**
- GAP: purpose element not addressed in output
- DRIFT: output addresses something not in stated purpose (scope creep)
- MISALIGNMENT: output contradicts stated purpose

### Dimension 2: Boundary Compliance

**Question:** Does this output honor all applicable constraints?

**Measurement method:**
- Hard constraint verification: every hard constraint checked against output (automated where constraint is machine-verifiable; flagged for human review where not)
- Soft constraint monitoring: soft constraint deviations logged, not blocked
- Invariant checking: invariants verified at specified frequency
- Environmental constraint compliance: regulatory requirements checked

**Signal types:**
- VIOLATION: hard constraint breached (critical severity)
- DEVIATION: soft constraint not honored (advisory severity)
- INVARIANT_BREAK: invariant no longer holds (urgent severity)

### Dimension 3: Rationale Consistency

**Question:** Is this output consistent with documented reasoning and decisions?

**Measurement method:**
- Decision alignment: does the output follow from recorded decisions in Context & Rationale?
- Assumption currency: are the assumptions that supported those decisions still valid?
- Alternative resurface: has a previously rejected alternative become viable due to changed conditions?

**Signal types:**
- STALE_ASSUMPTION: assumption marked active but conditions suggest it may no longer hold
- DECISION_CONFLICT: output contradicts a documented decision without recorded rationale for the change
- ALTERNATIVE_VIABLE: a previously rejected approach may now apply (environmental change)

### Dimension 4: Agent Agreement

**Question:** Do different agents interpreting the same artifacts reach consistent conclusions?

**Measurement method:**
- Cross-SLM consistency: when multiple SLMs process the same patient data, do their outputs align?
- Human-AI agreement: when a human reviews an SLM output, how much modification occurs?
- Cross-role interpretation: do different roles reading the same artifact act consistently?

**Signal types:**
- DISAGREEMENT: agents produce conflicting interpretations (triggers P6: empirical resolution)
- SYSTEMATIC_OVERRIDE: human agents consistently modify a specific type of SLM output (triggers evaluation review)
- CROSS_ROLE_CONFLICT: roles interpret shared artifacts differently (triggers P11: cross-domain learning)

### Dimension 5: Temporal Stability

**Question:** Has meaning shifted since the last observation cycle?

**Measurement method:**
- Vocabulary drift: are the same terms being used differently than in previous cycles?
- Scope drift: has the effective scope of work expanded or contracted without formal artifact update?
- Priority drift: have de facto priorities diverged from declared priorities?

**Signal types:**
- VOCABULARY_SHIFT: terms used inconsistently across time
- SCOPE_CREEP: work occurring outside declared scope
- PRIORITY_INVERSION: declared priority not reflected in actual resource allocation or attention

---

## COHERENCE SCORE

The composite coherence score is calculated per scope (patient, protocol, department, institution):

```
coherence_score = weighted_average(
    purpose_alignment     * weight_purpose,
    boundary_compliance   * weight_boundary,
    rationale_consistency * weight_rationale,
    agent_agreement       * weight_agreement,
    temporal_stability    * weight_temporal
)
```

**Default weights:**

| Dimension | Weight | Rationale |
|-----------|--------|-----------|
| Purpose alignment | 0.30 | Primary; if purpose is not served, nothing else matters |
| Boundary compliance | 0.25 | Safety-critical; hard constraints are non-negotiable |
| Rationale consistency | 0.20 | Knowledge continuity; prevents vaporization |
| Agent agreement | 0.15 | Cross-role coherence; prevents silo drift |
| Temporal stability | 0.10 | Long-term health; prevents gradual erosion |

Weights are configurable per institution and per clinical context. ICU contexts may weight boundary compliance higher; chronic care contexts may weight temporal stability higher.

**Score interpretation:**

| Range | State | System Response |
|-------|-------|-----------------|
| 0.85 - 1.00 | Strong coherence | Cycle decelerates (P5); outputs delivered with minimal flagging |
| 0.70 - 0.84 | Adequate coherence | Cycle maintains rate; advisory signals generated |
| 0.50 - 0.69 | Degraded coherence | Cycle accelerates; urgent signals generated; governance review recommended |
| Below 0.50 | Critical incoherence | Emergency cycle; critical signals; governance escalation required |

---

## SIGNAL TRIAGE ENGINE

Raw coherence assessments generate signals. The triage engine classifies and routes them.

### Classification

Each signal is classified on three axes:

**Type:** GAP, DRIFT, CONFLICT, AMBIGUITY, VIOLATION (from dimension analysis above)

**Severity:**
- Informational: notable but not actionable now
- Advisory: should be addressed; not time-critical
- Urgent: should be addressed soon; clinical or compliance risk
- Critical: must be addressed immediately; patient safety risk

**Novelty:**
- Known pattern: this signal type has been seen before in similar contexts
- Recurring: this specific signal has fired multiple times for this scope
- Novel: first occurrence of this signal type in this context

### Routing

| Severity + Novelty | Routing |
|---------------------|---------|
| Informational, any | Dashboard log; batch presentation |
| Advisory, known | Signal panel; user convenience |
| Advisory, recurring | Signal panel with emphasis; pattern note |
| Advisory, novel | Signal panel with context explanation |
| Urgent, any | Notification; acknowledgment required |
| Critical, any | Workflow interruption; blocks output; escalates |
| Any severity, recurring 3+ times | Governance escalation (systematic issue) |

### Anti-Saturation Rules

1. **Deduplication:** Identical signals from the same source within a configurable window (default: 4 hours) are merged, not repeated.
2. **Grouping:** Related signals (same patient, same dimension, same shift) are grouped into a single notification with detail expandable.
3. **Budget enforcement:** Signal presentation respects the interruption budget (Spec-06). Advisory signals are deferred when budget is near limit.
4. **Escalation aggregation:** When multiple advisory signals accumulate without resolution, they are aggregated into a single governance-level signal rather than individually escalated.

---

## DUAL-TEST TRUTH ENGINE (P9)

The Coherence Observation System implements P9 by maintaining two parallel validation tracks:

**Results track:** Does the output produce good outcomes? Measured by: clinical outcomes, coding accuracy, handoff error rates, patient comprehension scores, PA approval rates.

**Consensus track:** Do agents agree this output is correct? Measured by: acceptance rates, modification rates, cross-role agreement, governance endorsement.

**Truth determination:**

| Results | Consensus | Status | Action |
|---------|-----------|--------|--------|
| Positive | Positive | Knowledge | Reinforce; increase confidence weight |
| Positive | Negative | Noise (results disagree with perception) | Investigate why consensus lags; present evidence |
| Negative | Positive | Groupthink risk | Alert; outcomes contradict agreement; re-evaluate |
| Negative | Negative | Failure | Immediate: enter failure metabolism pipeline (Spec-10) |

---

## CROSS-ROLE MONITORING

The Coherence Observation System specifically monitors role boundaries, the points where meaning is most vulnerable:

**Physician → Coder boundary:** Does the coding SLM's interpretation match the clinical intent? Tracked by: physician agreement with code suggestions, denial rates, coding audit results.

**Nurse → Nurse boundary:** Does the handoff summary preserve the offgoing nurse's understanding? Tracked by: oncoming nurse's modification rate, unresolved items from previous shift, adverse events in first hours of new shift.

**Clinician → Patient boundary:** Does the plain-language material preserve clinical accuracy? Tracked by: clinician approval rate, patient comprehension (if measurable), readmission rates for patients who received SLM-generated materials vs. baseline.

**Clinician → Payer boundary:** Does the PA draft accurately represent clinical reality to payer criteria? Tracked by: PA approval rate, appeal rate, clinician modification rate of PA drafts.

---

*The Coherence Observation System is the framework's eyes. Without it, the cycle runs blind; artifacts exist but no one knows if they serve their purpose. With it, every drift is detected, every gap is named, and every failure has a path to becoming a lesson.*

# Spec-09: Governance & Boundaries
## Coheara Specification

**Version:** 1.0
**Last Updated:** 2026-02-10
**Parent:** 00-MASTER-INDEX.md
**Dependencies:** Spec-01, Spec-02, Spec-03, Spec-04

---

## PURPOSE

This document specifies how authority is assigned, how boundaries are enforced, how power asymmetries are handled, and how external regulatory constraints integrate into the system. This is where P3, P4, P10, and the framework review's findings on non-negotiable constraints are encoded.

---

## EXPERTISE-BASED AUTHORITY (P3)

### Principle

Authority in Coheara does not derive from hierarchy. It derives from declared expertise, scoped within each context's realities, and held accountable by tracked outcomes.

### Implementation

Every agent (human or AI) registers an expertise declaration:

```
agent_id:               UUID
expertise_domain:       TEXT (specific competence area)
authority_scope:        TEXT (what decisions this agent can make)
boundaries:             TEXT (what this agent cannot do)
accountability:         TEXT (what outcomes this agent is responsible for)
credential_basis:       TEXT (license, certification, training, for humans;
                              model specification, evaluation results, for AI)
declaration_date:       TIMESTAMP
last_validated:         TIMESTAMP
validation_method:      TEXT (credential check, outcome audit, evaluation)
override_log:           ARRAY of {
                          override_id: UUID,
                          what_was_overridden: TEXT,
                          reason: TEXT,
                          outcome: TEXT (filled retrospectively),
                          timestamp: TIMESTAMP
                        }
```

### Authority Resolution Protocol

When a decision requires adjudication between agents:

1. **Identify scope:** Which agent's declared expertise covers this decision domain?
2. **If one agent covers it:** That agent has decision authority. Other agents can advise; the designated agent decides.
3. **If multiple agents cover it:** The system surfaces the overlap as a cross-role collaboration point (P11). Both contribute; the agent with more specific expertise for the particular context decides.
4. **If no agent covers it:** The system escalates to governance. The gap is logged as a Boundary Condition gap (requires new expertise declaration or institutional guidance).
5. **If an agent acts outside declared scope:** The action is logged, flagged, and tracked for outcomes. Systematic out-of-scope actions trigger governance review.

### SLM Authority Boundaries

SLMs have strict, non-negotiable authority limits:

| SLM | Can Do | Cannot Do |
|-----|--------|-----------|
| All SLMs | Generate draft outputs, suggest, flag, recommend | Commit to EHR, override human decision, suppress information |
| SLM-01 (Notes) | Draft clinical notes from encounter data | Interpret diagnostic findings, recommend treatments |
| SLM-02 (Coding) | Suggest codes with rationale | Assign final codes, modify clinical documentation |
| SLM-03 (Handoff) | Generate handoff summaries, flag changes | Triage patients by acuity, omit data without flagging |
| SLM-04 (Alerts) | Filter and contextualize alerts | Suppress critical alerts, make prescribing decisions |
| SLM-05 (Plain Language) | Translate clinical text to plain language | Provide medical advice beyond source text |
| SLM-06 (PA) | Draft PA requests, match evidence to criteria | Submit without human approval, fabricate evidence |

These limits are hard-coded in the Interpretation Interface artifact for each SLM and enforced at the system level. They are not configurable by end users; changes require governance-level review.

---

## LAYERED VALIDATION (P4)

### Human Organization Layer

- Credentials verified through institutional credentialing systems
- Role assignments managed by department leadership
- Peer review processes maintained by professional committees
- Institutional accountability enforced through existing structures

### AI Reinforcement Layer

The AI layer does not govern. It illuminates.

| AI Function | What It Detects | Who Receives It |
|-------------|-----------------|-----------------|
| Outcome pattern analysis | Which clinicians' override patterns correlate with better/worse outcomes | Governance dashboard (anonymized for pattern analysis; identified only for institutional quality review per existing protocols) |
| Expertise utilization analysis | When agents systematically act outside declared scope | Governance dashboard |
| Authority gap detection | When decisions fall into no agent's declared scope | Governance escalation |
| Cross-role conflict patterns | Recurring disagreements between roles at specific boundaries | Governance dashboard |

### Accountability Tracking

Every authority exercise is logged:
- What was decided
- By whom (agent_id)
- Under what declared authority
- What the outcome was (linked retrospectively)
- Whether the decision fell within declared scope

This creates a traceable chain from expertise declaration to decision to outcome. P3's promise, "you declared this expertise, outcomes are on you," becomes auditable fact.

---

## POWER ASYMMETRY HANDLING

### The Problem Restated

The framework review identified that healthcare involves structural power asymmetries, particularly at the clinician-payer boundary. These are not adversarial in the pure sense (both parties operate within healthcare), but they involve conflicting coherence criteria: the clinician measures patient outcome, the payer measures utilization efficiency.

### Framework Response (P3 + P6 + P9 + P11)

The system does not resolve the asymmetry directly. It makes the asymmetry's consequences visible, empirically, at scale.

**Step 1: Scope declaration (P3)**
- Payer criteria are registered as Boundary Conditions with `source: external_payer`
- Clinical criteria are registered as Purpose Articulation artifacts
- The system makes explicit that these originate from different authority domains

**Step 2: Empirical tracking (P6)**
- When a PA is denied, the denial reason and the clinical documentation are both recorded
- Patient outcome is tracked: did the denied treatment lead to worse outcomes?
- Approved alternative treatments are tracked: did the payer's suggested alternative achieve equivalent results?
- These are hypotheses tested by outcomes, not political arguments

**Step 3: Dual-test validation (P9)**
- Results track: outcome data across thousands of PA decisions
- Consensus track: clinician agreement with PA outcomes
- When results and consensus diverge (clinicians disagree with denials AND outcomes worsen), the system generates a governance-level signal

**Step 4: Cross-domain learning (P11)**
- Patterns of clinically harmful denials are surfaced with evidence
- Patterns of clinically unnecessary requests are also surfaced with evidence
- Both sides receive feedback grounded in outcomes, not opinion
- The accumulated evidence base supports institutional advocacy with payers (policy-level renegotiation)

### What the System Does NOT Do

- It does not override payer decisions
- It does not automatically approve denied treatments
- It does not take sides in the clinician-payer conflict
- It makes the conflict's consequences measurable, traceable, and reportable

This is governance by transparency and evidence, not by authority inversion.

---

## REGULATORY CONSTRAINT INTEGRATION

### Constraint Categories

| Category | Source | Negotiability | System Treatment |
|----------|--------|---------------|------------------|
| Federal regulatory (HIPAA, CMS, FDA) | External; federal law | Non-negotiable | Hard constraint; environment layer input |
| State regulatory (licensing, reporting) | External; state law | Non-negotiable | Hard constraint; environment layer input |
| Accreditation (Joint Commission, etc.) | External; voluntary but effectively mandatory | Non-negotiable in practice | Hard constraint; environment layer input |
| Payer policy | External; contractual | Negotiable through contract renegotiation | Boundary Condition, source: external_payer |
| Institutional policy | Internal; organizational | Negotiable through governance | Boundary Condition, source: internal |
| Clinical protocol | Internal; evidence-based | Negotiable through evidence and governance review | Soft constraint linked to evidence |

### Environmental Constraint Update Protocol

When a regulatory change occurs:

1. **Detection:** System monitors regulatory feeds (configurable sources) or receives manual input from compliance staff
2. **Impact analysis:** AI middleware identifies which artifacts are affected by the change
3. **Governance notification:** Affected stakeholders are notified with impact assessment
4. **Artifact update:** Boundary Conditions are updated to reflect new requirements
5. **Cascade:** All downstream artifacts and SLM configurations that depend on the changed constraint are flagged for review
6. **Validation:** Coherence Engine verifies that all affected components now comply

This is the Environment → Governance → Execution flow described in Spec-02, instantiated for regulatory changes.

---

## CONFLICT OF PRINCIPLES (P10)

When two valid principles conflict within the system:

**Example:** "Minimize documentation burden" (P7: unsuccessful artifacts are not kept) vs. "Maximize regulatory compliance" (environmental hard constraint requiring documentation).

**System response:**

1. The conflict is registered as a meta-principle artifact: both principles are valid, and neither can be eliminated.
2. The system tracks outcomes under each principle's influence: which documentation elements that exist purely for compliance also have clinical value? Which have none?
3. Over time, evidence may resolve the conflict: perhaps certain compliance elements can be auto-generated without physician time, satisfying both principles simultaneously.
4. Until resolved, the ambiguity is held explicitly. The system does not pretend the conflict does not exist; it makes the cost of each principle visible to governance.

**Conflict register fields:**
```
conflict_id:        UUID
principle_a:        TEXT (statement of first principle)
principle_b:        TEXT (statement of conflicting principle)
conflict_type:      ENUM [insufficient_results, insufficient_understanding,
                          true_contradiction]
current_resolution: ENUM [held_in_ambiguity, provisionally_resolved,
                          resolved_by_evidence]
evidence_log:       ARRAY of { entry_id, evidence_type, finding, date }
governance_notes:   TEXT
created_at:         TIMESTAMP
last_reviewed:      TIMESTAMP
```

---

## AUDIT TRAIL

All governance actions are permanently logged:

- Authority exercises (who decided what, under what declared scope)
- Boundary modifications (who changed a constraint, why, approved by whom)
- SLM boundary adjustments (what changed, what governance approved it)
- Regulatory constraint updates (what changed, when, impact assessment)
- Conflict resolutions (what was in conflict, what evidence resolved it, what was decided)
- Override actions (human overriding AI or AI flagging human; reason; outcome)

The audit trail is immutable. It cannot be edited or deleted, only appended. This satisfies both the framework's traceability requirement and healthcare's regulatory audit requirements.

---

*Governance is the framework's immune system. It does not prevent every infection; it detects anomalies, mobilizes the right resources, and learns from every encounter. The authority to decide is earned by declaring competence, bounded by scope, and tested by outcomes. Power asymmetries are not dissolved; they are made visible. External constraints are not fought; they are accommodated transparently. Ambiguity is not suppressed; it is held until evidence speaks.*

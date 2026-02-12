# Spec-04: Living Cycle Engine
## Coheara Specification

**Version:** 1.0
**Last Updated:** 2026-02-10
**Parent:** 00-MASTER-INDEX.md
**Dependencies:** Spec-01 (definitions), Spec-02 (architecture), Spec-03 (artifacts)

---

## PURPOSE

This document specifies the Living Cycle as a runtime process: how it starts, how stages transition, how the cycle rate adapts, and how the orchestrator manages concurrent cycles across patients, protocols, and institutional scopes.

---

## CORE PRINCIPLE

The cycle is the invariant. It runs continuously in the AI middleware. Humans do not operate the cycle; they govern it. The middleware executes stages 3-6 (interpretation, reification, observation, signal generation) at machine speed. Stages 1, 2, and 7 (purpose negotiation, artifact crystallization, re-negotiation) involve human participation, triggered by signals from the middleware.

---

## CYCLE STAGES

### Stage 1: Purpose Negotiation

**What happens:** Agents (human and AI) construct, contest, and stabilize the meaning of the work.

**Trigger:** P1 Bootstrap from Need; a felt difficulty expressed by a human agent. Or: a re-negotiation signal from Stage 7.

**Inputs:** Need statement (from human), existing artifacts (if cycle is recurring), coherence evidence (from previous cycle).

**Process:**
1. Human agent articulates the need or responds to a renegotiation signal
2. System presents relevant existing artifacts (prior purpose articulations, active constraints, historical context)
3. AI synthesizes: "Based on stated need and existing context, here is a draft purpose articulation"
4. Human reviews, modifies, approves
5. If multiple agents are involved, the system facilitates convergence (each agent's input recorded, disagreements surfaced)

**Output:** Draft or updated Purpose Articulation artifact.

**Human role:** Primary. Purpose cannot be set by AI alone.

**AI role:** Synthesis, recommendation, consistency checking against existing artifacts.

### Stage 2: Artifact Crystallization

**What happens:** The negotiated purpose is expressed as a constellation of linked artifacts.

**Trigger:** Stage 1 completion (purpose articulation approved).

**Process:**
1. System generates artifact scaffolds based on purpose type and scope: Boundary Conditions template, Interpretation Interface template, Context & Rationale template
2. AI pre-populates where possible: known regulatory constraints auto-load into Boundary Conditions; existing role definitions auto-load into Interpretation Interface; protocol-level templates pre-fill standard fields
3. Human agents review, modify, complete
4. System validates: completeness checks, required links, no orphan artifacts
5. Traceability links are established automatically where deterministic, proposed where inferential

**Output:** Complete artifact constellation in Active lifecycle state.

**Human role:** Review, completion, approval.

**AI role:** Scaffold generation, pre-population, validation, link proposal.

### Stage 3: Agent Interpretation

**What happens:** Agents (primarily SLMs at this stage) consume the artifact constellation and prepare to produce outputs.

**Trigger:** Active artifact constellation exists; clinical event occurs (encounter, order, shift change, discharge, etc.).

**Process:**
1. Clinical event detected (EHR integration or user action)
2. Cycle Orchestrator identifies which artifacts are relevant to this event
3. Context assembly: patient state + active artifacts + role boundaries + applicable constraints
4. Relevant SLM(s) receive assembled context within their declared expertise scope

**Output:** Assembled context package ready for SLM execution.

**Human role:** None at this stage (unless novel situation detected; see escalation rules below).

**AI role:** Primary. Context assembly and SLM activation are middleware operations.

### Stage 4: Reification

**What happens:** SLMs produce concrete outputs; tentative expressions of purpose.

**Trigger:** Stage 3 completion (context assembled, SLM activated).

**Process:**
1. SLM generates output within its expertise boundaries (note, code suggestion, handoff summary, filtered alert, plain-language text, PA draft)
2. Output is tagged as a reification with metadata: which SLM produced it, what artifacts it consumed, what confidence level, what patient/encounter scope
3. Output enters the coherence check pipeline before delivery

**Output:** Draft reification (not yet delivered to end user).

**Human role:** None at this stage. (Human review occurs after coherence check, if required.)

**AI role:** Primary. SLM execution.

### Stage 5: Coherence Observation

**What happens:** The Coherence Engine compares the reification against the artifact constellation.

**Trigger:** Stage 4 completion (reification produced).

**Process:**
1. Purpose alignment check: Does this output serve the stated purpose?
2. Boundary compliance check: Does this output honor all hard and soft constraints?
3. Rationale consistency check: Is this output consistent with recorded decisions and assumptions?
4. Agent agreement check: Does this output fall within the producing agent's authority scope?
5. Temporal consistency check: Are the assumptions this output depends on still valid?
6. Cross-artifact consistency check: Does this output conflict with other active reifications?

**Output:** Coherence assessment (score + dimensional breakdown + any tension signals).

**Human role:** None at this stage.

**AI role:** Primary. Coherence Engine runs all checks.

### Stage 6: Signal Generation

**What happens:** The coherence assessment is classified, triaged, and routed.

**Trigger:** Stage 5 completion.

**Signal classification:**

| Signal Type | Meaning | Example |
|-------------|---------|---------|
| COHERENT | Output aligns with purpose and constraints | Note accurately reflects encounter within protocol |
| GAP | Missing element expected by artifacts | Handoff summary lacks allergy information required by protocol |
| DRIFT | Meaning has shifted from original intent | Code suggestion diverges from documented clinical reasoning |
| CONFLICT | Output contradicts another active artifact | PA criteria conflict with clinical protocol |
| AMBIGUITY | Insufficient information to assess coherence | Documentation unclear; multiple valid interpretations exist |

**Triage (severity determines routing):**

| Severity | Criteria | Routing |
|----------|----------|---------|
| Informational | Minor, no clinical impact | Logged; visible in dashboard |
| Advisory | Moderate, addressable by user | Flagged in user interface with context |
| Urgent | Significant clinical or compliance risk | Interrupts workflow; requires acknowledgment |
| Critical | Immediate patient safety concern | Blocks output delivery; escalates to governance |

**Output:** Classified, triaged signal; routed to appropriate agent(s).

**Human role:** Receives and responds to advisory/urgent/critical signals.

**AI role:** Classification, triage, routing.

### Stage 7: Re-Negotiation

**What happens:** Tension signals that cannot be resolved at the execution layer surface to purpose space.

**Trigger:** Accumulated tension signals exceed threshold; critical signal detected; scheduled review cycle.

**Process:**
1. System packages the tension signal(s) with evidence: what drifted, what conflicted, what outcomes resulted
2. Relevant human agents are convened (the system identifies who, based on Interpretation Interface artifact)
3. Human agents review the evidence, discuss, decide
4. Decision: revise purpose, update constraints, modify protocols, adjust SLM configuration, or accept current state with documented rationale
5. Changes flow back to Stage 2 (artifact crystallization)

**Output:** Updated artifacts; cycle restarts from Stage 2 or Stage 3 depending on scope of change.

**Human role:** Primary. Re-negotiation is a governance action.

**AI role:** Evidence packaging, recommendation, impact analysis.

---

## CYCLE RATE ADAPTATION (P5)

The Cycle Orchestrator adjusts cycle frequency based on coherence signal patterns:

```
coherence_score_trend = calculate_trend(recent_scores, window)

IF coherence_score_trend == IMPROVING:
    cycle_interval = cycle_interval * 1.2  (decelerate; max: domain ceiling)

IF coherence_score_trend == STABLE:
    cycle_interval = cycle_interval  (maintain)

IF coherence_score_trend == DEGRADING:
    cycle_interval = cycle_interval * 0.7  (accelerate; min: domain floor)

IF coherence_score_trend == VOLATILE:
    cycle_interval = cycle_interval * 0.5  (significant acceleration)

IF critical_signal_detected:
    cycle_interval = EMERGENCY_MINIMUM  (immediate cycle)
```

**Domain-specific rate bounds:**

| Context | Floor (fastest) | Ceiling (slowest) | Emergency |
|---------|-----------------|--------------------|-----------| 
| ICU patient | Per-hour | Per-shift (8-12h) | Immediate |
| Acute inpatient | Per-shift | Daily | Per-hour |
| Ambulatory encounter | Per-encounter | Weekly | Per-encounter |
| Chronic management | Weekly | Monthly | Daily |
| Protocol governance | Monthly | Quarterly | Weekly |
| Institutional governance | Quarterly | Annually | Monthly |

Rate bounds are configurable per institution and per clinical context.

---

## CONCURRENT CYCLE MANAGEMENT

Coheara runs multiple cycles simultaneously:

- Each active patient has at least one cycle
- Each active protocol has a governance cycle
- Each department has a governance cycle
- Each cross-role boundary has a monitoring cycle

**Orchestration rules:**

1. Patient-level cycles are independent; they do not block each other.
2. Protocol-level cycles affect all patient-level cycles under that protocol; when a protocol artifact changes, all child cycles receive the update.
3. Institutional-level cycles set constraints that all lower-level cycles must honor.
4. Cycle priority: critical signals from any cycle are processed before non-critical signals from other cycles.

**Concurrency limit:** The system caps the number of active cycles per user to prevent signal saturation (the alert fatigue pattern at the cycle level). Signals from low-priority cycles are batched and presented at scheduled intervals rather than in real time.

---

## BOOTSTRAP PROTOCOL (P1)

When a new collaboration begins (new patient, new protocol, new institutional initiative), the system bootstraps:

1. **Need reception:** Human agent states the need (free text or structured input)
2. **Context scan:** System searches for existing artifacts that may be relevant (similar patient populations, existing protocols, institutional templates)
3. **Scaffold proposal:** AI generates a draft artifact constellation based on the need + existing context
4. **Human review:** Agent reviews, modifies, approves
5. **First reification:** System produces a first tentative output
6. **First coherence check:** Output is assessed against the new artifacts
7. **First signal:** System reports coherence state; cycle is live

The system does not require a prior system to start. It needs only a need, parties, and willingness to negotiate. This is P1 encoded.

---

## ESCALATION RULES

Not every signal requires human intervention. The Cycle Orchestrator manages escalation:

| Condition | Action |
|-----------|--------|
| Coherent signal, all checks pass | Deliver output; log; no human interruption |
| Advisory signal, known pattern | Flag in dashboard; user reviews at convenience |
| Advisory signal, novel pattern | Flag with emphasis; system explanation provided |
| Urgent signal | Interrupt current workflow; require acknowledgment |
| Critical signal | Block output delivery; escalate to supervising agent |
| Accumulated advisory signals exceeding threshold | Trigger governance-level review (Stage 7) |
| Cross-role conflict detected | Route to both roles' designated leads |

---

*The cycle is a clock that runs beneath clinical work. Most of the time, clinicians do not see it; they see its outputs (notes, summaries, codes, filtered alerts) and its signals (flags, recommendations, governance prompts). The cycle's invisibility is a feature, not a limitation. It means the framework is working.*

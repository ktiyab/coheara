# Spec-01: Framework Definitions in Healthcare Context
## Coheara Specification

**Version:** 1.0
**Last Updated:** 2026-02-10
**Parent:** 00-MASTER-INDEX.md
**Dependencies:** None (foundational vocabulary document)

---

## PURPOSE

This document defines every term from the Intent Preservation Framework (IPF) as it applies within Coheara. These definitions are operational, not theoretical; they describe what the term means inside the running system. Every other spec document uses these definitions. If a term appears undefined elsewhere, look here.

---

## CORE CONCEPTS

### Intent

The articulated purpose behind a clinical act, decision, or communication. Intent is not what was said; it is what was meant, including the conditions under which the meaning holds, the constraints that bound it, and the rationale that produced it.

In Coheara: Intent is the composite of a Purpose Articulation artifact and its linked Boundary Conditions, Context & Rationale, and Interpretation Interface artifacts. No single field captures intent; it emerges from the constellation.

**Healthcare example:** A physician's treatment goal is not just "reduce blood pressure." It is: reduce blood pressure (purpose) to below 130/80 (boundary) using ACE inhibitors first-line (rationale: patient has diabetes) unless contraindicated by renal function (constraint), with the primary care team managing titration (interpretation interface) and quarterly labs as coherence evidence.

### Semantic Drift

The measurable divergence between original intent and current understanding as information passes through multiple interpreters across time. Drift is not error; it is entropy. It accumulates silently in the absence of feedback.

In Coheara: Drift is detected by the Coherence Observation System (Spec-07), which continuously compares current artifact states and reification outputs against declared purpose. Drift is quantified as a coherence score; a composite metric defined in Spec-07.

**Healthcare example:** A discharge summary states "continue current medications." By the time a home health nurse reads it three days later, two medications have been held in-hospital and one dose was changed. The summary's meaning has drifted from the physician's actual intent.

### Artifact

A persistent, structured record that participates in meaning-construction. Artifacts do not contain meaning; they trigger meaning-construction in whoever reads them, shaped by that reader's context. Artifacts are the substrate on which meaning is continuously reconstructed.

In Coheara: Artifacts are first-class data objects with schema, versioning, ownership, traceability links, and lifecycle state. The six artifact types are defined below. Artifacts are never free-form text; they conform to templates that ensure structural completeness.

### Reification

A tentative expression of purpose in concrete form. Reifications are outputs: a diagnosis, a treatment plan, a clinical note, a code assignment, a handoff summary. They are "tentative" because they are subject to coherence observation; a reification that fails the coherence test is not discarded but metabolized (Spec-10).

In Coheara: Every SLM output is classified as a reification. Every human clinical action recorded through the system is a reification. Reifications are linked to the artifacts that produced them through the Traceability artifact.

### Coherence

The degree to which current behavior, outputs, and interpretations align with declared purpose, within declared boundaries, for stated rationale. Coherence is not perfection; it is alignment. A system is coherent when its parts serve its purpose and incoherent when they diverge.

In Coheara: Coherence is a measurable state, not a binary. The Coherence Observation System (Spec-07) generates coherence scores across multiple dimensions (purpose alignment, boundary compliance, rationale validity, agent agreement).

### Coherence Signal

A system output indicating the current coherence state. Signals are either COHERENT (reinforce, continue) or TENSION (gap, drift, conflict, ambiguity detected). Tension signals carry classification: what type of tension, what severity, what triggered it, what resolution path is recommended.

In Coheara: Coherence signals are the primary communication from the AI middleware to human users. The system does not interrupt clinicians with raw data; it surfaces classified, prioritized signals at appropriate moments.

### Agent

Any entity that interprets artifacts and produces reifications. Agents include human professionals (physicians, nurses, pharmacists, coders), AI systems (SLMs, clinical decision support tools), and composite tool chains. Each agent has declared expertise, bounded authority, and tracked accountability.

In Coheara: Every agent is registered with an Interpretation Interface artifact that declares its expertise domain, authority scope, boundaries, and accountability. AI agents have these declarations as system configuration; human agents declare them at onboarding and role assignment.

### Living Cycle

The continuous process through which coherence is maintained: purpose negotiation → artifact crystallization → agent interpretation → reification → coherence observation → signal generation → re-negotiation (if needed) → return. The cycle is the invariant; everything else varies by domain.

In Coheara: The cycle runs as a background process in the AI middleware. It does not require human participation at every stage. Humans enter the cycle at governance moments: initial purpose negotiation, re-negotiation when tension signals are surfaced, and periodic review. The AI middleware handles interpretation, reification (SLM outputs), and coherence observation at machine speed.

---

## THE SIX ARTIFACT TYPES

### 1. Purpose Articulation

**What it answers:** What does success mean? Why does this matter?

**In healthcare context:** The clinical objective, including success criteria, scope, exclusions, assumptions, and validity conditions.

**Granularity levels:**
- Patient level: Treatment goal for a specific patient encounter
- Protocol level: Standard of care for a condition or procedure
- Institutional level: Departmental or system-wide quality objectives
- Cross-organizational level: Payer-provider shared quality measures

**Required fields:** Success definition (observable criteria), rationale (what problem this solves), scope (included/excluded), assumptions (what must remain true), validity conditions (when to re-negotiate).

### 2. Boundary Conditions

**What it answers:** What constrains us? What is out of scope?

**In healthcare context:** Clinical contraindications, regulatory requirements, resource limitations, patient preferences, institutional policies, payer restrictions.

**Constraint categories:**
- **Hard constraints (non-negotiable):** Patient safety limits, regulatory requirements (HIPAA, CMS), legal obligations, ethical boundaries
- **Soft constraints (negotiable within protocol):** Institutional preferences, workflow conventions, formulary defaults
- **Invariants (must remain true throughout):** Medication allergy status, diagnosis codes, patient identity verification
- **Environmental constraints (externally imposed):** Federal/state regulations, accreditation standards, payer policies. Source field: "External." Negotiability: "Requires external advocacy."

**Required fields:** Constraint statement, rationale, enforced by, violation response, source (internal/external), negotiability level.

### 3. Interpretation Interface

**What it answers:** Who decides what? How do agents interact?

**In healthcare context:** Clinical role authorities, scope of practice, AI system permissions, escalation paths, conflict resolution protocols.

**Agent registration fields:**
- Agent identifier (person or system)
- Type (human / AI / tool chain)
- Expertise domain (clinical specialty, coding expertise, etc.)
- Authority scope (what this agent can decide)
- Boundaries (what this agent cannot do)
- Accountability (what this agent is responsible for)

**Interaction protocols:** Situation-specific rules for who acts, who reviews, who decides, and when to escalate. Includes AI-specific protocols: what the SLM may generate, what requires human review, what is prohibited.

### 4. Context & Rationale

**What it answers:** Why these choices? What was considered and rejected?

**In healthcare context:** Clinical reasoning, differential diagnosis, treatment alternatives considered, patient-specific factors, evidence cited, decision triggers for reconsideration.

**Required fields:** Decision description, date, participating agents, alternatives considered (with rejection rationale), selected rationale, assumptions, review trigger.

**Special function in healthcare:** This artifact addresses knowledge vaporization directly. When a clinician leaves or a shift changes, the Context & Rationale artifact preserves not just what was decided but why, under what assumptions, and when to reconsider.

### 5. Traceability

**What it answers:** How do artifacts connect to each other?

**In healthcare context:** Links between treatment goals and their constraints, between constraints and their evidence, between decisions and their outcomes, between clinical notes and their coded representations.

**Link types:**
- Purpose → Boundary (why each constraint exists relative to this goal)
- Purpose → Evidence (how we will know if we succeeded)
- Boundary → Interpretation (who enforces each constraint)
- Decision → Rationale (why this choice)
- Reification → Purpose (which goal this output serves)
- Reification → Outcome (what happened)

**Special function in healthcare:** Traceability is the mechanism that makes the coding translation visible. When an SLM suggests an ICD code from clinical narrative, the Traceability artifact links the code to the source text, the clinical reasoning, and the coding rule applied. This is auditable evidence, not a black box.

### 6. Coherence Evidence

**What it answers:** Are we succeeding?

**In healthcare context:** Clinical outcomes, quality metrics, compliance rates, patient-reported outcomes, readmission rates, coding accuracy rates, handoff error rates, alert override patterns.

**Evidence types:**
- Outcome evidence (did the treatment work?)
- Process evidence (was the protocol followed?)
- Agreement evidence (do all agents interpret artifacts consistently?)
- Drift evidence (has vocabulary, scope, or assumption shifted since last cycle?)

---

## THE ELEVEN PROPOSITIONS (Healthcare Instantiation)

### P1: Bootstrap from Need
**System behavior:** Every new collaboration, care plan, or workflow begins from a stated clinical need. The system requires a need statement before generating any artifacts. No templates are pre-filled without a purpose.

### P2: Emergent Ontology
**System behavior:** Shared clinical vocabulary is not imposed top-down. When agents use different terms for the same concept (physician says "hypertension," coder needs "I10"), the system detects the mapping, proposes alignment, and tracks whether the alignment holds under outcomes. Ontology is grown, not installed.

### P3: Expertise-Based Authority
**System behavior:** Each agent's declared expertise determines their authority scope. A coding SLM can suggest codes; it cannot override a physician's clinical judgment. A physician can override a clinical decision support alert; the override is logged with rationale and tracked for outcomes. Authority is bounded, accountable, and auditable.

### P4: Layered Validation
**System behavior:** Human organizations manage credentials and role assignments. AI reinforces by detecting patterns across outcomes: which clinicians' overrides correlate with adverse events, which coding patterns correlate with denials, which handoff templates correlate with fewer errors. AI does not govern; it illuminates.

### P5: Adaptive Cycle Rate
**System behavior:** Cycle frequency adjusts automatically. Stable patients on established protocols: slow cycle (weekly review). Acute patients with changing status: fast cycle (per-shift observation). Detected tension: cycle accelerates. Consistent coherence: cycle decelerates. Thresholds are discovered from outcomes, not preset.

### P6: Empirical Disagreement Resolution
**System behavior:** When agents disagree (physician vs. CDS alert, clinician vs. payer criteria, coder vs. documentation), the disagreement is captured as a hypothesis. Both interpretations are recorded. Outcomes are tracked. Over time, the system accumulates evidence about which interpretation pattern produces better results. Political disagreements become empirical questions.

### P7: Evidence-Weighted Memory
**System behavior:** Artifacts that are validated by outcomes persist and gain weight. Artifacts that are contradicted by outcomes are flagged for review and, if confirmed as invalid, archived with a deprecation reason. The system's memory is curated by results, not by seniority or inertia.

### P8: Failure Metabolism
**System behavior:** Failed reifications (wrong code, missed handoff item, inappropriate alert) are not simply deleted. They enter a pipeline: failure → root cause analysis → lesson extraction → principle codification. The principle feeds back into SLM training data, alert configuration, or protocol revision. Failure is fuel.

### P9: Dual-Test Truth
**System behavior:** A knowledge claim in the system (e.g., "this alert rule reduces adverse drug events") requires both evidence (outcome data showing reduction) and consensus (clinical team agreement that the reduction is clinically meaningful). Either alone is insufficient. Consensus without evidence is groupthink. Evidence without consensus is noise.

### P10: Meta-Principles for Conflict
**System behavior:** When two valid principles conflict (e.g., "minimize documentation burden" vs. "maximize coding accuracy"), the system does not force resolution. It creates a meta-principle that holds both in tension, tracks outcomes under each approach, and waits for evidence to resolve. Ambiguity is held, not suppressed.

### P11: Cross-Domain Learning
**System behavior:** When roles collaborate (physician-coder, nurse-pharmacist, clinician-payer), their principle conflicts are surfaced and tracked. Each conflict is an opportunity: it reveals where one domain's assumptions break against another's reality. The system accumulates cross-role meta-knowledge that no single role possesses.

---

## GLOSSARY OF OPERATIONAL TERMS

| Term | Definition in Coheara |
|------|---------------------------|
| **Cycle stage** | One of the seven stages of the Living Cycle (Spec-04) |
| **Governance moment** | A point where the cycle surfaces to human decision-making |
| **Execution task** | A bounded translation task performed by the SLM layer |
| **Coherence score** | A composite metric measuring alignment across dimensions (Spec-07) |
| **Tension signal** | A coherence signal indicating drift, gap, conflict, or ambiguity |
| **Role boundary** | The seam between two agent roles where translation occurs |
| **Translation protocol** | A pre-negotiated rule for how information transforms at a role boundary |
| **Artifact lifecycle** | Draft → Active → Under Review → Deprecated → Archived |
| **Reification** | Any concrete output (note, code, summary, alert) produced by an agent |
| **Knowledge vaporization** | Loss of decision rationale when agents (humans) leave |
| **Context pollution** | Accumulated noise that obscures original intent |
| **Signal saturation** | When coherence signals exceed agent processing capacity (the alert fatigue pattern) |
| **Environmental constraint** | A boundary imposed externally (regulatory, legal) that the collaboration cannot negotiate internally |

---

*This vocabulary is the shared ontology of the project itself; Coheara eating its own cooking (P2).*

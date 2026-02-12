# Spec-02: System Architecture
## Coheara Specification

**Version:** 1.0
**Last Updated:** 2026-02-10
**Parent:** 00-MASTER-INDEX.md
**Dependencies:** Spec-01 (definitions)

---

## PURPOSE

This document specifies the structural architecture of Coheara: how the layers relate, how data flows, and how the framework's logic is encoded as system behavior rather than documentation.

---

## THREE-LAYER ARCHITECTURE

Coheara mirrors the IPF's own architecture with three distinct layers. Each layer has a different clock speed, a different primary agent, and a different function.

```
┌─────────────────────────────────────────────────────────────────────┐
│                     ENVIRONMENT LAYER                               │
│                                                                     │
│   External inputs the system cannot negotiate:                      │
│   Regulatory changes (HIPAA, CMS, FDA), payer policy updates,      │
│   new clinical guidelines, institutional policy, staffing changes   │
│                                                                     │
│   Clock speed: Weeks to years                                       │
│   Primary agent: External bodies                                    │
│   System response: Update environmental constraints in Spec-09      │
└─────────────────────────────────┬───────────────────────────────────┘
                                  │ constrains
                                  ▼
┌─────────────────────────────────────────────────────────────────────┐
│                     GOVERNANCE LAYER                                 │
│                                                                     │
│   The IPF Living Cycle as institutional process:                    │
│   Purpose negotiation, artifact design, protocol configuration,     │
│   principle evolution, cross-role boundary management               │
│                                                                     │
│   Clock speed: Days to months                                       │
│   Primary agent: Human governance (clinical leadership, committees) │
│   AI role: Pattern analysis, evidence synthesis, recommendation     │
│   Output: Configured artifacts, validated protocols, evolved rules  │
└─────────────────────────────────┬───────────────────────────────────┘
                                  │ configures
                                  ▼
┌─────────────────────────────────────────────────────────────────────┐
│                     EXECUTION LAYER                                  │
│                                                                     │
│   AI middleware running the cycle at clinical speed:                 │
│   SLM translation tasks, coherence observation, signal triage,      │
│   artifact generation, reification production                       │
│                                                                     │
│   Clock speed: Seconds to hours                                     │
│   Primary agent: AI middleware (SLMs + coherence engine)            │
│   Human role: Review flagged signals, approve high-stakes outputs   │
│   Output: Clinical notes, handoff summaries, code suggestions,      │
│           filtered alerts, patient materials, coherence signals     │
└─────────────────────────────────────────────────────────────────────┘
```

### Layer Interaction Rules

1. **Environment → Governance:** Environmental changes trigger governance-level re-negotiation. A new CMS billing rule does not propagate directly to the execution layer; it enters through governance, where its implications are assessed, artifacts updated, and protocols reconfigured.

2. **Governance → Execution:** Governance produces the artifacts that configure execution. When a clinical committee decides on a new handoff protocol, that decision crystallizes into artifact updates that the execution layer consumes. The SLMs do not decide what information belongs in a handoff; the governance layer decides, and the SLMs execute.

3. **Execution → Governance (feedback):** The execution layer generates coherence signals that feed back to governance. When the SLM detects systematic drift (e.g., handoff summaries consistently missing a data element that correlates with adverse events), that signal surfaces to the governance layer for re-negotiation. This is the cycle's feedback loop.

4. **Execution does not modify Governance directly.** The SLM layer cannot autonomously change protocols, artifact schemas, or authority boundaries. It can recommend changes by surfacing evidence, but changes require human governance action.

---

## MIDDLEWARE ARCHITECTURE

The AI middleware is the execution layer's engine. It runs continuously, processing clinical events as they occur.

### Component Map

```
┌─────────────────────────────────────────────────────────────────────┐
│                     AI MIDDLEWARE                                     │
│                                                                     │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────────┐  │
│  │  SLM CLUSTER │  │  COHERENCE   │  │  ARTIFACT MANAGER        │  │
│  │              │  │  ENGINE      │  │                          │  │
│  │ Note Gen     │  │              │  │ Schema enforcement       │  │
│  │ Code Suggest │  │ Drift detect │  │ Version control          │  │
│  │ Handoff Gen  │  │ Signal gen   │  │ Traceability maintenance │  │
│  │ Alert Filter │  │ Score calc   │  │ Lifecycle management     │  │
│  │ Plain Lang   │  │ Triage       │  │ Link integrity           │  │
│  │ PA Draft     │  │              │  │                          │  │
│  └──────┬───────┘  └──────┬───────┘  └────────────┬─────────────┘  │
│         │                 │                        │                │
│         └─────────────────┼────────────────────────┘                │
│                           │                                         │
│                    ┌──────┴───────┐                                 │
│                    │  CYCLE       │                                 │
│                    │  ORCHESTRATOR│                                 │
│                    │              │                                 │
│                    │ Stage mgmt   │                                 │
│                    │ Rate control │                                 │
│                    │ Signal route │                                 │
│                    │ Escalation   │                                 │
│                    └──────────────┘                                 │
└─────────────────────────────────────────────────────────────────────┘
```

### SLM Cluster

Multiple specialized SLMs, each with declared expertise boundaries (Spec-05):
- **Note Generation SLM:** Clinical encounter → structured note
- **Code Suggestion SLM:** Clinical narrative → ICD/CPT code suggestions
- **Handoff Generation SLM:** EHR state → structured handoff summary
- **Alert Filtering SLM:** Raw CDS alerts + patient context → prioritized, contextualized signals
- **Plain Language SLM:** Clinical text → patient-readable materials
- **PA Drafting SLM:** Clinical documentation + payer criteria → PA request draft

Each SLM is a bounded agent with explicit Interpretation Interface declarations. They do not share models; they share the artifact constellation.

### Coherence Engine

Continuously compares reifications against purpose (Spec-07):
- Drift detection: Has the meaning shifted since the last cycle?
- Signal generation: What type of tension exists? What severity?
- Score calculation: Multi-dimensional coherence metric
- Triage: Which signals require human attention? Which are informational?

### Artifact Manager

Maintains the artifact constellation (Spec-03):
- Schema enforcement: Artifacts conform to templates
- Version control: Every change is versioned with author, timestamp, rationale
- Traceability maintenance: Links between artifacts are maintained automatically
- Lifecycle management: Draft → Active → Under Review → Deprecated → Archived
- Link integrity: Orphan detection, broken link repair

### Cycle Orchestrator

Runs the Living Cycle as a process (Spec-04):
- Stage management: Tracks which stage each active cycle is in
- Rate control: Adjusts cycle frequency based on coherence signals (P5)
- Signal routing: Directs signals to appropriate agents (human or AI)
- Escalation: When execution-layer signals require governance-layer action

---

## DATA FLOW ARCHITECTURE

### Clinical Event Processing

```
CLINICAL EVENT (encounter, order, handoff, discharge)
        │
        ▼
┌───────────────────┐
│ EVENT CLASSIFIER  │──→ Determines which SLMs activate
└───────┬───────────┘     and which artifacts are relevant
        │
        ▼
┌───────────────────┐
│ CONTEXT ASSEMBLY  │──→ Gathers: patient state, active artifacts,
└───────┬───────────┘     role boundaries, applicable constraints
        │
        ▼
┌───────────────────┐
│ SLM EXECUTION     │──→ Produces reification (note, code, summary)
└───────┬───────────┘     within declared expertise boundaries
        │
        ▼
┌───────────────────┐
│ COHERENCE CHECK   │──→ Compares output against purpose, boundaries,
└───────┬───────────┘     rationale. Generates coherence signal.
        │
        ├──→ COHERENT: Output delivered to user
        │
        └──→ TENSION: Signal classified, triaged, routed
                │
                ├──→ Low severity: Logged, informational
                ├──→ Medium severity: Flagged to user with context
                └──→ High severity: Escalated, output held for review
```

### Feedback Loop (Execution → Governance)

```
COHERENCE SIGNALS (accumulated over time)
        │
        ▼
┌───────────────────┐
│ PATTERN DETECTOR  │──→ Identifies systematic issues:
└───────┬───────────┘     recurring drift, persistent overrides,
        │                  correlated failures
        ▼
┌───────────────────┐
│ EVIDENCE          │──→ Packages pattern with outcome data
│ SYNTHESIZER       │     into governance-ready report
└───────┬───────────┘
        │
        ▼
┌───────────────────┐
│ GOVERNANCE        │──→ Surfaces to human governance layer
│ ESCALATION        │     with recommended action
└───────────────────┘
```

---

## INTEGRATION ARCHITECTURE

### EHR Integration

Coheara does not replace the EHR. It sits alongside it as a collaboration layer.

**Inbound data:** Patient demographics, clinical notes, orders, results, medication lists, problem lists, allergy lists, scheduling data. Sourced via HL7 FHIR R4 APIs or institutional integration interfaces.

**Outbound data:** Generated notes (for clinician review and approval before EHR commit), code suggestions (for coder review), handoff summaries (for nursing review), patient education materials (for clinician approval before distribution).

**Principle:** Coheara never writes directly to the EHR without human approval. The SLM produces draft reifications; the human agent reviews, modifies if needed, and commits. The system records the review action as a coherence data point.

### Institutional System Integration

| System | Integration Type | Data Direction |
|--------|-----------------|----------------|
| EHR (Epic, Cerner, etc.) | FHIR R4 API | Bidirectional |
| Billing/Coding system | Code suggestion API | Outbound (suggestions) |
| Payer portals | PA submission API | Outbound (drafts) |
| Clinical decision support | Alert feed API | Inbound (raw alerts) |
| Identity/credentialing | Role/credential API | Inbound (agent declarations) |
| Quality reporting | Metrics API | Outbound (coherence evidence) |

---

## ENCODING THE FRAMEWORK

The framework is not a reference document within Coheara. It is the system's operating logic. Here is how each architectural component maps to framework elements:

| Framework Element | System Component | How It Is Encoded |
|-------------------|------------------|-------------------|
| Living Cycle | Cycle Orchestrator | State machine with seven stages; transitions triggered by events and signals |
| Artifact Constellation | Artifact Manager | Data model with six artifact types, enforced schemas, maintained links |
| Purpose Negotiation | Collaboration Space UI + Governance workflows | Human-facing interfaces for articulating and revising purpose |
| Coherence Observation | Coherence Engine | Continuous background process comparing reifications to purpose |
| Signal Generation | Coherence Engine + Signal Router | Classified, triaged outputs directed to appropriate agents |
| Re-Negotiation | Governance escalation + Collaboration Space | Triggered when tension signals exceed threshold |
| Adaptive Cycle Rate | Cycle Orchestrator rate controller | Algorithm adjusting frequency based on coherence score trends |
| Expertise Governance | Interpretation Interface artifacts + Role system | Enforced at every SLM call and every human action |
| Failure Metabolism | Memory & Learning System | Pipeline from failed reification → lesson → principle → config update |
| Cross-Domain Learning | Cross-Role Protocol engine | Conflict detection at role boundaries, meta-knowledge accumulation |

---

## NON-FUNCTIONAL REQUIREMENTS

| Requirement | Target | Rationale |
|-------------|--------|-----------|
| SLM response latency | < 3 seconds for point-of-care tasks | Clinical workflow tolerance (Spec-12) |
| Coherence check latency | < 1 second additional overhead | Must not block clinical action |
| Data residency | On-premise or institutional cloud | HIPAA compliance; no patient data to external APIs (Spec-12) |
| Availability | 99.9% uptime for execution layer | Hospital operations are continuous |
| Audit trail completeness | 100% of SLM inputs/outputs logged | Regulatory traceability (Spec-09) |
| Artifact versioning | Full history with diff capability | Knowledge vaporization prevention |
| Role-based access | Enforced at every data access point | P3 expertise boundaries + HIPAA minimum necessary |

---

*The architecture serves one purpose: ensuring that the cycle runs, the artifacts persist, the signals surface, and the humans govern. Everything else is implementation detail.*

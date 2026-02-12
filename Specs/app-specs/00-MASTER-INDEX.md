# Coheara: Master Specification Index
## Collaboration Infrastructure for Health Professionals
### Encoding the Intent Preservation Framework as Operational Logic

---

## CONTINUITY ANCHOR

This document is the root of a multi-document specification. It is designed to be compression-proof: any session, any agent, any point in time can read this document and reconstruct the full intent, scope, and status of the project. Every other document in this specification references back to this index. If context is lost, start here.

---

## WHAT THIS PROJECT IS

**Coheara** is a collaboration tool for health professionals that:

1. **Encodes the Intent Preservation Framework (IPF)** as its operating logic; not as documentation to be read, but as firmware that governs system behavior
2. **Deploys health-dedicated SLMs** as the execution layer for high-volume, time-critical, bounded translation tasks
3. **Serves as a role-agnostic collaboration space** where meaning is preserved across role boundaries (physician-coder, nurse-nurse, clinician-patient, clinician-payer)
4. **Resolves empirically documented healthcare problematics**: documentation burden, handover failures, coding errors, prior authorization friction, alert fatigue, and the health literacy gap

The tool operates on a two-layer architecture:
- **Governance layer**: The IPF Living Cycle, run primarily by AI middleware, governing how artifacts are created, evaluated, and evolved. Humans enter at governance moments.
- **Execution layer**: Health-dedicated SLMs performing bounded translation tasks (note → code, EHR data → handoff summary, clinical language → plain language) at clinical speed.

The connection between layers is the **Artifact Constellation**: the governance layer produces and refines artifacts; the execution layer consumes and generates them.

---

## WHY THIS PROJECT EXISTS

Healthcare professionals lose 125 million physician-hours annually to documentation. 80% of serious adverse events involve handoff miscommunication. 22% of coded items are incorrect. 90-96% of clinical alerts are overridden. These are not separate problems; they are symptoms of a single architectural failure: information moves forward through the system, but meaning does not come back. The system is open-loop. Coheara closes the loop.

---

## CORE DESIGN DECISIONS (INVARIANTS)

These decisions are fixed. They do not change without explicit renegotiation at the project governance level.

| Decision | Rationale | Reference |
|----------|-----------|-----------|
| Framework is encoded as logic, not documentation | A framework that is only read is a framework that drifts. Encoding ensures the cycle runs. | Spec-02 |
| AI operates as middleware, not endpoint | Humans govern; AI executes the cycle at machine speed and surfaces signals. | Spec-02, Spec-05 |
| Role-agnostic design | Meaning breaks at role boundaries. A tool that serves one role optimizes within a silo. | Spec-06, Spec-11 |
| SLMs over general LLMs | Declared expertise boundaries, traceability, auditability, latency, privacy, cost. | Spec-05 |
| Artifact constellation as shared memory | No single artifact carries meaning; meaning emerges from relationships among artifacts. | Spec-03 |
| Living Cycle as runtime process | Coherence is emergent, not assumed. The cycle runs continuously, not on demand. | Spec-04 |
| Hard boundary for regulatory constraints | HIPAA, CMS, FDA, state licensing: non-negotiable environment parameters. | Spec-09 |

---

## SPECIFICATION DOCUMENTS

| Code | Document | Scope | Status |
|------|----------|-------|--------|
| **00** | Master Index (this document) | Project anchor, continuity plan, invariants | Active |
| **01** | Framework Definitions | All IPF terms defined in healthcare context | Active |
| **02** | System Architecture | Three-layer architecture, middleware design, data flow | Active |
| **03** | Artifact Engine | Data model for the six artifact types, storage, versioning, traceability | Active |
| **04** | Living Cycle Engine | The cycle as runtime process, stage transitions, signal routing | Active |
| **05** | SLM Integration Layer | Model requirements, task boundaries, I/O protocols, evaluation | Active |
| **06** | Collaboration Space | Role-agnostic workspace, UI/UX principles, interaction model | Active |
| **07** | Coherence Observation System | Monitoring, drift detection, signal generation, alert triage | Active |
| **08** | Problematics Mapping | Each healthcare problematic → tool capabilities → framework mechanisms | Active |
| **09** | Governance & Boundaries | Expertise authority, regulatory constraints, power asymmetry handling | Active |
| **10** | Memory & Learning | Evidence-weighted memory, failure metabolism, principle evolution | Active |
| **11** | Cross-Role Protocols | Role boundary handling, translation protocols, handover specs | Active |
| **12** | Deployment & Infrastructure | Privacy, latency, on-premise requirements, regulatory compliance | Active |

---

## FRAMEWORK PRINCIPLES ENCODED (Quick Reference)

| Principle | Where It Lives in the Tool | Primary Spec |
|-----------|---------------------------|--------------|
| P1: Bootstrap from Need | System initialization; new collaboration starts from felt difficulty | Spec-04 |
| P2: Emergent Ontology | Shared vocabulary engine; terminology negotiation module | Spec-03, Spec-06 |
| P3: Expertise-Based Authority | Role declarations, scope boundaries, accountability tracking | Spec-09 |
| P4: Layered Validation | AI pattern detection reinforcing human governance | Spec-05, Spec-09 |
| P5: Adaptive Cycle Rate | Cycle engine self-regulation based on coherence signals | Spec-04 |
| P6: Empirical Disagreement | Hypothesis tracking, outcome comparison, conflict-to-test pipeline | Spec-04, Spec-07 |
| P7: Evidence-Weighted Memory | Artifact retention/pruning based on outcome validation | Spec-10 |
| P8: Failure Metabolism | Failed artifact → lesson → principle pipeline | Spec-10 |
| P9: Dual-Test Truth | Results + consensus validation for knowledge claims | Spec-07 |
| P10: Meta-Principles | Ambiguity holding, unresolved conflict management | Spec-09, Spec-10 |
| P11: Cross-Domain Learning | Cross-role conflict surfacing, boundary discovery | Spec-11 |

---

## PROBLEMATICS ADDRESSED (Quick Reference)

| Problematic | Primary Tool Response | Primary Spec |
|-------------|----------------------|--------------|
| Documentation burden (125M hrs/yr) | SLM note generation, structured data entry, artifact-based summarization | Spec-05, Spec-08 |
| Handover failures (80% adverse events) | Structured handoff generation, critical-change flagging, coherence observation | Spec-08, Spec-11 |
| Coding errors (22% error rate) | Code suggestion from narrative, documentation completeness checking | Spec-05, Spec-08 |
| Prior authorization ($35B system cost) | PA form generation, criteria matching, outcome tracking across denials | Spec-08, Spec-09 |
| Alert fatigue (90-96% override) | Context-aware filtering, patient-specific risk assessment, signal triage | Spec-07, Spec-08 |
| Health literacy gap (88% below proficient) | Plain-language translation, reading-level adaptation, multilingual output | Spec-05, Spec-08 |
| Burnout (62.8% physician burnout) | Aggregate reduction across all above; time-to-care metric tracking | Spec-07, Spec-08 |

---

## CONTINUITY PROTOCOL

**If you are a new agent (human or AI) entering this project:**

1. Read this document first. It is the map.
2. Read Spec-01 (Framework Definitions) to understand the vocabulary.
3. Read Spec-02 (System Architecture) to understand the structural design.
4. Read the specific spec document relevant to your task.
5. If you modify any spec, update the Status column in this index.
6. If you add a new spec, add it to the table above with a new code.
7. If you encounter a contradiction between specs, surface it as a coherence signal (the framework's own P6 applies to itself).

**Version control rule:** Each spec document carries its own version number. This index carries the project version. When any spec changes, this index's "Last Updated" field updates.

**Project Version:** 1.0
**Last Updated:** 2026-02-10
**Status:** Initial specification complete

---

*This document is the anchor. Everything else hangs from it.*

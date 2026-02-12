# Spec-06: Collaboration Space
## Coheara Specification

**Version:** 1.0
**Last Updated:** 2026-02-10
**Parent:** 00-MASTER-INDEX.md
**Dependencies:** Spec-01, Spec-02, Spec-03, Spec-04, Spec-05

---

## PURPOSE

This document specifies the user-facing collaboration space: the interface through which health professionals interact with the system, with each other through the system, and with AI agents. The collaboration space is role-agnostic by design; it serves the seams between roles, not the silos within them.

---

## DESIGN PRINCIPLES

1. **The tool serves the transition, not the task.** EHRs serve the clinical task. Coding systems serve the coding task. Coheara serves the space where these tasks hand off to each other; where meaning must survive translation across role boundaries.

2. **Invisible framework, visible value.** Clinicians do not see "Purpose Articulation artifacts" or "Coherence Observation scores." They see: a draft note ready for review, a handoff summary at shift change, a flag on a coding gap, a plain-language discharge instruction. The framework runs beneath; the value surfaces above.

3. **Interruption budget.** Every notification, flag, or signal consumes attention. The system has a strict interruption budget per user per session: signals are batched, prioritized, and presented at appropriate moments, not as a continuous stream. This is the anti-alert-fatigue principle encoded as a UI constraint.

4. **Role-aware, not role-restricted.** The interface adapts to the user's role (showing relevant SLM outputs, appropriate signal types, and role-specific actions) but does not prevent cross-role visibility. A physician can see the coding SLM's suggestions. A coder can see the clinical rationale. Transparency across boundaries is the mechanism for P11 (Cross-Domain Learning).

---

## USER EXPERIENCE ARCHITECTURE

### Primary Views

**1. Clinical Workspace**

The clinician's primary view during patient care.

| Element | Function |
|---------|----------|
| Patient context panel | Current state: active problems, medications, allergies, vitals, recent events. Sourced from EHR integration. |
| SLM output panel | Draft outputs relevant to current action: note draft, alert assessment, suggested orders. Presented as editable drafts, not final products. |
| Signal panel | Active coherence signals for this patient, triaged by severity. Critical signals are prominent; informational signals are collapsed. |
| Action bar | Accept draft, modify draft, reject draft (with reason), escalate, request re-generation. |

**2. Transition Dashboard**

The handoff and care transition view.

| Element | Function |
|---------|----------|
| Patient list with coherence indicators | Color-coded by coherence state: green (stable), yellow (advisory signals active), red (urgent signals). |
| Handoff summary panel | SLM-generated handoff for selected patient. Editable. Flagged items highlighted. |
| Unresolved items queue | Pending actions, unanswered questions, deferred decisions from previous shift. Each linked to source artifact. |
| Comparison view | Side-by-side: what the previous shift documented vs. current EHR state. Differences highlighted. |

**3. Coding Workspace**

The coder's primary view.

| Element | Function |
|---------|----------|
| Clinical narrative panel | Source documentation with relevant text highlighted by the coding SLM. |
| Code suggestion panel | Ranked suggestions with rationale, confidence, and evidence links. |
| Gap detection panel | Documentation elements that are insufficient for coding, with specific recommendations. |
| Traceability view | Visual link: clinical text → coding rule → suggested code. Auditable chain. |

**4. Patient Communication Workspace**

For generating and reviewing patient-facing materials.

| Element | Function |
|---------|----------|
| Source text panel | Clinical content to be translated. |
| Plain-language panel | SLM-generated translation. Reading level displayed. |
| Glossary panel | Key terms with plain-language definitions. |
| Cultural/language options | Reading level selector, language selector (where multilingual SLM available). |
| Clinician approval controls | Approve for distribution, modify, flag for verbal follow-up. |

**5. Governance Dashboard**

For clinical leadership, quality teams, and institutional governance.

| Element | Function |
|---------|----------|
| Coherence trend panel | Aggregate coherence scores across patients, departments, protocols. Trend lines. |
| Signal pattern view | Recurring tension signals, grouped by type, severity, and source. |
| Cross-role conflict log | Active disagreements between roles, with evidence from both sides. |
| Principle evolution tracker | Lessons extracted from failures, principles codified, principles challenged. |
| Protocol performance view | Per-protocol coherence metrics, outcome alignment, boundary violation rates. |

---

## INTERRUPTION BUDGET

The system enforces a signal presentation budget to prevent recreating the alert fatigue problem at the framework level.

**Budget rules:**

| Signal Severity | Presentation | Budget Impact |
|----------------|--------------|---------------|
| Informational | Batched; presented at session start or scheduled review | Zero (does not consume budget) |
| Advisory | Presented in signal panel; does not interrupt active task | Low |
| Urgent | Presented as prominent notification; requires acknowledgment | High |
| Critical | Interrupts workflow; blocks action until addressed | Maximum (overrides budget) |

**Budget calculation:** Each user role has a configurable maximum interruption score per hour. Advisory signals consume 1 point; urgent signals consume 5 points. When the budget is approaching its limit, advisory signals are deferred to the next scheduled review. Only critical signals override the budget.

**Rationale:** The alert fatigue problem (90-96% override rates) was caused by systems that had no interruption budget. Every alert was presented with equal urgency. Coheara treats clinician attention as a finite, precious resource and manages it accordingly.

---

## INTERACTION PATTERNS

### Pattern 1: Accept-Modify-Reject

Every SLM output is presented as a draft with three actions:
- **Accept:** Output proceeds as-is. The acceptance is logged as coherence evidence (human agreed with AI output).
- **Modify:** Human edits the output. The modification is logged with diff. Over time, modification patterns feed back into SLM evaluation (Spec-05) and governance review.
- **Reject:** Human rejects the output with a reason. The rejection enters the failure metabolism pipeline (Spec-10). Rejections with reasons are the richest learning signal the system receives.

### Pattern 2: Signal Response

When a coherence signal is presented:
- **Acknowledge:** "I see this; it's fine." Logged. If the same signal recurs, the pattern is tracked.
- **Investigate:** "Show me why." System presents the evidence chain: what triggered the signal, what artifacts are involved, what the coherence engine detected.
- **Escalate:** "This needs governance review." Signal is routed to Stage 7 (re-negotiation).
- **Resolve:** "Here's the fix." Human action resolves the tension; the resolution is logged and the signal is cleared.

### Pattern 3: Cross-Role Handshake

When work crosses a role boundary (physician note → coding, nurse handoff → oncoming nurse):
- The system presents the translation artifact alongside its source
- Both the source and translation are visible to both roles
- Disagreements between roles are captured as P6 hypotheses, not as errors
- Outcome tracking determines which role's interpretation aligns with results

---

## ONBOARDING

When a new user enters the system:

1. **Role declaration:** User declares their role, expertise domain, and institutional credentials. This populates their agent record in the Interpretation Interface.
2. **Context orientation:** System presents active artifacts relevant to their role and scope (active patients, active protocols, current coherence state).
3. **Knowledge continuity:** For patient-level context, the system presents the Context & Rationale artifact; the decisions made, why, by whom, and under what assumptions. This is the anti-knowledge-vaporization mechanism.
4. **Calibration period:** During the first [configurable] days, the system presents SLM outputs with higher visibility (more context, more explanation) to build user trust and calibrate expectations.

---

## ACCESSIBILITY AND CONSTRAINTS

| Requirement | Specification |
|-------------|---------------|
| Response time | SLM outputs visible within 3 seconds of request |
| Mobile access | Transition Dashboard and Signal Panel available on mobile devices |
| Offline capability | Read-only access to patient artifacts and most recent handoff summaries when network unavailable; sync on reconnect |
| Language | Interface language configurable; patient-facing materials multilingual per SLM-05 capabilities |
| Accessibility | WCAG 2.1 AA compliance minimum |

---

*The best tool is the one the clinician barely notices. It drafts the note while they examine the patient. It flags the gap before the coder encounters it. It translates the plan before the patient leaves. The framework runs underneath; the clinician works on top. The stethoscope comes off the desk and goes back where it belongs.*

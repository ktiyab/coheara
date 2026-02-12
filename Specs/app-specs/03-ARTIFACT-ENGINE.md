# Spec-03: Artifact Engine
## Coheara Specification

**Version:** 1.0
**Last Updated:** 2026-02-10
**Parent:** 00-MASTER-INDEX.md
**Dependencies:** Spec-01 (definitions), Spec-02 (architecture)

---

## PURPOSE

This document specifies the data model, storage, versioning, and traceability mechanisms for the six artifact types. The Artifact Engine is the shared memory of Coheara; it is where meaning persists between agents, across shifts, and over time.

---

## DESIGN PRINCIPLE

No single artifact carries meaning. Meaning emerges from the relationships among artifacts. Traceability is therefore the structural hub; it connects all others and makes their relationships explicit, queryable, and auditable.

Every artifact is:
- **Typed** (one of six types)
- **Versioned** (full history, diffable)
- **Owned** (creating agent identified)
- **Scoped** (linked to a purpose, a patient context, or an institutional context)
- **Linked** (traceability connections to other artifacts maintained)
- **Lifecycle-managed** (Draft → Active → Under Review → Deprecated → Archived)

---

## ARTIFACT DATA MODEL

### Common Fields (All Artifact Types)

```
artifact_id:        UUID (system-generated, immutable)
artifact_type:      ENUM [purpose_articulation, boundary_conditions,
                          interpretation_interface, context_rationale,
                          traceability, coherence_evidence]
version:            INTEGER (auto-incremented on change)
created_by:         agent_id (human or AI)
created_at:         TIMESTAMP (UTC)
last_modified_by:   agent_id
last_modified_at:   TIMESTAMP (UTC)
lifecycle_state:    ENUM [draft, active, under_review, deprecated, archived]
scope:              OBJECT {
                      scope_type: ENUM [patient, encounter, protocol,
                                        department, institution, cross_org],
                      scope_id: UUID (reference to scoped entity)
                    }
linked_artifacts:   ARRAY of {
                      target_artifact_id: UUID,
                      link_type: ENUM [derives_from, constrains, evidences,
                                       implements, conflicts_with, supersedes],
                      link_rationale: TEXT,
                      created_by: agent_id,
                      created_at: TIMESTAMP
                    }
change_history:     ARRAY of {
                      version: INTEGER,
                      changed_by: agent_id,
                      changed_at: TIMESTAMP,
                      change_type: ENUM [create, update, state_change, link_add,
                                         link_remove],
                      change_description: TEXT,
                      previous_state: SNAPSHOT
                    }
```

### Type-Specific Schemas

#### 1. Purpose Articulation

```
success_definition:     TEXT (observable criteria; what outcome means success)
rationale:              TEXT (what problem this solves; cost of not solving)
scope_includes:         ARRAY of TEXT
scope_excludes:         ARRAY of TEXT
assumptions:            ARRAY of {
                          statement: TEXT,
                          rationale: TEXT,
                          validity_check: TEXT,
                          status: ENUM [active, challenged, invalidated]
                        }
validity_conditions:    ARRAY of {
                          condition: TEXT,
                          monitoring_method: TEXT,
                          renegotiation_trigger: BOOLEAN
                        }
priority:               ENUM [critical, high, standard, low]
temporal_horizon:       ENUM [encounter, episode, ongoing, indefinite]
```

#### 2. Boundary Conditions

```
hard_constraints:       ARRAY of {
                          statement: TEXT,
                          rationale: TEXT,
                          enforced_by: agent_id or SYSTEM,
                          violation_response: TEXT,
                          source: ENUM [internal, external_regulatory,
                                        external_payer, external_legal],
                          negotiability: ENUM [non_negotiable, requires_external_advocacy]
                        }
soft_constraints:       ARRAY of {
                          statement: TEXT,
                          rationale: TEXT,
                          negotiation_process: TEXT,
                          current_status: ENUM [active, waived, under_review]
                        }
invariants:             ARRAY of {
                          statement: TEXT,
                          verification_method: TEXT,
                          check_frequency: TEXT,
                          last_verified: TIMESTAMP,
                          verified_by: agent_id
                        }
out_of_scope:           ARRAY of {
                          item: TEXT,
                          exclusion_rationale: TEXT
                        }
```

Note the `source` and `negotiability` fields on hard constraints. These encode the distinction between internally generated and externally imposed constraints, surfaced during framework review. Environmental constraints (HIPAA, CMS rules) carry `source: external_regulatory` and `negotiability: non_negotiable` or `requires_external_advocacy`.

#### 3. Interpretation Interface

```
agents:                 ARRAY of {
                          agent_id: UUID,
                          agent_type: ENUM [human, ai_slm, tool_chain, composite],
                          expertise_domain: TEXT,
                          authority_scope: TEXT,
                          boundaries: TEXT,
                          accountability: TEXT,
                          credential_reference: TEXT (for humans; license, certification)
                        }
interaction_protocols:  ARRAY of {
                          situation: TEXT,
                          actions: ARRAY of {
                            agent_id: UUID,
                            action: TEXT,
                            sequence_order: INTEGER
                          },
                          decision_authority: agent_id,
                          escalation_path: TEXT
                        }
ai_protocols:           {
                          permitted_actions: ARRAY of TEXT,
                          prohibited_actions: ARRAY of TEXT,
                          human_review_required_when: ARRAY of TEXT,
                          confidence_threshold_for_autonomous: FLOAT (0.0-1.0)
                        }
conflict_resolution:    {
                          steps: ARRAY of TEXT,
                          escalation_path: TEXT,
                          empirical_resolution_enabled: BOOLEAN (P6)
                        }
```

#### 4. Context & Rationale

```
decisions:              ARRAY of {
                          decision_id: UUID,
                          description: TEXT,
                          date: TIMESTAMP,
                          participating_agents: ARRAY of agent_id,
                          alternatives_considered: ARRAY of {
                            description: TEXT,
                            rejection_rationale: TEXT
                          },
                          selected_rationale: TEXT,
                          assumptions: ARRAY of TEXT,
                          review_trigger: TEXT,
                          linked_evidence: ARRAY of artifact_id
                        }
historical_context:     TEXT (narrative explaining background)
previous_approaches:    ARRAY of {
                          approach: TEXT,
                          outcome: TEXT,
                          lessons: TEXT
                        }
assumption_register:    ARRAY of {
                          assumption: TEXT,
                          rationale: TEXT,
                          validity_check: TEXT,
                          status: ENUM [active, challenged, invalidated],
                          invalidation_evidence: TEXT (if invalidated)
                        }
```

#### 5. Traceability

```
links:                  ARRAY of {
                          link_id: UUID,
                          source_artifact_id: UUID,
                          source_artifact_type: ENUM,
                          target_artifact_id: UUID,
                          target_artifact_type: ENUM,
                          link_type: ENUM [derives_from, constrains, evidences,
                                           implements, conflicts_with, supersedes,
                                           translates (cross-role mapping)],
                          link_rationale: TEXT,
                          strength: ENUM [strong, moderate, weak, hypothesized],
                          created_by: agent_id,
                          created_at: TIMESTAMP,
                          last_validated: TIMESTAMP,
                          validation_method: TEXT
                        }
orphan_alerts:          ARRAY of {
                          artifact_id: UUID,
                          alert_type: ENUM [no_incoming_links, no_outgoing_links,
                                            broken_link, stale_link],
                          detected_at: TIMESTAMP,
                          resolved: BOOLEAN
                        }
```

The `translates` link type is specific to cross-role boundaries: it records when one artifact is a translation of another (clinical note → coded entry, clinical language → plain language, EHR data → handoff summary). This makes the translation visible and auditable.

#### 6. Coherence Evidence

```
evidence_entries:       ARRAY of {
                          entry_id: UUID,
                          evidence_type: ENUM [outcome, process, agreement, drift],
                          measurement: TEXT,
                          value: VARIES (numeric, boolean, categorical),
                          measured_at: TIMESTAMP,
                          measured_by: agent_id,
                          linked_purpose: artifact_id,
                          linked_reification: reification_id (if applicable)
                        }
coherence_score:        {
                          overall: FLOAT (0.0-1.0),
                          dimensions: {
                            purpose_alignment: FLOAT,
                            boundary_compliance: FLOAT,
                            rationale_validity: FLOAT,
                            agent_agreement: FLOAT,
                            temporal_stability: FLOAT
                          },
                          last_calculated: TIMESTAMP,
                          trend: ENUM [improving, stable, degrading, volatile]
                        }
```

---

## VERSIONING

Every artifact modification creates a new version. The system stores full snapshots, not diffs, for auditability. Each version record includes:

- Who changed it (agent_id)
- When (timestamp)
- What changed (field-level diff, auto-generated)
- Why (change description, required for human agents; auto-generated for AI agents)
- Previous state (full snapshot)

Version history is immutable. Deprecated artifacts are never deleted; they are archived with their full history.

**Rationale:** This directly addresses knowledge vaporization. When a clinician leaves, their decisions remain traceable: what they decided, why, what alternatives they considered, what assumptions they made, and when they made them.

---

## TRACEABILITY INTEGRITY

The Artifact Manager runs continuous integrity checks:

1. **Orphan detection:** Artifacts without incoming or outgoing links are flagged. In a healthy constellation, every artifact connects to at least one other.

2. **Broken link detection:** When an artifact is deprecated or archived, all links pointing to it are flagged for review. The linked artifacts may need updating.

3. **Staleness detection:** Links that have not been validated within a configurable window (default: one cycle period) are flagged as potentially stale.

4. **Circular dependency detection:** Link chains are checked for cycles that could indicate confused traceability.

5. **Cross-role translation completeness:** When a Purpose Articulation exists for a patient, the system checks whether all expected translation artifacts exist (e.g., clinical note exists but coded entry does not → gap signal).

---

## ARTIFACT LIFECYCLE

```
DRAFT ──────→ ACTIVE ──────→ UNDER REVIEW ──────→ ACTIVE (updated)
                │                                         │
                │                                         └──→ DEPRECATED
                │                                                   │
                └──→ DEPRECATED ──────→ ARCHIVED                    └──→ ARCHIVED
```

**State transitions:**

| From | To | Trigger | Required |
|------|----|---------|----------|
| Draft | Active | Agent approval | Completeness check passes; all required fields populated |
| Active | Under Review | Coherence signal (tension), scheduled review, manual trigger | Reason recorded |
| Under Review | Active | Review complete, updates applied | Review outcome recorded |
| Under Review | Deprecated | Artifact no longer valid | Deprecation reason and replacement reference (if any) |
| Active | Deprecated | Direct deprecation (emergency) | Rationale required; escalation to governance |
| Deprecated | Archived | Retention period elapsed | Automatic; full history preserved |

---

## ONTOLOGY MANAGEMENT (P2)

The Artifact Engine includes a terminology alignment module that operationalizes P2 (Emergent Ontology).

**How it works:**
1. When agents create artifacts, they use their own vocabulary.
2. The system detects when different terms refer to the same concept (e.g., "HTN" and "hypertension" and "I10").
3. Proposed mappings are surfaced to agents for confirmation.
4. Confirmed mappings become part of the shared ontology.
5. Mappings are tested against outcomes: if a mapping consistently correlates with coding errors or misinterpretation, it is flagged for review.
6. The ontology grows through use, not through top-down imposition.

**Storage:** Ontology mappings are themselves artifacts (type: traceability, link_type: translates) and follow the same versioning and lifecycle rules.

---

## SCOPE HIERARCHY

Artifacts exist at different granularity levels. The scope hierarchy determines visibility and inheritance:

```
INSTITUTION
  └── DEPARTMENT
        └── PROTOCOL (condition-specific or procedure-specific)
              └── PATIENT
                    └── ENCOUNTER
```

**Inheritance rule:** Artifacts at a higher scope apply to all lower scopes unless explicitly overridden. An institutional boundary condition (e.g., HIPAA requirements) applies to every patient encounter. A protocol-level purpose articulation applies to every patient under that protocol. Overrides are tracked as explicit link entries with rationale.

---

*The Artifact Engine is the tool's memory. Without it, the cycle has nothing to observe, nothing to compare against, and nothing to renegotiate. With it, every decision persists, every translation is visible, and every failure has a lineage.*

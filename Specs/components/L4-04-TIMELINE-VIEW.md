# L4-04 — Timeline View

<!--
=============================================================================
COMPONENT SPEC — Chronological visualization of the patient's medical journey.
Engineer review: E-UX (UI/UX, lead), E-RS (Rust), E-DA (Data), E-QA (QA)
Marie opens this to SEE her medical story. Every medication change, every lab,
every symptom — laid out on a calm, scrollable timeline. Left to right,
oldest to newest. No alarms — just clarity.
=============================================================================
-->

## Table of Contents

| Section | Offset |
|---------|--------|
| [1] Identity | `offset=22 limit=35` |
| [2] Dependencies | `offset=57 limit=18` |
| [3] Interfaces | `offset=75 limit=120` |
| [4] Timeline Data Assembly | `offset=195 limit=80` |
| [5] SVG Rendering Approach | `offset=275 limit=75` |
| [6] Zoom Levels | `offset=350 limit=65` |
| [7] Event Rendering | `offset=415 limit=60` |
| [8] Correlation Lines | `offset=475 limit=65` |
| [9] Filter System | `offset=540 limit=55` |
| [10] Since Last Visit Mode | `offset=595 limit=55` |
| [11] Detail Popup | `offset=650 limit=60` |
| [12] Tauri Commands (IPC) | `offset=710 limit=75` |
| [13] Svelte Components | `offset=785 limit=250` |
| [14] Frontend API | `offset=1035 limit=30` |
| [15] Error Handling | `offset=1065 limit=25` |
| [16] Security | `offset=1090 limit=20` |
| [17] Testing | `offset=1110 limit=60` |
| [18] Performance | `offset=1170 limit=30` |
| [19] Open Questions | `offset=1200 limit=15` |

---

## [1] Identity

**What:** A chronological SVG-based visualization of the patient's entire medical journey. Renders events from ALL entity tables (medications, lab results, symptoms, procedures, appointments, documents, diagnoses) on a horizontal scrollable timeline. Supports zoom levels (day/week/month/year), filtering by event type and professional, correlation lines between related events (symptom onset near medication change), a "since last visit" highlight mode, and tap-to-detail cards for each event.

**After this session:**
- Timeline screen accessible from "More" tab bar menu and from appointment prep (L4-02)
- Horizontal SVG timeline renders all medical events left-to-right, oldest to newest
- Color-coded event markers: medications (blue), lab results (green), symptoms (orange), procedures (purple), appointments (teal), documents (gray), diagnoses (pink)
- Four zoom levels: day, week, month, year — with smooth transitions
- Filter bar: toggle event types on/off, filter by professional, date range picker
- "Since last visit" mode: select an appointment, highlight all events since that date
- Correlation lines drawn between symptom onset and temporally adjacent medication changes
- Tap any event marker to see a detail popup with full information and navigation to source
- Empty state for new profiles with no events
- Performant rendering for profiles with hundreds of events (virtualized viewport)
- All data fetched in a single Tauri command, assembled from all entity tables server-side

**Estimated complexity:** High
**Source:** Tech Spec v1.1 Section 9.4 (Screen Map — Timeline), Component Index [CI-08]

---

## [2] Dependencies

**Incoming:**
- L0-02 (data model — all entity tables: documents, medications, lab_results, symptoms, procedures, appointments, diagnoses, dose_changes, professionals)
- L0-03 (encryption — ProfileSession for decrypting entity fields)
- L3-01 (profile management — ProfileGuard ensures active session)

**Outgoing:**
- L4-02 (appointment prep — provides "since last visit" mode data; appointment prep links to timeline with a pre-selected appointment)

**No new Cargo.toml dependencies.** Uses existing repository traits, Tauri state, and SVG rendering in the frontend.

---

## [3] Interfaces

### Backend Types

```rust
// src-tauri/src/timeline.rs

use chrono::{NaiveDate, NaiveDateTime};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A single event on the timeline — unified across all entity tables
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineEvent {
    pub id: Uuid,
    pub event_type: EventType,
    pub date: NaiveDate,                     // Canonical date for timeline positioning
    pub title: String,                       // Short display label
    pub subtitle: Option<String>,            // Secondary info (dose, value, etc.)
    pub professional_id: Option<Uuid>,
    pub professional_name: Option<String>,
    pub document_id: Option<Uuid>,           // Source document for navigation
    pub severity: Option<EventSeverity>,     // For symptoms, lab abnormals
    pub metadata: EventMetadata,             // Type-specific extra data
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum EventType {
    MedicationStart,
    MedicationStop,
    MedicationDoseChange,
    LabResult,
    Symptom,
    Procedure,
    Appointment,
    Document,
    Diagnosis,
}

impl EventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::MedicationStart => "medication_start",
            Self::MedicationStop => "medication_stop",
            Self::MedicationDoseChange => "medication_dose_change",
            Self::LabResult => "lab_result",
            Self::Symptom => "symptom",
            Self::Procedure => "procedure",
            Self::Appointment => "appointment",
            Self::Document => "document",
            Self::Diagnosis => "diagnosis",
        }
    }

    /// Color category for frontend rendering
    pub fn color_group(&self) -> &'static str {
        match self {
            Self::MedicationStart
            | Self::MedicationStop
            | Self::MedicationDoseChange => "medication",
            Self::LabResult => "lab",
            Self::Symptom => "symptom",
            Self::Procedure => "procedure",
            Self::Appointment => "appointment",
            Self::Document => "document",
            Self::Diagnosis => "diagnosis",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventSeverity {
    Normal,
    Low,         // Lab low, symptom severity 1-2
    Moderate,    // Lab abnormal, symptom severity 3
    High,        // Lab high, symptom severity 4
    Critical,    // Lab critical, symptom severity 5
}

/// Type-specific metadata carried by each event
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum EventMetadata {
    Medication {
        generic_name: String,
        brand_name: Option<String>,
        dose: String,
        frequency: String,
        status: String,
        reason: Option<String>,          // reason_start or reason_stop
    },
    DoseChange {
        generic_name: String,
        old_dose: Option<String>,
        new_dose: String,
        old_frequency: Option<String>,
        new_frequency: Option<String>,
        reason: Option<String>,
    },
    Lab {
        test_name: String,
        value: Option<f64>,
        value_text: Option<String>,
        unit: Option<String>,
        reference_low: Option<f64>,
        reference_high: Option<f64>,
        abnormal_flag: String,
    },
    Symptom {
        category: String,
        specific: String,
        severity: u8,                    // 1-5
        body_region: Option<String>,
        still_active: bool,
    },
    Procedure {
        name: String,
        facility: Option<String>,
        outcome: Option<String>,
        follow_up_required: bool,
    },
    Appointment {
        appointment_type: String,        // "upcoming" or "completed"
        professional_specialty: Option<String>,
    },
    Document {
        document_type: String,
        verified: bool,
    },
    Diagnosis {
        name: String,
        icd_code: Option<String>,
        status: String,
    },
}

/// A correlation between two timeline events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineCorrelation {
    pub source_id: Uuid,                 // e.g., symptom
    pub target_id: Uuid,                 // e.g., medication change
    pub correlation_type: CorrelationType,
    pub description: String,             // "Headache appeared 3 days after dose increase"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CorrelationType {
    SymptomAfterMedicationChange,        // Symptom onset within N days of med change
    SymptomAfterMedicationStart,         // Symptom onset within N days of med start
    SymptomResolvedAfterMedicationStop,  // Symptom resolved near med stop
    LabAfterMedicationChange,            // Lab abnormal near dose change
    ExplicitLink,                        // User-created via symptom.related_medication_id
}

/// Filter parameters sent from frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineFilter {
    pub event_types: Option<Vec<EventType>>,    // None = all types
    pub professional_id: Option<Uuid>,
    pub date_from: Option<NaiveDate>,
    pub date_to: Option<NaiveDate>,
    pub since_appointment_id: Option<Uuid>,     // "Since last visit" mode
}

/// Zoom level — determines time scale rendering
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ZoomLevel {
    Day,
    Week,
    Month,
    Year,
}

/// Complete timeline data — single response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineData {
    pub events: Vec<TimelineEvent>,
    pub correlations: Vec<TimelineCorrelation>,
    pub date_range: DateRange,                   // Earliest to latest event
    pub event_counts: EventCounts,               // Per-type counts for filter badges
    pub professionals: Vec<ProfessionalSummary>, // For professional filter dropdown
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DateRange {
    pub earliest: Option<NaiveDate>,
    pub latest: Option<NaiveDate>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventCounts {
    pub medications: u32,
    pub lab_results: u32,
    pub symptoms: u32,
    pub procedures: u32,
    pub appointments: u32,
    pub documents: u32,
    pub diagnoses: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfessionalSummary {
    pub id: Uuid,
    pub name: String,
    pub specialty: Option<String>,
    pub event_count: u32,
}
```

### Frontend Types

```typescript
// src/lib/types/timeline.ts

export type EventType =
  | 'MedicationStart'
  | 'MedicationStop'
  | 'MedicationDoseChange'
  | 'LabResult'
  | 'Symptom'
  | 'Procedure'
  | 'Appointment'
  | 'Document'
  | 'Diagnosis';

export type EventSeverity = 'Normal' | 'Low' | 'Moderate' | 'High' | 'Critical';

export type ZoomLevel = 'Day' | 'Week' | 'Month' | 'Year';

export interface TimelineEvent {
  id: string;
  event_type: EventType;
  date: string;                          // ISO 8601 date YYYY-MM-DD
  title: string;
  subtitle: string | null;
  professional_id: string | null;
  professional_name: string | null;
  document_id: string | null;
  severity: EventSeverity | null;
  metadata: EventMetadata;
}

export type EventMetadata =
  | { kind: 'Medication'; generic_name: string; brand_name: string | null; dose: string; frequency: string; status: string; reason: string | null }
  | { kind: 'DoseChange'; generic_name: string; old_dose: string | null; new_dose: string; old_frequency: string | null; new_frequency: string | null; reason: string | null }
  | { kind: 'Lab'; test_name: string; value: number | null; value_text: string | null; unit: string | null; reference_low: number | null; reference_high: number | null; abnormal_flag: string }
  | { kind: 'Symptom'; category: string; specific: string; severity: number; body_region: string | null; still_active: boolean }
  | { kind: 'Procedure'; name: string; facility: string | null; outcome: string | null; follow_up_required: boolean }
  | { kind: 'Appointment'; appointment_type: string; professional_specialty: string | null }
  | { kind: 'Document'; document_type: string; verified: boolean }
  | { kind: 'Diagnosis'; name: string; icd_code: string | null; status: string };

export interface TimelineCorrelation {
  source_id: string;
  target_id: string;
  correlation_type: string;
  description: string;
}

export interface TimelineFilter {
  event_types: EventType[] | null;
  professional_id: string | null;
  date_from: string | null;
  date_to: string | null;
  since_appointment_id: string | null;
}

export interface TimelineData {
  events: TimelineEvent[];
  correlations: TimelineCorrelation[];
  date_range: DateRange;
  event_counts: EventCounts;
  professionals: ProfessionalSummary[];
}

export interface DateRange {
  earliest: string | null;
  latest: string | null;
}

export interface EventCounts {
  medications: number;
  lab_results: number;
  symptoms: number;
  procedures: number;
  appointments: number;
  documents: number;
  diagnoses: number;
}

export interface ProfessionalSummary {
  id: string;
  name: string;
  specialty: string | null;
  event_count: number;
}

/** Color palette for event types — soft pastels per design language */
export const EVENT_COLORS: Record<string, { fill: string; stroke: string; label: string }> = {
  medication:  { fill: '#DBEAFE', stroke: '#3B82F6', label: 'Medications' },
  lab:         { fill: '#DCFCE7', stroke: '#22C55E', label: 'Lab Results' },
  symptom:     { fill: '#FFF7ED', stroke: '#F97316', label: 'Symptoms' },
  procedure:   { fill: '#F3E8FF', stroke: '#A855F7', label: 'Procedures' },
  appointment: { fill: '#CCFBF1', stroke: '#14B8A6', label: 'Appointments' },
  document:    { fill: '#F5F5F4', stroke: '#A8A29E', label: 'Documents' },
  diagnosis:   { fill: '#FCE7F3', stroke: '#EC4899', label: 'Diagnoses' },
};

/** Maps EventType to color group key */
export function eventColorGroup(eventType: EventType): string {
  switch (eventType) {
    case 'MedicationStart':
    case 'MedicationStop':
    case 'MedicationDoseChange':
      return 'medication';
    case 'LabResult': return 'lab';
    case 'Symptom': return 'symptom';
    case 'Procedure': return 'procedure';
    case 'Appointment': return 'appointment';
    case 'Document': return 'document';
    case 'Diagnosis': return 'diagnosis';
  }
}
```

---

## [4] Timeline Data Assembly

**E-DA + E-RS:** All events assembled server-side into a flat `Vec<TimelineEvent>`, sorted by date ascending. This avoids N+1 queries from the frontend and keeps the IPC call count to 1.

### Assembly Strategy

```rust
// src-tauri/src/timeline.rs

/// Assembles timeline events from ALL entity tables.
/// Each table query maps rows into TimelineEvent with the correct EventType.
/// Results merged, sorted by date, and returned as one payload.
pub fn assemble_timeline_events(
    conn: &rusqlite::Connection,
    filter: &TimelineFilter,
) -> Result<Vec<TimelineEvent>, CohearaError> {
    let mut events: Vec<TimelineEvent> = Vec::new();

    // Date boundaries for queries
    let (date_from, date_to) = resolve_date_bounds(conn, filter)?;

    // 1. Medications — start events
    events.extend(fetch_medication_starts(conn, &date_from, &date_to, filter)?);

    // 2. Medications — stop events (only stopped medications with end_date)
    events.extend(fetch_medication_stops(conn, &date_from, &date_to, filter)?);

    // 3. Dose changes
    events.extend(fetch_dose_changes(conn, &date_from, &date_to, filter)?);

    // 4. Lab results
    events.extend(fetch_lab_events(conn, &date_from, &date_to, filter)?);

    // 5. Symptoms
    events.extend(fetch_symptom_events(conn, &date_from, &date_to, filter)?);

    // 6. Procedures
    events.extend(fetch_procedure_events(conn, &date_from, &date_to, filter)?);

    // 7. Appointments
    events.extend(fetch_appointment_events(conn, &date_from, &date_to, filter)?);

    // 8. Documents (ingestion events)
    events.extend(fetch_document_events(conn, &date_from, &date_to, filter)?);

    // 9. Diagnoses
    events.extend(fetch_diagnosis_events(conn, &date_from, &date_to, filter)?);

    // Apply event_type filter if specified
    if let Some(ref types) = filter.event_types {
        events.retain(|e| types.contains(&e.event_type));
    }

    // Apply professional filter if specified
    if let Some(ref prof_id) = filter.professional_id {
        events.retain(|e| e.professional_id.as_ref() == Some(prof_id));
    }

    // Sort chronologically (oldest first = left of timeline)
    events.sort_by(|a, b| a.date.cmp(&b.date));

    Ok(events)
}

/// Resolves effective date bounds.
/// If "since last visit" mode, date_from = appointment date.
fn resolve_date_bounds(
    conn: &rusqlite::Connection,
    filter: &TimelineFilter,
) -> Result<(Option<NaiveDate>, Option<NaiveDate>), CohearaError> {
    let date_from = if let Some(ref appt_id) = filter.since_appointment_id {
        let appt_date: String = conn.query_row(
            "SELECT date FROM appointments WHERE id = ?1",
            params![appt_id.to_string()],
            |row| row.get(0),
        ).map_err(|_| CohearaError::NotFound {
            entity: "appointment".into(),
            id: appt_id.to_string(),
        })?;
        Some(NaiveDate::parse_from_str(&appt_date, "%Y-%m-%d")
            .map_err(|e| CohearaError::ParseError(e.to_string()))?)
    } else {
        filter.date_from
    };

    Ok((date_from, filter.date_to))
}
```

### Per-Table Fetch Functions (Pattern)

Each table follows the same pattern. Shown here for medications:

```rust
fn fetch_medication_starts(
    conn: &rusqlite::Connection,
    date_from: &Option<NaiveDate>,
    date_to: &Option<NaiveDate>,
    filter: &TimelineFilter,
) -> Result<Vec<TimelineEvent>, CohearaError> {
    let mut sql = String::from(
        "SELECT m.id, m.generic_name, m.brand_name, m.dose, m.frequency,
                m.status, m.start_date, m.reason_start,
                m.prescriber_id, p.name AS prof_name, m.document_id
         FROM medications m
         LEFT JOIN professionals p ON m.prescriber_id = p.id
         WHERE m.start_date IS NOT NULL"
    );
    let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(ref d) = date_from {
        sql.push_str(&format!(" AND m.start_date >= ?{}", params_vec.len() + 1));
        params_vec.push(Box::new(d.format("%Y-%m-%d").to_string()));
    }
    if let Some(ref d) = date_to {
        sql.push_str(&format!(" AND m.start_date <= ?{}", params_vec.len() + 1));
        params_vec.push(Box::new(d.format("%Y-%m-%d").to_string()));
    }

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(params_vec.iter()), |row| {
        let date_str: String = row.get("start_date")?;
        Ok(TimelineEvent {
            id: Uuid::parse_str(&row.get::<_, String>("id")?).unwrap(),
            event_type: EventType::MedicationStart,
            date: NaiveDate::parse_from_str(&date_str, "%Y-%m-%d").unwrap(),
            title: format!("Started {}", row.get::<_, String>("generic_name")?),
            subtitle: Some(row.get::<_, String>("dose")?),
            professional_id: row.get::<_, Option<String>>("prescriber_id")?
                .and_then(|s| Uuid::parse_str(&s).ok()),
            professional_name: row.get("prof_name")?,
            document_id: row.get::<_, Option<String>>("document_id")?
                .and_then(|s| Uuid::parse_str(&s).ok()),
            severity: None,
            metadata: EventMetadata::Medication {
                generic_name: row.get("generic_name")?,
                brand_name: row.get("brand_name")?,
                dose: row.get("dose")?,
                frequency: row.get("frequency")?,
                status: row.get("status")?,
                reason: row.get("reason_start")?,
            },
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(CohearaError::from)
}
```

**Same pattern for all 9 fetch functions.** Each maps table-specific columns into the shared `TimelineEvent` struct.

### Correlation Detection

```rust
/// Detects temporal correlations between events.
/// Runs AFTER events are assembled. O(n*m) where n=symptoms, m=medication events.
/// Acceptable because total event count is bounded (hundreds, not thousands).
pub fn detect_correlations(
    events: &[TimelineEvent],
) -> Vec<TimelineCorrelation> {
    let correlation_window_days: i64 = 14; // 2-week window

    let symptoms: Vec<&TimelineEvent> = events.iter()
        .filter(|e| e.event_type == EventType::Symptom)
        .collect();

    let med_events: Vec<&TimelineEvent> = events.iter()
        .filter(|e| matches!(
            e.event_type,
            EventType::MedicationStart
            | EventType::MedicationStop
            | EventType::MedicationDoseChange
        ))
        .collect();

    let mut correlations = Vec::new();

    for symptom in &symptoms {
        for med in &med_events {
            let days_diff = (symptom.date - med.date).num_days();

            // Symptom appeared AFTER medication event (within window)
            if days_diff >= 0 && days_diff <= correlation_window_days {
                let corr_type = match med.event_type {
                    EventType::MedicationStart => CorrelationType::SymptomAfterMedicationStart,
                    EventType::MedicationDoseChange => CorrelationType::SymptomAfterMedicationChange,
                    _ => continue,
                };

                correlations.push(TimelineCorrelation {
                    source_id: symptom.id,
                    target_id: med.id,
                    correlation_type: corr_type,
                    description: format!(
                        "{} appeared {} day(s) after {}",
                        symptom.title,
                        days_diff,
                        med.title,
                    ),
                });
            }
        }
    }

    // Explicit links from symptoms table (related_medication_id)
    for symptom in &symptoms {
        if let EventMetadata::Symptom { .. } = &symptom.metadata {
            // Check if this symptom has related_medication_id set
            // The explicit link is encoded by matching symptom → med start event
            for med in &med_events {
                if med.event_type == EventType::MedicationStart {
                    // Explicit links detected via related_medication_id in the DB query
                    // (added during fetch_symptom_events as a field on the event)
                    // This is a fallback — explicit links always included regardless of window
                }
            }
        }
    }

    correlations
}
```

### Explicit Links from Database

```rust
/// Fetches explicit symptom↔medication links stored in symptoms.related_medication_id
fn fetch_explicit_correlations(
    conn: &rusqlite::Connection,
) -> Result<Vec<TimelineCorrelation>, CohearaError> {
    let mut stmt = conn.prepare(
        "SELECT s.id AS symptom_id, s.specific, s.onset_date,
                m.id AS med_id, m.generic_name, m.start_date
         FROM symptoms s
         JOIN medications m ON s.related_medication_id = m.id
         WHERE s.related_medication_id IS NOT NULL"
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(TimelineCorrelation {
            source_id: Uuid::parse_str(&row.get::<_, String>("symptom_id")?).unwrap(),
            target_id: Uuid::parse_str(&row.get::<_, String>("med_id")?).unwrap(),
            correlation_type: CorrelationType::ExplicitLink,
            description: format!(
                "{} linked to {}",
                row.get::<_, String>("specific")?,
                row.get::<_, String>("generic_name")?,
            ),
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(CohearaError::from)
}
```

---

## [5] SVG Rendering Approach

**E-UX:** The timeline is a horizontally scrollable SVG drawn entirely in the browser. No canvas, no WebGL — SVG allows accessibility (aria attributes on elements) and CSS transitions for zoom.

### Viewport Model

```
┌──────────────────────────────────────────────────────────┐
│  VISIBLE VIEWPORT (screen width)                         │
│  ┌────────────────────────────────────────────────────┐  │
│  │  ← scroll →                                        │  │
│  │                                                    │  │
│  │  FULL SVG CANVAS (total_width based on date range) │  │
│  │                                                    │  │
│  │  [event] [event]    [event] [event]  [event]       │  │
│  │     │                  │                            │  │
│  │     └──correlation line┘                            │  │
│  │                                                    │  │
│  └────────────────────────────────────────────────────┘  │
│                                                          │
│  FILTER BAR (fixed, above SVG)                           │
│  ZOOM CONTROLS (fixed, bottom-right)                     │
└──────────────────────────────────────────────────────────┘
```

### Coordinate System

```typescript
/** Timeline coordinate system.
 *  X axis = time (left = oldest, right = newest)
 *  Y axis = event lanes (stacked by type to avoid overlap)
 */

/** Pixels per time unit at each zoom level */
export const SCALE: Record<ZoomLevel, number> = {
  Day:   120,   // 120px per day
  Week:  60,    // ~8.6px per day (60px per week)
  Month: 20,    // ~0.66px per day (20px per month)
  Year:  4,     // ~0.33px per day (4px per year ≈ 1.1px per month)
};

/** Vertical lane heights */
export const LANE_HEIGHT = 48;          // px per event lane
export const LANE_GAP = 8;             // px between lanes
export const HEADER_HEIGHT = 40;       // px for date axis labels
export const MARKER_RADIUS = 8;        // px — event circle radius
export const TOUCH_TARGET_RADIUS = 22; // px — invisible hit area (meets 44px target)
export const PADDING_X = 40;           // px left/right padding
export const PADDING_Y = 16;           // px top/bottom padding

/** Calculate total SVG width from date range and zoom */
export function calculateCanvasWidth(
  earliest: Date,
  latest: Date,
  zoom: ZoomLevel,
): number {
  const days = Math.ceil((latest.getTime() - earliest.getTime()) / (1000 * 60 * 60 * 24));
  switch (zoom) {
    case 'Day':   return days * SCALE.Day + PADDING_X * 2;
    case 'Week':  return Math.ceil(days / 7) * SCALE.Week + PADDING_X * 2;
    case 'Month': return Math.ceil(days / 30) * SCALE.Month + PADDING_X * 2;
    case 'Year':  return Math.ceil(days / 365) * SCALE.Year + PADDING_X * 2;
  }
}

/** Convert a date to X position on the SVG canvas */
export function dateToX(
  date: Date,
  earliest: Date,
  zoom: ZoomLevel,
): number {
  const days = (date.getTime() - earliest.getTime()) / (1000 * 60 * 60 * 24);
  switch (zoom) {
    case 'Day':   return PADDING_X + days * SCALE.Day;
    case 'Week':  return PADDING_X + (days / 7) * SCALE.Week;
    case 'Month': return PADDING_X + (days / 30) * SCALE.Month;
    case 'Year':  return PADDING_X + (days / 365) * SCALE.Year;
  }
}
```

### Lane Assignment

Events on the same date are stacked vertically by type to avoid overlap. Each event type has a dedicated Y band (lane).

```typescript
/** Event type → lane index mapping (top to bottom) */
const LANE_ORDER: Record<string, number> = {
  appointment: 0,
  medication:  1,
  diagnosis:   2,
  lab:         3,
  symptom:     4,
  procedure:   5,
  document:    6,
};

/** Calculate Y position for an event */
export function eventToY(eventType: EventType): number {
  const group = eventColorGroup(eventType);
  const laneIndex = LANE_ORDER[group] ?? 6;
  return HEADER_HEIGHT + PADDING_Y + laneIndex * (LANE_HEIGHT + LANE_GAP) + LANE_HEIGHT / 2;
}

/** Total SVG height (all lanes + header + padding) */
export const CANVAS_HEIGHT =
  HEADER_HEIGHT
  + PADDING_Y * 2
  + Object.keys(LANE_ORDER).length * (LANE_HEIGHT + LANE_GAP);
```

---

## [6] Zoom Levels

### Zoom Level Definitions

| Level | Time Unit | Pixels Per Unit | Date Axis Labels | Typical Use |
|-------|-----------|----------------|------------------|-------------|
| Day | 1 day | 120px | "Mon 15", "Tue 16" | Last 2 weeks, detailed |
| Week | 1 week | 60px | "Jan 13-19", "Jan 20-26" | Last 2-3 months |
| Month | 1 month | 20px | "Jan", "Feb", "Mar" | 6 months to 1 year |
| Year | 1 year | 4px | "2024", "2025", "2026" | Full multi-year history |

### Auto-Zoom Selection

On initial load, the zoom level is chosen based on the total date range:

```typescript
/** Select the best initial zoom level based on total date range */
export function autoSelectZoom(earliest: Date, latest: Date): ZoomLevel {
  const days = Math.ceil((latest.getTime() - earliest.getTime()) / (1000 * 60 * 60 * 24));
  if (days <= 30) return 'Day';
  if (days <= 180) return 'Week';
  if (days <= 730) return 'Month';   // ~2 years
  return 'Year';
}
```

### Date Axis Tick Generation

```typescript
/** Generate date axis tick marks for a zoom level */
export function generateTicks(
  earliest: Date,
  latest: Date,
  zoom: ZoomLevel,
): Array<{ date: Date; label: string; x: number }> {
  const ticks: Array<{ date: Date; label: string; x: number }> = [];
  const current = new Date(earliest);

  switch (zoom) {
    case 'Day':
      while (current <= latest) {
        ticks.push({
          date: new Date(current),
          label: current.toLocaleDateString('en-US', { weekday: 'short', day: 'numeric' }),
          x: dateToX(current, earliest, zoom),
        });
        current.setDate(current.getDate() + 1);
      }
      break;

    case 'Week':
      // Align to Monday
      current.setDate(current.getDate() - current.getDay() + 1);
      while (current <= latest) {
        const weekEnd = new Date(current);
        weekEnd.setDate(weekEnd.getDate() + 6);
        ticks.push({
          date: new Date(current),
          label: `${current.toLocaleDateString('en-US', { month: 'short', day: 'numeric' })}`,
          x: dateToX(current, earliest, zoom),
        });
        current.setDate(current.getDate() + 7);
      }
      break;

    case 'Month':
      current.setDate(1);
      while (current <= latest) {
        ticks.push({
          date: new Date(current),
          label: current.toLocaleDateString('en-US', { month: 'short' }),
          x: dateToX(current, earliest, zoom),
        });
        current.setMonth(current.getMonth() + 1);
      }
      break;

    case 'Year':
      current.setMonth(0, 1);
      while (current <= latest) {
        ticks.push({
          date: new Date(current),
          label: current.getFullYear().toString(),
          x: dateToX(current, earliest, zoom),
        });
        current.setFullYear(current.getFullYear() + 1);
      }
      break;
  }

  return ticks;
}
```

### Zoom Transition

When the user changes zoom level:
1. Capture the current center date (date at viewport midpoint)
2. Recalculate SVG width for new zoom
3. Scroll to position the same center date at viewport midpoint
4. Apply CSS `transition: transform 200ms ease-out` for smooth visual

---

## [7] Event Rendering

### Event Markers

Each event renders as a colored circle on the SVG. The color is determined by event type. The circle has:
- A visible filled circle (radius = `MARKER_RADIUS` = 8px) with pastel fill and colored stroke
- An invisible hit-area circle (radius = `TOUCH_TARGET_RADIUS` = 22px) for touch/click — meets 44px minimum

```svg
<!-- Example: A medication start event -->
<g role="button" tabindex="0" aria-label="Started Metformin on Jan 15, 2026">
  <!-- Invisible touch target -->
  <circle cx="240" cy="104" r="22" fill="transparent" class="cursor-pointer" />
  <!-- Visible marker -->
  <circle cx="240" cy="104" r="8" fill="#DBEAFE" stroke="#3B82F6" stroke-width="2" />
  <!-- Label (shown at Day/Week zoom, hidden at Month/Year) -->
  <text x="240" y="136" text-anchor="middle" class="text-xs fill-stone-600">Metformin</text>
</g>
```

### Label Visibility Rules

| Zoom Level | Labels Visible | Collision Strategy |
|------------|---------------|-------------------|
| Day | All labels shown | Stack vertically if overlapping |
| Week | Labels shown (truncated if needed) | Hide if within 30px of neighbor |
| Month | No labels — tooltip on hover only | N/A |
| Year | No labels — tooltip on hover only | N/A |

### Severity Indicators

Events with severity information get a visual accent:

| Severity | Visual Treatment |
|----------|-----------------|
| Normal | Standard pastel fill, regular stroke |
| Low | Standard pastel fill, slightly thicker stroke (2.5px) |
| Moderate | Slightly saturated fill, thick stroke (3px) |
| High | Saturated fill, thick stroke (3px), subtle pulse animation |
| Critical | Filled with amber warning color, thick stroke, aria-live="polite" |

### Special Event Markers

| Event Type | Marker Shape | Extra Visual |
|------------|-------------|-------------|
| MedicationStart | Circle with small "+" inside | Green plus accent |
| MedicationStop | Circle with small "x" inside | Red x accent |
| MedicationDoseChange | Circle with small delta inside | Blue delta accent |
| Appointment | Circle with calendar icon inside | Teal border |
| All others | Plain circle | Standard by color group |

### Lane Labels

A fixed left column (outside the scrolling SVG) shows lane names:

```
Appointments   ─────────────────────
Medications    ─────────────────────
Diagnoses      ─────────────────────
Lab Results    ─────────────────────
Symptoms       ─────────────────────
Procedures     ─────────────────────
Documents      ─────────────────────
```

These labels remain visible as the timeline scrolls horizontally.

---

## [8] Correlation Lines

### Drawing Correlations

Correlation lines connect two event markers with a curved SVG path. The line indicates a temporal relationship between events (e.g., symptom appeared after medication change).

```typescript
/** Generate SVG path for a correlation line between two events */
export function correlationPath(
  sourceX: number,
  sourceY: number,
  targetX: number,
  targetY: number,
): string {
  // Bezier curve: gentle arc connecting source → target
  const midX = (sourceX + targetX) / 2;
  const controlY = Math.min(sourceY, targetY) - 30; // Arc above both points

  return `M ${sourceX} ${sourceY} Q ${midX} ${controlY} ${targetX} ${targetY}`;
}
```

### Correlation Visual Style

```css
/* Correlation lines — subtle, not distracting */
.correlation-line {
  fill: none;
  stroke: #D6D3D1;           /* stone-300 — very subtle */
  stroke-width: 1.5;
  stroke-dasharray: 4 3;     /* Dashed — distinguishes from axis lines */
  opacity: 0.6;
  pointer-events: none;      /* Don't intercept clicks */
}

.correlation-line.highlighted {
  stroke: #F97316;            /* orange-500 — when either endpoint is selected */
  stroke-width: 2;
  opacity: 1.0;
  stroke-dasharray: none;
}
```

### Correlation Interaction

When a user taps an event that has correlations:
1. The event's detail popup shows a "Related events" section
2. All correlation lines connected to this event become `.highlighted`
3. Connected events get a subtle glow ring (box-shadow via SVG filter)
4. Tapping the correlation line description in the detail popup scrolls to the connected event

### Correlation Visibility Rules

| Zoom Level | Correlation Lines Visible |
|------------|--------------------------|
| Day | Always visible |
| Week | Visible if source and target are both in viewport |
| Month | Hidden — mentioned in tooltip only |
| Year | Hidden — mentioned in tooltip only |

---

## [9] Filter System

### Filter Bar Layout

The filter bar is fixed above the scrollable SVG. It contains:

```
┌──────────────────────────────────────────────────────────────┐
│ FILTER BAR                                                    │
│                                                               │
│ [All] [Meds ●12] [Labs ●8] [Symptoms ●5] [Procedures ●2]   │
│ [Appts ●3] [Docs ●15] [Diagnoses ●4]                        │
│                                                               │
│ Professional: [All ▾]    Date: [From] — [To]                 │
│                                                               │
│ [Since last visit ▾]                                         │
└──────────────────────────────────────────────────────────────┘
```

### Filter Chips (Event Types)

Each event type is a toggle chip. Active chips have their colored background. Inactive chips are dimmed. The count badge shows how many events of that type exist (from `event_counts`).

```typescript
interface FilterChip {
  eventTypes: EventType[];    // Grouped: medication types together
  label: string;
  colorGroup: string;
  count: number;
  active: boolean;
}

const FILTER_CHIPS: FilterChip[] = [
  {
    eventTypes: ['MedicationStart', 'MedicationStop', 'MedicationDoseChange'],
    label: 'Meds',
    colorGroup: 'medication',
    count: 0,  // Filled from event_counts
    active: true,
  },
  { eventTypes: ['LabResult'], label: 'Labs', colorGroup: 'lab', count: 0, active: true },
  { eventTypes: ['Symptom'], label: 'Symptoms', colorGroup: 'symptom', count: 0, active: true },
  { eventTypes: ['Procedure'], label: 'Procedures', colorGroup: 'procedure', count: 0, active: true },
  { eventTypes: ['Appointment'], label: 'Appts', colorGroup: 'appointment', count: 0, active: true },
  { eventTypes: ['Document'], label: 'Docs', colorGroup: 'document', count: 0, active: true },
  { eventTypes: ['Diagnosis'], label: 'Diagnoses', colorGroup: 'diagnosis', count: 0, active: true },
];
```

### Professional Filter

A dropdown listing all professionals from `TimelineData.professionals`. Shows name, specialty, and event count. Default: "All professionals".

### Date Range Filter

Two date inputs (from/to). Defaults: earliest event date to today. Changing the date range triggers a re-fetch with the new filter.

### Filter Application

Filters are applied client-side when toggling event type chips (no re-fetch needed — just hide/show SVG groups). Professional and date range filters trigger a server re-fetch because they may reduce the dataset significantly.

```typescript
/** Determine which events to show based on active filters */
export function applyClientFilters(
  events: TimelineEvent[],
  activeTypes: EventType[],
): TimelineEvent[] {
  return events.filter(e => activeTypes.includes(e.event_type));
}
```

---

## [10] Since Last Visit Mode

### Purpose

"Since last visit" mode helps Marie prepare for a doctor appointment by highlighting everything that happened since her last visit with that professional. This is the bridge to L4-02 (appointment prep).

### Activation

Two entry points:
1. **From timeline filter bar:** "Since last visit" dropdown lists recent completed appointments. Selecting one activates the mode.
2. **From L4-02 appointment prep:** Navigates to timeline with `since_appointment_id` pre-set.

### Visual Treatment

When "since last visit" is active:

```
BEFORE appointment date:
  - Events rendered at 30% opacity
  - Gray overlay on SVG region before the appointment date
  - Correlation lines hidden in the "before" region

AFTER appointment date (highlighted zone):
  - Events rendered at 100% opacity with subtle glow
  - Light pastel background highlight on SVG region
  - Vertical dashed line at the appointment date with label: "Last visit — Dr. Chen — Jan 15"
  - All correlation lines visible in this zone

HEADER:
  - Banner: "Showing changes since your visit with Dr. Chen on Jan 15"
  - [Clear] button to exit mode
```

### Data Flow

```typescript
// When "since last visit" is activated:
// 1. Set filter.since_appointment_id
// 2. Re-fetch timeline data (server applies date_from = appointment date)
// 3. All returned events are post-appointment
// 4. Frontend renders the appointment date marker + highlight zone

// The server still returns events BEFORE the appointment if no date_from filter
// is set, so the frontend can show the dimmed "before" region for context.
// When since_appointment_id is set, the server sets date_from to 30 days BEFORE
// the appointment (for context) and marks the appointment date in the response.
```

### Appointment Selector Dropdown

```typescript
interface AppointmentOption {
  id: string;
  date: string;
  professional_name: string;
  professional_specialty: string | null;
}

// Populated from TimelineData.events filtered to EventType.Appointment
// with appointment_type === 'completed'
// Sorted by date descending (most recent first)
```

---

## [11] Detail Popup

### Trigger

Tap (or click, or Enter key on focused element) on any event marker opens a detail popup card anchored near the event.

### Popup Layout

```
┌──────────────────────────────────┐
│  ● Started Metformin             │  ← Title with colored dot
│  January 15, 2026                │  ← Date
│  Dr. Chen · Endocrinology        │  ← Professional
│                                  │
│  Dose: 500mg twice daily         │  ← Type-specific details
│  Reason: Blood sugar management  │
│                                  │
│  Related events (2):             │  ← Correlation section
│  · Nausea appeared 5 days later  │
│  · Blood glucose normalized      │
│                                  │
│  [View document]  [Go to source] │  ← Action buttons
└──────────────────────────────────┘
```

### Detail Content by Event Type

| Event Type | Detail Fields |
|------------|--------------|
| MedicationStart | Generic name, brand name, dose, frequency, route, prescriber, reason for starting, document link |
| MedicationStop | Generic name, dose at stop, prescriber, reason for stopping, document link |
| MedicationDoseChange | Generic name, old dose → new dose, old freq → new freq, reason, prescriber, document link |
| LabResult | Test name, value + unit, reference range, abnormal flag (with calm explanation), collection date, facility, ordering physician, document link |
| Symptom | Category, specific symptom, severity (face scale), body region, onset date, duration, character, aggravating/relieving factors, still active badge, related medication link |
| Procedure | Name, date, facility, performing professional, outcome, follow-up status, document link |
| Appointment | Professional name + specialty, date, type (completed/upcoming), pre-summary status |
| Document | Document type, date, professional, verified status, link to review screen |
| Diagnosis | Name, ICD code, date diagnosed, diagnosing professional, status (active/resolved/monitoring) |

### Popup Positioning

The popup anchors to the tapped event marker:
- If event is in the left half of viewport: popup appears to the RIGHT
- If event is in the right half of viewport: popup appears to the LEFT
- If event is near the top: popup appears BELOW
- If event is near the bottom: popup appears ABOVE
- Maximum width: 320px. Maximum height: 400px (scrollable if content exceeds).

### Popup Dismissal

- Tap outside the popup
- Press Escape key
- Tap another event (closes current, opens new)
- Scroll the timeline (closes popup)

### Navigation from Popup

| Button | Action |
|--------|--------|
| "View document" | Navigate to document detail view (L3-04 or document reader) — shown only if `document_id` is set |
| "Go to source" | Navigate to entity-specific screen (e.g., medication list, lab detail, symptom journal entry) |
| "View related" | Scroll timeline to and highlight the correlated event |

---

## [12] Tauri Commands (IPC)

```rust
// src-tauri/src/commands/timeline.rs

use tauri::State;

/// Fetches all timeline data in a single call.
/// Assembles events from all entity tables, detects correlations,
/// and returns the complete payload.
#[tauri::command]
pub async fn get_timeline_data(
    state: State<'_, AppState>,
    filter: TimelineFilter,
) -> Result<TimelineData, String> {
    let session_guard = state.active_session.lock()
        .map_err(|e| format!("Lock error: {e}"))?;
    let session = session_guard.as_ref()
        .ok_or("No active profile session")?;

    let conn = session.db_connection()?;

    // Assemble events from all tables
    let events = assemble_timeline_events(&conn, &filter)
        .map_err(|e| e.to_string())?;

    // Detect temporal correlations
    let mut correlations = detect_correlations(&events);

    // Add explicit links from symptoms.related_medication_id
    let explicit = fetch_explicit_correlations(&conn)
        .map_err(|e| e.to_string())?;
    correlations.extend(explicit);

    // Deduplicate correlations (explicit + detected may overlap)
    correlations.sort_by(|a, b| {
        (&a.source_id, &a.target_id).cmp(&(&b.source_id, &b.target_id))
    });
    correlations.dedup_by(|a, b| {
        a.source_id == b.source_id && a.target_id == b.target_id
    });

    // Compute date range
    let date_range = DateRange {
        earliest: events.first().map(|e| e.date),
        latest: events.last().map(|e| e.date),
    };

    // Compute event counts (before filtering — for filter badge counts)
    let event_counts = compute_event_counts(&conn)
        .map_err(|e| e.to_string())?;

    // Fetch professionals for filter dropdown
    let professionals = fetch_professionals_with_counts(&conn)
        .map_err(|e| e.to_string())?;

    state.update_activity();

    Ok(TimelineData {
        events,
        correlations,
        date_range,
        event_counts,
        professionals,
    })
}

/// Computes total event counts across all tables (unfiltered).
/// Used for filter badge counts.
fn compute_event_counts(
    conn: &rusqlite::Connection,
) -> Result<EventCounts, CohearaError> {
    let count = |sql: &str| -> Result<u32, CohearaError> {
        conn.query_row(sql, [], |row| row.get(0))
            .map_err(CohearaError::from)
    };

    Ok(EventCounts {
        medications: count(
            "SELECT COUNT(*) FROM medications WHERE start_date IS NOT NULL"
        )? + count(
            "SELECT COUNT(*) FROM dose_changes"
        )?,
        lab_results: count("SELECT COUNT(*) FROM lab_results")?,
        symptoms: count("SELECT COUNT(*) FROM symptoms")?,
        procedures: count("SELECT COUNT(*) FROM procedures")?,
        appointments: count("SELECT COUNT(*) FROM appointments")?,
        documents: count("SELECT COUNT(*) FROM documents")?,
        diagnoses: count("SELECT COUNT(*) FROM diagnoses")?,
    })
}

/// Fetches professionals with their event counts for the filter dropdown.
fn fetch_professionals_with_counts(
    conn: &rusqlite::Connection,
) -> Result<Vec<ProfessionalSummary>, CohearaError> {
    let mut stmt = conn.prepare(
        "SELECT p.id, p.name, p.specialty,
                (SELECT COUNT(*) FROM medications m WHERE m.prescriber_id = p.id)
                + (SELECT COUNT(*) FROM lab_results l WHERE l.ordering_physician_id = p.id)
                + (SELECT COUNT(*) FROM procedures pr WHERE pr.performing_professional_id = p.id)
                + (SELECT COUNT(*) FROM appointments a WHERE a.professional_id = p.id)
                + (SELECT COUNT(*) FROM documents d WHERE d.professional_id = p.id)
                + (SELECT COUNT(*) FROM diagnoses dg WHERE dg.diagnosing_professional_id = p.id)
                AS event_count
         FROM professionals p
         HAVING event_count > 0
         ORDER BY event_count DESC"
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(ProfessionalSummary {
            id: Uuid::parse_str(&row.get::<_, String>(0)?).unwrap(),
            name: row.get(1)?,
            specialty: row.get(2)?,
            event_count: row.get(3)?,
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(CohearaError::from)
}
```

---

## [13] Svelte Components

### TimelineScreen (Container)

```svelte
<!-- src/lib/components/timeline/TimelineScreen.svelte -->
<script lang="ts">
  import { onMount } from 'svelte';
  import { getTimelineData } from '$lib/api/timeline';
  import type {
    TimelineData, TimelineFilter, TimelineEvent,
    ZoomLevel, EventType,
  } from '$lib/types/timeline';
  import { autoSelectZoom } from '$lib/utils/timeline';
  import FilterBar from './FilterBar.svelte';
  import ZoomControls from './ZoomControls.svelte';
  import TimelineCanvas from './TimelineCanvas.svelte';
  import EventDetailPopup from './EventDetailPopup.svelte';
  import EmptyTimeline from './EmptyTimeline.svelte';

  interface Props {
    sinceAppointmentId?: string;
    onNavigate: (screen: string, params?: Record<string, string>) => void;
  }
  let { sinceAppointmentId, onNavigate }: Props = $props();

  let timelineData: TimelineData | null = $state(null);
  let loading = $state(true);
  let error: string | null = $state(null);

  let zoom: ZoomLevel = $state('Month');
  let activeTypes: EventType[] = $state([
    'MedicationStart', 'MedicationStop', 'MedicationDoseChange',
    'LabResult', 'Symptom', 'Procedure', 'Appointment', 'Document', 'Diagnosis',
  ]);
  let selectedProfessionalId: string | null = $state(null);
  let dateFrom: string | null = $state(null);
  let dateTo: string | null = $state(null);
  let sinceAppointment: string | null = $state(sinceAppointmentId ?? null);

  let selectedEvent: TimelineEvent | null = $state(null);
  let popupAnchor: { x: number; y: number } | null = $state(null);

  let filter = $derived<TimelineFilter>({
    event_types: activeTypes.length < 9 ? activeTypes : null,
    professional_id: selectedProfessionalId,
    date_from: dateFrom,
    date_to: dateTo,
    since_appointment_id: sinceAppointment,
  });

  let visibleEvents = $derived(
    timelineData
      ? timelineData.events.filter(e => activeTypes.includes(e.event_type))
      : []
  );

  let visibleCorrelations = $derived(
    timelineData
      ? timelineData.correlations.filter(c => {
          const sourceVisible = visibleEvents.some(e => e.id === c.source_id);
          const targetVisible = visibleEvents.some(e => e.id === c.target_id);
          return sourceVisible && targetVisible;
        })
      : []
  );

  async function fetchData() {
    try {
      loading = true;
      error = null;
      timelineData = await getTimelineData(filter);

      // Auto-select zoom on first load
      if (timelineData.date_range.earliest && timelineData.date_range.latest) {
        zoom = autoSelectZoom(
          new Date(timelineData.date_range.earliest),
          new Date(timelineData.date_range.latest),
        );
      }
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  function handleEventTap(event: TimelineEvent, anchor: { x: number; y: number }) {
    selectedEvent = event;
    popupAnchor = anchor;
  }

  function handleClosePopup() {
    selectedEvent = null;
    popupAnchor = null;
  }

  function handleFilterChange(types: EventType[]) {
    activeTypes = types;
  }

  async function handleProfessionalChange(profId: string | null) {
    selectedProfessionalId = profId;
    await fetchData();
  }

  async function handleDateRangeChange(from: string | null, to: string | null) {
    dateFrom = from;
    dateTo = to;
    await fetchData();
  }

  async function handleSinceVisitChange(appointmentId: string | null) {
    sinceAppointment = appointmentId;
    await fetchData();
  }

  onMount(() => {
    fetchData();
  });
</script>

<div class="flex flex-col h-full bg-stone-50">
  <!-- Header -->
  <header class="px-4 pt-4 pb-2">
    <h1 class="text-xl font-bold text-stone-800">Timeline</h1>
    <p class="text-sm text-stone-500 mt-0.5">Your medical journey</p>
  </header>

  {#if loading}
    <div class="flex items-center justify-center flex-1">
      <div class="animate-pulse text-stone-400">Loading timeline...</div>
    </div>
  {:else if error}
    <div class="px-6 py-8 text-center">
      <p class="text-red-600 mb-4">Something went wrong: {error}</p>
      <button
        class="px-6 py-3 bg-stone-200 rounded-xl text-stone-700 min-h-[44px]"
        onclick={fetchData}
      >
        Try again
      </button>
    </div>
  {:else if timelineData && timelineData.events.length === 0}
    <EmptyTimeline {onNavigate} />
  {:else if timelineData}
    <!-- Filter bar -->
    <FilterBar
      eventCounts={timelineData.event_counts}
      professionals={timelineData.professionals}
      {activeTypes}
      {selectedProfessionalId}
      {sinceAppointment}
      completedAppointments={timelineData.events.filter(
        e => e.event_type === 'Appointment' && e.metadata.kind === 'Appointment' && e.metadata.appointment_type === 'completed'
      )}
      onTypeToggle={handleFilterChange}
      onProfessionalChange={handleProfessionalChange}
      onDateRangeChange={handleDateRangeChange}
      onSinceVisitChange={handleSinceVisitChange}
    />

    <!-- Since last visit banner -->
    {#if sinceAppointment}
      {@const appt = timelineData.events.find(e => e.id === sinceAppointment)}
      {#if appt}
        <div class="mx-4 mb-2 px-4 py-2 bg-teal-50 border border-teal-200 rounded-lg
                    flex items-center justify-between">
          <span class="text-sm text-teal-800">
            Changes since {appt.professional_name ?? 'visit'} on {new Date(appt.date).toLocaleDateString()}
          </span>
          <button
            class="text-sm text-teal-600 font-medium min-h-[44px] min-w-[44px] px-2"
            onclick={() => handleSinceVisitChange(null)}
            aria-label="Clear since last visit filter"
          >
            Clear
          </button>
        </div>
      {/if}
    {/if}

    <!-- Timeline canvas -->
    <div class="flex-1 relative overflow-hidden">
      <TimelineCanvas
        events={visibleEvents}
        correlations={visibleCorrelations}
        dateRange={timelineData.date_range}
        {zoom}
        sinceDate={sinceAppointment
          ? timelineData.events.find(e => e.id === sinceAppointment)?.date ?? null
          : null}
        onEventTap={handleEventTap}
        selectedEventId={selectedEvent?.id ?? null}
      />

      <!-- Zoom controls (floating) -->
      <ZoomControls
        currentZoom={zoom}
        onZoomChange={(z) => { zoom = z; }}
      />

      <!-- Event detail popup -->
      {#if selectedEvent && popupAnchor}
        <EventDetailPopup
          event={selectedEvent}
          correlations={timelineData.correlations.filter(
            c => c.source_id === selectedEvent.id || c.target_id === selectedEvent.id
          )}
          anchor={popupAnchor}
          onClose={handleClosePopup}
          onNavigate={onNavigate}
          onScrollToEvent={(eventId) => {
            // Close current popup, let canvas scroll to event
            handleClosePopup();
            const target = timelineData!.events.find(e => e.id === eventId);
            if (target) handleEventTap(target, { x: 0, y: 0 }); // Re-open at target
          }}
        />
      {/if}
    </div>
  {/if}
</div>
```

### TimelineCanvas (SVG Renderer)

```svelte
<!-- src/lib/components/timeline/TimelineCanvas.svelte -->
<script lang="ts">
  import type {
    TimelineEvent, TimelineCorrelation, DateRange, ZoomLevel,
  } from '$lib/types/timeline';
  import {
    calculateCanvasWidth, dateToX, eventToY,
    generateTicks, correlationPath, CANVAS_HEIGHT,
    MARKER_RADIUS, TOUCH_TARGET_RADIUS, HEADER_HEIGHT, PADDING_X,
    EVENT_COLORS, eventColorGroup,
  } from '$lib/utils/timeline';

  interface Props {
    events: TimelineEvent[];
    correlations: TimelineCorrelation[];
    dateRange: DateRange;
    zoom: ZoomLevel;
    sinceDate: string | null;
    onEventTap: (event: TimelineEvent, anchor: { x: number; y: number }) => void;
    selectedEventId: string | null;
  }
  let {
    events, correlations, dateRange, zoom, sinceDate,
    onEventTap, selectedEventId,
  }: Props = $props();

  let scrollContainer: HTMLDivElement | undefined = $state(undefined);

  let earliest = $derived(dateRange.earliest ? new Date(dateRange.earliest) : new Date());
  let latest = $derived(dateRange.latest ? new Date(dateRange.latest) : new Date());
  let canvasWidth = $derived(calculateCanvasWidth(earliest, latest, zoom));
  let ticks = $derived(generateTicks(earliest, latest, zoom));

  let sinceDateX = $derived(
    sinceDate ? dateToX(new Date(sinceDate), earliest, zoom) : null
  );

  /** Map event ID → {x, y} for correlation line endpoints */
  let eventPositions = $derived<Map<string, { x: number; y: number }>>(
    new Map(events.map(e => [
      e.id,
      {
        x: dateToX(new Date(e.date), earliest, zoom),
        y: eventToY(e.event_type),
      },
    ]))
  );

  function handleMarkerClick(event: TimelineEvent, svgEvent: MouseEvent) {
    const rect = scrollContainer?.getBoundingClientRect();
    if (!rect) return;
    onEventTap(event, {
      x: svgEvent.clientX - rect.left,
      y: svgEvent.clientY - rect.top,
    });
  }

  function handleMarkerKeydown(event: TimelineEvent, keyEvent: KeyboardEvent) {
    if (keyEvent.key === 'Enter' || keyEvent.key === ' ') {
      keyEvent.preventDefault();
      const target = keyEvent.target as SVGElement;
      const rect = target.getBoundingClientRect();
      const containerRect = scrollContainer?.getBoundingClientRect();
      if (!containerRect) return;
      onEventTap(event, {
        x: rect.left + rect.width / 2 - containerRect.left,
        y: rect.top + rect.height / 2 - containerRect.top,
      });
    }
  }

  /** Marker icon symbol for medication events */
  function markerSymbol(eventType: string): string {
    switch (eventType) {
      case 'MedicationStart': return '+';
      case 'MedicationStop': return '\u00d7';
      case 'MedicationDoseChange': return '\u0394';
      default: return '';
    }
  }

  /** Whether to show text labels at current zoom */
  let showLabels = $derived(zoom === 'Day' || zoom === 'Week');
</script>

<div
  bind:this={scrollContainer}
  class="w-full h-full overflow-x-auto overflow-y-hidden"
  role="application"
  aria-label="Medical timeline"
  aria-roledescription="Scrollable timeline of medical events"
>
  <svg
    width={canvasWidth}
    height={CANVAS_HEIGHT}
    viewBox="0 0 {canvasWidth} {CANVAS_HEIGHT}"
    class="select-none"
    aria-hidden="false"
  >
    <!-- "Since last visit" dimming overlay -->
    {#if sinceDateX !== null}
      <rect
        x="0" y={HEADER_HEIGHT}
        width={sinceDateX} height={CANVAS_HEIGHT - HEADER_HEIGHT}
        fill="#78716C" opacity="0.08"
      />
      <line
        x1={sinceDateX} y1={HEADER_HEIGHT}
        x2={sinceDateX} y2={CANVAS_HEIGHT}
        stroke="#14B8A6" stroke-width="2" stroke-dasharray="6 4"
      />
      <text
        x={sinceDateX + 6} y={HEADER_HEIGHT + 14}
        class="text-xs fill-teal-600 font-medium"
      >
        Last visit
      </text>
    {/if}

    <!-- Date axis ticks -->
    {#each ticks as tick}
      <g>
        <line
          x1={tick.x} y1={HEADER_HEIGHT - 4}
          x2={tick.x} y2={HEADER_HEIGHT}
          stroke="#D6D3D1" stroke-width="1"
        />
        <line
          x1={tick.x} y1={HEADER_HEIGHT}
          x2={tick.x} y2={CANVAS_HEIGHT}
          stroke="#F5F5F4" stroke-width="1"
        />
        <text
          x={tick.x} y={HEADER_HEIGHT - 8}
          text-anchor="middle"
          class="text-xs fill-stone-400"
        >
          {tick.label}
        </text>
      </g>
    {/each}

    <!-- Horizontal lane separator lines -->
    {#each [0, 1, 2, 3, 4, 5, 6] as laneIdx}
      <line
        x1={0} y1={eventToY('MedicationStart') - 24 + laneIdx * 56}
        x2={canvasWidth} y2={eventToY('MedicationStart') - 24 + laneIdx * 56}
        stroke="#F5F5F4" stroke-width="1"
      />
    {/each}

    <!-- Correlation lines (behind event markers) -->
    {#each correlations as corr}
      {@const source = eventPositions.get(corr.source_id)}
      {@const target = eventPositions.get(corr.target_id)}
      {#if source && target && (zoom === 'Day' || zoom === 'Week')}
        <path
          d={correlationPath(source.x, source.y, target.x, target.y)}
          class="fill-none stroke-stone-300 opacity-60"
          class:!stroke-orange-500={selectedEventId === corr.source_id || selectedEventId === corr.target_id}
          class:!opacity-100={selectedEventId === corr.source_id || selectedEventId === corr.target_id}
          stroke-width={selectedEventId === corr.source_id || selectedEventId === corr.target_id ? 2 : 1.5}
          stroke-dasharray={selectedEventId === corr.source_id || selectedEventId === corr.target_id ? 'none' : '4 3'}
          pointer-events="none"
        />
      {/if}
    {/each}

    <!-- Event markers -->
    {#each events as event}
      {@const pos = eventPositions.get(event.id)}
      {@const colorGroup = eventColorGroup(event.event_type)}
      {@const colors = EVENT_COLORS[colorGroup]}
      {#if pos && colors}
        <g
          role="button"
          tabindex="0"
          aria-label="{event.title} on {new Date(event.date).toLocaleDateString()}"
          onclick={(e) => handleMarkerClick(event, e)}
          onkeydown={(e) => handleMarkerKeydown(event, e)}
          class="cursor-pointer focus:outline-none"
          style="opacity: {sinceDate && pos.x < (sinceDateX ?? 0) ? 0.3 : 1}"
        >
          <!-- Invisible touch target (44px diameter) -->
          <circle
            cx={pos.x} cy={pos.y}
            r={TOUCH_TARGET_RADIUS}
            fill="transparent"
          />

          <!-- Selection ring -->
          {#if selectedEventId === event.id}
            <circle
              cx={pos.x} cy={pos.y}
              r={MARKER_RADIUS + 4}
              fill="none" stroke={colors.stroke} stroke-width="2" opacity="0.4"
            />
          {/if}

          <!-- Visible marker -->
          <circle
            cx={pos.x} cy={pos.y}
            r={MARKER_RADIUS}
            fill={colors.fill} stroke={colors.stroke} stroke-width="2"
          />

          <!-- Marker symbol for medication events -->
          {#if markerSymbol(event.event_type)}
            <text
              x={pos.x} y={pos.y + 4}
              text-anchor="middle"
              class="text-xs font-bold"
              fill={colors.stroke}
              pointer-events="none"
            >
              {markerSymbol(event.event_type)}
            </text>
          {/if}

          <!-- Label (only at Day/Week zoom) -->
          {#if showLabels}
            <text
              x={pos.x} y={pos.y + MARKER_RADIUS + 14}
              text-anchor="middle"
              class="text-xs fill-stone-600"
              pointer-events="none"
            >
              {event.title.length > 20 ? event.title.slice(0, 18) + '...' : event.title}
            </text>
          {/if}
        </g>
      {/if}
    {/each}
  </svg>
</div>
```

### FilterBar

```svelte
<!-- src/lib/components/timeline/FilterBar.svelte -->
<script lang="ts">
  import type {
    EventType, EventCounts, ProfessionalSummary, TimelineEvent,
  } from '$lib/types/timeline';
  import { EVENT_COLORS } from '$lib/utils/timeline';

  interface Props {
    eventCounts: EventCounts;
    professionals: ProfessionalSummary[];
    activeTypes: EventType[];
    selectedProfessionalId: string | null;
    sinceAppointment: string | null;
    completedAppointments: TimelineEvent[];
    onTypeToggle: (types: EventType[]) => void;
    onProfessionalChange: (id: string | null) => void;
    onDateRangeChange: (from: string | null, to: string | null) => void;
    onSinceVisitChange: (appointmentId: string | null) => void;
  }
  let {
    eventCounts, professionals, activeTypes, selectedProfessionalId,
    sinceAppointment, completedAppointments,
    onTypeToggle, onProfessionalChange, onDateRangeChange, onSinceVisitChange,
  }: Props = $props();

  interface ChipDef {
    types: EventType[];
    label: string;
    colorGroup: string;
    countKey: keyof EventCounts;
  }

  const chips: ChipDef[] = [
    { types: ['MedicationStart', 'MedicationStop', 'MedicationDoseChange'], label: 'Meds', colorGroup: 'medication', countKey: 'medications' },
    { types: ['LabResult'], label: 'Labs', colorGroup: 'lab', countKey: 'lab_results' },
    { types: ['Symptom'], label: 'Symptoms', colorGroup: 'symptom', countKey: 'symptoms' },
    { types: ['Procedure'], label: 'Procedures', colorGroup: 'procedure', countKey: 'procedures' },
    { types: ['Appointment'], label: 'Appts', colorGroup: 'appointment', countKey: 'appointments' },
    { types: ['Document'], label: 'Docs', colorGroup: 'document', countKey: 'documents' },
    { types: ['Diagnosis'], label: 'Diagnoses', colorGroup: 'diagnosis', countKey: 'diagnoses' },
  ];

  function isChipActive(chipTypes: EventType[]): boolean {
    return chipTypes.every(t => activeTypes.includes(t));
  }

  function toggleChip(chipTypes: EventType[]) {
    const allActive = isChipActive(chipTypes);
    let newTypes: EventType[];
    if (allActive) {
      newTypes = activeTypes.filter(t => !chipTypes.includes(t));
    } else {
      newTypes = [...new Set([...activeTypes, ...chipTypes])];
    }
    onTypeToggle(newTypes);
  }

  let showFiltersExpanded = $state(false);
</script>

<div class="px-4 pb-2 border-b border-stone-200 bg-white">
  <!-- Type filter chips (scrollable row) -->
  <div class="flex gap-2 overflow-x-auto pb-2 -mx-1 px-1 scrollbar-hide">
    {#each chips as chip}
      {@const active = isChipActive(chip.types)}
      {@const colors = EVENT_COLORS[chip.colorGroup]}
      <button
        class="flex items-center gap-1.5 px-3 py-1.5 rounded-full text-sm whitespace-nowrap
               min-h-[36px] transition-colors border
               {active
                 ? 'border-transparent text-stone-800'
                 : 'border-stone-200 text-stone-400 bg-white'}"
        style={active ? `background-color: ${colors.fill}; border-color: ${colors.stroke}40` : ''}
        onclick={() => toggleChip(chip.types)}
        aria-pressed={active}
        aria-label="{chip.label}: {eventCounts[chip.countKey]} events"
      >
        <span class="w-2 h-2 rounded-full"
              style="background-color: {active ? colors.stroke : '#D6D3D1'}"></span>
        {chip.label}
        <span class="text-xs opacity-70">{eventCounts[chip.countKey]}</span>
      </button>
    {/each}
  </div>

  <!-- Expandable filters row -->
  <button
    class="text-xs text-stone-500 py-1 min-h-[44px] w-full text-left"
    onclick={() => { showFiltersExpanded = !showFiltersExpanded; }}
    aria-expanded={showFiltersExpanded}
    aria-controls="timeline-filters-expanded"
  >
    {showFiltersExpanded ? 'Hide filters' : 'More filters...'}
  </button>

  {#if showFiltersExpanded}
    <div id="timeline-filters-expanded" class="flex flex-wrap gap-3 py-2">
      <!-- Professional dropdown -->
      <div class="flex flex-col gap-1">
        <label for="prof-filter" class="text-xs text-stone-500">Professional</label>
        <select
          id="prof-filter"
          class="text-sm border border-stone-200 rounded-lg px-3 py-2 min-h-[44px]
                 bg-white text-stone-700"
          value={selectedProfessionalId ?? ''}
          onchange={(e) => onProfessionalChange(
            (e.target as HTMLSelectElement).value || null
          )}
        >
          <option value="">All professionals</option>
          {#each professionals as prof}
            <option value={prof.id}>
              {prof.name}{prof.specialty ? ` (${prof.specialty})` : ''} — {prof.event_count}
            </option>
          {/each}
        </select>
      </div>

      <!-- Since last visit dropdown -->
      <div class="flex flex-col gap-1">
        <label for="since-visit" class="text-xs text-stone-500">Since last visit</label>
        <select
          id="since-visit"
          class="text-sm border border-stone-200 rounded-lg px-3 py-2 min-h-[44px]
                 bg-white text-stone-700"
          value={sinceAppointment ?? ''}
          onchange={(e) => onSinceVisitChange(
            (e.target as HTMLSelectElement).value || null
          )}
        >
          <option value="">All time</option>
          {#each completedAppointments as appt}
            <option value={appt.id}>
              {appt.professional_name ?? 'Visit'} — {new Date(appt.date).toLocaleDateString()}
            </option>
          {/each}
        </select>
      </div>
    </div>
  {/if}
</div>
```

### ZoomControls

```svelte
<!-- src/lib/components/timeline/ZoomControls.svelte -->
<script lang="ts">
  import type { ZoomLevel } from '$lib/types/timeline';

  interface Props {
    currentZoom: ZoomLevel;
    onZoomChange: (zoom: ZoomLevel) => void;
  }
  let { currentZoom, onZoomChange }: Props = $props();

  const levels: ZoomLevel[] = ['Day', 'Week', 'Month', 'Year'];
</script>

<div class="absolute bottom-4 right-4 flex flex-col bg-white rounded-xl shadow-lg
            border border-stone-200 overflow-hidden z-10"
     role="radiogroup"
     aria-label="Timeline zoom level">
  {#each levels as level}
    <button
      class="px-4 py-2 text-sm min-h-[44px] min-w-[44px] transition-colors
             {currentZoom === level
               ? 'bg-stone-800 text-white font-medium'
               : 'text-stone-600 hover:bg-stone-50'}"
      role="radio"
      aria-checked={currentZoom === level}
      onclick={() => onZoomChange(level)}
    >
      {level}
    </button>
  {/each}
</div>
```

### EventDetailPopup

```svelte
<!-- src/lib/components/timeline/EventDetailPopup.svelte -->
<script lang="ts">
  import { onMount } from 'svelte';
  import type { TimelineEvent, TimelineCorrelation } from '$lib/types/timeline';
  import { EVENT_COLORS, eventColorGroup } from '$lib/utils/timeline';

  interface Props {
    event: TimelineEvent;
    correlations: TimelineCorrelation[];
    anchor: { x: number; y: number };
    onClose: () => void;
    onNavigate: (screen: string, params?: Record<string, string>) => void;
    onScrollToEvent: (eventId: string) => void;
  }
  let { event, correlations, anchor, onClose, onNavigate, onScrollToEvent }: Props = $props();

  let popupEl: HTMLDivElement | undefined = $state(undefined);

  // Position: avoid going off-screen
  let popupStyle = $derived(() => {
    const maxWidth = 320;
    const viewportWidth = typeof window !== 'undefined' ? window.innerWidth : 800;
    const viewportHeight = typeof window !== 'undefined' ? window.innerHeight : 600;

    let left = anchor.x + 16;
    let top = anchor.y - 20;

    // Flip left if would overflow right
    if (left + maxWidth > viewportWidth - 16) {
      left = anchor.x - maxWidth - 16;
    }
    // Flip up if would overflow bottom
    if (top + 300 > viewportHeight - 16) {
      top = anchor.y - 300;
    }
    // Clamp
    left = Math.max(8, left);
    top = Math.max(8, top);

    return `left: ${left}px; top: ${top}px; max-width: ${maxWidth}px;`;
  });

  let colorGroup = $derived(eventColorGroup(event.event_type));
  let colors = $derived(EVENT_COLORS[colorGroup]);

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') onClose();
  }

  onMount(() => {
    document.addEventListener('keydown', handleKeydown);
    popupEl?.focus();
    return () => document.removeEventListener('keydown', handleKeydown);
  });

  function formatDate(dateStr: string): string {
    return new Date(dateStr).toLocaleDateString('en-US', {
      weekday: 'long', year: 'numeric', month: 'long', day: 'numeric',
    });
  }
</script>

<!-- Backdrop (click to close) -->
<button
  class="fixed inset-0 z-20 bg-transparent"
  onclick={onClose}
  aria-label="Close event details"
  tabindex="-1"
></button>

<!-- Popup card -->
<div
  bind:this={popupEl}
  class="fixed z-30 bg-white rounded-xl shadow-xl border border-stone-200
         overflow-y-auto p-4"
  style={popupStyle()}
  role="dialog"
  aria-label="Event details: {event.title}"
  tabindex="-1"
>
  <!-- Header -->
  <div class="flex items-start gap-2 mb-3">
    <span class="w-3 h-3 rounded-full mt-1 flex-shrink-0"
          style="background-color: {colors.stroke}"></span>
    <div class="flex-1 min-w-0">
      <h3 class="font-medium text-stone-800 text-sm">{event.title}</h3>
      <p class="text-xs text-stone-500 mt-0.5">{formatDate(event.date)}</p>
      {#if event.professional_name}
        <p class="text-xs text-stone-500">{event.professional_name}</p>
      {/if}
    </div>
    <button
      class="text-stone-400 hover:text-stone-600 min-h-[44px] min-w-[44px]
             flex items-center justify-center -mr-2 -mt-2"
      onclick={onClose}
      aria-label="Close"
    >
      &times;
    </button>
  </div>

  <!-- Type-specific details -->
  <div class="text-sm text-stone-700 space-y-1 mb-3">
    {#if event.metadata.kind === 'Medication'}
      <p><span class="text-stone-500">Dose:</span> {event.metadata.dose} {event.metadata.frequency}</p>
      {#if event.metadata.brand_name}
        <p><span class="text-stone-500">Brand:</span> {event.metadata.brand_name}</p>
      {/if}
      {#if event.metadata.reason}
        <p><span class="text-stone-500">Reason:</span> {event.metadata.reason}</p>
      {/if}
    {:else if event.metadata.kind === 'DoseChange'}
      <p><span class="text-stone-500">Changed:</span> {event.metadata.old_dose ?? '?'} &rarr; {event.metadata.new_dose}</p>
      {#if event.metadata.old_frequency && event.metadata.new_frequency}
        <p><span class="text-stone-500">Frequency:</span> {event.metadata.old_frequency} &rarr; {event.metadata.new_frequency}</p>
      {/if}
      {#if event.metadata.reason}
        <p><span class="text-stone-500">Reason:</span> {event.metadata.reason}</p>
      {/if}
    {:else if event.metadata.kind === 'Lab'}
      <p>
        <span class="text-stone-500">Result:</span>
        {event.metadata.value ?? event.metadata.value_text ?? 'N/A'}
        {event.metadata.unit ?? ''}
      </p>
      {#if event.metadata.reference_low !== null && event.metadata.reference_high !== null}
        <p><span class="text-stone-500">Range:</span> {event.metadata.reference_low} — {event.metadata.reference_high} {event.metadata.unit ?? ''}</p>
      {/if}
      {#if event.metadata.abnormal_flag !== 'normal'}
        <p class="text-amber-700 text-xs">
          This result is outside the normal range. Consider discussing with your doctor.
        </p>
      {/if}
    {:else if event.metadata.kind === 'Symptom'}
      <p><span class="text-stone-500">Severity:</span> {event.metadata.severity}/5</p>
      {#if event.metadata.body_region}
        <p><span class="text-stone-500">Location:</span> {event.metadata.body_region}</p>
      {/if}
      <p>
        <span class="text-xs px-2 py-0.5 rounded-full
               {event.metadata.still_active ? 'bg-orange-100 text-orange-700' : 'bg-green-100 text-green-700'}">
          {event.metadata.still_active ? 'Still active' : 'Resolved'}
        </span>
      </p>
    {:else if event.metadata.kind === 'Procedure'}
      {#if event.metadata.facility}
        <p><span class="text-stone-500">Facility:</span> {event.metadata.facility}</p>
      {/if}
      {#if event.metadata.outcome}
        <p><span class="text-stone-500">Outcome:</span> {event.metadata.outcome}</p>
      {/if}
      {#if event.metadata.follow_up_required}
        <p class="text-amber-700 text-xs">Follow-up recommended</p>
      {/if}
    {:else if event.metadata.kind === 'Appointment'}
      <p><span class="text-stone-500">Type:</span> {event.metadata.appointment_type}</p>
      {#if event.metadata.professional_specialty}
        <p><span class="text-stone-500">Specialty:</span> {event.metadata.professional_specialty}</p>
      {/if}
    {:else if event.metadata.kind === 'Document'}
      <p><span class="text-stone-500">Type:</span> {event.metadata.document_type}</p>
      <p>
        <span class="text-xs px-2 py-0.5 rounded-full
               {event.metadata.verified ? 'bg-green-100 text-green-700' : 'bg-amber-100 text-amber-700'}">
          {event.metadata.verified ? 'Verified' : 'Not verified'}
        </span>
      </p>
    {:else if event.metadata.kind === 'Diagnosis'}
      {#if event.metadata.icd_code}
        <p><span class="text-stone-500">ICD:</span> {event.metadata.icd_code}</p>
      {/if}
      <p><span class="text-stone-500">Status:</span> {event.metadata.status}</p>
    {/if}
  </div>

  <!-- Correlations -->
  {#if correlations.length > 0}
    <div class="border-t border-stone-100 pt-2 mb-3">
      <p class="text-xs text-stone-500 font-medium mb-1">Related events ({correlations.length})</p>
      {#each correlations as corr}
        <button
          class="w-full text-left text-xs text-stone-600 py-1.5 hover:text-stone-800
                 min-h-[44px] flex items-center"
          onclick={() => {
            const targetId = corr.source_id === event.id ? corr.target_id : corr.source_id;
            onScrollToEvent(targetId);
          }}
        >
          <span class="text-stone-400 mr-1">&rarr;</span>
          {corr.description}
        </button>
      {/each}
    </div>
  {/if}

  <!-- Action buttons -->
  <div class="flex gap-2 border-t border-stone-100 pt-2">
    {#if event.document_id}
      <button
        class="flex-1 text-sm text-center py-2 rounded-lg bg-stone-100 text-stone-700
               hover:bg-stone-200 min-h-[44px]"
        onclick={() => onNavigate('document-detail', { documentId: event.document_id! })}
      >
        View document
      </button>
    {/if}
    <button
      class="flex-1 text-sm text-center py-2 rounded-lg bg-stone-100 text-stone-700
             hover:bg-stone-200 min-h-[44px]"
      onclick={() => {
        const route = event.metadata.kind === 'Medication' || event.metadata.kind === 'DoseChange'
          ? 'medications'
          : event.metadata.kind === 'Lab' ? 'lab-detail'
          : event.metadata.kind === 'Symptom' ? 'journal'
          : event.metadata.kind === 'Appointment' ? 'appointments'
          : 'documents';
        onNavigate(route, { entityId: event.id });
      }}
    >
      Go to source
    </button>
  </div>
</div>
```

### EmptyTimeline

```svelte
<!-- src/lib/components/timeline/EmptyTimeline.svelte -->
<script lang="ts">
  interface Props {
    onNavigate: (screen: string) => void;
  }
  let { onNavigate }: Props = $props();
</script>

<div class="flex flex-col items-center justify-center flex-1 px-8 py-12 text-center">
  <!-- Simple timeline illustration placeholder -->
  <div class="w-24 h-24 bg-stone-100 rounded-2xl flex items-center justify-center mb-6">
    <span class="text-4xl text-stone-300">&#x1F4C5;</span>
  </div>

  <h2 class="text-lg font-medium text-stone-700 mb-2">
    Your timeline is empty
  </h2>
  <p class="text-sm text-stone-500 mb-6 max-w-[280px]">
    Load your first medical document and your timeline will start building automatically.
  </p>

  <button
    class="px-8 py-4 bg-[var(--color-primary)] text-white rounded-xl text-base font-medium
           hover:brightness-110 focus-visible:outline focus-visible:outline-2
           focus-visible:outline-offset-2 focus-visible:outline-[var(--color-primary)]
           min-h-[44px]"
    onclick={() => onNavigate('import')}
  >
    Load a document
  </button>
</div>
```

---

## [14] Frontend API

```typescript
// src/lib/api/timeline.ts
import { invoke } from '@tauri-apps/api/core';
import type { TimelineData, TimelineFilter } from '$lib/types/timeline';

export async function getTimelineData(filter: TimelineFilter): Promise<TimelineData> {
  return invoke<TimelineData>('get_timeline_data', { filter });
}
```

### Utility Module

```typescript
// src/lib/utils/timeline.ts
// Re-exports all coordinate/scaling/color functions defined in sections [5], [6], [7].
// This is a single-file utility module imported by all timeline Svelte components.

export {
  SCALE, LANE_HEIGHT, LANE_GAP, HEADER_HEIGHT,
  MARKER_RADIUS, TOUCH_TARGET_RADIUS,
  PADDING_X, PADDING_Y, CANVAS_HEIGHT,
  calculateCanvasWidth, dateToX, eventToY,
  generateTicks, autoSelectZoom, correlationPath,
  EVENT_COLORS, eventColorGroup,
} from '$lib/types/timeline';
```

---

## [15] Error Handling

| Error | User Message | Recovery |
|-------|-------------|----------|
| Database query fails (any table) | "Something went wrong loading your timeline. Try again." | Retry button on screen |
| Session expired (timeout) | Redirected to profile unlock (ProfileGuard handles) | Re-enter password |
| Single event fails to parse | Skip that event, log warning, show remaining events | Degraded but functional |
| Appointment not found (since last visit) | "That appointment could not be found. Showing full timeline." | Clear since_appointment_id, reload |
| No events exist | Show EmptyTimeline state | Prompt to load first document |
| SVG render error (invalid coordinate) | Clamp coordinate to canvas bounds, log warning | Degraded display |

All errors logged via `tracing::warn!` or `tracing::error!`. No sensitive patient data in error messages or frontend console output.

---

## [16] Security

- All entity fields decrypted via `ProfileSession.decrypt()` before assembly into `TimelineEvent`
- No raw UUIDs logged to frontend console
- Timeline data never cached in `localStorage` or `sessionStorage` — always fetched from encrypted database
- Event metadata does not include raw document content — only summaries and extracted fields
- Activity timestamp updated on every `get_timeline_data` call (prevents false inactivity timeout)
- `TimelineFilter` validated server-side: date strings parsed and sanitized, UUID strings validated
- No SQL injection risk: all queries use parameterized statements (`params![]`)

---

## [17] Testing

### Unit Tests (Rust)

| Test | What |
|------|------|
| `test_assemble_empty_database` | Returns empty events vec when all tables empty |
| `test_assemble_medications_start` | Medication with start_date produces MedicationStart event |
| `test_assemble_medications_stop` | Stopped medication with end_date produces MedicationStop event |
| `test_assemble_dose_changes` | Dose change rows produce MedicationDoseChange events |
| `test_assemble_lab_results` | Lab results mapped with correct abnormal_flag → severity |
| `test_assemble_symptoms` | Symptom rows mapped with severity and active status |
| `test_assemble_procedures` | Procedure rows mapped with follow_up flag |
| `test_assemble_appointments` | Appointment rows mapped with type (upcoming/completed) |
| `test_assemble_documents` | Document rows mapped with type and verified status |
| `test_assemble_diagnoses` | Diagnosis rows mapped with status |
| `test_events_sorted_by_date` | All events returned in chronological order |
| `test_filter_by_event_type` | Filtering by medication types excludes labs, symptoms |
| `test_filter_by_professional` | Filtering by professional_id returns only matching events |
| `test_filter_by_date_range` | Events outside date range excluded |
| `test_since_appointment_resolves_date` | since_appointment_id resolves to appointment date as date_from |
| `test_since_appointment_not_found` | Invalid appointment ID returns error |
| `test_detect_correlations_within_window` | Symptom 5 days after med start → correlation detected |
| `test_detect_correlations_outside_window` | Symptom 30 days after med start → no correlation |
| `test_explicit_correlations` | related_medication_id link produces ExplicitLink correlation |
| `test_correlation_deduplication` | Same pair from detection + explicit → deduplicated to one |
| `test_event_counts_all_tables` | compute_event_counts sums correctly across all tables |
| `test_professionals_with_counts` | Only professionals with events returned, sorted by count |
| `test_event_type_as_str_round_trip` | Every EventType serializes and deserializes correctly |
| `test_event_severity_from_lab_flag` | Lab abnormal_flag maps to correct EventSeverity |
| `test_timeline_data_structure` | Full TimelineData response has all required fields |

### Frontend Tests

| Test | What |
|------|------|
| `test_timeline_screen_renders_header` | Header shows "Timeline" and subtitle |
| `test_timeline_empty_state` | Shows empty state when no events |
| `test_timeline_canvas_renders_events` | SVG contains circle elements for each event |
| `test_event_marker_color_by_type` | Medication markers use blue fill, lab markers use green |
| `test_event_tap_opens_popup` | Tapping event marker opens detail popup |
| `test_popup_shows_event_details` | Popup displays title, date, professional, metadata |
| `test_popup_escape_closes` | Pressing Escape closes popup |
| `test_popup_outside_click_closes` | Clicking outside popup closes it |
| `test_filter_chip_toggles` | Toggling a chip hides/shows corresponding event type |
| `test_filter_chip_badges` | Chips show correct event count badges |
| `test_zoom_controls_render` | All four zoom levels shown |
| `test_zoom_change_rescales` | Changing zoom updates canvas width |
| `test_since_visit_banner` | Banner shown when since_appointment_id is set |
| `test_since_visit_dimming` | Events before appointment date have reduced opacity |
| `test_since_visit_clear` | Clear button resets to full timeline |
| `test_correlation_lines_visible_day_zoom` | Correlation paths rendered at Day zoom |
| `test_correlation_lines_hidden_year_zoom` | Correlation paths not rendered at Year zoom |
| `test_correlation_highlight_on_select` | Selecting an event highlights its correlation lines |
| `test_auto_zoom_selection` | 2-week range selects Day, 6-month selects Week, 3-year selects Year |
| `test_date_to_x_calculation` | Known dates map to expected X coordinates |
| `test_accessibility_markers_have_aria_labels` | Every event g element has aria-label |
| `test_touch_targets_44px` | Invisible hit-area circles have r=22 (44px diameter) |
| `test_professional_filter_dropdown` | Dropdown lists professionals with event counts |
| `test_navigate_to_document` | "View document" button navigates with correct document ID |
| `test_navigate_to_source` | "Go to source" button navigates to correct entity screen |

---

## [18] Performance

| Metric | Target | Strategy |
|--------|--------|----------|
| Initial timeline load (100 events) | < 200ms | Single Tauri command, server-side assembly |
| Initial timeline load (500 events) | < 500ms | Same — SQL queries indexed on date columns |
| SVG render (100 events) | < 50ms | Flat SVG, no complex gradients or filters |
| SVG render (500 events) | < 150ms | Consider virtualization if exceeded |
| Zoom transition | < 200ms | CSS transition on transform, recalculate widths |
| Filter toggle (client-side) | < 16ms | Array filter, no re-fetch |
| Professional/date filter (server re-fetch) | < 300ms | Parameterized query with indexed columns |
| Memory footprint (500 events) | < 5MB | Flat event structs, no document content |

### Optimization Strategies

1. **Server-side assembly:** All 9 table queries run in a single `spawn_blocking` call. No N+1 queries.
2. **Indexed columns:** All date columns have indexes (see L0-02 schema). Professional FK columns indexed.
3. **Client-side type filtering:** Toggling event type chips does not trigger a server re-fetch — just hides/shows SVG groups.
4. **Virtualized rendering (Phase 2):** If profiles exceed 500 events, implement viewport-based rendering — only draw events whose X coordinate falls within `scrollLeft - buffer` to `scrollLeft + viewportWidth + buffer`.
5. **Correlation detection bounded:** O(n*m) where n = symptoms and m = medication events. With the 14-day window, most profiles produce < 50 correlations.
6. **No content decryption on timeline:** Only metadata fields (names, dates, values) are decrypted. Full document content is never loaded.

---

## [19] Open Questions

| # | Question | Status |
|---|---------|--------|
| OQ-01 | Should we support user-created correlation links (patient manually connects symptom to medication)? Current: only automatic detection + DB-stored `related_medication_id`. | Deferred to Phase 2 |
| OQ-02 | Should zoom support pinch-to-zoom gesture on desktop (trackpad)? Current: button-only zoom. | Deferred — evaluate after user testing |
| OQ-03 | Should the timeline support vertical orientation for narrow mobile viewports? Current: horizontal only. | Deferred — Tauri desktop-first |
| OQ-04 | Should we add a "print timeline" feature (export SVG to PDF)? Would be useful for appointment prep (L4-02). | Evaluate after L4-02 completion |
| OQ-05 | How should we handle events with no date (e.g., medication with null start_date)? Current: exclude from timeline. Alternative: place at document ingestion date with a "date estimated" indicator. | Needs design decision |

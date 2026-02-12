# L3-05 — Medication List

<!--
=============================================================================
COMPONENT SPEC — The pharmacist's and caregiver's primary screen.
Engineer review: E-UX (UI/UX, lead), E-RS (Rust), E-DA (Data), E-SC (Security), E-QA (QA)
Safety-critical display: medication names, doses, and frequencies must be
typographically clear, unambiguous, and readable at a glance.
Marie and Sophie rely on this screen to know exactly what Marie is taking.
=============================================================================
-->

## Table of Contents

| Section | Offset |
|---------|--------|
| [1] Identity | `offset=22 limit=35` |
| [2] Dependencies | `offset=57 limit=22` |
| [3] Interfaces | `offset=79 limit=120` |
| [4] Active Medications View | `offset=199 limit=65` |
| [5] Medication Detail Card | `offset=264 limit=80` |
| [6] Dose Change History | `offset=344 limit=55` |
| [7] OTC Medication Entry Form | `offset=399 limit=70` |
| [8] Brand/Generic Display | `offset=469 limit=40` |
| [9] Filter and Search | `offset=509 limit=55` |
| [10] Tauri Commands (IPC) | `offset=564 limit=90` |
| [11] Svelte Components | `offset=654 limit=220` |
| [12] Frontend API | `offset=874 limit=40` |
| [13] Error Handling | `offset=914 limit=25` |
| [14] Security | `offset=939 limit=25` |
| [15] Testing | `offset=964 limit=60` |
| [16] Performance | `offset=1024 limit=15` |
| [17] Open Questions | `offset=1039 limit=15` |

---

## [1] Identity

**What:** The medication list screen -- the pharmacist's and caregiver's primary view. Shows all medications (active, paused, stopped) in a structured card-based list with full clinical detail: dose, frequency, route, prescriber, start date, instructions, compound ingredients (for compound medications), and tapering schedules (for medications being tapered). Includes per-medication dose change history, an OTC medication manual entry form, brand/generic name resolution via the medication_aliases table, and filtering by prescriber, status, or search term. Also surfaces coherence engine observations (conflicts, duplicates) inline on relevant medication cards.

**After this session:**
- Tab bar "Meds" tab navigates here from any screen
- Active medications displayed as cards, sorted by start_date DESC (newest first)
- Each card shows: generic name (primary), brand name (secondary), dose, frequency, route, prescriber name, status badge
- Tapping a card expands to detail view with instructions, compound ingredients, tapering schedule
- Dose change history accessible per medication (chronological timeline)
- OTC entry form: patient can manually add over-the-counter medications
- Brand/generic name lookup via medication_aliases table (bundled + user-added)
- Filter bar: by status (active/paused/stopped), by prescriber, by text search
- Coherence observations shown inline on affected medication cards (non-alarming framing)
- Empty state for profiles with no medications yet
- Medication count shown in header

**Estimated complexity:** Medium-High
**Source:** Tech Spec v1.1 Section 9.4 (Screen Map — Medications), Section 5.2 (Data Model — medications, compound_ingredients, tapering_schedules, dose_changes, medication_aliases)

---

## [2] Dependencies

**Incoming:**
- L0-02 (data model -- medications, compound_ingredients, tapering_schedules, dose_changes, medication_aliases, medication_instructions tables; MedicationRepository trait; MedicationFilter struct)
- L0-03 (encryption -- ProfileSession for decrypting medication fields)
- L1-04 (storage pipeline -- medications stored here after document processing)

**Outgoing:**
- L2-03 (coherence engine -- medication conflicts, duplicates, allergy cross-references shown inline on cards)
- L4-02 (appointment prep -- references medication list for pre-appointment summaries)
- L3-02 (home screen -- medication count in ProfileStats)

**No new Cargo.toml dependencies.** Uses existing repository traits, Tauri state, and MedicationFilter from L0-02.

---

## [3] Interfaces

### Backend Types

```rust
// src-tauri/src/medications.rs

use chrono::{NaiveDate, NaiveDateTime};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A medication card for the list view
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MedicationCard {
    pub id: Uuid,
    pub generic_name: String,
    pub brand_name: Option<String>,
    pub dose: String,
    pub frequency: String,
    pub frequency_type: String,         // "scheduled", "as_needed", "tapering"
    pub route: String,
    pub prescriber_name: Option<String>,
    pub prescriber_specialty: Option<String>,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub status: String,                  // "active", "stopped", "paused"
    pub reason_start: Option<String>,
    pub is_otc: bool,
    pub is_compound: bool,
    pub has_tapering: bool,
    pub dose_type: String,               // "fixed", "sliding_scale", "weight_based", "variable"
    pub administration_instructions: Option<String>,
    pub condition: Option<String>,        // "For pain", "If blood sugar > 250"
    pub coherence_alerts: Vec<MedicationAlert>,
}

/// Coherence alert specific to a medication (shown inline on card)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MedicationAlert {
    pub id: Uuid,
    pub alert_type: String,              // "conflict", "duplicate", "allergy", "dose"
    pub severity: String,                // "Info", "Warning", "Critical"
    pub summary: String,                 // Patient-facing calm text
}

/// Full medication detail (expanded view)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MedicationDetail {
    pub medication: MedicationCard,
    pub instructions: Vec<MedicationInstructionView>,
    pub compound_ingredients: Vec<CompoundIngredientView>,
    pub tapering_steps: Vec<TaperingStepView>,
    pub aliases: Vec<MedicationAliasView>,
    pub dose_changes: Vec<DoseChangeView>,
    pub document_title: Option<String>,
    pub document_date: Option<NaiveDate>,
}

/// A single instruction for a medication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MedicationInstructionView {
    pub id: Uuid,
    pub instruction: String,
    pub timing: Option<String>,
}

/// A compound ingredient display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompoundIngredientView {
    pub id: Uuid,
    pub ingredient_name: String,
    pub ingredient_dose: Option<String>,
    pub maps_to_generic: Option<String>,
}

/// A tapering step display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaperingStepView {
    pub step_number: i32,
    pub dose: String,
    pub duration_days: i32,
    pub start_date: Option<NaiveDate>,
    pub instructions: Option<String>,
    pub is_current: bool,                // Computed: is this the active step?
}

/// A dose change record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoseChangeView {
    pub id: Uuid,
    pub old_dose: Option<String>,
    pub new_dose: String,
    pub old_frequency: Option<String>,
    pub new_frequency: Option<String>,
    pub change_date: NaiveDate,
    pub changed_by_name: Option<String>,
    pub reason: Option<String>,
    pub document_title: Option<String>,
}

/// Brand/generic alias entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MedicationAliasView {
    pub generic_name: String,
    pub brand_name: String,
    pub country: String,
    pub source: String,                   // "bundled" or "user_added"
}

/// Filter parameters for medication list
#[derive(Debug, Clone, Deserialize)]
pub struct MedicationListFilter {
    pub status: Option<String>,           // "active", "stopped", "paused"
    pub prescriber_id: Option<String>,
    pub search_query: Option<String>,
    pub include_otc: bool,
}

/// Data for the medication list screen header
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MedicationListData {
    pub medications: Vec<MedicationCard>,
    pub total_active: u32,
    pub total_paused: u32,
    pub total_stopped: u32,
    pub prescribers: Vec<PrescriberOption>,
}

/// A prescriber option for the filter dropdown
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrescriberOption {
    pub id: Uuid,
    pub name: String,
    pub specialty: Option<String>,
    pub medication_count: u32,
}

/// OTC medication entry input
#[derive(Debug, Clone, Deserialize)]
pub struct OtcMedicationInput {
    pub name: String,
    pub dose: String,
    pub frequency: String,
    pub route: String,
    pub reason: Option<String>,
    pub start_date: Option<String>,       // ISO 8601 date
    pub instructions: Option<String>,
}

/// Alias search result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AliasSearchResult {
    pub generic_name: String,
    pub brand_names: Vec<String>,
    pub source: String,
}
```

### Frontend Types

```typescript
// src/lib/types/medication.ts

export interface MedicationCard {
  id: string;
  generic_name: string;
  brand_name: string | null;
  dose: string;
  frequency: string;
  frequency_type: 'scheduled' | 'as_needed' | 'tapering';
  route: string;
  prescriber_name: string | null;
  prescriber_specialty: string | null;
  start_date: string | null;
  end_date: string | null;
  status: 'active' | 'stopped' | 'paused';
  reason_start: string | null;
  is_otc: boolean;
  is_compound: boolean;
  has_tapering: boolean;
  dose_type: 'fixed' | 'sliding_scale' | 'weight_based' | 'variable';
  administration_instructions: string | null;
  condition: string | null;
  coherence_alerts: MedicationAlert[];
}

export interface MedicationAlert {
  id: string;
  alert_type: string;
  severity: 'Info' | 'Warning' | 'Critical';
  summary: string;
}

export interface MedicationDetail {
  medication: MedicationCard;
  instructions: MedicationInstructionView[];
  compound_ingredients: CompoundIngredientView[];
  tapering_steps: TaperingStepView[];
  aliases: MedicationAliasView[];
  dose_changes: DoseChangeView[];
  document_title: string | null;
  document_date: string | null;
}

export interface MedicationInstructionView {
  id: string;
  instruction: string;
  timing: string | null;
}

export interface CompoundIngredientView {
  id: string;
  ingredient_name: string;
  ingredient_dose: string | null;
  maps_to_generic: string | null;
}

export interface TaperingStepView {
  step_number: number;
  dose: string;
  duration_days: number;
  start_date: string | null;
  instructions: string | null;
  is_current: boolean;
}

export interface DoseChangeView {
  id: string;
  old_dose: string | null;
  new_dose: string;
  old_frequency: string | null;
  new_frequency: string | null;
  change_date: string;
  changed_by_name: string | null;
  reason: string | null;
  document_title: string | null;
}

export interface MedicationAliasView {
  generic_name: string;
  brand_name: string;
  country: string;
  source: 'bundled' | 'user_added';
}

export interface MedicationListFilter {
  status: 'active' | 'stopped' | 'paused' | null;
  prescriber_id: string | null;
  search_query: string | null;
  include_otc: boolean;
}

export interface MedicationListData {
  medications: MedicationCard[];
  total_active: number;
  total_paused: number;
  total_stopped: number;
  prescribers: PrescriberOption[];
}

export interface PrescriberOption {
  id: string;
  name: string;
  specialty: string | null;
  medication_count: number;
}

export interface OtcMedicationInput {
  name: string;
  dose: string;
  frequency: string;
  route: string;
  reason: string | null;
  start_date: string | null;
  instructions: string | null;
}

export interface AliasSearchResult {
  generic_name: string;
  brand_names: string[];
  source: string;
}
```

---

## [4] Active Medications View

**E-UX lead:** This is a safety-critical display. Medication names must be typographically unambiguous -- large font for generic name, clear dose and frequency, distinct status badges. No decorative elements that could obscure clinical information. Calm design, generous spacing, clear hierarchy.

### Layout

```
+--------------------------------------------+
|  HEADER                                    |
|  Medications                               |
|  5 active . 1 paused . 2 stopped          |
+--------------------------------------------+
|  FILTER BAR                                |
|  [Search...         ] [Status v] [Dr. v]   |
+--------------------------------------------+
|  STATUS TABS (horizontal scroll)           |
|  [ All (8) ] [ Active (5) ] [ Paused (1) ]|
|  [ Stopped (2) ]                           |
+--------------------------------------------+
|  MEDICATION CARDS                          |
|  +----------------------------------------+|
|  | Metformin                          500mg||
|  | (Glucophage)         Twice daily        ||
|  | Dr. Chen . Oral . Active               ||
|  | "1 observation about this medication"   ||
|  +----------------------------------------+|
|  +----------------------------------------+|
|  | Lisinopril                        10mg  ||
|  | (Zestril)            Once daily         ||
|  | Dr. Chen . Oral . Active               ||
|  +----------------------------------------+|
|  +----------------------------------------+|
|  | Ibuprofen                       400mg   ||
|  | (Advil)              As needed          ||
|  | OTC . Oral . Active                     ||
|  +----------------------------------------+|
+--------------------------------------------+
|  [+ Add OTC medication]                    |
+--------------------------------------------+
```

### Card Display Rules

| Field | Display Rule |
|-------|-------------|
| Generic name | Primary line, `text-lg font-semibold text-stone-800`. Always shown. |
| Brand name | Secondary line in parentheses, `text-sm text-stone-500`. Shown if available. |
| Dose | Right-aligned on first line, `text-lg font-semibold text-stone-800`. Safety-critical: must be visually prominent. |
| Frequency | Right-aligned on second line, `text-sm text-stone-600`. Human-readable: "Twice daily", "Every 8 hours", "As needed". |
| Prescriber | Third line: "Dr. {name}" or "OTC" for self-reported. `text-xs text-stone-400`. |
| Route | Third line after prescriber, dot-separated. Capitalized: "Oral", "Topical", "IV". |
| Status badge | Third line, rightmost. Color-coded (see below). |
| Condition | If present, shown below third line in italics: "For pain", "If blood sugar > 250". |
| Coherence alert | If present, amber background bar at bottom of card with calm text. |

### Status Badges

| Status | Badge Style | Text |
|--------|------------|------|
| Active | `bg-green-100 text-green-700` | "Active" |
| Paused | `bg-amber-100 text-amber-700` | "Paused" |
| Stopped | `bg-stone-100 text-stone-500` | "Stopped" |

### Sorting

Default sort: Active medications first, then paused, then stopped. Within each group, sorted by `start_date DESC` (newest first). OTC medications are intermixed with prescribed medications (not separated).

### Empty State

When profile has zero medications:
- Hide filter bar and status tabs
- Show centered illustration placeholder (simple pill SVG)
- Text: "No medications recorded yet"
- Subtext: "Medications will appear here when you load a prescription or add an over-the-counter medication."
- Prominent [+ Add OTC medication] button

---

## [5] Medication Detail Card

**E-UX:** Tapping a medication card navigates to a detail view (slide-in panel or full screen). All clinical information for a single medication is shown here. This is what a pharmacist would want to see.

### Detail Layout

```
+--------------------------------------------+
|  <- Back                                   |
+--------------------------------------------+
|  MEDICATION HEADER                         |
|  Metformin 500mg                           |
|  (Glucophage)                              |
|  Active . Oral . Twice daily               |
|  Prescribed by Dr. Chen (Endocrinologist)  |
|  Started Jan 15, 2025                      |
|  Reason: Type 2 diabetes management        |
+--------------------------------------------+
|  INSTRUCTIONS                              |
|  - Take with food                          |
|  - Take 2 hours apart from iron            |
|  - Do not crush or chew                    |
+--------------------------------------------+
|  BRAND/GENERIC NAMES                       |
|  Generic: Metformin                        |
|  Known brands: Glucophage, Fortamet,       |
|  Glumetza, Riomet                          |
+--------------------------------------------+
|  COMPOUND INGREDIENTS (if compound)        |
|  - Lidocaine 2%                            |
|  - Prilocaine 2.5%                         |
|  - Gabapentin 6%                           |
+--------------------------------------------+
|  TAPERING SCHEDULE (if tapering)           |
|  Step 1: 40mg for 7 days (Jan 15-21)      |
|  Step 2: 30mg for 7 days (Jan 22-28) <--  |
|  Step 3: 20mg for 7 days (Jan 29-Feb 4)   |
|  Step 4: 10mg for 7 days (Feb 5-11)       |
|  Step 5: 5mg for 3 days (Feb 12-14)       |
+--------------------------------------------+
|  DOSE HISTORY                              |
|  [View dose change history]                |
+--------------------------------------------+
|  SOURCE DOCUMENT                           |
|  Prescription . Dr. Chen . Jan 15, 2025    |
+--------------------------------------------+
```

### Compound Ingredients Section

Shown only when `is_compound == true`. Displays all ingredients from `compound_ingredients` table.

```
Each ingredient row:
  - ingredient_name (bold)
  - ingredient_dose (if available)
  - maps_to_generic (if resolved): "(also known as {generic})"
```

### Tapering Schedule Section

Shown only when `frequency_type == "tapering"`. Displays all steps from `tapering_schedules` table, ordered by `step_number ASC`.

```
Each step row:
  - "Step {n}: {dose} for {duration_days} days"
  - date range computed from start_date + cumulative prior durations
  - current step highlighted with "<-- current" indicator and subtle background
  - completed steps shown with checkmark, grayed out
  - future steps shown in normal weight
```

### Instructions Section

Shown when the medication has entries in `medication_instructions` table or `administration_instructions` field. Each instruction on its own line with a bullet point. Timing shown in parentheses if available.

---

## [6] Dose Change History

**E-UX:** Accessible from the medication detail view via a "View dose change history" link. Shows a chronological timeline of all dose changes for a specific medication.

### Timeline Layout

```
+--------------------------------------------+
|  <- Back to Metformin                      |
+--------------------------------------------+
|  DOSE HISTORY                              |
|  Metformin                                 |
+--------------------------------------------+
|  TIMELINE                                  |
|                                            |
|  o  Jan 15, 2025                           |
|  |  Started at 250mg, once daily           |
|  |  Dr. Chen . "Initial dose"              |
|  |  Source: Prescription Jan 15            |
|  |                                         |
|  o  Feb 1, 2025                            |
|  |  250mg -> 500mg                         |
|  |  Once daily -> Twice daily              |
|  |  Dr. Chen . "Dose increase, HbA1c 7.8" |
|  |  Source: Prescription Feb 1             |
|  |                                         |
|  o  Current                                |
|     500mg, Twice daily                     |
|                                            |
+--------------------------------------------+
```

### Dose Change Display Rules

| Field | Display Rule |
|-------|-------------|
| Date | `text-sm font-medium text-stone-700`. Formatted as "MMM DD, YYYY". |
| Dose change | `text-base text-stone-800`. Format: "{old_dose} -> {new_dose}" or "Started at {new_dose}" for first entry. |
| Frequency change | Shown below dose if changed: "{old_frequency} -> {new_frequency}". |
| Changed by | `text-xs text-stone-400`. "Dr. {name}" or omitted if unknown. |
| Reason | `text-xs text-stone-500 italic`. Shown in quotes if available. |
| Document link | `text-xs text-stone-400`. "Source: {document_title}" -- tappable, navigates to document detail. |

### Timeline Visual

Vertical line (`border-l-2 border-stone-200`) with circle markers (`w-3 h-3 rounded-full bg-stone-400`) at each change point. Current state uses a filled primary-color circle. The line extends from top to bottom. Each entry is a row connected to the line.

---

## [7] OTC Medication Entry Form

**E-UX:** Patients can manually add over-the-counter medications they are taking. The form is accessible from the medication list screen via a prominent button. The form is simple -- minimal required fields, patient-friendly labels.

### Form Layout

```
+--------------------------------------------+
|  <- Back                                   |
+--------------------------------------------+
|  Add an over-the-counter medication        |
|                                            |
|  Medication name *                         |
|  [Ibuprofen                           ]   |
|  (start typing to search known names)      |
|                                            |
|  Dose *                                    |
|  [400mg                               ]   |
|                                            |
|  How often? *                              |
|  [As needed for pain              ]        |
|                                            |
|  How do you take it?                       |
|  ( Oral )  ( Topical )  ( Other )          |
|                                            |
|  Why are you taking it?                    |
|  [Headaches and muscle pain           ]    |
|                                            |
|  When did you start?                       |
|  [   /   /      ]                          |
|                                            |
|  Special instructions                      |
|  [Take with food                      ]    |
|                                            |
|  [ Add medication ]                        |
+--------------------------------------------+
```

### Form Fields

| Field | Required | Type | Validation |
|-------|----------|------|-----------|
| Medication name | Yes | Text with autocomplete | Non-empty, max 200 chars |
| Dose | Yes | Text | Non-empty, max 100 chars |
| Frequency | Yes | Text | Non-empty, max 200 chars |
| Route | No (default: "oral") | Radio group | One of: oral, topical, other |
| Reason | No | Text | Max 500 chars |
| Start date | No | Date picker | Not in the future |
| Instructions | No | Textarea | Max 1000 chars |

### Autocomplete Behavior

When the user types in the medication name field:
1. After 2+ characters, query `medication_aliases` table for matching generic or brand names
2. Show dropdown of matches: "{generic_name} ({brand_name})" or just "{generic_name}"
3. Selecting a match fills in the generic name
4. User can also type a custom name not in the database

### On Submit

1. Validate required fields
2. Call `add_otc_medication` Tauri command
3. On success: navigate back to medication list, show brief confirmation toast "Medication added"
4. On error: show inline error message, keep form populated

### Backend Processing

When an OTC medication is added:
- `is_otc = true`
- `status = "active"`
- `prescriber_id = NULL` (no prescriber for OTC)
- `document_id` references a synthetic "patient-reported" document (or is NULL with a special flag)
- `dose_type = "fixed"` (default for OTC)

---

## [8] Brand/Generic Display

**E-UX + E-DA:** Medications are stored by generic name (primary identifier). Brand names are resolved via the `medication_aliases` table. The UI shows both when available.

### Display Logic

```
Priority:
1. Always show generic_name as primary label (safety: pharmacists use generic names)
2. If brand_name is set on the medication record, show it in parentheses
3. In detail view, show all known aliases from medication_aliases table
```

### Alias Resolution

```rust
/// Fetch known aliases for a medication's generic name
pub fn fetch_medication_aliases(
    conn: &rusqlite::Connection,
    generic_name: &str,
) -> Result<Vec<MedicationAliasView>, DatabaseError> {
    conn.prepare(
        "SELECT generic_name, brand_name, country, source
         FROM medication_aliases
         WHERE generic_name = ?1 COLLATE NOCASE
         ORDER BY source ASC, brand_name ASC"
    )?
    .query_map(params![generic_name], |row| {
        Ok(MedicationAliasView {
            generic_name: row.get(0)?,
            brand_name: row.get(1)?,
            country: row.get(2)?,
            source: row.get(3)?,
        })
    })?
    .collect::<Result<Vec<_>, _>>()
    .map_err(DatabaseError::from)
}
```

### Alias Search (for OTC form autocomplete)

```rust
/// Search medication aliases by partial name (generic or brand)
pub fn search_medication_aliases(
    conn: &rusqlite::Connection,
    query: &str,
    limit: u32,
) -> Result<Vec<AliasSearchResult>, DatabaseError> {
    let pattern = format!("%{}%", query);
    let rows = conn.prepare(
        "SELECT generic_name, GROUP_CONCAT(brand_name, ', '), source
         FROM medication_aliases
         WHERE generic_name LIKE ?1 COLLATE NOCASE
            OR brand_name LIKE ?1 COLLATE NOCASE
         GROUP BY generic_name
         ORDER BY generic_name ASC
         LIMIT ?2"
    )?
    .query_map(params![pattern, limit], |row| {
        let brand_str: String = row.get(1)?;
        Ok(AliasSearchResult {
            generic_name: row.get(0)?,
            brand_names: brand_str.split(", ").map(String::from).collect(),
            source: row.get(2)?,
        })
    })?
    .collect::<Result<Vec<_>, _>>()
    .map_err(DatabaseError::from)
}
```

---

## [9] Filter and Search

### Filter Bar

Horizontal bar below the header with three controls:

1. **Search input**: Text field with magnifying glass icon. Filters medications by generic_name, brand_name, or condition. Debounced at 300ms.
2. **Status filter**: Dropdown or tab-based. Options: All, Active, Paused, Stopped. Default: All.
3. **Prescriber filter**: Dropdown listing all prescribers with medication counts. Options: All prescribers, then each prescriber with count.

### Filter Query

```rust
/// Fetch medications with filters applied
pub fn fetch_medications_filtered(
    conn: &rusqlite::Connection,
    filter: &MedicationListFilter,
    session: &ProfileSession,
) -> Result<Vec<MedicationCard>, DatabaseError> {
    let mut sql = String::from(
        "SELECT m.id, m.generic_name, m.brand_name, m.dose, m.frequency,
                m.frequency_type, m.route, m.status, m.start_date, m.end_date,
                m.reason_start, m.is_otc, m.is_compound, m.dose_type,
                m.administration_instructions, m.condition, m.max_daily_dose,
                p.name AS prescriber_name, p.specialty AS prescriber_specialty
         FROM medications m
         LEFT JOIN professionals p ON m.prescriber_id = p.id
         WHERE 1=1"
    );

    let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut param_idx = 1;

    // Status filter
    if let Some(status) = &filter.status {
        sql.push_str(&format!(" AND m.status = ?{}", param_idx));
        params_vec.push(Box::new(status.clone()));
        param_idx += 1;
    }

    // Prescriber filter
    if let Some(prescriber_id) = &filter.prescriber_id {
        sql.push_str(&format!(" AND m.prescriber_id = ?{}", param_idx));
        params_vec.push(Box::new(prescriber_id.clone()));
        param_idx += 1;
    }

    // OTC inclusion
    if !filter.include_otc {
        sql.push_str(" AND m.is_otc = 0");
    }

    // Text search (generic name, brand name, or condition)
    if let Some(query) = &filter.search_query {
        let pattern = format!("%{}%", query);
        sql.push_str(&format!(
            " AND (m.generic_name LIKE ?{} COLLATE NOCASE
               OR m.brand_name LIKE ?{} COLLATE NOCASE
               OR m.condition LIKE ?{} COLLATE NOCASE)",
            param_idx, param_idx, param_idx
        ));
        params_vec.push(Box::new(pattern));
        param_idx += 1;
    }

    // Sort: active first, then paused, then stopped; within group by start_date DESC
    sql.push_str(
        " ORDER BY CASE m.status
            WHEN 'active' THEN 1
            WHEN 'paused' THEN 2
            WHEN 'stopped' THEN 3
          END ASC,
          m.start_date DESC"
    );

    // Execute and map rows to MedicationCard
    // (Implementation maps each row, checks for tapering steps, and fetches coherence alerts)
    let params_refs: Vec<&dyn rusqlite::types::ToSql> = params_vec.iter()
        .map(|p| p.as_ref())
        .collect();

    let mut stmt = conn.prepare(&sql)?;
    let cards = stmt.query_map(params_refs.as_slice(), |row| {
        Ok(MedicationCard {
            id: row.get::<_, String>(0)?.parse().unwrap_or_default(),
            generic_name: row.get(1)?,
            brand_name: row.get(2)?,
            dose: row.get(3)?,
            frequency: row.get(4)?,
            frequency_type: row.get(5)?,
            route: row.get(6)?,
            status: row.get(7)?,
            start_date: row.get::<_, Option<String>>(8)?
                .and_then(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok()),
            end_date: row.get::<_, Option<String>>(9)?
                .and_then(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok()),
            reason_start: row.get(10)?,
            is_otc: row.get::<_, i32>(11)? != 0,
            is_compound: row.get::<_, i32>(12)? != 0,
            has_tapering: false, // Set below
            dose_type: row.get(13)?,
            administration_instructions: row.get(14)?,
            condition: row.get(15)?,
            prescriber_name: row.get(17)?,
            prescriber_specialty: row.get(18)?,
            coherence_alerts: Vec::new(), // Populated below
        })
    })?
    .collect::<Result<Vec<_>, _>>()?;

    Ok(cards)
}
```

### Status Tab Counts

```rust
/// Fetch medication counts by status for the tab bar
pub fn fetch_medication_status_counts(
    conn: &rusqlite::Connection,
) -> Result<(u32, u32, u32), DatabaseError> {
    let active: u32 = conn.query_row(
        "SELECT COUNT(*) FROM medications WHERE status = 'active'",
        [], |row| row.get(0)
    )?;
    let paused: u32 = conn.query_row(
        "SELECT COUNT(*) FROM medications WHERE status = 'paused'",
        [], |row| row.get(0)
    )?;
    let stopped: u32 = conn.query_row(
        "SELECT COUNT(*) FROM medications WHERE status = 'stopped'",
        [], |row| row.get(0)
    )?;
    Ok((active, paused, stopped))
}
```

### Prescriber Options

```rust
/// Fetch prescribers who have medications, with counts
pub fn fetch_prescriber_options(
    conn: &rusqlite::Connection,
) -> Result<Vec<PrescriberOption>, DatabaseError> {
    conn.prepare(
        "SELECT p.id, p.name, p.specialty, COUNT(m.id) AS med_count
         FROM professionals p
         INNER JOIN medications m ON m.prescriber_id = p.id
         GROUP BY p.id
         ORDER BY med_count DESC, p.name ASC"
    )?
    .query_map([], |row| {
        Ok(PrescriberOption {
            id: row.get::<_, String>(0)?.parse().unwrap_or_default(),
            name: row.get(1)?,
            specialty: row.get(2)?,
            medication_count: row.get(3)?,
        })
    })?
    .collect::<Result<Vec<_>, _>>()
    .map_err(DatabaseError::from)
}
```

---

## [10] Tauri Commands (IPC)

```rust
// src-tauri/src/commands/medications.rs

use tauri::State;

/// Fetches all medication list data in a single call
#[tauri::command]
pub async fn get_medications(
    state: State<'_, AppState>,
    filter: MedicationListFilter,
) -> Result<MedicationListData, String> {
    let session_guard = state.active_session.lock()
        .map_err(|e| format!("Lock error: {e}"))?;
    let session = session_guard.as_ref()
        .ok_or("No active profile session")?;

    let conn = session.db_connection()?;

    let medications = fetch_medications_filtered(&conn, &filter, session)
        .map_err(|e| e.to_string())?;

    // Enrich each card with tapering flag and coherence alerts
    let medications = medications.into_iter().map(|mut card| {
        // Check if this medication has tapering steps
        card.has_tapering = conn.query_row(
            "SELECT COUNT(*) FROM tapering_schedules WHERE medication_id = ?1",
            params![card.id.to_string()],
            |row| row.get::<_, u32>(0),
        ).unwrap_or(0) > 0;

        // Fetch coherence alerts for this medication
        card.coherence_alerts = fetch_medication_alerts(&conn, &card.id)
            .unwrap_or_default();

        card
    }).collect();

    let (total_active, total_paused, total_stopped) =
        fetch_medication_status_counts(&conn)
            .map_err(|e| e.to_string())?;

    let prescribers = fetch_prescriber_options(&conn)
        .map_err(|e| e.to_string())?;

    state.update_activity();

    Ok(MedicationListData {
        medications,
        total_active,
        total_paused,
        total_stopped,
        prescribers,
    })
}

/// Fetches full detail for a single medication
#[tauri::command]
pub async fn get_medication_detail(
    state: State<'_, AppState>,
    medication_id: String,
) -> Result<MedicationDetail, String> {
    let session_guard = state.active_session.lock()
        .map_err(|e| format!("Lock error: {e}"))?;
    let session = session_guard.as_ref()
        .ok_or("No active profile session")?;

    let conn = session.db_connection()?;
    let med_uuid = Uuid::parse_str(&medication_id)
        .map_err(|e| format!("Invalid medication ID: {e}"))?;

    // Fetch the medication card
    let card = fetch_single_medication_card(&conn, &med_uuid, session)
        .map_err(|e| e.to_string())?
        .ok_or("Medication not found")?;

    // Fetch instructions
    let instructions = conn.prepare(
        "SELECT id, instruction, timing
         FROM medication_instructions
         WHERE medication_id = ?1
         ORDER BY id ASC"
    )
    .map_err(|e| e.to_string())?
    .query_map(params![medication_id], |row| {
        Ok(MedicationInstructionView {
            id: row.get::<_, String>(0)?.parse().unwrap_or_default(),
            instruction: row.get(1)?,
            timing: row.get(2)?,
        })
    })
    .map_err(|e| e.to_string())?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| e.to_string())?;

    // Fetch compound ingredients
    let compound_ingredients = if card.is_compound {
        conn.prepare(
            "SELECT id, ingredient_name, ingredient_dose, maps_to_generic
             FROM compound_ingredients
             WHERE medication_id = ?1
             ORDER BY ingredient_name ASC"
        )
        .map_err(|e| e.to_string())?
        .query_map(params![medication_id], |row| {
            Ok(CompoundIngredientView {
                id: row.get::<_, String>(0)?.parse().unwrap_or_default(),
                ingredient_name: row.get(1)?,
                ingredient_dose: row.get(2)?,
                maps_to_generic: row.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?
    } else {
        Vec::new()
    };

    // Fetch tapering steps
    let tapering_steps = if card.has_tapering {
        fetch_tapering_steps(&conn, &med_uuid)
            .map_err(|e| e.to_string())?
    } else {
        Vec::new()
    };

    // Fetch aliases
    let aliases = fetch_medication_aliases(&conn, &card.generic_name)
        .map_err(|e| e.to_string())?;

    // Fetch dose changes
    let dose_changes = fetch_dose_history(&conn, &med_uuid)
        .map_err(|e| e.to_string())?;

    // Fetch source document info
    let (document_title, document_date) = conn.query_row(
        "SELECT d.title, d.document_date
         FROM documents d
         INNER JOIN medications m ON m.document_id = d.id
         WHERE m.id = ?1",
        params![medication_id],
        |row| Ok((row.get::<_, Option<String>>(0)?, row.get::<_, Option<String>>(1)?)),
    ).unwrap_or((None, None));

    state.update_activity();

    Ok(MedicationDetail {
        medication: card,
        instructions,
        compound_ingredients,
        tapering_steps,
        aliases,
        dose_changes,
        document_title,
        document_date: document_date
            .and_then(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok()),
    })
}

/// Adds a patient-reported OTC medication
#[tauri::command]
pub async fn add_otc_medication(
    state: State<'_, AppState>,
    input: OtcMedicationInput,
) -> Result<String, String> {
    let session_guard = state.active_session.lock()
        .map_err(|e| format!("Lock error: {e}"))?;
    let session = session_guard.as_ref()
        .ok_or("No active profile session")?;

    let conn = session.db_connection()?;

    // Validate required fields
    if input.name.trim().is_empty() {
        return Err("Medication name is required".into());
    }
    if input.dose.trim().is_empty() {
        return Err("Dose is required".into());
    }
    if input.frequency.trim().is_empty() {
        return Err("Frequency is required".into());
    }

    let med_id = Uuid::new_v4();
    let start_date = input.start_date
        .as_deref()
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());

    // Create a synthetic "patient-reported" document if one doesn't exist
    let patient_doc_id = get_or_create_patient_reported_document(&conn)?;

    conn.execute(
        "INSERT INTO medications (
            id, generic_name, brand_name, dose, frequency, frequency_type,
            route, prescriber_id, start_date, end_date, reason_start,
            reason_stop, is_otc, status, administration_instructions,
            max_daily_dose, condition, dose_type, is_compound, document_id
        ) VALUES (
            ?1, ?2, NULL, ?3, ?4, 'scheduled',
            ?5, NULL, ?6, NULL, ?7,
            NULL, 1, 'active', ?8,
            NULL, NULL, 'fixed', 0, ?9
        )",
        params![
            med_id.to_string(),
            input.name.trim(),
            input.dose.trim(),
            input.frequency.trim(),
            input.route.trim(),
            start_date.map(|d| d.to_string()),
            input.reason.as_deref().map(str::trim),
            input.instructions.as_deref().map(str::trim),
            patient_doc_id.to_string(),
        ],
    ).map_err(|e| format!("Failed to add medication: {e}"))?;

    // If instructions were provided, also store in medication_instructions table
    if let Some(instructions) = &input.instructions {
        if !instructions.trim().is_empty() {
            let instr_id = Uuid::new_v4();
            conn.execute(
                "INSERT INTO medication_instructions (id, medication_id, instruction, timing, source_document_id)
                 VALUES (?1, ?2, ?3, NULL, ?4)",
                params![
                    instr_id.to_string(),
                    med_id.to_string(),
                    instructions.trim(),
                    patient_doc_id.to_string(),
                ],
            ).map_err(|e| format!("Failed to store instruction: {e}"))?;
        }
    }

    state.update_activity();

    tracing::info!(
        medication_id = %med_id,
        name = %input.name,
        "OTC medication added by patient"
    );

    Ok(med_id.to_string())
}

/// Fetches dose change history for a medication
#[tauri::command]
pub async fn get_dose_history(
    state: State<'_, AppState>,
    medication_id: String,
) -> Result<Vec<DoseChangeView>, String> {
    let session_guard = state.active_session.lock()
        .map_err(|e| format!("Lock error: {e}"))?;
    let session = session_guard.as_ref()
        .ok_or("No active profile session")?;

    let conn = session.db_connection()?;
    let med_uuid = Uuid::parse_str(&medication_id)
        .map_err(|e| format!("Invalid medication ID: {e}"))?;

    let history = fetch_dose_history(&conn, &med_uuid)
        .map_err(|e| e.to_string())?;

    state.update_activity();

    Ok(history)
}

/// Searches medication aliases for autocomplete
#[tauri::command]
pub async fn search_medication_alias(
    state: State<'_, AppState>,
    query: String,
    limit: Option<u32>,
) -> Result<Vec<AliasSearchResult>, String> {
    let session_guard = state.active_session.lock()
        .map_err(|e| format!("Lock error: {e}"))?;
    let session = session_guard.as_ref()
        .ok_or("No active profile session")?;

    let conn = session.db_connection()?;
    let clamped_limit = limit.unwrap_or(10).min(50);

    let results = search_medication_aliases(&conn, &query, clamped_limit)
        .map_err(|e| e.to_string())?;

    state.update_activity();

    Ok(results)
}

// ─── Internal helper functions ────────────────────────────────────

/// Fetch dose change history ordered chronologically
fn fetch_dose_history(
    conn: &rusqlite::Connection,
    medication_id: &Uuid,
) -> Result<Vec<DoseChangeView>, DatabaseError> {
    conn.prepare(
        "SELECT dc.id, dc.old_dose, dc.new_dose, dc.old_frequency,
                dc.new_frequency, dc.change_date, dc.reason,
                p.name AS changed_by_name,
                d.title AS document_title
         FROM dose_changes dc
         LEFT JOIN professionals p ON dc.changed_by_id = p.id
         LEFT JOIN documents d ON dc.document_id = d.id
         WHERE dc.medication_id = ?1
         ORDER BY dc.change_date ASC"
    )?
    .query_map(params![medication_id.to_string()], |row| {
        Ok(DoseChangeView {
            id: row.get::<_, String>(0)?.parse().unwrap_or_default(),
            old_dose: row.get(1)?,
            new_dose: row.get(2)?,
            old_frequency: row.get(3)?,
            new_frequency: row.get(4)?,
            change_date: NaiveDate::parse_from_str(
                &row.get::<_, String>(5)?,
                "%Y-%m-%d",
            ).unwrap_or_else(|_| chrono::Local::now().date_naive()),
            reason: row.get(6)?,
            changed_by_name: row.get(7)?,
            document_title: row.get(8)?,
        })
    })?
    .collect::<Result<Vec<_>, _>>()
    .map_err(DatabaseError::from)
}

/// Fetch tapering steps with current-step computation
fn fetch_tapering_steps(
    conn: &rusqlite::Connection,
    medication_id: &Uuid,
) -> Result<Vec<TaperingStepView>, DatabaseError> {
    let today = chrono::Local::now().date_naive();

    let steps: Vec<TaperingStepView> = conn.prepare(
        "SELECT step_number, dose, duration_days, start_date
         FROM tapering_schedules
         WHERE medication_id = ?1
         ORDER BY step_number ASC"
    )?
    .query_map(params![medication_id.to_string()], |row| {
        let start_date = row.get::<_, Option<String>>(3)?
            .and_then(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok());
        Ok(TaperingStepView {
            step_number: row.get(0)?,
            dose: row.get(1)?,
            duration_days: row.get(2)?,
            start_date,
            instructions: None,
            is_current: false,
        })
    })?
    .collect::<Result<Vec<_>, _>>()?;

    // Compute which step is current based on dates
    let mut enriched = steps;
    let mut found_current = false;
    for step in enriched.iter_mut() {
        if let Some(start) = step.start_date {
            let end = start + chrono::Duration::days(step.duration_days as i64);
            if !found_current && today >= start && today < end {
                step.is_current = true;
                found_current = true;
            }
        }
    }

    // If no step matched by date, mark the first step with no start_date as current
    if !found_current {
        if let Some(first) = enriched.first_mut() {
            first.is_current = true;
        }
    }

    Ok(enriched)
}

/// Fetch coherence alerts for a specific medication
fn fetch_medication_alerts(
    conn: &rusqlite::Connection,
    medication_id: &Uuid,
) -> Result<Vec<MedicationAlert>, DatabaseError> {
    // Query coherence_observations that reference this medication
    // entity_ids is a JSON array of UUIDs
    let med_id_str = medication_id.to_string();

    conn.prepare(
        "SELECT co.id, co.alert_type, co.severity, co.summary
         FROM coherence_observations co
         LEFT JOIN dismissed_alerts da ON co.id = da.id
         WHERE da.id IS NULL
           AND co.entity_ids LIKE ?1
         ORDER BY co.severity DESC"
    )?
    .query_map(params![format!("%{}%", med_id_str)], |row| {
        Ok(MedicationAlert {
            id: row.get::<_, String>(0)?.parse().unwrap_or_default(),
            alert_type: row.get(1)?,
            severity: row.get(2)?,
            summary: row.get(3)?,
        })
    })?
    .collect::<Result<Vec<_>, _>>()
    .map_err(DatabaseError::from)
}

/// Get or create the synthetic "patient-reported" document for OTC entries
fn get_or_create_patient_reported_document(
    conn: &rusqlite::Connection,
) -> Result<Uuid, DatabaseError> {
    // Check if a patient-reported document already exists
    let existing: Option<String> = conn.query_row(
        "SELECT id FROM documents WHERE type = 'other' AND title = 'Patient-reported medications'",
        [],
        |row| row.get(0),
    ).ok();

    if let Some(id_str) = existing {
        return Ok(id_str.parse().unwrap_or_else(|_| Uuid::new_v4()));
    }

    // Create a new synthetic document
    let doc_id = Uuid::new_v4();
    conn.execute(
        "INSERT INTO documents (id, type, title, ingestion_date, source_file, verified)
         VALUES (?1, 'other', 'Patient-reported medications', datetime('now'), 'patient-reported', 1)",
        params![doc_id.to_string()],
    )?;

    Ok(doc_id)
}

/// Fetch a single medication card by ID
fn fetch_single_medication_card(
    conn: &rusqlite::Connection,
    medication_id: &Uuid,
    session: &ProfileSession,
) -> Result<Option<MedicationCard>, DatabaseError> {
    let result = conn.query_row(
        "SELECT m.id, m.generic_name, m.brand_name, m.dose, m.frequency,
                m.frequency_type, m.route, m.status, m.start_date, m.end_date,
                m.reason_start, m.is_otc, m.is_compound, m.dose_type,
                m.administration_instructions, m.condition,
                p.name, p.specialty
         FROM medications m
         LEFT JOIN professionals p ON m.prescriber_id = p.id
         WHERE m.id = ?1",
        params![medication_id.to_string()],
        |row| {
            Ok(MedicationCard {
                id: row.get::<_, String>(0)?.parse().unwrap_or_default(),
                generic_name: row.get(1)?,
                brand_name: row.get(2)?,
                dose: row.get(3)?,
                frequency: row.get(4)?,
                frequency_type: row.get(5)?,
                route: row.get(6)?,
                status: row.get(7)?,
                start_date: row.get::<_, Option<String>>(8)?
                    .and_then(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok()),
                end_date: row.get::<_, Option<String>>(9)?
                    .and_then(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok()),
                reason_start: row.get(10)?,
                is_otc: row.get::<_, i32>(11)? != 0,
                is_compound: row.get::<_, i32>(12)? != 0,
                has_tapering: false,
                dose_type: row.get(13)?,
                administration_instructions: row.get(14)?,
                condition: row.get(15)?,
                prescriber_name: row.get(16)?,
                prescriber_specialty: row.get(17)?,
                coherence_alerts: Vec::new(),
            })
        },
    );

    match result {
        Ok(mut card) => {
            card.has_tapering = conn.query_row(
                "SELECT COUNT(*) FROM tapering_schedules WHERE medication_id = ?1",
                params![medication_id.to_string()],
                |row| row.get::<_, u32>(0),
            ).unwrap_or(0) > 0;

            card.coherence_alerts = fetch_medication_alerts(conn, medication_id)
                .unwrap_or_default();

            Ok(Some(card))
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(DatabaseError::from(e)),
    }
}
```

---

## [11] Svelte Components

### MedicationListScreen

```svelte
<!-- src/lib/components/medications/MedicationListScreen.svelte -->
<script lang="ts">
  import { onMount } from 'svelte';
  import { getMedications, searchMedicationAlias } from '$lib/api/medications';
  import { listen } from '@tauri-apps/api/event';
  import type {
    MedicationListData,
    MedicationCard,
    MedicationListFilter,
  } from '$lib/types/medication';
  import MedicationCardView from './MedicationCardView.svelte';
  import MedicationSearch from './MedicationSearch.svelte';
  import EmptyMedicationState from './EmptyMedicationState.svelte';

  interface Props {
    onNavigate: (screen: string, params?: Record<string, string>) => void;
  }
  let { onNavigate }: Props = $props();

  let data: MedicationListData | null = $state(null);
  let loading = $state(true);
  let error: string | null = $state(null);

  // Filter state
  let statusFilter = $state<string | null>(null);
  let prescriberFilter = $state<string | null>(null);
  let searchQuery = $state('');
  let includeOtc = $state(true);

  let currentFilter = $derived<MedicationListFilter>({
    status: statusFilter,
    prescriber_id: prescriberFilter,
    search_query: searchQuery.trim() || null,
    include_otc: includeOtc,
  });

  // Debounce search
  let searchTimeout: ReturnType<typeof setTimeout> | null = $state(null);

  function handleSearchInput(value: string) {
    searchQuery = value;
    if (searchTimeout) clearTimeout(searchTimeout);
    searchTimeout = setTimeout(() => refresh(), 300);
  }

  async function refresh() {
    try {
      loading = data === null; // Only show full loading on first load
      error = null;
      data = await getMedications(currentFilter);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  onMount(() => {
    refresh();
    const unlisten = listen('document-imported', () => refresh());
    return () => { unlisten.then(fn => fn()); };
  });

  // Re-fetch when filter changes (except search, which is debounced)
  $effect(() => {
    // Track statusFilter and prescriberFilter
    statusFilter;
    prescriberFilter;
    includeOtc;
    refresh();
  });

  let totalCount = $derived(
    data
      ? data.total_active + data.total_paused + data.total_stopped
      : 0
  );

  let statusTabs = $derived([
    { label: 'All', value: null, count: totalCount },
    { label: 'Active', value: 'active', count: data?.total_active ?? 0 },
    { label: 'Paused', value: 'paused', count: data?.total_paused ?? 0 },
    { label: 'Stopped', value: 'stopped', count: data?.total_stopped ?? 0 },
  ]);
</script>

<div class="flex flex-col min-h-screen pb-20 bg-stone-50">
  <!-- Header -->
  <header class="px-6 pt-6 pb-4">
    <h1 class="text-2xl font-bold text-stone-800">Medications</h1>
    {#if data}
      <p class="text-sm text-stone-500 mt-1">
        {data.total_active} active{data.total_paused > 0 ? ` · ${data.total_paused} paused` : ''}{data.total_stopped > 0 ? ` · ${data.total_stopped} stopped` : ''}
      </p>
    {/if}
  </header>

  {#if loading && !data}
    <div class="flex items-center justify-center flex-1">
      <div class="animate-pulse text-stone-400">Loading medications...</div>
    </div>
  {:else if error}
    <div class="px-6 py-8 text-center">
      <p class="text-red-600 mb-4">Something went wrong: {error}</p>
      <button
        class="px-6 py-3 bg-stone-200 rounded-xl text-stone-700 min-h-[44px]"
        onclick={refresh}
      >
        Try again
      </button>
    </div>
  {:else if data && totalCount === 0 && !searchQuery}
    <EmptyMedicationState
      onAddOtc={() => onNavigate('otc-entry')}
    />
  {:else if data}
    <!-- Search and filter bar -->
    <MedicationSearch
      value={searchQuery}
      onInput={handleSearchInput}
      prescribers={data.prescribers}
      selectedPrescriber={prescriberFilter}
      onPrescriberChange={(id) => { prescriberFilter = id; }}
    />

    <!-- Status tabs -->
    <div class="px-6 py-2 flex gap-2 overflow-x-auto">
      {#each statusTabs as tab}
        <button
          class="px-4 py-2 rounded-full text-sm font-medium whitespace-nowrap
                 min-h-[44px] transition-colors
                 {statusFilter === tab.value
                   ? 'bg-[var(--color-primary)] text-white'
                   : 'bg-white text-stone-600 border border-stone-200 hover:bg-stone-50'}"
          onclick={() => { statusFilter = tab.value; }}
          aria-pressed={statusFilter === tab.value}
        >
          {tab.label} ({tab.count})
        </button>
      {/each}
    </div>

    <!-- Medication cards -->
    <div class="px-6 py-3 flex flex-col gap-3">
      {#each data.medications as medication (medication.id)}
        <MedicationCardView
          {medication}
          onTap={(med) => onNavigate('medication-detail', { medicationId: med.id })}
        />
      {:else}
        <div class="text-center py-8 text-stone-400 text-sm">
          No medications match your filters.
        </div>
      {/each}
    </div>

    <!-- Add OTC button -->
    <div class="px-6 py-4">
      <button
        class="w-full px-6 py-4 border border-dashed border-stone-300 rounded-xl
               text-stone-500 hover:border-[var(--color-primary)]
               hover:text-[var(--color-primary)] transition-all min-h-[44px]"
        onclick={() => onNavigate('otc-entry')}
      >
        + Add an over-the-counter medication
      </button>
    </div>
  {/if}
</div>
```

### MedicationCardView

```svelte
<!-- src/lib/components/medications/MedicationCardView.svelte -->
<script lang="ts">
  import type { MedicationCard } from '$lib/types/medication';

  interface Props {
    medication: MedicationCard;
    onTap: (medication: MedicationCard) => void;
  }
  let { medication, onTap }: Props = $props();

  let statusBadge = $derived(() => {
    switch (medication.status) {
      case 'active': return { text: 'Active', color: 'bg-green-100 text-green-700' };
      case 'paused': return { text: 'Paused', color: 'bg-amber-100 text-amber-700' };
      case 'stopped': return { text: 'Stopped', color: 'bg-stone-100 text-stone-500' };
      default: return { text: medication.status, color: 'bg-stone-100 text-stone-500' };
    }
  });

  let frequencyDisplay = $derived(() => {
    if (medication.frequency_type === 'as_needed') return 'As needed';
    if (medication.frequency_type === 'tapering') return 'Tapering';
    return medication.frequency;
  });

  let prescriberDisplay = $derived(() => {
    if (medication.is_otc) return 'OTC';
    if (medication.prescriber_name) return medication.prescriber_name;
    return 'Unknown prescriber';
  });

  function formatRoute(route: string): string {
    if (!route) return '';
    return route.charAt(0).toUpperCase() + route.slice(1).toLowerCase();
  }
</script>

<button
  class="w-full text-left bg-white rounded-xl p-4 shadow-sm border border-stone-100
         hover:shadow-md transition-shadow min-h-[44px]"
  onclick={() => onTap(medication)}
  aria-label="Medication: {medication.generic_name} {medication.dose}"
>
  <!-- Row 1: Generic name + Dose -->
  <div class="flex items-baseline justify-between gap-3">
    <span class="text-lg font-semibold text-stone-800 truncate">
      {medication.generic_name}
    </span>
    <span class="text-lg font-semibold text-stone-800 flex-shrink-0">
      {medication.dose}
    </span>
  </div>

  <!-- Row 2: Brand name + Frequency -->
  <div class="flex items-baseline justify-between gap-3 mt-0.5">
    {#if medication.brand_name}
      <span class="text-sm text-stone-500 truncate">
        ({medication.brand_name})
      </span>
    {:else}
      <span></span>
    {/if}
    <span class="text-sm text-stone-600 flex-shrink-0">
      {frequencyDisplay()}
    </span>
  </div>

  <!-- Row 3: Prescriber + Route + Status badge -->
  <div class="flex items-center justify-between gap-2 mt-2">
    <div class="flex items-center gap-1 text-xs text-stone-400 truncate">
      <span>{prescriberDisplay()}</span>
      <span aria-hidden="true">·</span>
      <span>{formatRoute(medication.route)}</span>
      {#if medication.is_compound}
        <span aria-hidden="true">·</span>
        <span class="text-indigo-500">Compound</span>
      {/if}
      {#if medication.has_tapering}
        <span aria-hidden="true">·</span>
        <span class="text-blue-500">Tapering</span>
      {/if}
    </div>
    <span class="text-xs px-2 py-0.5 rounded-full flex-shrink-0 {statusBadge().color}">
      {statusBadge().text}
    </span>
  </div>

  <!-- Row 4: Condition (if present) -->
  {#if medication.condition}
    <p class="text-xs text-stone-500 italic mt-1">
      {medication.condition}
    </p>
  {/if}

  <!-- Row 5: Coherence alerts (if any) -->
  {#if medication.coherence_alerts.length > 0}
    {#each medication.coherence_alerts as alert}
      <div
        class="mt-2 px-3 py-2 rounded-lg text-xs
               {alert.severity === 'Critical'
                 ? 'bg-amber-50 text-amber-800 border border-amber-200'
                 : alert.severity === 'Warning'
                   ? 'bg-blue-50 text-blue-700 border border-blue-100'
                   : 'bg-stone-50 text-stone-600 border border-stone-100'}"
        role="status"
      >
        {alert.summary}
      </div>
    {/each}
  {/if}
</button>
```

### MedicationDetail

```svelte
<!-- src/lib/components/medications/MedicationDetail.svelte -->
<script lang="ts">
  import { onMount } from 'svelte';
  import { getMedicationDetail } from '$lib/api/medications';
  import type { MedicationDetail as MedicationDetailType } from '$lib/types/medication';
  import DoseHistory from './DoseHistory.svelte';
  import TaperingSchedule from './TaperingSchedule.svelte';
  import CompoundIngredients from './CompoundIngredients.svelte';

  interface Props {
    medicationId: string;
    onBack: () => void;
    onNavigate: (screen: string, params?: Record<string, string>) => void;
  }
  let { medicationId, onBack, onNavigate }: Props = $props();

  let detail: MedicationDetailType | null = $state(null);
  let loading = $state(true);
  let error: string | null = $state(null);
  let showDoseHistory = $state(false);

  onMount(async () => {
    try {
      detail = await getMedicationDetail(medicationId);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  });

  function formatDate(dateStr: string | null): string {
    if (!dateStr) return '';
    return new Date(dateStr).toLocaleDateString('en-US', {
      month: 'short', day: 'numeric', year: 'numeric',
    });
  }

  function formatRoute(route: string): string {
    if (!route) return '';
    return route.charAt(0).toUpperCase() + route.slice(1).toLowerCase();
  }
</script>

<div class="flex flex-col min-h-screen pb-20 bg-stone-50">
  <!-- Back button -->
  <header class="px-6 pt-4 pb-2">
    <button
      class="text-stone-400 hover:text-stone-600 min-h-[44px] min-w-[44px]"
      onclick={onBack}
      aria-label="Back to medication list"
    >
      &larr; Back
    </button>
  </header>

  {#if loading}
    <div class="flex items-center justify-center flex-1">
      <div class="animate-pulse text-stone-400">Loading...</div>
    </div>
  {:else if error}
    <div class="px-6 py-8 text-center">
      <p class="text-red-600 mb-4">{error}</p>
      <button class="px-6 py-3 bg-stone-200 rounded-xl text-stone-700 min-h-[44px]"
              onclick={onBack}>
        Go back
      </button>
    </div>
  {:else if detail}
    <!-- Medication header -->
    <section class="px-6 py-4">
      <h2 class="text-2xl font-bold text-stone-800">
        {detail.medication.generic_name}
        <span class="text-xl font-semibold text-stone-600">{detail.medication.dose}</span>
      </h2>
      {#if detail.medication.brand_name}
        <p class="text-sm text-stone-500 mt-0.5">
          ({detail.medication.brand_name})
        </p>
      {/if}

      <div class="flex items-center gap-2 mt-3 text-sm text-stone-600">
        <span class="px-2 py-0.5 rounded-full text-xs
                     {detail.medication.status === 'active' ? 'bg-green-100 text-green-700'
                       : detail.medication.status === 'paused' ? 'bg-amber-100 text-amber-700'
                       : 'bg-stone-100 text-stone-500'}">
          {detail.medication.status.charAt(0).toUpperCase() + detail.medication.status.slice(1)}
        </span>
        <span aria-hidden="true">·</span>
        <span>{formatRoute(detail.medication.route)}</span>
        <span aria-hidden="true">·</span>
        <span>{detail.medication.frequency}</span>
      </div>

      {#if detail.medication.prescriber_name}
        <p class="text-sm text-stone-500 mt-2">
          Prescribed by {detail.medication.prescriber_name}
          {#if detail.medication.prescriber_specialty}
            ({detail.medication.prescriber_specialty})
          {/if}
        </p>
      {:else if detail.medication.is_otc}
        <p class="text-sm text-stone-500 mt-2">Over-the-counter (self-reported)</p>
      {/if}

      {#if detail.medication.start_date}
        <p class="text-sm text-stone-400 mt-1">
          Started {formatDate(detail.medication.start_date)}
          {#if detail.medication.end_date}
            · Ended {formatDate(detail.medication.end_date)}
          {/if}
        </p>
      {/if}

      {#if detail.medication.reason_start}
        <p class="text-sm text-stone-600 mt-2 italic">
          Reason: {detail.medication.reason_start}
        </p>
      {/if}

      {#if detail.medication.condition}
        <p class="text-sm text-stone-600 mt-1 italic">
          {detail.medication.condition}
        </p>
      {/if}
    </section>

    <!-- Coherence alerts -->
    {#if detail.medication.coherence_alerts.length > 0}
      <section class="px-6 py-2">
        {#each detail.medication.coherence_alerts as alert}
          <div
            class="px-4 py-3 rounded-xl mb-2 text-sm
                   {alert.severity === 'Critical'
                     ? 'bg-amber-50 text-amber-800 border border-amber-200'
                     : alert.severity === 'Warning'
                       ? 'bg-blue-50 text-blue-700 border border-blue-100'
                       : 'bg-stone-50 text-stone-600 border border-stone-100'}"
            role="status"
          >
            {alert.summary}
          </div>
        {/each}
      </section>
    {/if}

    <!-- Instructions -->
    {#if detail.instructions.length > 0 || detail.medication.administration_instructions}
      <section class="px-6 py-4 border-t border-stone-100">
        <h3 class="text-sm font-medium text-stone-500 mb-2">Instructions</h3>
        <ul class="flex flex-col gap-2">
          {#if detail.medication.administration_instructions}
            <li class="flex items-start gap-2 text-sm text-stone-700">
              <span class="text-stone-400 mt-0.5" aria-hidden="true">&#x2022;</span>
              <span>{detail.medication.administration_instructions}</span>
            </li>
          {/if}
          {#each detail.instructions as instr}
            <li class="flex items-start gap-2 text-sm text-stone-700">
              <span class="text-stone-400 mt-0.5" aria-hidden="true">&#x2022;</span>
              <span>
                {instr.instruction}
                {#if instr.timing}
                  <span class="text-stone-400">({instr.timing})</span>
                {/if}
              </span>
            </li>
          {/each}
        </ul>
      </section>
    {/if}

    <!-- Brand/Generic aliases -->
    {#if detail.aliases.length > 0}
      <section class="px-6 py-4 border-t border-stone-100">
        <h3 class="text-sm font-medium text-stone-500 mb-2">Known names</h3>
        <p class="text-sm text-stone-600">
          <span class="font-medium">Generic:</span> {detail.medication.generic_name}
        </p>
        <p class="text-sm text-stone-600 mt-1">
          <span class="font-medium">Brand names:</span>
          {detail.aliases.map(a => a.brand_name).join(', ')}
        </p>
      </section>
    {/if}

    <!-- Compound ingredients -->
    {#if detail.compound_ingredients.length > 0}
      <section class="px-6 py-4 border-t border-stone-100">
        <CompoundIngredients ingredients={detail.compound_ingredients} />
      </section>
    {/if}

    <!-- Tapering schedule -->
    {#if detail.tapering_steps.length > 0}
      <section class="px-6 py-4 border-t border-stone-100">
        <TaperingSchedule steps={detail.tapering_steps} />
      </section>
    {/if}

    <!-- Dose change history link -->
    {#if detail.dose_changes.length > 0}
      <section class="px-6 py-4 border-t border-stone-100">
        {#if showDoseHistory}
          <DoseHistory
            changes={detail.dose_changes}
            medicationName={detail.medication.generic_name}
            onClose={() => { showDoseHistory = false; }}
          />
        {:else}
          <button
            class="w-full text-left text-sm text-[var(--color-primary)] font-medium
                   min-h-[44px] flex items-center gap-2"
            onclick={() => { showDoseHistory = true; }}
          >
            View dose change history ({detail.dose_changes.length} change{detail.dose_changes.length === 1 ? '' : 's'})
          </button>
        {/if}
      </section>
    {/if}

    <!-- Source document -->
    {#if detail.document_title}
      <section class="px-6 py-4 border-t border-stone-100">
        <h3 class="text-sm font-medium text-stone-500 mb-2">Source document</h3>
        <p class="text-sm text-stone-600">
          {detail.document_title}
          {#if detail.document_date}
            · {formatDate(detail.document_date)}
          {/if}
        </p>
      </section>
    {/if}
  {/if}
</div>
```

### DoseHistory

```svelte
<!-- src/lib/components/medications/DoseHistory.svelte -->
<script lang="ts">
  import type { DoseChangeView } from '$lib/types/medication';

  interface Props {
    changes: DoseChangeView[];
    medicationName: string;
    onClose: () => void;
  }
  let { changes, medicationName, onClose }: Props = $props();

  function formatDate(dateStr: string): string {
    return new Date(dateStr).toLocaleDateString('en-US', {
      month: 'short', day: 'numeric', year: 'numeric',
    });
  }
</script>

<div>
  <div class="flex items-center justify-between mb-3">
    <h3 class="text-sm font-medium text-stone-500">
      Dose history for {medicationName}
    </h3>
    <button
      class="text-xs text-stone-400 hover:text-stone-600 min-h-[44px] min-w-[44px]
             flex items-center justify-center"
      onclick={onClose}
      aria-label="Close dose history"
    >
      Hide
    </button>
  </div>

  <!-- Timeline -->
  <div class="relative pl-6">
    <!-- Vertical line -->
    <div class="absolute left-[7px] top-2 bottom-2 w-0.5 bg-stone-200" aria-hidden="true"></div>

    {#each changes as change, i}
      <div class="relative pb-6 last:pb-0">
        <!-- Circle marker -->
        <div
          class="absolute left-[-17px] top-1 w-3 h-3 rounded-full border-2
                 {i === changes.length - 1
                   ? 'bg-[var(--color-primary)] border-[var(--color-primary)]'
                   : 'bg-white border-stone-400'}"
          aria-hidden="true"
        ></div>

        <!-- Content -->
        <div>
          <p class="text-sm font-medium text-stone-700">
            {formatDate(change.change_date)}
          </p>
          <p class="text-sm text-stone-800 mt-0.5">
            {#if change.old_dose}
              {change.old_dose} &rarr; {change.new_dose}
            {:else}
              Started at {change.new_dose}
            {/if}
          </p>
          {#if change.old_frequency && change.new_frequency}
            <p class="text-xs text-stone-600 mt-0.5">
              {change.old_frequency} &rarr; {change.new_frequency}
            </p>
          {/if}
          {#if change.changed_by_name}
            <p class="text-xs text-stone-400 mt-0.5">
              {change.changed_by_name}
            </p>
          {/if}
          {#if change.reason}
            <p class="text-xs text-stone-500 italic mt-0.5">
              "{change.reason}"
            </p>
          {/if}
          {#if change.document_title}
            <p class="text-xs text-stone-400 mt-0.5">
              Source: {change.document_title}
            </p>
          {/if}
        </div>
      </div>
    {/each}
  </div>
</div>
```

### OtcEntryForm

```svelte
<!-- src/lib/components/medications/OtcEntryForm.svelte -->
<script lang="ts">
  import { addOtcMedication, searchMedicationAlias } from '$lib/api/medications';
  import type { AliasSearchResult } from '$lib/types/medication';

  interface Props {
    onBack: () => void;
    onAdded: () => void;
  }
  let { onBack, onAdded }: Props = $props();

  let name = $state('');
  let dose = $state('');
  let frequency = $state('');
  let route = $state('oral');
  let reason = $state('');
  let startDate = $state('');
  let instructions = $state('');
  let loading = $state(false);
  let error = $state('');

  // Autocomplete state
  let suggestions = $state<AliasSearchResult[]>([]);
  let showSuggestions = $state(false);
  let searchTimeout: ReturnType<typeof setTimeout> | null = $state(null);

  function handleNameInput(value: string) {
    name = value;
    if (searchTimeout) clearTimeout(searchTimeout);
    if (value.trim().length >= 2) {
      searchTimeout = setTimeout(async () => {
        try {
          suggestions = await searchMedicationAlias(value.trim(), 8);
          showSuggestions = suggestions.length > 0;
        } catch {
          suggestions = [];
          showSuggestions = false;
        }
      }, 200);
    } else {
      suggestions = [];
      showSuggestions = false;
    }
  }

  function selectSuggestion(result: AliasSearchResult) {
    name = result.generic_name;
    showSuggestions = false;
    suggestions = [];
  }

  async function handleSubmit() {
    error = '';

    if (!name.trim()) { error = 'Please enter a medication name.'; return; }
    if (!dose.trim()) { error = 'Please enter a dose.'; return; }
    if (!frequency.trim()) { error = 'Please enter how often you take it.'; return; }

    loading = true;
    try {
      await addOtcMedication({
        name: name.trim(),
        dose: dose.trim(),
        frequency: frequency.trim(),
        route,
        reason: reason.trim() || null,
        start_date: startDate || null,
        instructions: instructions.trim() || null,
      });
      onAdded();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  const routeOptions = [
    { value: 'oral', label: 'Oral' },
    { value: 'topical', label: 'Topical' },
    { value: 'other', label: 'Other' },
  ];
</script>

<div class="flex flex-col min-h-screen pb-20 bg-stone-50">
  <header class="px-6 pt-4 pb-2">
    <button
      class="text-stone-400 hover:text-stone-600 min-h-[44px] min-w-[44px]"
      onclick={onBack}
      aria-label="Back to medication list"
    >
      &larr; Back
    </button>
  </header>

  <div class="px-6 py-4">
    <h2 class="text-xl font-bold text-stone-800 mb-6">
      Add an over-the-counter medication
    </h2>

    <div class="flex flex-col gap-5">
      <!-- Medication name with autocomplete -->
      <label class="flex flex-col gap-1 relative">
        <span class="text-stone-600 text-sm font-medium">
          Medication name <span class="text-red-500">*</span>
        </span>
        <input
          type="text"
          value={name}
          oninput={(e) => handleNameInput(e.currentTarget.value)}
          onfocus={() => { if (suggestions.length > 0) showSuggestions = true; }}
          onblur={() => { setTimeout(() => { showSuggestions = false; }, 200); }}
          placeholder="Ibuprofen"
          class="px-4 py-3 rounded-lg border border-stone-300 text-base min-h-[44px]
                 focus:border-[var(--color-primary)] focus:outline-none"
          autocomplete="off"
          aria-label="Medication name"
          aria-autocomplete="list"
        />
        <span class="text-xs text-stone-400">Start typing to search known medications</span>

        <!-- Autocomplete dropdown -->
        {#if showSuggestions}
          <div
            class="absolute top-full left-0 right-0 mt-1 bg-white rounded-lg shadow-lg
                   border border-stone-200 max-h-[200px] overflow-y-auto z-10"
            role="listbox"
          >
            {#each suggestions as result}
              <button
                class="w-full text-left px-4 py-3 hover:bg-stone-50 text-sm
                       min-h-[44px] border-b border-stone-50 last:border-0"
                role="option"
                onmousedown={() => selectSuggestion(result)}
              >
                <span class="font-medium text-stone-800">{result.generic_name}</span>
                {#if result.brand_names.length > 0}
                  <span class="text-stone-400 ml-1">
                    ({result.brand_names.slice(0, 3).join(', ')})
                  </span>
                {/if}
              </button>
            {/each}
          </div>
        {/if}
      </label>

      <!-- Dose -->
      <label class="flex flex-col gap-1">
        <span class="text-stone-600 text-sm font-medium">
          Dose <span class="text-red-500">*</span>
        </span>
        <input
          type="text"
          bind:value={dose}
          placeholder="400mg"
          class="px-4 py-3 rounded-lg border border-stone-300 text-base min-h-[44px]
                 focus:border-[var(--color-primary)] focus:outline-none"
        />
      </label>

      <!-- Frequency -->
      <label class="flex flex-col gap-1">
        <span class="text-stone-600 text-sm font-medium">
          How often? <span class="text-red-500">*</span>
        </span>
        <input
          type="text"
          bind:value={frequency}
          placeholder="As needed for pain"
          class="px-4 py-3 rounded-lg border border-stone-300 text-base min-h-[44px]
                 focus:border-[var(--color-primary)] focus:outline-none"
        />
      </label>

      <!-- Route -->
      <fieldset class="flex flex-col gap-1">
        <legend class="text-stone-600 text-sm font-medium">How do you take it?</legend>
        <div class="flex gap-3 mt-1">
          {#each routeOptions as option}
            <label
              class="flex items-center justify-center px-4 py-2 rounded-lg border
                     min-h-[44px] cursor-pointer transition-colors
                     {route === option.value
                       ? 'border-[var(--color-primary)] bg-blue-50 text-[var(--color-primary)]'
                       : 'border-stone-200 bg-white text-stone-600 hover:bg-stone-50'}"
            >
              <input
                type="radio"
                name="route"
                value={option.value}
                bind:group={route}
                class="sr-only"
              />
              <span class="text-sm font-medium">{option.label}</span>
            </label>
          {/each}
        </div>
      </fieldset>

      <!-- Reason -->
      <label class="flex flex-col gap-1">
        <span class="text-stone-600 text-sm font-medium">Why are you taking it?</span>
        <input
          type="text"
          bind:value={reason}
          placeholder="Headaches and muscle pain"
          class="px-4 py-3 rounded-lg border border-stone-300 text-base min-h-[44px]
                 focus:border-[var(--color-primary)] focus:outline-none"
        />
      </label>

      <!-- Start date -->
      <label class="flex flex-col gap-1">
        <span class="text-stone-600 text-sm font-medium">When did you start?</span>
        <input
          type="date"
          bind:value={startDate}
          max={new Date().toISOString().split('T')[0]}
          class="px-4 py-3 rounded-lg border border-stone-300 text-base min-h-[44px]
                 focus:border-[var(--color-primary)] focus:outline-none"
        />
      </label>

      <!-- Instructions -->
      <label class="flex flex-col gap-1">
        <span class="text-stone-600 text-sm font-medium">Special instructions</span>
        <textarea
          bind:value={instructions}
          placeholder="Take with food"
          rows="2"
          class="px-4 py-3 rounded-lg border border-stone-300 text-base min-h-[44px]
                 focus:border-[var(--color-primary)] focus:outline-none resize-none"
        ></textarea>
      </label>

      {#if error}
        <p class="text-red-600 text-sm" role="alert">{error}</p>
      {/if}

      <button
        class="mt-2 px-8 py-4 bg-[var(--color-primary)] text-white rounded-xl text-lg
               font-medium hover:brightness-110 disabled:opacity-50 min-h-[44px]"
        onclick={handleSubmit}
        disabled={loading || !name.trim() || !dose.trim() || !frequency.trim()}
      >
        {loading ? 'Adding...' : 'Add medication'}
      </button>
    </div>
  </div>
</div>
```

### MedicationSearch

```svelte
<!-- src/lib/components/medications/MedicationSearch.svelte -->
<script lang="ts">
  import type { PrescriberOption } from '$lib/types/medication';

  interface Props {
    value: string;
    onInput: (value: string) => void;
    prescribers: PrescriberOption[];
    selectedPrescriber: string | null;
    onPrescriberChange: (id: string | null) => void;
  }
  let { value, onInput, prescribers, selectedPrescriber, onPrescriberChange }: Props = $props();
</script>

<div class="px-6 py-2 flex gap-3 items-center">
  <!-- Search input -->
  <div class="flex-1 relative">
    <input
      type="text"
      value={value}
      oninput={(e) => onInput(e.currentTarget.value)}
      placeholder="Search medications..."
      class="w-full px-4 py-2.5 pl-10 rounded-lg border border-stone-200 bg-white
             text-sm min-h-[44px]
             focus:border-[var(--color-primary)] focus:outline-none"
      aria-label="Search medications"
    />
    <!-- Magnifying glass icon placeholder -->
    <span class="absolute left-3 top-1/2 -translate-y-1/2 text-stone-400 text-sm"
          aria-hidden="true">
      &#x1F50D;
    </span>
  </div>

  <!-- Prescriber filter dropdown -->
  {#if prescribers.length > 0}
    <select
      class="px-3 py-2.5 rounded-lg border border-stone-200 bg-white text-sm
             min-h-[44px] text-stone-600
             focus:border-[var(--color-primary)] focus:outline-none"
      value={selectedPrescriber ?? ''}
      onchange={(e) => {
        const val = e.currentTarget.value;
        onPrescriberChange(val === '' ? null : val);
      }}
      aria-label="Filter by prescriber"
    >
      <option value="">All prescribers</option>
      {#each prescribers as prescriber}
        <option value={prescriber.id}>
          {prescriber.name} ({prescriber.medication_count})
        </option>
      {/each}
    </select>
  {/if}
</div>
```

### TaperingSchedule

```svelte
<!-- src/lib/components/medications/TaperingSchedule.svelte -->
<script lang="ts">
  import type { TaperingStepView } from '$lib/types/medication';

  interface Props {
    steps: TaperingStepView[];
  }
  let { steps }: Props = $props();

  function formatDateRange(step: TaperingStepView): string {
    if (!step.start_date) return `${step.duration_days} days`;
    const start = new Date(step.start_date);
    const end = new Date(start.getTime() + step.duration_days * 86400000);
    const fmt = (d: Date) => d.toLocaleDateString('en-US', { month: 'short', day: 'numeric' });
    return `${fmt(start)} - ${fmt(end)}`;
  }
</script>

<div>
  <h3 class="text-sm font-medium text-stone-500 mb-3">Tapering schedule</h3>
  <div class="flex flex-col gap-2">
    {#each steps as step}
      <div
        class="flex items-center gap-3 px-3 py-2 rounded-lg
               {step.is_current
                 ? 'bg-blue-50 border border-blue-200'
                 : 'bg-white border border-stone-100'}"
      >
        <span class="text-xs text-stone-400 w-12 flex-shrink-0">
          Step {step.step_number}
        </span>
        <div class="flex-1">
          <p class="text-sm font-medium text-stone-800">
            {step.dose}
            <span class="text-stone-500 font-normal">for {step.duration_days} days</span>
          </p>
          <p class="text-xs text-stone-400">{formatDateRange(step)}</p>
        </div>
        {#if step.is_current}
          <span class="text-xs text-blue-600 font-medium flex-shrink-0">Current</span>
        {/if}
      </div>
    {/each}
  </div>
</div>
```

### CompoundIngredients

```svelte
<!-- src/lib/components/medications/CompoundIngredients.svelte -->
<script lang="ts">
  import type { CompoundIngredientView } from '$lib/types/medication';

  interface Props {
    ingredients: CompoundIngredientView[];
  }
  let { ingredients }: Props = $props();
</script>

<div>
  <h3 class="text-sm font-medium text-stone-500 mb-3">Compound ingredients</h3>
  <div class="flex flex-col gap-2">
    {#each ingredients as ingredient}
      <div class="flex items-baseline gap-2 px-3 py-2 bg-white rounded-lg border border-stone-100">
        <span class="text-sm font-medium text-stone-800">
          {ingredient.ingredient_name}
        </span>
        {#if ingredient.ingredient_dose}
          <span class="text-sm text-stone-600">{ingredient.ingredient_dose}</span>
        {/if}
        {#if ingredient.maps_to_generic}
          <span class="text-xs text-stone-400">(also known as {ingredient.maps_to_generic})</span>
        {/if}
      </div>
    {/each}
  </div>
</div>
```

### EmptyMedicationState

```svelte
<!-- src/lib/components/medications/EmptyMedicationState.svelte -->
<script lang="ts">
  interface Props {
    onAddOtc: () => void;
  }
  let { onAddOtc }: Props = $props();
</script>

<div class="flex flex-col items-center justify-center px-8 py-12 text-center">
  <!-- Simple pill illustration placeholder -->
  <div class="w-24 h-24 bg-stone-100 rounded-2xl flex items-center justify-center mb-6">
    <span class="text-4xl text-stone-300" aria-hidden="true">&#x1F48A;</span>
  </div>

  <h2 class="text-lg font-medium text-stone-700 mb-2">
    No medications recorded yet
  </h2>
  <p class="text-sm text-stone-500 mb-6 max-w-[300px]">
    Medications will appear here when you load a prescription or add an over-the-counter medication.
  </p>

  <button
    class="px-8 py-4 bg-[var(--color-primary)] text-white rounded-xl text-base font-medium
           hover:brightness-110 focus-visible:outline focus-visible:outline-2
           focus-visible:outline-offset-2 focus-visible:outline-[var(--color-primary)]
           min-h-[44px]"
    onclick={onAddOtc}
  >
    + Add an over-the-counter medication
  </button>
</div>
```

---

## [12] Frontend API

```typescript
// src/lib/api/medications.ts
import { invoke } from '@tauri-apps/api/core';
import type {
  MedicationListData,
  MedicationDetail,
  MedicationListFilter,
  DoseChangeView,
  AliasSearchResult,
  OtcMedicationInput,
} from '$lib/types/medication';

export async function getMedications(
  filter: MedicationListFilter,
): Promise<MedicationListData> {
  return invoke<MedicationListData>('get_medications', { filter });
}

export async function getMedicationDetail(
  medicationId: string,
): Promise<MedicationDetail> {
  return invoke<MedicationDetail>('get_medication_detail', { medicationId });
}

export async function addOtcMedication(
  input: OtcMedicationInput,
): Promise<string> {
  return invoke<string>('add_otc_medication', { input });
}

export async function getDoseHistory(
  medicationId: string,
): Promise<DoseChangeView[]> {
  return invoke<DoseChangeView[]>('get_dose_history', { medicationId });
}

export async function searchMedicationAlias(
  query: string,
  limit?: number,
): Promise<AliasSearchResult[]> {
  return invoke<AliasSearchResult[]>('search_medication_alias', { query, limit });
}
```

---

## [13] Error Handling

User-facing error messages follow the calm design language:

| Error | User Message | Recovery |
|-------|-------------|----------|
| Database query fails | "Something went wrong loading your medications. Please try again." | Retry button on screen |
| Session expired | Redirected to profile unlock (ProfileGuard handles) | Re-enter password |
| Medication not found | "This medication could not be found." | Navigate back to list |
| OTC add fails (validation) | Inline field-specific error (e.g., "Please enter a medication name.") | Fix field, retry |
| OTC add fails (database) | "Couldn't save the medication. Please try again." | Retry submit |
| Alias search fails | Silently fail, no suggestions shown | User types custom name |
| Dose history empty | "No dose changes recorded for this medication." | Informational, no action needed |

All errors logged via `tracing::warn!` or `tracing::error!`. No medication names, doses, or patient data in error messages sent to logs.

---

## [14] Security

| Concern | Mitigation |
|---------|-----------|
| Medication names in memory | Decrypted only for display. Svelte state cleared on component unmount (garbage collected). |
| OTC input sanitization | Trimmed and validated before insertion. Parameterized SQL only (no string concatenation). |
| Search query injection | Alias search uses LIKE with parameterized queries. Pattern wildcards are safe within LIKE context. |
| Activity timestamp | Updated on every Tauri command (prevents false timeouts while browsing medications). |
| Medication data in frontend logs | NEVER log medication names, doses, or patient data. Only log medication IDs and operation types. |
| Coherence alerts exposure | Alert summaries are patient-facing text only. No internal IDs or technical details exposed to frontend. |

---

## [15] Testing

### Unit Tests (Rust)

| # | Test | Expected |
|---|------|----------|
| `test_fetch_medications_empty` | Returns empty vec when no medications exist |
| `test_fetch_medications_active_only` | Status filter "active" returns only active medications |
| `test_fetch_medications_paused_only` | Status filter "paused" returns only paused medications |
| `test_fetch_medications_stopped_only` | Status filter "stopped" returns only stopped medications |
| `test_fetch_medications_by_prescriber` | Prescriber filter returns only that prescriber's medications |
| `test_fetch_medications_search_generic` | Search "metformin" matches generic_name |
| `test_fetch_medications_search_brand` | Search "glucophage" matches brand_name |
| `test_fetch_medications_search_condition` | Search "diabetes" matches condition field |
| `test_fetch_medications_sort_order` | Active first, then paused, then stopped; within group by start_date DESC |
| `test_fetch_medications_otc_excluded` | include_otc=false excludes OTC medications |
| `test_fetch_medications_otc_included` | include_otc=true includes OTC medications |
| `test_fetch_single_medication` | Returns full card with prescriber join |
| `test_fetch_single_medication_not_found` | Returns None for nonexistent ID |
| `test_fetch_medication_detail_instructions` | Instructions loaded from medication_instructions table |
| `test_fetch_medication_detail_compound` | Compound ingredients loaded for compound medication |
| `test_fetch_medication_detail_tapering` | Tapering steps loaded with current step computed |
| `test_fetch_medication_detail_aliases` | Aliases loaded from medication_aliases table |
| `test_fetch_dose_history_chronological` | Dose changes returned in chronological order |
| `test_fetch_dose_history_with_prescriber` | Changed_by_name populated from professionals join |
| `test_fetch_dose_history_empty` | Returns empty vec when no dose changes exist |
| `test_add_otc_medication_success` | OTC medication inserted with is_otc=true, status=active |
| `test_add_otc_medication_empty_name_rejected` | Empty name returns validation error |
| `test_add_otc_medication_empty_dose_rejected` | Empty dose returns validation error |
| `test_add_otc_medication_creates_patient_document` | Synthetic document created if not exists |
| `test_add_otc_medication_reuses_patient_document` | Existing synthetic document reused |
| `test_add_otc_medication_with_instructions` | Instructions stored in medication_instructions table |
| `test_search_aliases_generic_match` | "metformin" matches generic_name entries |
| `test_search_aliases_brand_match` | "glucophage" matches brand_name entries |
| `test_search_aliases_case_insensitive` | "METFORMIN" matches "metformin" entries |
| `test_search_aliases_limit` | Limit parameter caps results |
| `test_status_counts` | Correct counts for active, paused, stopped |
| `test_prescriber_options` | Prescribers returned with correct medication counts |
| `test_tapering_current_step_computation` | Current step correctly identified by date range |
| `test_medication_alerts_fetched` | Coherence alerts with matching entity_ids returned |
| `test_medication_alerts_dismissed_excluded` | Dismissed alerts not included |

### Frontend Tests

| # | Test | Expected |
|---|------|----------|
| `test_medication_list_renders_cards` | Medication cards rendered with correct data |
| `test_medication_list_empty_state` | Empty state shown when no medications |
| `test_medication_card_dose_prominent` | Dose text is large and visually prominent |
| `test_medication_card_status_badge` | Correct color badge for each status |
| `test_medication_card_otc_label` | OTC medications show "OTC" instead of prescriber |
| `test_medication_card_compound_indicator` | "Compound" tag shown for compound medications |
| `test_medication_card_tapering_indicator` | "Tapering" tag shown for tapering medications |
| `test_medication_card_coherence_alert` | Alert bar visible when coherence_alerts present |
| `test_status_tab_filtering` | Clicking status tab filters medications |
| `test_prescriber_dropdown_filtering` | Selecting prescriber filters medications |
| `test_search_debounced` | Search input debounces at 300ms |
| `test_medication_detail_renders` | Detail view shows all medication fields |
| `test_medication_detail_instructions` | Instructions section visible when instructions exist |
| `test_medication_detail_compound_section` | Compound ingredients section visible for compound medications |
| `test_medication_detail_tapering_section` | Tapering schedule visible for tapering medications |
| `test_medication_detail_aliases_section` | Known names section visible when aliases exist |
| `test_dose_history_timeline` | Timeline renders chronological entries |
| `test_dose_history_current_highlighted` | Last entry has primary color marker |
| `test_otc_form_validates_required` | Submit disabled until required fields filled |
| `test_otc_form_autocomplete_shows` | Suggestions dropdown appears after 2+ chars |
| `test_otc_form_autocomplete_selects` | Selecting suggestion fills name field |
| `test_otc_form_submit_success` | Successful submit navigates back with confirmation |
| `test_otc_form_submit_error` | Error displayed inline on submit failure |

---

## [16] Performance

| Metric | Target |
|--------|--------|
| Medication list load (100 medications) | < 100ms |
| Single medication detail load | < 50ms |
| Dose history load (50 changes) | < 30ms |
| Alias search (autocomplete) | < 30ms |
| OTC medication insert | < 20ms |
| Filter change (re-query) | < 100ms |
| Search debounce | 300ms delay before query |

**Notes:**
- Single `get_medications` call on mount -- no waterfall. Status counts included.
- Prescriber options included in initial fetch to avoid separate round-trip.
- Medication detail is a separate call (only when card tapped) to avoid loading heavy data for all medications.
- Alias search is lightweight (indexed query on medication_aliases table).
- All SQLite queries use indexed columns (see L0-02 schema indexes: `idx_medications_generic`, `idx_medications_status`, `idx_dose_changes_medication`, `idx_compound_medication`, `idx_tapering_medication`, `idx_aliases_brand`).

---

## [17] Open Questions

| # | Question | Status |
|---|---------|--------|
| OQ-01 | Should stopped medications be hidden by default or shown with a visual de-emphasis? | Show all by default with visual de-emphasis (gray text, lower opacity). User can filter to active-only via status tab. |
| OQ-02 | Should we support medication photo (e.g., pill image) for identification? | Deferred to Phase 2. Text-only for Phase 1. |
| OQ-03 | Should OTC medication entry trigger coherence engine for conflict detection? | Yes, Phase 1. After OTC insert, emit event for coherence engine to run checks. Captured as future integration point. |
| OQ-04 | Should we allow patients to mark a prescribed medication as stopped? | Deferred to Phase 2. Currently only the extraction pipeline sets medication status. Patient-initiated status changes require careful UX design. |
| OQ-05 | Should medication_aliases be pre-populated with a bundled database (e.g., RxNorm subset)? | Yes, bundled with installer. Source field distinguishes "bundled" from "user_added". Exact dataset TBD. |

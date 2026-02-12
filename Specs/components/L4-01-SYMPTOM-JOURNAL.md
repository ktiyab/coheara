# L4-01 — Symptom Journal

<!--
=============================================================================
COMPONENT SPEC — The patient's voice. Where Marie records how she feels.
Engineer review: E-UX (UI/UX, lead), E-DA (Data), E-RS (Rust), E-ML (AI/ML), E-QA (QA)
This is the only place in Coheara where the patient CREATES data (not imports it).
It must be effortless. Marie records in 30 seconds or skips forever.
The OLDCARTS clinical framework is embedded invisibly — Marie sees friendly questions.
=============================================================================
-->

## Table of Contents

| Section | Offset |
|---------|--------|
| [1] Identity | `offset=22 limit=35` |
| [2] Dependencies | `offset=57 limit=22` |
| [3] Interfaces | `offset=79 limit=85` |
| [4] Guided Recording Flow | `offset=164 limit=90` |
| [5] Body Map | `offset=254 limit=60` |
| [6] Severity Face Scale | `offset=314 limit=40` |
| [7] Temporal Correlation | `offset=354 limit=55` |
| [8] Check-In Nudges | `offset=409 limit=50` |
| [9] Symptom History View | `offset=459 limit=60` |
| [10] Tauri Commands (IPC) | `offset=519 limit=65` |
| [11] Svelte Components | `offset=584 limit=150` |
| [12] Error Handling | `offset=734 limit=25` |
| [13] Security | `offset=759 limit=20` |
| [14] Testing | `offset=779 limit=55` |
| [15] Performance | `offset=834 limit=15` |
| [16] Open Questions | `offset=849 limit=10` |

---

## [1] Identity

**What:** The symptom journal — where patients record how they feel using the OLDCARTS clinical framework adapted into friendly, non-clinical language. Includes: guided multi-step recording flow (category → specific → severity → when → expanded details → notes), visual body map for location, face-based severity scale, temporal correlation detection (linking symptoms to recent medication changes), configurable check-in nudges, and a symptom history view with filtering.

**After this session:**
- Patient opens Journal tab → "How are you feeling today?" guided flow
- Step 1: Category selector (8 categories with sub-selectors)
- Step 2: Severity via visual face scale (1-5, no numbers shown)
- Step 3: When it started (date picker, default today)
- Step 4: Expanded details (body map, duration, character, aggravating, relieving, timing) — progressive disclosure via "Tell me more" button
- Step 5: Free-text notes
- Save → stored in `symptoms` table + embedded in LanceDB for semantic search
- Temporal correlation auto-detection: if medication change within 14 days, show calm note
- Daily check-in nudge after 3 days of no entries (if active symptoms exist)
- Post-medication-change nudge when new medication detected
- Symptom history list with filter by category, severity, date range, active/resolved
- Resolve symptom flow (mark as no longer active)

**Estimated complexity:** Medium
**Source:** Tech Spec v1.1 Section 9.4 (Guided Symptom Recording), Section 5 (symptoms table)

---

## [2] Dependencies

**Incoming:**
- L0-02 (data model — symptoms table, OLDCARTS fields)
- L0-03 (encryption — ProfileSession for field-level encryption)
- L1-04 (storage pipeline — embedding model for symptom text embedding into LanceDB)
- L3-01 (profile management — active session required)

**Outgoing:**
- L2-01 (RAG pipeline — symptom entries searchable via semantic search)
- L2-03 (coherence engine — temporal correlation detection)
- L4-02 (appointment prep — symptom data included in prep summaries)
- L4-04 (timeline view — symptom events displayed on timeline)

**No new Cargo.toml dependencies.** Uses existing embedding model from L1-04, repository traits from L0-02.

---

## [3] Interfaces

### Backend Types

```rust
// src-tauri/src/journal.rs

use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Categories for symptom recording (OLDCARTS: Onset → Category)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SymptomCategory {
    Pain,
    Digestive,
    Respiratory,
    Neurological,
    General,
    Mood,
    Skin,
    Other,
}

/// Sub-categories per main category
impl SymptomCategory {
    pub fn subcategories(&self) -> Vec<&'static str> {
        match self {
            Self::Pain => vec![
                "Headache", "Back pain", "Joint pain", "Chest pain",
                "Abdominal pain", "Muscle pain", "Neck pain", "Other",
            ],
            Self::Digestive => vec![
                "Nausea", "Vomiting", "Diarrhea", "Constipation",
                "Bloating", "Heartburn", "Loss of appetite", "Other",
            ],
            Self::Respiratory => vec![
                "Shortness of breath", "Cough", "Wheezing",
                "Chest tightness", "Sore throat", "Congestion", "Other",
            ],
            Self::Neurological => vec![
                "Dizziness", "Numbness", "Tingling", "Tremor",
                "Memory issues", "Confusion", "Other",
            ],
            Self::General => vec![
                "Fatigue", "Fever", "Chills", "Weight change",
                "Night sweats", "Swelling", "Other",
            ],
            Self::Mood => vec![
                "Anxiety", "Low mood", "Irritability", "Sleep difficulty",
                "Difficulty concentrating", "Other",
            ],
            Self::Skin => vec![
                "Rash", "Itching", "Bruising", "Dryness",
                "Swelling", "Color change", "Other",
            ],
            Self::Other => vec!["Other"],
        }
    }
}

/// Duration options (OLDCARTS: Duration)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SymptomDuration {
    Constant,
    Minutes,
    Hours,
    Days,
}

/// Character options (OLDCARTS: Character)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SymptomCharacter {
    Sharp,
    Dull,
    Burning,
    Pressure,
    Throbbing,
}

/// Timing pattern (OLDCARTS: Timing)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TimingPattern {
    Morning,
    Night,
    AfterMeals,
    Random,
    AllTheTime,
}

/// Complete symptom entry for recording
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymptomEntry {
    pub category: SymptomCategory,
    pub specific: String,             // Sub-category or custom text
    pub severity: u8,                 // 1-5 (face scale maps to this)
    pub onset_date: NaiveDate,
    pub onset_time: Option<NaiveTime>,

    // Expanded fields (optional — progressive disclosure)
    pub body_region: Option<String>,  // From body map: "head", "chest_left", etc.
    pub duration: Option<SymptomDuration>,
    pub character: Option<SymptomCharacter>,
    pub aggravating: Vec<String>,     // Multiple factors
    pub relieving: Vec<String>,       // Multiple factors
    pub timing_pattern: Option<TimingPattern>,
    pub notes: Option<String>,        // Free text
}

/// Stored symptom with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredSymptom {
    pub id: Uuid,
    pub category: String,
    pub specific: String,
    pub severity: u8,
    pub body_region: Option<String>,
    pub duration: Option<String>,
    pub character: Option<String>,
    pub aggravating: Option<String>,
    pub relieving: Option<String>,
    pub timing_pattern: Option<String>,
    pub onset_date: NaiveDate,
    pub onset_time: Option<NaiveTime>,
    pub recorded_date: NaiveDateTime,
    pub still_active: bool,
    pub resolved_date: Option<NaiveDate>,
    pub related_medication_name: Option<String>,  // Joined from medications
    pub related_diagnosis_name: Option<String>,   // Joined from diagnoses
    pub notes: Option<String>,
    pub source: String,  // "patient_reported", "guided_checkin", "free_text"
}

/// Temporal correlation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalCorrelation {
    pub medication_name: String,
    pub medication_start_date: NaiveDate,
    pub days_since_change: i64,
    pub message: String,  // Patient-facing calm text
}

/// Check-in nudge decision
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NudgeDecision {
    pub should_nudge: bool,
    pub nudge_type: Option<NudgeType>,
    pub message: Option<String>,
    pub related_medication: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NudgeType {
    DailyCheckIn,           // 3 days no entry + active symptoms
    PostMedicationChange,   // New medication detected recently
}

/// Symptom history filter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymptomFilter {
    pub category: Option<String>,
    pub severity_min: Option<u8>,
    pub severity_max: Option<u8>,
    pub date_from: Option<NaiveDate>,
    pub date_to: Option<NaiveDate>,
    pub still_active: Option<bool>,
}

/// Body map regions
pub const BODY_REGIONS: &[&str] = &[
    "head", "face", "neck",
    "chest_left", "chest_right", "chest_center",
    "abdomen_upper", "abdomen_lower",
    "back_upper", "back_lower",
    "shoulder_left", "shoulder_right",
    "arm_left", "arm_right",
    "hand_left", "hand_right",
    "hip_left", "hip_right",
    "leg_left", "leg_right",
    "knee_left", "knee_right",
    "foot_left", "foot_right",
];
```

### Frontend Types

```typescript
// src/lib/types/journal.ts

export interface SymptomEntry {
  category: string;
  specific: string;
  severity: number;           // 1-5
  onset_date: string;         // YYYY-MM-DD
  onset_time: string | null;  // HH:MM
  body_region: string | null;
  duration: string | null;
  character: string | null;
  aggravating: string[];
  relieving: string[];
  timing_pattern: string | null;
  notes: string | null;
}

export interface StoredSymptom {
  id: string;
  category: string;
  specific: string;
  severity: number;
  body_region: string | null;
  duration: string | null;
  character: string | null;
  aggravating: string | null;
  relieving: string | null;
  timing_pattern: string | null;
  onset_date: string;
  onset_time: string | null;
  recorded_date: string;
  still_active: boolean;
  resolved_date: string | null;
  related_medication_name: string | null;
  related_diagnosis_name: string | null;
  notes: string | null;
  source: string;
}

export interface TemporalCorrelation {
  medication_name: string;
  medication_start_date: string;
  days_since_change: number;
  message: string;
}

export interface NudgeDecision {
  should_nudge: boolean;
  nudge_type: 'DailyCheckIn' | 'PostMedicationChange' | null;
  message: string | null;
  related_medication: string | null;
}

export interface SymptomFilter {
  category: string | null;
  severity_min: number | null;
  severity_max: number | null;
  date_from: string | null;
  date_to: string | null;
  still_active: boolean | null;
}

export const CATEGORIES = [
  'Pain', 'Digestive', 'Respiratory', 'Neurological',
  'General', 'Mood', 'Skin', 'Other',
] as const;

export const SUBCATEGORIES: Record<string, string[]> = {
  Pain: ['Headache', 'Back pain', 'Joint pain', 'Chest pain', 'Abdominal pain', 'Muscle pain', 'Neck pain', 'Other'],
  Digestive: ['Nausea', 'Vomiting', 'Diarrhea', 'Constipation', 'Bloating', 'Heartburn', 'Loss of appetite', 'Other'],
  Respiratory: ['Shortness of breath', 'Cough', 'Wheezing', 'Chest tightness', 'Sore throat', 'Congestion', 'Other'],
  Neurological: ['Dizziness', 'Numbness', 'Tingling', 'Tremor', 'Memory issues', 'Confusion', 'Other'],
  General: ['Fatigue', 'Fever', 'Chills', 'Weight change', 'Night sweats', 'Swelling', 'Other'],
  Mood: ['Anxiety', 'Low mood', 'Irritability', 'Sleep difficulty', 'Difficulty concentrating', 'Other'],
  Skin: ['Rash', 'Itching', 'Bruising', 'Dryness', 'Swelling', 'Color change', 'Other'],
  Other: ['Other'],
};
```

---

## [4] Guided Recording Flow

**E-UX lead:** This is a progressive disclosure flow. Marie sees 3 mandatory steps (what, severity, when) and an optional "Tell me more" expansion. The flow must complete in under 30 seconds for the basic case. Every step uses tappable cards — no typing required for the core flow.

### Flow State Machine

```
                ┌─────────────┐
                │   IDLE      │  (Journal tab or nudge tap)
                └──────┬──────┘
                       │
                       ▼
                ┌─────────────┐
                │ STEP 1:     │  Category → Subcategory
                │ WHAT        │  Tappable cards, two levels
                └──────┬──────┘
                       │
                       ▼
                ┌─────────────┐
                │ STEP 2:     │  Face scale 1-5
                │ SEVERITY    │  Tappable faces, no numbers
                └──────┬──────┘
                       │
                       ▼
                ┌─────────────┐
                │ STEP 3:     │  Date picker (default today)
                │ WHEN        │  Calendar widget
                └──────┬──────┘
                       │
                ┌──────┴──────┐
                │             │
           [Save]        [Tell me more]
                │             │
                ▼             ▼
         ┌──────────┐  ┌─────────────┐
         │ SAVING   │  │ STEP 4:     │
         │          │  │ EXPANDED    │
         └──────────┘  │ Body map    │
                       │ Duration    │
                       │ Character   │
                       │ Aggravating │
                       │ Relieving   │
                       │ Timing      │
                       └──────┬──────┘
                              │
                              ▼
                       ┌─────────────┐
                       │ STEP 5:     │  Free text (optional)
                       │ NOTES       │
                       └──────┬──────┘
                              │
                              ▼
                        [Save]
                              │
                              ▼
                       ┌─────────────┐
                       │ SAVED       │  Show correlation if any
                       │ + FEEDBACK  │  Return to history
                       └─────────────┘
```

### Step 1: Category Selection

Two-level tappable card grid. First: 8 category cards (2x4 grid). On category tap: subcategory list slides in (vertical list of tappable items).

```
"What's bothering you?"

┌──────┐ ┌──────┐ ┌──────┐ ┌──────┐
│ Pain │ │Digest│ │Breath│ │Neuro │
└──────┘ └──────┘ └──────┘ └──────┘
┌──────┐ ┌──────┐ ┌──────┐ ┌──────┐
│Genera│ │ Mood │ │ Skin │ │Other │
└──────┘ └──────┘ └──────┘ └──────┘
```

On "Pain" tap:
```
"What kind of pain?"

  Headache
  Back pain
  Joint pain
  Chest pain
  Abdominal pain
  Muscle pain
  Neck pain
  Other → [free text input]
```

### Step 2: Severity (Face Scale)

Five tappable faces arranged horizontally. No numbers visible. Face expressions progress from slight discomfort (1) to severe distress (5). Accessibility: aria-labels "Barely noticeable", "Mild", "Moderate", "Severe", "Very severe".

```
"How bad is it?"

  😊    🙂    😐    😟    😣
 (1)   (2)   (3)   (4)   (5)
```

The faces are rendered as SVG (not emoji) for consistent cross-platform appearance. Each face is a tappable circle with inner expression lines.

### Step 3: Date/Time Picker

```
"When did this start?"

  [Today] [Yesterday] [Pick a date...]

  (Optional) What time? [Morning | Afternoon | Evening | Not sure]
```

Default: today. "Yesterday" shortcut for common case. Calendar picker for older dates. Time is optional — mapped to approximate NaiveTime values:
- Morning → 09:00
- Afternoon → 14:00
- Evening → 20:00
- Not sure → None

### Step 4: Expanded Details (Progressive Disclosure)

Only shown if patient taps "Tell me more". Each sub-step is a card with tappable options.

**Body Map:** Front/back human silhouette (simple SVG). Patient taps a region → region highlights. Multiple regions selectable. Region mapped to `body_region` string.

**Duration:** "How long does it last each time?" → [Constant | A few minutes | A few hours | Days or more]

**Character:** "What does it feel like?" → Visual icon cards: [Sharp ⚡ | Dull ● | Burning 🔥 | Pressure ⬇ | Throbbing ~]

**Aggravating:** "What makes it worse?" → Multi-select chips: [Activity | Food | Stress | Position | Time of day | Other → free text]

**Relieving:** "What makes it better?" → Multi-select chips: [Rest | Medication | Position change | Cold/Heat | Other → free text]

**Timing:** "When does it usually happen?" → [Morning | Night | After meals | Random | All the time]

### Step 5: Notes

"Anything else you want to note?" → Optional free text area (max 500 characters). Soft keyboard opens. Submit button below.

---

## [5] Body Map

### SVG Structure

Two views: front and back. Toggle between them. Simple human silhouette with 24 tappable regions defined by SVG `<path>` elements.

```
Front View                    Back View
┌─────────────────┐          ┌─────────────────┐
│      (head)     │          │      (head)     │
│     (neck)      │          │     (neck)      │
│  (sh_l)(sh_r)   │          │  (sh_l)(sh_r)   │
│ (arm_l)(arm_r)  │          │ (back_upper)    │
│ (chest_l)(ch_r) │          │                 │
│ (chest_center)  │          │ (back_lower)    │
│ (abdomen_upper) │          │                 │
│ (abdomen_lower) │          │ (hip_l)(hip_r)  │
│ (hip_l)(hip_r)  │          │ (leg_l)(leg_r)  │
│ (leg_l)(leg_r)  │          │ (knee_l)(knee_r)│
│ (knee_l)(knee_r)│          │ (foot_l)(foot_r)│
│ (foot_l)(foot_r)│          │                 │
└─────────────────┘          └─────────────────┘
```

### Region Selection Behavior

- Tap region → fills with highlight color (soft blue)
- Tap again → deselects
- Multiple regions selectable
- Selected regions listed below the body map as text chips
- Region IDs map to `BODY_REGIONS` constant in Rust

### SVG Implementation Notes

- SVG paths are simple outlines (not detailed anatomical)
- Designed for clarity, not realism
- Each region is a `<g>` element with `data-region` attribute
- Touch/click handler on each `<g>`
- Highlight via CSS class toggle: `fill: var(--color-primary-light)` when selected
- Viewbox scaled to fit mobile-like widths (max 280px wide)

---

## [6] Severity Face Scale

### SVG Faces

Five custom SVG faces. Each is a 48x48 circle with expression. No emoji — platform-consistent rendering.

| Level | Expression | Accessible Label | Color |
|-------|-----------|------------------|-------|
| 1 | Slight smile, open eyes | "Barely noticeable" | Green |
| 2 | Neutral-slight frown | "Mild" | Yellow-green |
| 3 | Neutral, flat mouth | "Moderate" | Yellow |
| 4 | Frown, slightly closed eyes | "Severe" | Orange |
| 5 | Deep frown, creased brow | "Very severe" | Soft red |

### Implementation

```svelte
<!-- Inline in recording flow — NOT a separate file -->
<div class="flex items-center justify-between gap-2 px-4">
  {#each [1, 2, 3, 4, 5] as level}
    {@const labels = ['Barely noticeable', 'Mild', 'Moderate', 'Severe', 'Very severe']}
    {@const colors = ['#4ade80', '#a3e635', '#facc15', '#fb923c', '#f87171']}
    <button
      class="w-14 h-14 rounded-full border-2 flex items-center justify-center
             transition-all min-h-[44px] min-w-[44px]
             {severity === level
               ? 'border-[var(--color-primary)] scale-110 shadow-md'
               : 'border-stone-200'}"
      style="background-color: {severity === level ? colors[level - 1] + '30' : 'transparent'}"
      aria-label={labels[level - 1]}
      onclick={() => severity = level}
    >
      <!-- SVG face expression rendered here -->
      <svg viewBox="0 0 48 48" class="w-10 h-10">
        <!-- Circle face -->
        <circle cx="24" cy="24" r="22" fill={colors[level - 1]} opacity="0.3" />
        <circle cx="24" cy="24" r="22" fill="none" stroke={colors[level - 1]} stroke-width="2" />
        <!-- Eyes and mouth vary by level — defined in face rendering function -->
      </svg>
    </button>
  {/each}
</div>
```

---

## [7] Temporal Correlation

### Detection Logic

After saving a symptom, check for recent medication changes within a 14-day window.

```rust
/// Checks for medication changes within 14 days of symptom onset
pub fn detect_temporal_correlation(
    conn: &rusqlite::Connection,
    onset_date: NaiveDate,
) -> Result<Vec<TemporalCorrelation>, CohearaError> {
    let window_start = onset_date - chrono::Duration::days(14);

    // Check for new medications started within the window
    let new_meds: Vec<TemporalCorrelation> = conn.prepare(
        "SELECT m.name, m.start_date
         FROM medications m
         WHERE m.start_date BETWEEN ?1 AND ?2
         ORDER BY m.start_date DESC"
    )?
    .query_map(params![window_start, onset_date], |row| {
        let name: String = row.get(0)?;
        let start_date: NaiveDate = row.get(1)?;
        let days = (onset_date - start_date).num_days();
        Ok(TemporalCorrelation {
            medication_name: name.clone(),
            medication_start_date: start_date,
            days_since_change: days,
            message: format!(
                "You started {} on {}. If you think this might be related, mention it to your doctor at your next visit.",
                name,
                start_date.format("%B %d")
            ),
        })
    })?
    .collect::<Result<Vec<_>, _>>()?;

    // Check for dose changes within the window
    let dose_changes: Vec<TemporalCorrelation> = conn.prepare(
        "SELECT m.name, dc.change_date, dc.old_dose, dc.new_dose
         FROM dose_changes dc
         JOIN medications m ON dc.medication_id = m.id
         WHERE dc.change_date BETWEEN ?1 AND ?2
         ORDER BY dc.change_date DESC"
    )?
    .query_map(params![window_start, onset_date], |row| {
        let name: String = row.get(0)?;
        let change_date: NaiveDate = row.get(1)?;
        let days = (onset_date - change_date).num_days();
        Ok(TemporalCorrelation {
            medication_name: name.clone(),
            medication_start_date: change_date,
            days_since_change: days,
            message: format!(
                "Your dose of {} was changed on {}. If you think this might be related, mention it to your doctor at your next visit.",
                name,
                change_date.format("%B %d")
            ),
        })
    })?
    .collect::<Result<Vec<_>, _>>()?;

    let mut all = new_meds;
    all.extend(dose_changes);
    all.sort_by(|a, b| a.days_since_change.cmp(&b.days_since_change));
    Ok(all)
}
```

### Display After Save

When correlations are found, show a calm informational card below the "Saved" confirmation:

```
Your symptom has been recorded.

Note: You started Metformin on January 28. If you think this
might be related, mention it to your doctor at your next visit.
```

- Background: soft blue-gray
- No alarm wording — purely informational
- Automatically links `related_medication_id` on the symptom if patient taps "Link this"

---

## [8] Check-In Nudges

### Nudge Decision Logic

```rust
/// Determines whether to show a check-in nudge
pub fn check_nudge(
    conn: &rusqlite::Connection,
) -> Result<NudgeDecision, CohearaError> {
    // 1. Post-medication-change nudge (higher priority)
    let recent_med = conn.query_row(
        "SELECT m.name, m.start_date
         FROM medications m
         WHERE m.start_date >= date('now', '-7 days')
         AND m.status = 'active'
         ORDER BY m.start_date DESC
         LIMIT 1",
        [],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, NaiveDate>(1)?)),
    );

    if let Ok((med_name, start_date)) = recent_med {
        // Check if there's already a symptom entry since this medication started
        let has_entry: bool = conn.query_row(
            "SELECT COUNT(*) > 0 FROM symptoms
             WHERE recorded_date >= ?1",
            params![start_date],
            |row| row.get(0),
        )?;

        if !has_entry {
            return Ok(NudgeDecision {
                should_nudge: true,
                nudge_type: Some(NudgeType::PostMedicationChange),
                message: Some(format!(
                    "You started {} on {}. Over the next few days, would you like to track how you're feeling? This can help your doctor understand how you're responding.",
                    med_name,
                    start_date.format("%B %d")
                )),
                related_medication: Some(med_name),
            });
        }
    }

    // 2. Daily check-in nudge (3 days no entry + active symptoms)
    let last_entry_date: Option<NaiveDate> = conn.query_row(
        "SELECT MAX(DATE(recorded_date)) FROM symptoms",
        [],
        |row| row.get(0),
    ).ok().flatten();

    let active_symptoms: u32 = conn.query_row(
        "SELECT COUNT(*) FROM symptoms WHERE still_active = 1",
        [],
        |row| row.get(0),
    )?;

    if let Some(last_date) = last_entry_date {
        let days_since = (chrono::Local::now().date_naive() - last_date).num_days();
        if days_since >= 3 && active_symptoms > 0 {
            return Ok(NudgeDecision {
                should_nudge: true,
                nudge_type: Some(NudgeType::DailyCheckIn),
                message: Some(
                    "It's been a few days — would you like to note how you're feeling?".into()
                ),
                related_medication: None,
            });
        }
    }

    Ok(NudgeDecision {
        should_nudge: false,
        nudge_type: None,
        message: None,
        related_medication: None,
    })
}
```

### Nudge Display

Nudges appear as a gentle card on the Home screen (L3-02 integration) and at the top of the Journal tab.

**Daily Check-In Nudge:**
```
"It's been a few days — would you like to note how you're feeling?"
[Yes] [Not now] [Don't remind me]
```

**Post-Medication Nudge:**
```
"You started Metformin on January 28. Over the next few days, would you
 like to track how you're feeling? This can help your doctor understand
 how you're responding."
[Yes, remind me] [No thanks]
```

"Don't remind me" / "No thanks" → stores a `nudge_dismissed` preference (via memo or config). Dismissal lasts 30 days, then resets if conditions still met.

---

## [9] Symptom History View

### History Layout

```
┌────────────────────────────────────────────┐
│ SYMPTOM HISTORY                            │
│                                            │
│ [Filter: All categories ▼]  [Active only] │
│                                            │
│ ── Today ──                                │
│ ┌────────────────────────────────────────┐ │
│ │ Headache · Moderate · Pain             │ │
│ │ Started today · Still active           │ │
│ │ Note: You started Metformin 3 days ago │ │
│ └────────────────────────────────────────┘ │
│                                            │
│ ── January 28 ──                           │
│ ┌────────────────────────────────────────┐ │
│ │ Nausea · Mild · Digestive              │ │
│ │ Started Jan 28 · Resolved Jan 30       │ │
│ └────────────────────────────────────────┘ │
│                                            │
│ ── January 20 ──                           │
│ ┌────────────────────────────────────────┐ │
│ │ Back pain · Severe · Pain              │ │
│ │ Started Jan 15 · Still active          │ │
│ │ Region: lower back                     │ │
│ └────────────────────────────────────────┘ │
└────────────────────────────────────────────┘
```

### History Card Display Rules

| Field | Display |
|-------|---------|
| Title | "{specific} · {severity_label} · {category}" |
| Status | "Still active" (green dot) or "Resolved {date}" (gray) |
| Region | If body_region set: "Region: {region_label}" |
| Correlation | If related_medication set: temporal correlation note |
| Duration | If set: "Lasts {duration}" |
| Character | If set: "Feels {character}" |

### Card Tap → Detail View

Tapping a history card opens a detail view showing all recorded fields, with options to:
- **Resolve** ("I'm no longer experiencing this") → sets `still_active = false`, `resolved_date = today`
- **Update** ("Record another instance") → opens recording flow pre-filled with category + specific
- **Delete** ("Remove this entry") → confirmation dialog → hard delete

### Filtering

- Category dropdown: "All categories" + each category name
- Active toggle: show only `still_active = true`
- Date range: "Last 7 days", "Last 30 days", "Last 3 months", "All time"
- Severity: optional range slider (1-5)

---

## [10] Tauri Commands (IPC)

```rust
// src-tauri/src/commands/journal.rs

use tauri::State;

/// Records a new symptom entry
#[tauri::command]
pub async fn record_symptom(
    state: State<'_, AppState>,
    entry: SymptomEntry,
) -> Result<RecordResult, String> {
    let session_guard = state.active_session.lock()
        .map_err(|e| format!("Lock error: {e}"))?;
    let session = session_guard.as_ref()
        .ok_or("No active profile session")?;

    let conn = session.db_connection()?;
    let symptom_id = Uuid::new_v4();

    // Validate severity range
    if entry.severity < 1 || entry.severity > 5 {
        return Err("Severity must be between 1 and 5".into());
    }

    // Store in SQLite symptoms table
    conn.execute(
        "INSERT INTO symptoms (id, category, specific, severity, body_region,
         duration, character, aggravating, relieving, timing_pattern,
         onset_date, onset_time, recorded_date, still_active, source, notes)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12,
                 datetime('now'), 1, 'patient_reported', ?13)",
        params![
            symptom_id,
            serde_json::to_string(&entry.category).unwrap().trim_matches('"'),
            entry.specific,
            entry.severity,
            entry.body_region,
            entry.duration.map(|d| serde_json::to_string(&d).unwrap().trim_matches('"').to_string()),
            entry.character.map(|c| serde_json::to_string(&c).unwrap().trim_matches('"').to_string()),
            if entry.aggravating.is_empty() { None } else { Some(entry.aggravating.join(", ")) },
            if entry.relieving.is_empty() { None } else { Some(entry.relieving.join(", ")) },
            entry.timing_pattern.map(|t| serde_json::to_string(&t).unwrap().trim_matches('"').to_string()),
            entry.onset_date,
            entry.onset_time,
            entry.notes,
        ],
    ).map_err(|e| format!("Failed to save symptom: {e}"))?;

    // Embed symptom text in LanceDB for semantic search
    let embed_text = format!(
        "{} {} severity {} started {} {}",
        entry.category.to_string(),
        entry.specific,
        entry.severity,
        entry.onset_date,
        entry.notes.unwrap_or_default()
    );
    // Use the embedding model from L1-04 to generate vector
    // Store in LanceDB with metadata { type: "symptom", symptom_id, date }
    embed_and_store_symptom(session, &symptom_id, &embed_text)?;

    // Check temporal correlations
    let correlations = detect_temporal_correlation(&conn, entry.onset_date)?;

    state.update_activity();

    Ok(RecordResult {
        symptom_id,
        correlations,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordResult {
    pub symptom_id: Uuid,
    pub correlations: Vec<TemporalCorrelation>,
}

/// Fetches symptom history with optional filters
#[tauri::command]
pub async fn get_symptom_history(
    state: State<'_, AppState>,
    filter: Option<SymptomFilter>,
) -> Result<Vec<StoredSymptom>, String> {
    let session_guard = state.active_session.lock()
        .map_err(|e| format!("Lock error: {e}"))?;
    let session = session_guard.as_ref()
        .ok_or("No active profile session")?;

    let conn = session.db_connection()?;

    let mut query = String::from(
        "SELECT s.*, m.name as med_name, d.name as diag_name
         FROM symptoms s
         LEFT JOIN medications m ON s.related_medication_id = m.id
         LEFT JOIN diagnoses d ON s.related_diagnosis_id = d.id
         WHERE 1=1"
    );
    let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(ref f) = filter {
        if let Some(ref cat) = f.category {
            query.push_str(" AND s.category = ?");
            params_vec.push(Box::new(cat.clone()));
        }
        if let Some(min) = f.severity_min {
            query.push_str(" AND s.severity >= ?");
            params_vec.push(Box::new(min));
        }
        if let Some(max) = f.severity_max {
            query.push_str(" AND s.severity <= ?");
            params_vec.push(Box::new(max));
        }
        if let Some(ref from) = f.date_from {
            query.push_str(" AND s.onset_date >= ?");
            params_vec.push(Box::new(from.clone()));
        }
        if let Some(ref to) = f.date_to {
            query.push_str(" AND s.onset_date <= ?");
            params_vec.push(Box::new(to.clone()));
        }
        if let Some(active) = f.still_active {
            query.push_str(" AND s.still_active = ?");
            params_vec.push(Box::new(active));
        }
    }

    query.push_str(" ORDER BY s.recorded_date DESC");

    state.update_activity();

    // Execute query with dynamic params and map to StoredSymptom
    // (simplified — actual implementation uses rusqlite dynamic params)
    fetch_symptoms_with_query(&conn, &query, &params_vec)
        .map_err(|e| e.to_string())
}

/// Resolves a symptom (marks as no longer active)
#[tauri::command]
pub async fn resolve_symptom(
    state: State<'_, AppState>,
    symptom_id: String,
) -> Result<(), String> {
    let session_guard = state.active_session.lock()
        .map_err(|e| format!("Lock error: {e}"))?;
    let session = session_guard.as_ref()
        .ok_or("No active profile session")?;

    let conn = session.db_connection()?;
    let uuid = Uuid::parse_str(&symptom_id)
        .map_err(|e| format!("Invalid symptom ID: {e}"))?;

    conn.execute(
        "UPDATE symptoms SET still_active = 0, resolved_date = date('now')
         WHERE id = ?1",
        params![uuid],
    ).map_err(|e| format!("Failed to resolve symptom: {e}"))?;

    state.update_activity();
    Ok(())
}

/// Deletes a symptom entry
#[tauri::command]
pub async fn delete_symptom(
    state: State<'_, AppState>,
    symptom_id: String,
) -> Result<(), String> {
    let session_guard = state.active_session.lock()
        .map_err(|e| format!("Lock error: {e}"))?;
    let session = session_guard.as_ref()
        .ok_or("No active profile session")?;

    let conn = session.db_connection()?;
    let uuid = Uuid::parse_str(&symptom_id)
        .map_err(|e| format!("Invalid symptom ID: {e}"))?;

    conn.execute(
        "DELETE FROM symptoms WHERE id = ?1",
        params![uuid],
    ).map_err(|e| format!("Failed to delete symptom: {e}"))?;

    // Also remove from LanceDB
    remove_symptom_embedding(session, &uuid)?;

    state.update_activity();
    Ok(())
}

/// Checks if a nudge should be shown
#[tauri::command]
pub async fn check_journal_nudge(
    state: State<'_, AppState>,
) -> Result<NudgeDecision, String> {
    let session_guard = state.active_session.lock()
        .map_err(|e| format!("Lock error: {e}"))?;
    let session = session_guard.as_ref()
        .ok_or("No active profile session")?;

    let conn = session.db_connection()?;
    state.update_activity();

    check_nudge(&conn).map_err(|e| e.to_string())
}

/// Gets symptom categories and subcategories
#[tauri::command]
pub async fn get_symptom_categories() -> Result<Vec<CategoryInfo>, String> {
    Ok(vec![
        CategoryInfo { name: "Pain".into(), subcategories: SymptomCategory::Pain.subcategories().iter().map(|s| s.to_string()).collect() },
        CategoryInfo { name: "Digestive".into(), subcategories: SymptomCategory::Digestive.subcategories().iter().map(|s| s.to_string()).collect() },
        CategoryInfo { name: "Respiratory".into(), subcategories: SymptomCategory::Respiratory.subcategories().iter().map(|s| s.to_string()).collect() },
        CategoryInfo { name: "Neurological".into(), subcategories: SymptomCategory::Neurological.subcategories().iter().map(|s| s.to_string()).collect() },
        CategoryInfo { name: "General".into(), subcategories: SymptomCategory::General.subcategories().iter().map(|s| s.to_string()).collect() },
        CategoryInfo { name: "Mood".into(), subcategories: SymptomCategory::Mood.subcategories().iter().map(|s| s.to_string()).collect() },
        CategoryInfo { name: "Skin".into(), subcategories: SymptomCategory::Skin.subcategories().iter().map(|s| s.to_string()).collect() },
        CategoryInfo { name: "Other".into(), subcategories: SymptomCategory::Other.subcategories().iter().map(|s| s.to_string()).collect() },
    ])
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryInfo {
    pub name: String,
    pub subcategories: Vec<String>,
}
```

### Frontend API

```typescript
// src/lib/api/journal.ts
import { invoke } from '@tauri-apps/api/core';
import type {
  SymptomEntry, StoredSymptom, SymptomFilter,
  NudgeDecision, TemporalCorrelation
} from '$lib/types/journal';

interface RecordResult {
  symptom_id: string;
  correlations: TemporalCorrelation[];
}

interface CategoryInfo {
  name: string;
  subcategories: string[];
}

export async function recordSymptom(entry: SymptomEntry): Promise<RecordResult> {
  return invoke<RecordResult>('record_symptom', { entry });
}

export async function getSymptomHistory(filter?: SymptomFilter): Promise<StoredSymptom[]> {
  return invoke<StoredSymptom[]>('get_symptom_history', { filter: filter ?? null });
}

export async function resolveSymptom(symptomId: string): Promise<void> {
  return invoke('resolve_symptom', { symptomId });
}

export async function deleteSymptom(symptomId: string): Promise<void> {
  return invoke('delete_symptom', { symptomId });
}

export async function checkJournalNudge(): Promise<NudgeDecision> {
  return invoke<NudgeDecision>('check_journal_nudge');
}

export async function getSymptomCategories(): Promise<CategoryInfo[]> {
  return invoke<CategoryInfo[]>('get_symptom_categories');
}
```

---

## [11] Svelte Components

### Journal Screen (Main Container)

```svelte
<!-- src/lib/components/journal/JournalScreen.svelte -->
<script lang="ts">
  import { onMount } from 'svelte';
  import { getSymptomHistory, checkJournalNudge } from '$lib/api/journal';
  import type { StoredSymptom, NudgeDecision } from '$lib/types/journal';
  import RecordingFlow from './RecordingFlow.svelte';
  import SymptomHistory from './SymptomHistory.svelte';
  import NudgeBanner from './NudgeBanner.svelte';

  interface Props {
    onNavigate: (screen: string, params?: Record<string, string>) => void;
  }
  let { onNavigate }: Props = $props();

  let view: 'history' | 'recording' = $state('history');
  let symptoms: StoredSymptom[] = $state([]);
  let nudge: NudgeDecision | null = $state(null);
  let loading = $state(true);

  async function refresh() {
    loading = true;
    try {
      const [history, nudgeResult] = await Promise.all([
        getSymptomHistory(),
        checkJournalNudge(),
      ]);
      symptoms = history;
      nudge = nudgeResult;
    } catch (e) {
      console.error('Failed to load journal:', e);
    } finally {
      loading = false;
    }
  }

  onMount(() => { refresh(); });
</script>

<div class="flex flex-col min-h-screen pb-20 bg-stone-50">
  <header class="px-6 pt-6 pb-4 flex items-center justify-between">
    <h1 class="text-2xl font-bold text-stone-800">Journal</h1>
    {#if view === 'history'}
      <button
        class="px-4 py-2 bg-[var(--color-primary)] text-white rounded-xl text-sm
               font-medium min-h-[44px]"
        onclick={() => view = 'recording'}
      >
        + Record
      </button>
    {/if}
  </header>

  {#if view === 'recording'}
    <RecordingFlow
      onComplete={async () => { view = 'history'; await refresh(); }}
      onCancel={() => view = 'history'}
    />
  {:else}
    <!-- Nudge banner -->
    {#if nudge?.should_nudge}
      <NudgeBanner
        {nudge}
        onAccept={() => view = 'recording'}
        onDismiss={() => { nudge = null; }}
      />
    {/if}

    <!-- Symptom history -->
    <SymptomHistory
      {symptoms}
      {loading}
      onRefresh={refresh}
      {onNavigate}
    />
  {/if}
</div>
```

### Recording Flow (Multi-Step)

```svelte
<!-- src/lib/components/journal/RecordingFlow.svelte -->
<script lang="ts">
  import { recordSymptom, getSymptomCategories } from '$lib/api/journal';
  import type { SymptomEntry, TemporalCorrelation } from '$lib/types/journal';
  import { SUBCATEGORIES } from '$lib/types/journal';
  import CategorySelector from './CategorySelector.svelte';
  import SeverityScale from './SeverityScale.svelte';
  import DateSelector from './DateSelector.svelte';
  import ExpandedDetails from './ExpandedDetails.svelte';
  import CorrelationCard from './CorrelationCard.svelte';

  interface Props {
    onComplete: () => void;
    onCancel: () => void;
  }
  let { onComplete, onCancel }: Props = $props();

  type Step = 'category' | 'severity' | 'when' | 'expanded' | 'notes' | 'saving' | 'done';

  let step: Step = $state('category');
  let category = $state('');
  let specific = $state('');
  let severity = $state(0);
  let onsetDate = $state(new Date().toISOString().split('T')[0]);
  let onsetTime: string | null = $state(null);

  // Expanded fields
  let bodyRegion: string | null = $state(null);
  let duration: string | null = $state(null);
  let character: string | null = $state(null);
  let aggravating: string[] = $state([]);
  let relieving: string[] = $state([]);
  let timingPattern: string | null = $state(null);
  let notes: string | null = $state(null);

  let correlations: TemporalCorrelation[] = $state([]);
  let saving = $state(false);
  let error: string | null = $state(null);

  async function save() {
    saving = true;
    error = null;
    try {
      const entry: SymptomEntry = {
        category,
        specific,
        severity,
        onset_date: onsetDate,
        onset_time: onsetTime,
        body_region: bodyRegion,
        duration,
        character,
        aggravating,
        relieving,
        timing_pattern: timingPattern,
        notes,
      };
      const result = await recordSymptom(entry);
      correlations = result.correlations;
      step = 'done';
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
      saving = false;
    }
  }

  function canSave(): boolean {
    return category !== '' && specific !== '' && severity >= 1 && severity <= 5;
  }
</script>

<div class="px-6 py-4">
  <!-- Back/Cancel button -->
  <button
    class="text-stone-500 text-sm mb-4 min-h-[44px]"
    onclick={onCancel}
  >
    &larr; Cancel
  </button>

  {#if step === 'category'}
    <h2 class="text-xl font-semibold text-stone-800 mb-4">What's bothering you?</h2>
    <CategorySelector
      onSelect={(cat, spec) => {
        category = cat;
        specific = spec;
        step = 'severity';
      }}
    />

  {:else if step === 'severity'}
    <h2 class="text-xl font-semibold text-stone-800 mb-4">How bad is it?</h2>
    <SeverityScale
      bind:value={severity}
      onNext={() => step = 'when'}
    />

  {:else if step === 'when'}
    <h2 class="text-xl font-semibold text-stone-800 mb-4">When did this start?</h2>
    <DateSelector
      bind:date={onsetDate}
      bind:time={onsetTime}
    />
    <div class="flex gap-3 mt-6">
      <button
        class="flex-1 px-4 py-3 bg-[var(--color-primary)] text-white rounded-xl
               font-medium min-h-[44px] disabled:opacity-50"
        disabled={!canSave() || saving}
        onclick={save}
      >
        Save
      </button>
      <button
        class="px-4 py-3 bg-stone-100 text-stone-700 rounded-xl
               font-medium min-h-[44px]"
        onclick={() => step = 'expanded'}
      >
        Tell me more
      </button>
    </div>

  {:else if step === 'expanded'}
    <h2 class="text-xl font-semibold text-stone-800 mb-4">Tell me more</h2>
    <ExpandedDetails
      bind:bodyRegion
      bind:duration
      bind:character
      bind:aggravating
      bind:relieving
      bind:timingPattern
      onNext={() => step = 'notes'}
    />

  {:else if step === 'notes'}
    <h2 class="text-xl font-semibold text-stone-800 mb-4">Anything else?</h2>
    <textarea
      class="w-full h-32 p-4 rounded-xl border border-stone-200 text-stone-700
             resize-none focus:outline-none focus:ring-2 focus:ring-[var(--color-primary)]"
      placeholder="Optional notes..."
      maxlength={500}
      bind:value={notes}
    ></textarea>
    <p class="text-xs text-stone-400 mt-1 text-right">{(notes?.length ?? 0)}/500</p>
    <button
      class="w-full mt-4 px-4 py-3 bg-[var(--color-primary)] text-white rounded-xl
             font-medium min-h-[44px] disabled:opacity-50"
      disabled={!canSave() || saving}
      onclick={save}
    >
      {saving ? 'Saving...' : 'Save'}
    </button>

  {:else if step === 'done'}
    <div class="text-center py-8">
      <div class="w-16 h-16 bg-green-100 rounded-full flex items-center justify-center mx-auto mb-4">
        <span class="text-green-600 text-2xl">&#x2713;</span>
      </div>
      <h2 class="text-xl font-semibold text-stone-800 mb-2">Recorded</h2>
      <p class="text-stone-500 text-sm mb-6">Your symptom has been saved.</p>

      {#each correlations as correlation}
        <CorrelationCard {correlation} />
      {/each}

      <button
        class="mt-6 px-6 py-3 bg-stone-100 text-stone-700 rounded-xl
               font-medium min-h-[44px]"
        onclick={onComplete}
      >
        Done
      </button>
    </div>
  {/if}

  {#if error}
    <p class="text-red-600 text-sm mt-4">{error}</p>
  {/if}
</div>
```

### Category Selector

```svelte
<!-- src/lib/components/journal/CategorySelector.svelte -->
<script lang="ts">
  import { CATEGORIES, SUBCATEGORIES } from '$lib/types/journal';

  interface Props {
    onSelect: (category: string, specific: string) => void;
  }
  let { onSelect }: Props = $props();

  let selectedCategory: string | null = $state(null);
  let customText = $state('');

  const categoryIcons: Record<string, string> = {
    Pain: 'pain',
    Digestive: 'stomach',
    Respiratory: 'lungs',
    Neurological: 'brain',
    General: 'body',
    Mood: 'heart',
    Skin: 'skin',
    Other: 'more',
  };
</script>

{#if !selectedCategory}
  <!-- Category grid -->
  <div class="grid grid-cols-4 gap-3">
    {#each CATEGORIES as cat}
      <button
        class="flex flex-col items-center justify-center gap-2 p-4 rounded-xl
               bg-white border border-stone-200 hover:border-[var(--color-primary)]
               hover:bg-stone-50 transition-colors min-h-[80px]"
        onclick={() => selectedCategory = cat}
      >
        <!-- Icon placeholder -->
        <span class="text-lg text-stone-400">{categoryIcons[cat]}</span>
        <span class="text-xs text-stone-600 font-medium">{cat}</span>
      </button>
    {/each}
  </div>
{:else}
  <!-- Subcategory list -->
  <button
    class="text-sm text-stone-500 mb-3 min-h-[44px]"
    onclick={() => selectedCategory = null}
  >
    &larr; {selectedCategory}
  </button>

  <div class="flex flex-col gap-2">
    {#each SUBCATEGORIES[selectedCategory] ?? [] as sub}
      {#if sub === 'Other'}
        <div class="flex gap-2">
          <input
            type="text"
            class="flex-1 px-4 py-3 rounded-xl border border-stone-200
                   text-stone-700 focus:outline-none focus:ring-2
                   focus:ring-[var(--color-primary)]"
            placeholder="Describe..."
            bind:value={customText}
          />
          <button
            class="px-4 py-3 bg-[var(--color-primary)] text-white rounded-xl
                   font-medium min-h-[44px] disabled:opacity-50"
            disabled={customText.trim().length === 0}
            onclick={() => onSelect(selectedCategory!, customText.trim())}
          >
            Next
          </button>
        </div>
      {:else}
        <button
          class="w-full text-left px-4 py-3 rounded-xl bg-white border border-stone-200
                 hover:border-[var(--color-primary)] hover:bg-stone-50
                 text-stone-700 transition-colors min-h-[44px]"
          onclick={() => onSelect(selectedCategory!, sub)}
        >
          {sub}
        </button>
      {/if}
    {/each}
  </div>
{/if}
```

### Severity Scale Component

```svelte
<!-- src/lib/components/journal/SeverityScale.svelte -->
<script lang="ts">
  interface Props {
    value: number;
    onNext: () => void;
  }
  let { value = $bindable(), onNext }: Props = $props();

  const levels = [
    { n: 1, label: 'Barely noticeable', color: '#4ade80' },
    { n: 2, label: 'Mild', color: '#a3e635' },
    { n: 3, label: 'Moderate', color: '#facc15' },
    { n: 4, label: 'Severe', color: '#fb923c' },
    { n: 5, label: 'Very severe', color: '#f87171' },
  ];
</script>

<div class="flex items-center justify-between gap-3 px-2 mb-6">
  {#each levels as level}
    <button
      class="flex flex-col items-center gap-2 transition-all min-h-[44px] min-w-[44px]"
      class:scale-110={value === level.n}
      aria-label={level.label}
      onclick={() => { value = level.n; }}
    >
      <div
        class="w-14 h-14 rounded-full border-2 flex items-center justify-center transition-all"
        style="border-color: {value === level.n ? 'var(--color-primary)' : '#d6d3d1'};
               background-color: {value === level.n ? level.color + '30' : 'transparent'}"
      >
        <!-- SVG face placeholder — actual SVG faces per level -->
        <svg viewBox="0 0 48 48" class="w-10 h-10">
          <circle cx="24" cy="24" r="22" fill={level.color} opacity="0.3" />
          <circle cx="24" cy="24" r="22" fill="none" stroke={level.color} stroke-width="2" />
        </svg>
      </div>
      <span class="text-xs text-stone-500 {value === level.n ? 'font-medium' : ''}">
        {level.label}
      </span>
    </button>
  {/each}
</div>

{#if value >= 1}
  <button
    class="w-full px-4 py-3 bg-[var(--color-primary)] text-white rounded-xl
           font-medium min-h-[44px]"
    onclick={onNext}
  >
    Next
  </button>
{/if}
```

### Nudge Banner

```svelte
<!-- src/lib/components/journal/NudgeBanner.svelte -->
<script lang="ts">
  import type { NudgeDecision } from '$lib/types/journal';

  interface Props {
    nudge: NudgeDecision;
    onAccept: () => void;
    onDismiss: () => void;
  }
  let { nudge, onAccept, onDismiss }: Props = $props();
</script>

<div class="mx-6 mb-4 p-4 bg-blue-50 border border-blue-100 rounded-xl">
  <p class="text-sm text-blue-800 mb-3">{nudge.message}</p>
  <div class="flex gap-2">
    <button
      class="px-4 py-2 bg-[var(--color-primary)] text-white rounded-lg text-sm
             font-medium min-h-[44px]"
      onclick={onAccept}
    >
      {nudge.nudge_type === 'PostMedicationChange' ? 'Yes, remind me' : 'Yes'}
    </button>
    <button
      class="px-4 py-2 bg-white text-stone-600 rounded-lg text-sm
             border border-stone-200 min-h-[44px]"
      onclick={onDismiss}
    >
      {nudge.nudge_type === 'PostMedicationChange' ? 'No thanks' : 'Not now'}
    </button>
  </div>
</div>
```

### Correlation Card

```svelte
<!-- src/lib/components/journal/CorrelationCard.svelte -->
<script lang="ts">
  import type { TemporalCorrelation } from '$lib/types/journal';

  interface Props {
    correlation: TemporalCorrelation;
  }
  let { correlation }: Props = $props();
</script>

<div class="mx-auto max-w-sm p-4 bg-blue-50 border border-blue-100 rounded-xl mb-3 text-left">
  <p class="text-sm text-blue-800">{correlation.message}</p>
  <p class="text-xs text-blue-600 mt-1">
    ({correlation.days_since_change} day{correlation.days_since_change === 1 ? '' : 's'} ago)
  </p>
</div>
```

### Symptom History Component

```svelte
<!-- src/lib/components/journal/SymptomHistory.svelte -->
<script lang="ts">
  import { resolveSymptom, deleteSymptom } from '$lib/api/journal';
  import type { StoredSymptom } from '$lib/types/journal';
  import { CATEGORIES } from '$lib/types/journal';

  interface Props {
    symptoms: StoredSymptom[];
    loading: boolean;
    onRefresh: () => Promise<void>;
    onNavigate: (screen: string, params?: Record<string, string>) => void;
  }
  let { symptoms, loading, onRefresh, onNavigate }: Props = $props();

  let filterCategory = $state('all');
  let filterActive = $state(false);

  let filtered = $derived(
    symptoms.filter(s => {
      if (filterCategory !== 'all' && s.category !== filterCategory) return false;
      if (filterActive && !s.still_active) return false;
      return true;
    })
  );

  // Group by date
  let grouped = $derived(() => {
    const groups: Map<string, StoredSymptom[]> = new Map();
    for (const s of filtered) {
      const dateKey = new Date(s.recorded_date).toLocaleDateString();
      if (!groups.has(dateKey)) groups.set(dateKey, []);
      groups.get(dateKey)!.push(s);
    }
    return groups;
  });

  const severityLabels = ['', 'Barely noticeable', 'Mild', 'Moderate', 'Severe', 'Very severe'];

  async function handleResolve(id: string) {
    await resolveSymptom(id);
    await onRefresh();
  }

  async function handleDelete(id: string) {
    if (confirm('Remove this symptom entry?')) {
      await deleteSymptom(id);
      await onRefresh();
    }
  }
</script>

<div class="px-6">
  <!-- Filters -->
  <div class="flex gap-3 mb-4">
    <select
      class="px-3 py-2 rounded-lg border border-stone-200 text-sm text-stone-700
             bg-white min-h-[44px]"
      bind:value={filterCategory}
    >
      <option value="all">All categories</option>
      {#each CATEGORIES as cat}
        <option value={cat}>{cat}</option>
      {/each}
    </select>

    <label class="flex items-center gap-2 text-sm text-stone-600 min-h-[44px]">
      <input type="checkbox" bind:checked={filterActive}
             class="w-4 h-4 rounded border-stone-300" />
      Active only
    </label>
  </div>

  {#if loading}
    <div class="text-center py-12 text-stone-400">Loading...</div>
  {:else if filtered.length === 0}
    <div class="text-center py-12">
      <p class="text-stone-500 mb-2">No symptoms recorded yet.</p>
      <p class="text-sm text-stone-400">Tap "+ Record" to log how you're feeling.</p>
    </div>
  {:else}
    {#each [...grouped().entries()] as [date, items]}
      <h3 class="text-xs font-medium text-stone-400 uppercase mt-4 mb-2">{date}</h3>
      {#each items as symptom}
        <div class="bg-white rounded-xl p-4 mb-2 border border-stone-100 shadow-sm">
          <div class="flex items-start justify-between">
            <div>
              <span class="font-medium text-stone-800">{symptom.specific}</span>
              <span class="text-stone-400 mx-1">·</span>
              <span class="text-sm text-stone-500">{severityLabels[symptom.severity]}</span>
              <span class="text-stone-400 mx-1">·</span>
              <span class="text-sm text-stone-500">{symptom.category}</span>
            </div>
            <span class="text-xs px-2 py-0.5 rounded-full
                        {symptom.still_active
                          ? 'bg-green-100 text-green-700'
                          : 'bg-stone-100 text-stone-500'}">
              {symptom.still_active ? 'Active' : 'Resolved'}
            </span>
          </div>
          {#if symptom.body_region}
            <p class="text-xs text-stone-400 mt-1">Region: {symptom.body_region}</p>
          {/if}
          {#if symptom.related_medication_name}
            <p class="text-xs text-blue-600 mt-1">
              Note: started {symptom.related_medication_name} recently
            </p>
          {/if}

          <!-- Actions -->
          <div class="flex gap-2 mt-3">
            {#if symptom.still_active}
              <button
                class="text-xs text-stone-500 underline min-h-[44px] px-1"
                onclick={() => handleResolve(symptom.id)}
              >
                Mark resolved
              </button>
            {/if}
            <button
              class="text-xs text-red-400 underline min-h-[44px] px-1"
              onclick={() => handleDelete(symptom.id)}
            >
              Remove
            </button>
          </div>
        </div>
      {/each}
    {/each}
  {/if}
</div>
```

---

## [12] Error Handling

| Error | User Message | Recovery |
|-------|-------------|----------|
| Save fails (DB write) | "Couldn't save your symptom. Please try again." | Retry button, no data loss (form still populated) |
| History load fails | "Couldn't load your symptom history." | Retry on pull-to-refresh |
| Invalid severity (not 1-5) | Prevented by UI — no free input for severity | N/A |
| Embedding fails | Log warning, save symptom without embedding | Symptom still searchable via SQLite |
| Session expired | Redirect to profile unlock | ProfileGuard handles |
| Delete fails | "Couldn't remove this entry. Please try again." | Retry on next tap |

---

## [13] Security

- All symptom text fields encrypted via ProfileSession before SQLite write
- Body region and notes are sensitive health data — encrypted at rest
- Free text input sanitized (max 500 chars, no script injection)
- Symptoms never sent over network — local only
- Delete is hard delete from SQLite + LanceDB (respects cryptographic erasure)
- Activity timestamp updated on every command

---

## [14] Testing

### Unit Tests (Rust)

| Test | What |
|------|------|
| `test_record_symptom_basic` | Basic 3-field entry saves correctly |
| `test_record_symptom_expanded` | Full OLDCARTS entry with all fields |
| `test_record_symptom_invalid_severity` | Rejects severity outside 1-5 |
| `test_temporal_correlation_found` | Detects medication started 5 days ago |
| `test_temporal_correlation_none` | No correlation when no recent med changes |
| `test_temporal_correlation_dose_change` | Detects dose change within 14 days |
| `test_temporal_correlation_outside_window` | No correlation when change > 14 days ago |
| `test_nudge_daily_checkin` | Nudge after 3 days + active symptoms |
| `test_nudge_no_active_symptoms` | No nudge when no active symptoms |
| `test_nudge_post_medication` | Nudge when new medication and no recent entry |
| `test_nudge_already_recorded` | No nudge when entry exists since medication start |
| `test_resolve_symptom` | Sets still_active=false and resolved_date |
| `test_delete_symptom` | Removes from SQLite |
| `test_history_filter_category` | Filters by category correctly |
| `test_history_filter_active` | Filters by still_active correctly |
| `test_history_filter_date_range` | Filters by date range |
| `test_history_ordered` | Results ordered by recorded_date DESC |
| `test_subcategories_complete` | All 8 categories have subcategories |
| `test_body_regions_valid` | All body regions are recognized strings |

### Frontend Tests

| Test | What |
|------|------|
| `test_category_grid_renders` | 8 category cards displayed |
| `test_subcategory_selection` | Tapping category shows subcategories |
| `test_severity_scale_selection` | Tapping face sets severity value |
| `test_save_basic_flow` | Category + severity + date → save succeeds |
| `test_expanded_flow` | "Tell me more" → expanded fields → save |
| `test_correlation_card_shown` | Correlation displayed after save |
| `test_history_filtering` | Category and active filters work |
| `test_resolve_symptom_ui` | "Mark resolved" updates symptom |
| `test_nudge_banner_shown` | Nudge displayed when nudge.should_nudge |
| `test_nudge_banner_dismissed` | Dismiss hides nudge |

---

## [15] Performance

- Recording flow is instant — all category/subcategory data is static (no DB queries)
- Save is single SQLite INSERT + async embedding (embedding doesn't block UI)
- History loads max 100 recent symptoms, lazy-load more on scroll
- Temporal correlation check is a simple date-range SQL query (fast)
- Nudge check is 2 SQL queries (max)

---

## [16] Open Questions

- **Q1:** Should symptoms be editable after recording, or only resolvable/deletable? Current answer: resolve + delete only — keeps audit trail clean.
- **Q2:** Should the body map support free-form drawing, or only region taps? Current answer: region taps only — simpler, less ambiguous for appointment prep.

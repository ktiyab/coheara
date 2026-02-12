# L4-02 — Appointment Prep

<!--
=============================================================================
COMPONENT SPEC — The bridge between patient and professional.
Engineer review: E-UX (UI/UX, lead), E-RS (Rust), E-ML (AI/ML), E-DA (Data), E-QA (QA)
This is where Coheara delivers its highest value: a patient walks into
a doctor's appointment prepared, organized, and confident.
Two artifacts: patient questions (plain language) and professional summary (structured).
=============================================================================
-->

## Table of Contents

| Section | Offset |
|---------|--------|
| [1] Identity | `offset=20 limit=35` |
| [2] Dependencies | `offset=55 limit=22` |
| [3] Interfaces | `offset=77 limit=90` |
| [4] Preparation Flow | `offset=167 limit=65` |
| [5] Patient Copy Generation | `offset=232 limit=65` |
| [6] Professional Copy Generation | `offset=297 limit=75` |
| [7] PDF Export | `offset=372 limit=55` |
| [8] Post-Appointment Notes | `offset=427 limit=50` |
| [9] Appointment History | `offset=477 limit=40` |
| [10] Tauri Commands (IPC) | `offset=517 limit=75` |
| [11] Svelte Components | `offset=592 limit=120` |
| [12] Error Handling | `offset=712 limit=25` |
| [13] Security | `offset=737 limit=20` |
| [14] Testing | `offset=757 limit=50` |
| [15] Performance | `offset=807 limit=15` |
| [16] Open Questions | `offset=822 limit=10` |

---

## [1] Identity

**What:** Appointment preparation — generates two printable artifacts before a doctor visit: (1) a patient copy with plain-language questions ranked by relevance, and (2) a professional copy with a structured medical summary. Includes: professional selector from known professionals, date picker, MedGemma-powered generation of both copies, PDF export for printing, post-appointment guided note-taking, and appointment history.

**After this session:**
- Patient taps "Prepare for appointment" → selects professional → sets date
- System generates patient copy (questions, symptoms to mention, medication changes)
- System generates professional copy (structured summary of constellation)
- Both copies viewable on screen and exportable as PDF
- Critical coherence observations included as priority items
- Post-appointment flow: "How did it go?" → guided note capture
- Appointment history with prep status and post-notes
- Generation completes in < 15 seconds for typical constellation

**Estimated complexity:** High
**Source:** Tech Spec v1.1 Section 9.5 (Appointment Preparation Flow)

---

## [2] Dependencies

**Incoming:**
- L0-02 (data model — appointments table, professionals table)
- L0-03 (encryption — ProfileSession for decrypting data for generation)
- L1-03 (medical structuring — LlmClient for MedGemma generation)
- L2-01 (RAG pipeline — context assembly for summary generation)
- L2-03 (coherence engine — observations for priority items)
- L4-01 (symptom journal — recent symptoms for summary)

**Outgoing:**
- L3-02 (home screen — shows upcoming appointments)
- L4-04 (timeline view — appointment events displayed)

**No new Cargo.toml dependencies** except for PDF generation:
```toml
# PDF generation
printpdf = "0.7"  # Pure Rust PDF generation
```

---

## [3] Interfaces

### Backend Types

```rust
// src-tauri/src/appointment.rs

use chrono::{NaiveDate, NaiveDateTime};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Appointment creation request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppointmentRequest {
    pub professional_id: Option<Uuid>,   // Existing professional
    pub new_professional: Option<NewProfessional>,  // Or create new
    pub date: NaiveDate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewProfessional {
    pub name: String,
    pub specialty: String,
    pub institution: Option<String>,
}

/// Generated appointment prep result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppointmentPrep {
    pub appointment_id: Uuid,
    pub professional_name: String,
    pub professional_specialty: String,
    pub appointment_date: NaiveDate,
    pub patient_copy: PatientCopy,
    pub professional_copy: ProfessionalCopy,
    pub generated_at: NaiveDateTime,
}

/// Patient-facing questions and reminders
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatientCopy {
    pub title: String,                    // "Questions for Dr. Chen — February 20"
    pub priority_items: Vec<PrepItem>,    // Critical observations (if any)
    pub questions: Vec<PrepQuestion>,     // Ranked questions
    pub symptoms_to_mention: Vec<SymptomMention>,
    pub medication_changes: Vec<MedicationChange>,
    pub reminder: String,                 // "Bring this to your appointment"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrepItem {
    pub text: String,          // Plain language observation
    pub source: String,        // "Lab report from January 10"
    pub priority: PrepPriority,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PrepPriority {
    Critical,   // Critical lab values, allergy matches
    Important,  // Conflicts, gaps, drift
    Standard,   // General observations
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrepQuestion {
    pub question: String,       // Plain language question
    pub context: String,        // Why this question matters
    pub relevance_score: f64,   // For ranking (not shown to patient)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymptomMention {
    pub description: String,    // "Headache — moderate — since January 25"
    pub severity: u8,
    pub onset_date: NaiveDate,
    pub still_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MedicationChange {
    pub description: String,    // "Started Metformin 500mg on January 20"
    pub change_type: String,    // "started", "stopped", "dose_changed"
    pub date: NaiveDate,
}

/// Professional-facing structured summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfessionalCopy {
    pub header: ProfessionalHeader,
    pub current_medications: Vec<MedicationSummary>,
    pub changes_since_last_visit: Vec<ChangeSummary>,
    pub lab_results: Vec<LabSummary>,
    pub patient_reported_symptoms: Vec<SymptomSummary>,
    pub observations_for_discussion: Vec<ObservationSummary>,
    pub source_documents: Vec<DocumentReference>,
    pub disclaimer: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfessionalHeader {
    pub title: String,            // "COHEARA PATIENT SUMMARY"
    pub date: NaiveDate,
    pub professional: String,     // "For: Dr. Chen (GP)"
    pub disclaimer: String,       // "AI-generated from patient-loaded documents. Not clinical advice."
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MedicationSummary {
    pub name: String,
    pub dose: String,
    pub frequency: String,
    pub prescriber: String,
    pub start_date: NaiveDate,
    pub is_recent_change: bool,  // Highlight if changed since last visit
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeSummary {
    pub description: String,
    pub date: NaiveDate,
    pub change_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LabSummary {
    pub test_name: String,
    pub value: String,
    pub unit: String,
    pub reference_range: String,
    pub abnormal_flag: String,
    pub date: NaiveDate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymptomSummary {
    pub description: String,
    pub severity: u8,
    pub onset_date: NaiveDate,
    pub duration: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservationSummary {
    pub observation: String,
    pub severity: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentReference {
    pub document_type: String,
    pub date: NaiveDate,
    pub professional: String,
}

/// Post-appointment notes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostAppointmentNotes {
    pub appointment_id: Uuid,
    pub doctor_said: String,         // "What did the doctor say?"
    pub changes_made: String,        // "Any changes to your medications or treatment?"
    pub follow_up: Option<String>,   // "Any follow-up needed?"
    pub general_notes: Option<String>,
}

/// Stored appointment for history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredAppointment {
    pub id: Uuid,
    pub professional_name: String,
    pub professional_specialty: String,
    pub date: NaiveDate,
    pub appointment_type: String,  // "upcoming" or "completed"
    pub prep_generated: bool,
    pub has_post_notes: bool,
}
```

### Frontend Types

```typescript
// src/lib/types/appointment.ts

export interface AppointmentRequest {
  professional_id: string | null;
  new_professional: NewProfessional | null;
  date: string;  // YYYY-MM-DD
}

export interface NewProfessional {
  name: string;
  specialty: string;
  institution: string | null;
}

export interface AppointmentPrep {
  appointment_id: string;
  professional_name: string;
  professional_specialty: string;
  appointment_date: string;
  patient_copy: PatientCopy;
  professional_copy: ProfessionalCopy;
  generated_at: string;
}

export interface PatientCopy {
  title: string;
  priority_items: PrepItem[];
  questions: PrepQuestion[];
  symptoms_to_mention: SymptomMention[];
  medication_changes: MedicationChange[];
  reminder: string;
}

export interface PrepItem {
  text: string;
  source: string;
  priority: 'Critical' | 'Important' | 'Standard';
}

export interface PrepQuestion {
  question: string;
  context: string;
  relevance_score: number;
}

export interface ProfessionalCopy {
  header: { title: string; date: string; professional: string; disclaimer: string };
  current_medications: MedicationSummary[];
  changes_since_last_visit: ChangeSummary[];
  lab_results: LabSummary[];
  patient_reported_symptoms: SymptomSummary[];
  observations_for_discussion: ObservationSummary[];
  source_documents: DocumentReference[];
  disclaimer: string;
}

// ... (matching Rust types — abbreviated for spec clarity)

export interface PostAppointmentNotes {
  appointment_id: string;
  doctor_said: string;
  changes_made: string;
  follow_up: string | null;
  general_notes: string | null;
}

export interface StoredAppointment {
  id: string;
  professional_name: string;
  professional_specialty: string;
  date: string;
  appointment_type: string;
  prep_generated: boolean;
  has_post_notes: boolean;
}
```

---

## [4] Preparation Flow

### State Machine

```
                ┌─────────────────┐
                │   SELECT        │  Choose professional
                │   PROFESSIONAL  │  (existing or new)
                └────────┬────────┘
                         │
                         ▼
                ┌─────────────────┐
                │   SELECT DATE   │  Date picker
                └────────┬────────┘
                         │
                         ▼
                ┌─────────────────┐
                │   GENERATING    │  MedGemma builds both copies
                │   (< 15 sec)   │  Progress indicator shown
                └────────┬────────┘
                         │
                         ▼
                ┌─────────────────┐
                │   VIEW PREP     │  Tab view: Patient | Professional
                │                 │  [Print Patient] [Print Pro] [Print Both]
                └────────┬────────┘
                         │
               (After appointment)
                         │
                         ▼
                ┌─────────────────┐
                │  POST-APPT      │  "How did it go?"
                │  NOTES          │  Guided note capture
                └─────────────────┘
```

### Professional Selector

Shows known professionals from the `professionals` table. Ordered by `last_seen_date DESC`.

```
"Which doctor is this appointment with?"

  Dr. Chen — GP (last visit: Jan 15)
  Dr. Moreau — Cardiologist (last visit: Dec 20)
  Pharmacist Dubois — Pharmacist (last visit: Jan 5)

  [+ Add new professional]
```

If "Add new": inline form with name, specialty dropdown (GP, Cardiologist, Neurologist, Dermatologist, Pharmacist, Nurse, Specialist, Other), optional institution.

### Date Picker

Standard date picker. Default: next available weekday. Shows relative label: "In 3 days", "Next week", etc.

---

## [5] Patient Copy Generation

### Data Assembly (Rust)

```rust
/// Assembles all data needed for patient copy generation
pub fn assemble_patient_prep_data(
    conn: &rusqlite::Connection,
    session: &ProfileSession,
    professional_id: Uuid,
    appointment_date: NaiveDate,
) -> Result<PatientPrepData, CohearaError> {
    // Find last visit to this professional
    let last_visit: Option<NaiveDate> = conn.query_row(
        "SELECT MAX(date) FROM appointments
         WHERE professional_id = ?1 AND type = 'completed'",
        params![professional_id],
        |row| row.get(0),
    ).ok().flatten();

    let since_date = last_visit.unwrap_or(NaiveDate::MIN);

    // Gather coherence observations (critical first)
    let observations = fetch_undismissed_observations(conn)?;

    // Gather symptoms since last visit (or all if first visit)
    let symptoms = conn.prepare(
        "SELECT * FROM symptoms WHERE onset_date >= ?1
         ORDER BY severity DESC, onset_date DESC"
    )?.query_map(params![since_date], map_symptom)?
    .collect::<Result<Vec<_>, _>>()?;

    // Gather medication changes since last visit
    let med_changes = fetch_medication_changes_since(conn, since_date)?;

    // Current active medications
    let medications = fetch_active_medications(conn, session)?;

    // Recent lab results
    let labs = fetch_recent_labs(conn, session, since_date)?;

    Ok(PatientPrepData {
        professional_name: fetch_professional_name(conn, professional_id)?,
        appointment_date,
        observations,
        symptoms,
        medication_changes: med_changes,
        medications,
        labs,
        since_date,
    })
}
```

### MedGemma Prompt for Patient Questions

```rust
pub const PATIENT_QUESTIONS_PROMPT: &str = r#"
You are helping a patient prepare questions for their doctor appointment.

RULES:
- Generate exactly 5 questions ranked by relevance
- Use plain, simple language (reading level: grade 6)
- Questions should be specific to THIS patient's situation
- Frame as things the patient might want to ask
- NEVER suggest diagnoses or treatments
- NEVER use alarm language

Based on the following patient data, generate 5 questions:

OBSERVATIONS (discuss these):
{observations}

RECENT SYMPTOMS:
{symptoms}

MEDICATION CHANGES SINCE LAST VISIT:
{medication_changes}

Output format (JSON array):
[
  {"question": "...", "context": "why this matters"},
  ...
]
"#;
```

### Patient Copy Format

```
Questions for Dr. Chen — February 20, 2026

PRIORITY:
  ⚠ Your lab report from January 10 flags potassium as needing
    prompt attention. Please discuss this with your doctor.

YOUR QUESTIONS:
  1. My records show I'm taking both Metformin and a new medication
     from a different doctor. Should I continue taking both?

  2. I've been having headaches since starting the new medication
     two weeks ago. Could this be related?

  3. My potassium level was flagged on my last lab report.
     What does this mean for me?

  4. I noticed my blood pressure medication dose was changed.
     How will I know if the new dose is working?

  5. I have a referral to a cardiologist from Dr. Moreau.
     When should I schedule this?

SYMPTOMS TO MENTION:
  · Headache — moderate — since January 25 (still active)
  · Nausea — mild — January 20 to January 22

MEDICATION CHANGES:
  · Started Metformin 500mg on January 20
  · Lisinopril dose changed from 10mg to 20mg on January 15

Bring this to your appointment.
```

---

## [6] Professional Copy Generation

### Data Assembly

Uses the same data as patient copy but formats it as a structured clinical summary. No MedGemma generation needed — this is a template-based construction from structured SQLite data.

### Professional Copy Format

```
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
COHEARA PATIENT SUMMARY — 2026-02-20
For: Dr. Chen (GP)
AI-generated from patient-loaded documents.
Not clinical advice.
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

CURRENT MEDICATIONS:
  Metformin 500mg — 2x daily — Dr. Chen — started 2026-01-20  [NEW]
  Lisinopril 20mg — 1x daily — Dr. Chen — dose changed 2026-01-15  [CHANGED]
  Atorvastatin 10mg — 1x daily — Dr. Moreau — started 2025-06-01
  Aspirin 100mg — 1x daily — Dr. Chen — started 2025-03-15

CHANGES SINCE LAST VISIT (2026-01-15):
  · New: Metformin 500mg (2026-01-20)
  · Changed: Lisinopril 10mg → 20mg (2026-01-15)

LAB RESULTS:
  Potassium: 5.8 mEq/L (ref: 3.5-5.0) [CRITICAL HIGH] — 2026-01-10
  HbA1c: 7.2% (ref: <5.7) [HIGH] — 2026-01-10
  Creatinine: 1.1 mg/dL (ref: 0.7-1.3) [NORMAL] — 2026-01-10

PATIENT-REPORTED SYMPTOMS:
  · Headache — moderate (3/5) — onset 2026-01-25 — still active
  · Nausea — mild (2/5) — 2026-01-20 to 2026-01-22

OBSERVATIONS FOR DISCUSSION:
  [CRITICAL] Potassium 5.8 mEq/L flagged critical on lab report
  [IMPORTANT] Headache onset correlates with Metformin start (5 days)
  [STANDARD] HbA1c above reference range

SOURCE DOCUMENTS:
  · Prescription — Dr. Chen — 2026-01-20
  · Lab Report — Lab Central — 2026-01-10
  · Referral — Dr. Moreau — 2025-12-15
```

### Construction Logic

```rust
/// Builds the professional copy from assembled data (no LLM needed)
pub fn build_professional_copy(
    data: &PatientPrepData,
) -> ProfessionalCopy {
    let header = ProfessionalHeader {
        title: "COHEARA PATIENT SUMMARY".into(),
        date: data.appointment_date,
        professional: format!("For: {} ({})",
            data.professional_name, data.professional_specialty),
        disclaimer: "AI-generated from patient-loaded documents. Not clinical advice.".into(),
    };

    // Medications — flag recent changes
    let current_medications = data.medications.iter().map(|m| {
        let is_recent = m.start_date > data.since_date
            || data.medication_changes.iter().any(|c| c.medication_name == m.name);
        MedicationSummary {
            name: m.name.clone(),
            dose: m.dose.clone(),
            frequency: m.frequency.clone(),
            prescriber: m.prescriber_name.clone(),
            start_date: m.start_date,
            is_recent_change: is_recent,
        }
    }).collect();

    // Changes since last visit
    let changes_since = data.medication_changes.iter().map(|c| {
        ChangeSummary {
            description: c.description.clone(),
            date: c.date,
            change_type: c.change_type.clone(),
        }
    }).collect();

    // Lab results — most recent per test
    let lab_results = data.labs.iter().map(|l| {
        LabSummary {
            test_name: l.test_name.clone(),
            value: l.value.clone(),
            unit: l.unit.clone(),
            reference_range: format!("{}-{}", l.range_low, l.range_high),
            abnormal_flag: l.abnormal_flag.clone(),
            date: l.date,
        }
    }).collect();

    // Symptoms
    let patient_symptoms = data.symptoms.iter().map(|s| {
        SymptomSummary {
            description: format!("{} — {}", s.specific, s.category),
            severity: s.severity,
            onset_date: s.onset_date,
            duration: s.duration.clone(),
        }
    }).collect();

    // Observations — sorted by severity
    let observations = data.observations.iter().map(|o| {
        ObservationSummary {
            observation: o.summary.clone(),
            severity: format!("{:?}", o.severity),
            source: o.source_description.clone(),
        }
    }).collect();

    ProfessionalCopy {
        header,
        current_medications,
        changes_since_last_visit: changes_since,
        lab_results,
        patient_reported_symptoms: patient_symptoms,
        observations_for_discussion: observations,
        source_documents: data.source_documents.clone(),
        disclaimer: "This summary is AI-generated from patient-loaded documents. It is not a clinical record and should not replace professional assessment.".into(),
    }
}
```

---

## [7] PDF Export

### PDF Generation

Using `printpdf` crate for pure Rust PDF generation. Two templates: patient copy and professional copy.

```rust
use printpdf::*;

/// Generates a PDF from the patient copy
pub fn generate_patient_pdf(
    copy: &PatientCopy,
) -> Result<Vec<u8>, CohearaError> {
    let (doc, page1, layer1) = PdfDocument::new(
        &copy.title, Mm(210.0), Mm(297.0), "Layer 1"
    );
    let layer = doc.get_page(page1).get_layer(layer1);
    let font = doc.add_builtin_font(BuiltinFont::Helvetica)?;
    let bold = doc.add_builtin_font(BuiltinFont::HelveticaBold)?;

    let mut y = Mm(280.0);  // Start from top

    // Title
    layer.use_text(&copy.title, 16.0, Mm(20.0), y, &bold);
    y -= Mm(10.0);

    // Priority items
    if !copy.priority_items.is_empty() {
        layer.use_text("PRIORITY:", 12.0, Mm(20.0), y, &bold);
        y -= Mm(7.0);
        for item in &copy.priority_items {
            let text = format!("  {} ({})", item.text, item.source);
            // Word-wrap and render
            render_wrapped_text(&layer, &text, 10.0, Mm(25.0), &mut y, &font, Mm(165.0));
            y -= Mm(3.0);
        }
        y -= Mm(5.0);
    }

    // Questions
    layer.use_text("YOUR QUESTIONS:", 12.0, Mm(20.0), y, &bold);
    y -= Mm(7.0);
    for (i, q) in copy.questions.iter().enumerate() {
        let text = format!("  {}. {}", i + 1, q.question);
        render_wrapped_text(&layer, &text, 10.0, Mm(25.0), &mut y, &font, Mm(165.0));
        y -= Mm(5.0);

        // Check if we need a new page
        if y < Mm(30.0) {
            // Add new page logic
        }
    }

    // Symptoms to mention
    if !copy.symptoms_to_mention.is_empty() {
        y -= Mm(5.0);
        layer.use_text("SYMPTOMS TO MENTION:", 12.0, Mm(20.0), y, &bold);
        y -= Mm(7.0);
        for s in &copy.symptoms_to_mention {
            let text = format!("  · {}", s.description);
            render_wrapped_text(&layer, &text, 10.0, Mm(25.0), &mut y, &font, Mm(165.0));
            y -= Mm(3.0);
        }
    }

    // Medication changes
    if !copy.medication_changes.is_empty() {
        y -= Mm(5.0);
        layer.use_text("MEDICATION CHANGES:", 12.0, Mm(20.0), y, &bold);
        y -= Mm(7.0);
        for mc in &copy.medication_changes {
            let text = format!("  · {}", mc.description);
            render_wrapped_text(&layer, &text, 10.0, Mm(25.0), &mut y, &font, Mm(165.0));
            y -= Mm(3.0);
        }
    }

    // Reminder footer
    y -= Mm(10.0);
    layer.use_text(&copy.reminder, 10.0, Mm(20.0), y, &bold);

    doc.save_to_bytes().map_err(CohearaError::from)
}

/// Generates a PDF from the professional copy
pub fn generate_professional_pdf(
    copy: &ProfessionalCopy,
) -> Result<Vec<u8>, CohearaError> {
    // Similar structure — more tabular, clinical formatting
    // Uses monospace font sections for lab results alignment
    // Includes disclaimer at top and bottom
    // Implementation follows same pattern as patient PDF
    todo!("Implementation follows patient PDF pattern with clinical formatting")
}
```

### Save and Print Flow

```rust
/// Saves PDF to file and optionally opens for printing
pub fn export_pdf(
    pdf_bytes: &[u8],
    filename: &str,
    profile_data_dir: &Path,
) -> Result<PathBuf, CohearaError> {
    let exports_dir = profile_data_dir.join("exports");
    std::fs::create_dir_all(&exports_dir)?;

    let path = exports_dir.join(filename);
    std::fs::write(&path, pdf_bytes)?;

    Ok(path)
}
```

Frontend uses Tauri's `shell.open()` to open the PDF in the system's default viewer for printing.

---

## [8] Post-Appointment Notes

### Guided Note Flow

Triggered from appointment history or a nudge after the appointment date passes.

```
"How did the appointment with Dr. Chen go?"

What did the doctor say?
[text area — required]

Any changes to your medications or treatment?
[text area — required]

Any follow-up needed?
[text area — optional]

Anything else you want to note?
[text area — optional]

[Save notes]
```

### Storage

Post-appointment notes stored in the `appointments.post_notes` field as JSON:

```rust
pub fn save_post_notes(
    conn: &rusqlite::Connection,
    notes: &PostAppointmentNotes,
) -> Result<(), CohearaError> {
    let notes_json = serde_json::to_string(notes)?;

    conn.execute(
        "UPDATE appointments SET post_notes = ?1, type = 'completed'
         WHERE id = ?2",
        params![notes_json, notes.appointment_id],
    )?;

    Ok(())
}
```

### Ingestion Into Constellation

Post-appointment notes are also:
1. Embedded in LanceDB (semantic searchable — "what did the doctor say about X?")
2. Checked by coherence engine (if notes mention medication changes, cross-reference with existing data)

---

## [9] Appointment History

### History View

```
┌────────────────────────────────────────────┐
│ APPOINTMENTS                               │
│                                            │
│ UPCOMING                                   │
│ ┌────────────────────────────────────────┐ │
│ │ Dr. Chen · GP · February 20           │ │
│ │ Prep ready ✓           [View prep]     │ │
│ └────────────────────────────────────────┘ │
│                                            │
│ PAST                                       │
│ ┌────────────────────────────────────────┐ │
│ │ Dr. Moreau · Cardiology · January 5   │ │
│ │ Notes recorded ✓        [View notes]   │ │
│ └────────────────────────────────────────┘ │
│ ┌────────────────────────────────────────┐ │
│ │ Pharmacist Dubois · Dec 20            │ │
│ │ No prep generated      [Prepare now]   │ │
│ └────────────────────────────────────────┘ │
│                                            │
│      [+ Prepare for new appointment]       │
└────────────────────────────────────────────┘
```

### Post-Appointment Nudge

If appointment date has passed and no post-notes recorded:
"Your appointment with Dr. Chen was 2 days ago. How did it go?"
[Add notes] [Skip]

---

## [10] Tauri Commands (IPC)

```rust
// src-tauri/src/commands/appointment.rs

use tauri::State;

/// Lists known professionals for the selector
#[tauri::command]
pub async fn list_professionals(
    state: State<'_, AppState>,
) -> Result<Vec<ProfessionalInfo>, String> {
    let session_guard = state.active_session.lock()
        .map_err(|e| format!("Lock error: {e}"))?;
    let session = session_guard.as_ref()
        .ok_or("No active profile session")?;
    let conn = session.db_connection()?;
    state.update_activity();

    conn.prepare(
        "SELECT id, name, specialty, institution, last_seen_date
         FROM professionals ORDER BY last_seen_date DESC"
    )?
    .query_map([], |row| {
        Ok(ProfessionalInfo {
            id: row.get(0)?,
            name: row.get(1)?,
            specialty: row.get(2)?,
            institution: row.get(3)?,
            last_seen_date: row.get(4)?,
        })
    })?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| e.to_string())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfessionalInfo {
    pub id: Uuid,
    pub name: String,
    pub specialty: String,
    pub institution: Option<String>,
    pub last_seen_date: Option<NaiveDate>,
}

/// Creates appointment and generates prep
#[tauri::command]
pub async fn prepare_appointment(
    state: State<'_, AppState>,
    request: AppointmentRequest,
) -> Result<AppointmentPrep, String> {
    let session_guard = state.active_session.lock()
        .map_err(|e| format!("Lock error: {e}"))?;
    let session = session_guard.as_ref()
        .ok_or("No active profile session")?;
    let conn = session.db_connection()?;

    // Resolve professional (existing or create new)
    let professional_id = match request.professional_id {
        Some(id) => id,
        None => {
            let new_prof = request.new_professional
                .ok_or("Must provide professional_id or new_professional")?;
            create_professional(&conn, &new_prof)?
        }
    };

    // Create appointment record
    let appointment_id = Uuid::new_v4();
    conn.execute(
        "INSERT INTO appointments (id, professional_id, date, type, pre_summary_generated)
         VALUES (?1, ?2, ?3, 'upcoming', 0)",
        params![appointment_id, professional_id, request.date],
    ).map_err(|e| format!("Failed to create appointment: {e}"))?;

    // Assemble data for both copies
    let prep_data = assemble_patient_prep_data(&conn, session, professional_id, request.date)
        .map_err(|e| e.to_string())?;

    // Generate patient copy (uses MedGemma for questions)
    let patient_copy = generate_patient_copy(&prep_data, session)
        .map_err(|e| format!("Failed to generate patient copy: {e}"))?;

    // Build professional copy (template-based, no LLM)
    let professional_copy = build_professional_copy(&prep_data);

    // Mark as generated
    conn.execute(
        "UPDATE appointments SET pre_summary_generated = 1 WHERE id = ?1",
        params![appointment_id],
    ).map_err(|e| format!("Failed to update appointment: {e}"))?;

    state.update_activity();

    Ok(AppointmentPrep {
        appointment_id,
        professional_name: prep_data.professional_name.clone(),
        professional_specialty: prep_data.professional_specialty.clone(),
        appointment_date: request.date,
        patient_copy,
        professional_copy,
        generated_at: chrono::Local::now().naive_local(),
    })
}

/// Exports prep as PDF
#[tauri::command]
pub async fn export_prep_pdf(
    state: State<'_, AppState>,
    prep: AppointmentPrep,
    copy_type: String,  // "patient", "professional", "both"
) -> Result<Vec<String>, String> {
    let session_guard = state.active_session.lock()
        .map_err(|e| format!("Lock error: {e}"))?;
    let session = session_guard.as_ref()
        .ok_or("No active profile session")?;

    let data_dir = session.profile_data_dir();
    let mut paths = Vec::new();

    if copy_type == "patient" || copy_type == "both" {
        let pdf = generate_patient_pdf(&prep.patient_copy)
            .map_err(|e| format!("Patient PDF error: {e}"))?;
        let filename = format!("patient-prep-{}-{}.pdf",
            prep.professional_name.replace(' ', "-"),
            prep.appointment_date);
        let path = export_pdf(&pdf, &filename, &data_dir)
            .map_err(|e| e.to_string())?;
        paths.push(path.to_string_lossy().into_owned());
    }

    if copy_type == "professional" || copy_type == "both" {
        let pdf = generate_professional_pdf(&prep.professional_copy)
            .map_err(|e| format!("Professional PDF error: {e}"))?;
        let filename = format!("professional-summary-{}-{}.pdf",
            prep.professional_name.replace(' ', "-"),
            prep.appointment_date);
        let path = export_pdf(&pdf, &filename, &data_dir)
            .map_err(|e| e.to_string())?;
        paths.push(path.to_string_lossy().into_owned());
    }

    state.update_activity();
    Ok(paths)
}

/// Saves post-appointment notes
#[tauri::command]
pub async fn save_appointment_notes(
    state: State<'_, AppState>,
    notes: PostAppointmentNotes,
) -> Result<(), String> {
    let session_guard = state.active_session.lock()
        .map_err(|e| format!("Lock error: {e}"))?;
    let session = session_guard.as_ref()
        .ok_or("No active profile session")?;
    let conn = session.db_connection()?;

    save_post_notes(&conn, &notes)
        .map_err(|e| e.to_string())?;

    // Embed notes in LanceDB for semantic search
    let embed_text = format!(
        "Appointment with {} on {}: {} Changes: {} Follow-up: {}",
        fetch_appointment_professional(&conn, notes.appointment_id)?,
        fetch_appointment_date(&conn, notes.appointment_id)?,
        notes.doctor_said,
        notes.changes_made,
        notes.follow_up.unwrap_or_default()
    );
    embed_post_notes(session, notes.appointment_id, &embed_text)?;

    state.update_activity();
    Ok(())
}

/// Lists all appointments
#[tauri::command]
pub async fn list_appointments(
    state: State<'_, AppState>,
) -> Result<Vec<StoredAppointment>, String> {
    let session_guard = state.active_session.lock()
        .map_err(|e| format!("Lock error: {e}"))?;
    let session = session_guard.as_ref()
        .ok_or("No active profile session")?;
    let conn = session.db_connection()?;
    state.update_activity();

    conn.prepare(
        "SELECT a.id, p.name, p.specialty, a.date, a.type,
                a.pre_summary_generated, (a.post_notes IS NOT NULL) as has_notes
         FROM appointments a
         JOIN professionals p ON a.professional_id = p.id
         ORDER BY a.date DESC"
    )?
    .query_map([], |row| {
        Ok(StoredAppointment {
            id: row.get(0)?,
            professional_name: row.get(1)?,
            professional_specialty: row.get(2)?,
            date: row.get(3)?,
            appointment_type: row.get(4)?,
            prep_generated: row.get(5)?,
            has_post_notes: row.get(6)?,
        })
    })?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| e.to_string())
}
```

### Frontend API

```typescript
// src/lib/api/appointment.ts
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-shell';
import type {
  AppointmentRequest, AppointmentPrep, PostAppointmentNotes,
  StoredAppointment, ProfessionalInfo
} from '$lib/types/appointment';

export async function listProfessionals(): Promise<ProfessionalInfo[]> {
  return invoke<ProfessionalInfo[]>('list_professionals');
}

export async function prepareAppointment(request: AppointmentRequest): Promise<AppointmentPrep> {
  return invoke<AppointmentPrep>('prepare_appointment', { request });
}

export async function exportPrepPdf(
  prep: AppointmentPrep,
  copyType: 'patient' | 'professional' | 'both'
): Promise<string[]> {
  const paths = await invoke<string[]>('export_prep_pdf', { prep, copyType });
  // Open first PDF in default viewer
  if (paths.length > 0) {
    await open(paths[0]);
  }
  return paths;
}

export async function saveAppointmentNotes(notes: PostAppointmentNotes): Promise<void> {
  return invoke('save_appointment_notes', { notes });
}

export async function listAppointments(): Promise<StoredAppointment[]> {
  return invoke<StoredAppointment[]>('list_appointments');
}
```

---

## [11] Svelte Components

### Appointment Screen

```svelte
<!-- src/lib/components/appointment/AppointmentScreen.svelte -->
<script lang="ts">
  import { onMount } from 'svelte';
  import { listAppointments, listProfessionals } from '$lib/api/appointment';
  import type { StoredAppointment } from '$lib/types/appointment';
  import PrepFlow from './PrepFlow.svelte';
  import AppointmentHistory from './AppointmentHistory.svelte';
  import PostNotesFlow from './PostNotesFlow.svelte';

  interface Props {
    onNavigate: (screen: string, params?: Record<string, string>) => void;
  }
  let { onNavigate }: Props = $props();

  type View = 'history' | 'prep' | 'post-notes';
  let view: View = $state('history');
  let appointments: StoredAppointment[] = $state([]);
  let selectedAppointmentId: string | null = $state(null);
  let loading = $state(true);

  async function refresh() {
    loading = true;
    appointments = await listAppointments();
    loading = false;
  }

  onMount(() => { refresh(); });
</script>

<div class="flex flex-col min-h-screen pb-20 bg-stone-50">
  <header class="px-6 pt-6 pb-4 flex items-center justify-between">
    <h1 class="text-2xl font-bold text-stone-800">Appointments</h1>
    {#if view === 'history'}
      <button
        class="px-4 py-2 bg-[var(--color-primary)] text-white rounded-xl text-sm
               font-medium min-h-[44px]"
        onclick={() => view = 'prep'}
      >
        + Prepare
      </button>
    {/if}
  </header>

  {#if view === 'prep'}
    <PrepFlow
      onComplete={async () => { view = 'history'; await refresh(); }}
      onCancel={() => view = 'history'}
    />
  {:else if view === 'post-notes' && selectedAppointmentId}
    <PostNotesFlow
      appointmentId={selectedAppointmentId}
      onComplete={async () => { view = 'history'; await refresh(); }}
      onCancel={() => view = 'history'}
    />
  {:else}
    <AppointmentHistory
      {appointments}
      {loading}
      onPrepare={() => view = 'prep'}
      onAddNotes={(id) => { selectedAppointmentId = id; view = 'post-notes'; }}
      {onNavigate}
    />
  {/if}
</div>
```

### Prep Flow Component

```svelte
<!-- src/lib/components/appointment/PrepFlow.svelte -->
<script lang="ts">
  import { listProfessionals, prepareAppointment, exportPrepPdf } from '$lib/api/appointment';
  import type { AppointmentPrep, ProfessionalInfo } from '$lib/types/appointment';
  import ProfessionalSelector from './ProfessionalSelector.svelte';
  import PrepViewer from './PrepViewer.svelte';

  interface Props {
    onComplete: () => void;
    onCancel: () => void;
  }
  let { onComplete, onCancel }: Props = $props();

  type Step = 'professional' | 'date' | 'generating' | 'viewing';

  let step: Step = $state('professional');
  let professionals: ProfessionalInfo[] = $state([]);
  let selectedProfessionalId: string | null = $state(null);
  let newProfessional: { name: string; specialty: string; institution: string | null } | null = $state(null);
  let appointmentDate = $state('');
  let prep: AppointmentPrep | null = $state(null);
  let error: string | null = $state(null);

  import { onMount } from 'svelte';
  onMount(async () => {
    professionals = await listProfessionals();
  });

  async function generate() {
    step = 'generating';
    error = null;
    try {
      prep = await prepareAppointment({
        professional_id: selectedProfessionalId,
        new_professional: newProfessional,
        date: appointmentDate,
      });
      step = 'viewing';
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
      step = 'date';
    }
  }
</script>

<div class="px-6 py-4">
  <button class="text-stone-500 text-sm mb-4 min-h-[44px]" onclick={onCancel}>
    &larr; Cancel
  </button>

  {#if step === 'professional'}
    <h2 class="text-xl font-semibold text-stone-800 mb-4">
      Which doctor is this appointment with?
    </h2>
    <ProfessionalSelector
      {professionals}
      onSelect={(id) => { selectedProfessionalId = id; step = 'date'; }}
      onCreateNew={(prof) => { newProfessional = prof; step = 'date'; }}
    />

  {:else if step === 'date'}
    <h2 class="text-xl font-semibold text-stone-800 mb-4">When is the appointment?</h2>
    <input
      type="date"
      class="w-full px-4 py-3 rounded-xl border border-stone-200 text-stone-700
             focus:outline-none focus:ring-2 focus:ring-[var(--color-primary)] min-h-[44px]"
      bind:value={appointmentDate}
    />
    {#if error}
      <p class="text-red-600 text-sm mt-2">{error}</p>
    {/if}
    <button
      class="w-full mt-6 px-4 py-3 bg-[var(--color-primary)] text-white rounded-xl
             font-medium min-h-[44px] disabled:opacity-50"
      disabled={!appointmentDate}
      onclick={generate}
    >
      Generate preparation
    </button>

  {:else if step === 'generating'}
    <div class="flex flex-col items-center justify-center py-16">
      <div class="animate-spin w-8 h-8 border-2 border-[var(--color-primary)]
                  border-t-transparent rounded-full mb-4"></div>
      <p class="text-stone-500">Preparing your appointment summary...</p>
      <p class="text-xs text-stone-400 mt-1">This may take up to 15 seconds</p>
    </div>

  {:else if step === 'viewing' && prep}
    <PrepViewer
      {prep}
      onExport={async (type) => { await exportPrepPdf(prep!, type); }}
      onDone={onComplete}
    />
  {/if}
</div>
```

### Prep Viewer (Tab View)

```svelte
<!-- src/lib/components/appointment/PrepViewer.svelte -->
<script lang="ts">
  import type { AppointmentPrep } from '$lib/types/appointment';

  interface Props {
    prep: AppointmentPrep;
    onExport: (type: 'patient' | 'professional' | 'both') => Promise<void>;
    onDone: () => void;
  }
  let { prep, onExport, onDone }: Props = $props();

  let activeTab: 'patient' | 'professional' = $state('patient');
  let exporting = $state(false);

  async function handleExport(type: 'patient' | 'professional' | 'both') {
    exporting = true;
    try {
      await onExport(type);
    } finally {
      exporting = false;
    }
  }
</script>

<div>
  <h2 class="text-xl font-semibold text-stone-800 mb-2">
    Appointment with {prep.professional_name}
  </h2>
  <p class="text-sm text-stone-500 mb-4">{prep.appointment_date}</p>

  <!-- Tab switcher -->
  <div class="flex gap-2 mb-4">
    <button
      class="px-4 py-2 rounded-lg text-sm font-medium min-h-[44px]
             {activeTab === 'patient' ? 'bg-[var(--color-primary)] text-white' : 'bg-stone-100 text-stone-600'}"
      onclick={() => activeTab = 'patient'}
    >
      Your questions
    </button>
    <button
      class="px-4 py-2 rounded-lg text-sm font-medium min-h-[44px]
             {activeTab === 'professional' ? 'bg-[var(--color-primary)] text-white' : 'bg-stone-100 text-stone-600'}"
      onclick={() => activeTab = 'professional'}
    >
      Doctor summary
    </button>
  </div>

  <!-- Content -->
  <div class="bg-white rounded-xl p-6 border border-stone-100 shadow-sm mb-4
              max-h-[60vh] overflow-y-auto">
    {#if activeTab === 'patient'}
      <h3 class="font-bold text-stone-800 mb-4">{prep.patient_copy.title}</h3>

      {#if prep.patient_copy.priority_items.length > 0}
        <div class="mb-4 p-3 bg-amber-50 rounded-lg border border-amber-200">
          <h4 class="text-sm font-medium text-amber-800 mb-2">PRIORITY</h4>
          {#each prep.patient_copy.priority_items as item}
            <p class="text-sm text-amber-700 mb-1">{item.text}</p>
            <p class="text-xs text-amber-600">{item.source}</p>
          {/each}
        </div>
      {/if}

      <h4 class="text-sm font-medium text-stone-600 mb-2">YOUR QUESTIONS</h4>
      {#each prep.patient_copy.questions as q, i}
        <div class="mb-3">
          <p class="text-sm text-stone-800">{i + 1}. {q.question}</p>
          <p class="text-xs text-stone-500 mt-0.5">{q.context}</p>
        </div>
      {/each}

      {#if prep.patient_copy.symptoms_to_mention.length > 0}
        <h4 class="text-sm font-medium text-stone-600 mt-4 mb-2">SYMPTOMS TO MENTION</h4>
        {#each prep.patient_copy.symptoms_to_mention as s}
          <p class="text-sm text-stone-700 mb-1">· {s.description}</p>
        {/each}
      {/if}

      {#if prep.patient_copy.medication_changes.length > 0}
        <h4 class="text-sm font-medium text-stone-600 mt-4 mb-2">MEDICATION CHANGES</h4>
        {#each prep.patient_copy.medication_changes as mc}
          <p class="text-sm text-stone-700 mb-1">· {mc.description}</p>
        {/each}
      {/if}

      <p class="text-sm font-medium text-stone-600 mt-6">{prep.patient_copy.reminder}</p>

    {:else}
      <pre class="text-xs text-stone-700 whitespace-pre-wrap font-mono leading-relaxed">
{prep.professional_copy.header.title} — {prep.professional_copy.header.date}
{prep.professional_copy.header.professional}
{prep.professional_copy.header.disclaimer}

CURRENT MEDICATIONS:
{#each prep.professional_copy.current_medications as m}
  {m.name} {m.dose} — {m.frequency} — {m.prescriber}{m.is_recent_change ? ' [CHANGED]' : ''}
{/each}

LAB RESULTS:
{#each prep.professional_copy.lab_results as l}
  {l.test_name}: {l.value} {l.unit} (ref: {l.reference_range}) [{l.abnormal_flag}] — {l.date}
{/each}

PATIENT-REPORTED SYMPTOMS:
{#each prep.professional_copy.patient_reported_symptoms as s}
  · {s.description} — severity {s.severity}/5 — onset {s.onset_date}
{/each}

OBSERVATIONS FOR DISCUSSION:
{#each prep.professional_copy.observations_for_discussion as o}
  [{o.severity}] {o.observation}
{/each}
      </pre>
      <p class="text-xs text-stone-400 mt-4">{prep.professional_copy.disclaimer}</p>
    {/if}
  </div>

  <!-- Export buttons -->
  <div class="flex gap-2">
    <button
      class="flex-1 px-4 py-3 bg-white border border-stone-200 rounded-xl
             text-sm font-medium text-stone-700 min-h-[44px] disabled:opacity-50"
      disabled={exporting}
      onclick={() => handleExport('patient')}
    >
      Print patient copy
    </button>
    <button
      class="flex-1 px-4 py-3 bg-white border border-stone-200 rounded-xl
             text-sm font-medium text-stone-700 min-h-[44px] disabled:opacity-50"
      disabled={exporting}
      onclick={() => handleExport('professional')}
    >
      Print doctor copy
    </button>
  </div>
  <button
    class="w-full mt-2 px-4 py-3 bg-[var(--color-primary)] text-white rounded-xl
           text-sm font-medium min-h-[44px] disabled:opacity-50"
    disabled={exporting}
    onclick={() => handleExport('both')}
  >
    Print both
  </button>
  <button
    class="w-full mt-2 px-4 py-3 text-stone-500 text-sm min-h-[44px]"
    onclick={onDone}
  >
    Done
  </button>
</div>
```

---

## [12] Error Handling

| Error | User Message | Recovery |
|-------|-------------|----------|
| MedGemma generation fails | "Couldn't generate your questions. Would you like to try again?" | Retry button, data still available for template-only fallback |
| Ollama not running | "The AI model isn't available. The doctor summary can still be generated from your records." | Generate professional copy only (template-based, no LLM) |
| PDF export fails | "Couldn't create PDF. Your preparation is still saved on screen." | Retry, or copy text manually |
| No medications/data | "You don't have much data yet. Loading more documents will make your preparation more helpful." | Generate with available data (may be sparse) |
| Database error | "Something went wrong. Please try again." | Retry |

---

## [13] Security

- All patient data decrypted only during generation — never written unencrypted to disk except as PDF in profile exports dir
- PDFs stored in profile-specific exports directory (encrypted at profile level)
- Professional copy includes disclaimer: "AI-generated, not clinical advice"
- Post-appointment notes encrypted before SQLite storage
- No data sent over network — generation is local via Ollama

---

## [14] Testing

### Unit Tests (Rust)

| Test | What |
|------|------|
| `test_assemble_prep_data` | Correct data gathered from all tables |
| `test_assemble_prep_data_first_visit` | Handles no previous visit (all data since epoch) |
| `test_build_professional_copy` | Correct template construction |
| `test_professional_copy_recent_changes_flagged` | [CHANGED] and [NEW] flags on recent medications |
| `test_professional_copy_lab_abnormal_flags` | Correct abnormal flags in lab section |
| `test_patient_questions_prompt_construction` | Correct prompt with patient data injected |
| `test_pdf_patient_generation` | PDF bytes generated without error |
| `test_pdf_professional_generation` | PDF bytes generated without error |
| `test_save_post_notes` | Notes saved and appointment marked completed |
| `test_post_notes_embedded` | Post-notes embedded in LanceDB |
| `test_list_appointments_ordered` | Appointments ordered by date DESC |
| `test_create_new_professional` | New professional created during prep flow |
| `test_appointment_with_existing_professional` | Uses existing professional |
| `test_export_pdf_creates_file` | PDF file created in exports directory |
| `test_medication_changes_since_last_visit` | Correct changes detected |
| `test_observations_included` | Coherence observations included in prep |

### Frontend Tests

| Test | What |
|------|------|
| `test_professional_selector_list` | Known professionals displayed |
| `test_professional_selector_new` | Can create new professional |
| `test_date_picker_default` | Default date reasonable |
| `test_generating_spinner` | Loading state shown during generation |
| `test_patient_copy_rendered` | Questions, symptoms, changes displayed |
| `test_professional_copy_rendered` | Structured summary displayed |
| `test_tab_switching` | Can switch between patient and professional views |
| `test_export_patient_pdf` | Export triggers invoke |
| `test_export_both_pdfs` | Both PDFs generated |
| `test_post_notes_flow` | Guided note capture works |

---

## [15] Performance

- Professional copy is template-based (no LLM): generates in < 100ms
- Patient questions require MedGemma: target < 15 seconds
- Data assembly is SQL queries only: < 500ms for typical constellation
- PDF generation: < 2 seconds per document
- Total flow: < 20 seconds end-to-end

---

## [16] Open Questions

- **Q1:** Should we offer a "quick prep" mode that skips MedGemma and uses template-only patient questions? Useful when Ollama is unavailable or slow. Current answer: yes, as fallback when LLM fails.
- **Q2:** Should PDFs include a Coheara watermark/logo? Current answer: minimal branding only — focus is on content.

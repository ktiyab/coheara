//! L4-02 Appointment Prep — backend types, data assembly, copy generation, PDF export.
//!
//! Two artifacts per appointment:
//! 1. Patient copy — plain-language questions + symptoms + medication changes
//! 2. Professional copy — structured clinical summary (template-based, no LLM)
//!
//! PDF generation via `printpdf`. Post-appointment notes stored as JSON in
//! `appointments.post_notes`.

use chrono::{Local, NaiveDate, NaiveDateTime};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::DatabaseError;

// ─── Types ────────────────────────────────────────────────────────────────────

/// Request to create an appointment and generate prep.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppointmentRequest {
    pub professional_id: Option<String>,
    pub new_professional: Option<NewProfessional>,
    pub date: String, // YYYY-MM-DD
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewProfessional {
    pub name: String,
    pub specialty: String,
    pub institution: Option<String>,
}

/// Professional info for selector.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfessionalInfo {
    pub id: String,
    pub name: String,
    pub specialty: Option<String>,
    pub institution: Option<String>,
    pub last_seen_date: Option<String>,
}

/// Generated appointment preparation result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppointmentPrep {
    pub appointment_id: String,
    pub professional_name: String,
    pub professional_specialty: String,
    pub appointment_date: String,
    pub patient_copy: PatientCopy,
    pub professional_copy: ProfessionalCopy,
    pub generated_at: String,
}

/// Patient-facing questions and reminders.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatientCopy {
    pub title: String,
    pub priority_items: Vec<PrepItem>,
    pub questions: Vec<PrepQuestion>,
    pub symptoms_to_mention: Vec<SymptomMention>,
    pub medication_changes: Vec<MedicationChange>,
    pub reminder: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrepItem {
    pub text: String,
    pub source: String,
    pub priority: String, // "Critical", "Important", "Standard"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrepQuestion {
    pub question: String,
    pub context: String,
    pub relevance_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymptomMention {
    pub description: String,
    pub severity: u8,
    pub onset_date: String,
    pub still_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MedicationChange {
    pub description: String,
    pub change_type: String,
    pub date: String,
}

/// Professional-facing structured summary.
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
    pub title: String,
    pub date: String,
    pub professional: String,
    pub disclaimer: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MedicationSummary {
    pub name: String,
    pub dose: String,
    pub frequency: String,
    pub prescriber: String,
    pub start_date: String,
    pub is_recent_change: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeSummary {
    pub description: String,
    pub date: String,
    pub change_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LabSummary {
    pub test_name: String,
    pub value: String,
    pub unit: String,
    pub reference_range: String,
    pub abnormal_flag: String,
    pub date: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymptomSummary {
    pub description: String,
    pub severity: u8,
    pub onset_date: String,
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
    pub date: String,
    pub professional: String,
}

/// Post-appointment notes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostAppointmentNotes {
    pub appointment_id: String,
    pub doctor_said: String,
    pub changes_made: String,
    pub follow_up: Option<String>,
    pub general_notes: Option<String>,
}

/// Stored appointment for history list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredAppointment {
    pub id: String,
    pub professional_name: String,
    pub professional_specialty: String,
    pub date: String,
    pub appointment_type: String,
    pub prep_generated: bool,
    pub has_post_notes: bool,
}

// ─── Internal data assembly types ─────────────────────────────────────────────

/// Intermediate data assembled from multiple tables for copy generation.
struct PrepData {
    professional_name: String,
    professional_specialty: String,
    appointment_date: NaiveDate,
    since_date: NaiveDate,
    medications: Vec<ActiveMedication>,
    med_changes: Vec<MedChange>,
    labs: Vec<RecentLab>,
    symptoms: Vec<RecentSymptom>,
    source_docs: Vec<SourceDoc>,
}

struct ActiveMedication {
    name: String,
    dose: String,
    frequency: String,
    prescriber_name: String,
    start_date: String,
}

struct MedChange {
    medication_name: String,
    old_dose: Option<String>,
    new_dose: String,
    change_date: String,
    change_type: String,
}

struct RecentLab {
    test_name: String,
    value: String,
    unit: String,
    range_low: String,
    range_high: String,
    abnormal_flag: String,
    collection_date: String,
}

struct RecentSymptom {
    specific: String,
    category: String,
    severity: u8,
    onset_date: String,
    still_active: bool,
    duration: Option<String>,
}

struct SourceDoc {
    doc_type: String,
    date: String,
    professional: String,
}

// ─── MedGemma prompt (ready for future LLM integration) ──────────────────────

/// Prompt template for MedGemma-powered patient question generation.
/// Currently unused — patient copy uses template-based generation.
/// When Ollama integration is wired up, pass assembled data through this prompt.
#[allow(dead_code)]
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

RECENT SYMPTOMS:
{symptoms}

MEDICATION CHANGES SINCE LAST VISIT:
{medication_changes}

CURRENT MEDICATIONS:
{medications}

RECENT LAB RESULTS:
{labs}

Output format (JSON array):
[
  {"question": "...", "context": "why this matters", "relevance_score": 0.9},
  ...
]
"#;

// ─── Specialty options ────────────────────────────────────────────────────────

pub const SPECIALTIES: &[&str] = &[
    "GP",
    "Cardiologist",
    "Neurologist",
    "Dermatologist",
    "Endocrinologist",
    "Gastroenterologist",
    "Oncologist",
    "Pharmacist",
    "Nurse",
    "Specialist",
    "Other",
];

// ─── Repository functions ─────────────────────────────────────────────────────

/// Lists known professionals ordered by last_seen_date DESC.
pub fn list_professionals(conn: &Connection) -> Result<Vec<ProfessionalInfo>, DatabaseError> {
    let mut stmt = conn.prepare(
        "SELECT id, name, specialty, institution, last_seen_date
         FROM professionals
         ORDER BY last_seen_date DESC NULLS LAST, name ASC",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(ProfessionalInfo {
            id: row.get(0)?,
            name: row.get(1)?,
            specialty: row.get(2)?,
            institution: row.get(3)?,
            last_seen_date: row.get(4)?,
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>().map_err(DatabaseError::from)
}

/// Creates a new professional and returns the UUID.
pub fn create_professional(
    conn: &Connection,
    new_prof: &NewProfessional,
) -> Result<String, DatabaseError> {
    let id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO professionals (id, name, specialty, institution)
         VALUES (?1, ?2, ?3, ?4)",
        params![id, new_prof.name, new_prof.specialty, new_prof.institution],
    )?;
    Ok(id)
}

/// Creates an appointment record and returns the UUID.
pub fn create_appointment(
    conn: &Connection,
    professional_id: &str,
    date: &NaiveDate,
) -> Result<String, DatabaseError> {
    let id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO appointments (id, professional_id, date, type, pre_summary_generated)
         VALUES (?1, ?2, ?3, 'upcoming', 0)",
        params![id, professional_id, date.to_string()],
    )?;
    Ok(id)
}

/// Marks an appointment as prep-generated.
pub fn mark_prep_generated(conn: &Connection, appointment_id: &str) -> Result<(), DatabaseError> {
    let changed = conn.execute(
        "UPDATE appointments SET pre_summary_generated = 1 WHERE id = ?1",
        params![appointment_id],
    )?;
    if changed == 0 {
        return Err(DatabaseError::NotFound {
            entity_type: "Appointment".into(),
            id: appointment_id.into(),
        });
    }
    Ok(())
}

/// Updates the professional's last_seen_date.
fn update_last_seen(conn: &Connection, professional_id: &str, date: &NaiveDate) -> Result<(), DatabaseError> {
    conn.execute(
        "UPDATE professionals SET last_seen_date = ?1
         WHERE id = ?2 AND (last_seen_date IS NULL OR last_seen_date < ?1)",
        params![date.to_string(), professional_id],
    )?;
    Ok(())
}

/// Assembles all data needed for appointment prep from the database.
fn assemble_prep_data(
    conn: &Connection,
    professional_id: &str,
    appointment_date: NaiveDate,
) -> Result<PrepData, DatabaseError> {
    // Fetch professional name + specialty
    let (prof_name, prof_specialty): (String, Option<String>) = conn.query_row(
        "SELECT name, specialty FROM professionals WHERE id = ?1",
        params![professional_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    ).map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => DatabaseError::NotFound {
            entity_type: "Professional".into(),
            id: professional_id.into(),
        },
        other => DatabaseError::from(other),
    })?;

    // Find last completed visit to this professional
    let last_visit: Option<String> = conn.query_row(
        "SELECT MAX(date) FROM appointments
         WHERE professional_id = ?1 AND type = 'completed'",
        params![professional_id],
        |row| row.get(0),
    ).unwrap_or(None);

    let since_date = last_visit
        .and_then(|d| NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok())
        .unwrap_or(NaiveDate::from_ymd_opt(2000, 1, 1).unwrap());

    let since_str = since_date.to_string();

    // Active medications with prescriber names
    let medications = fetch_active_medications(conn)?;

    // Medication changes since last visit (new meds + dose changes)
    let med_changes = fetch_medication_changes(conn, &since_str)?;

    // Recent lab results since last visit
    let labs = fetch_recent_labs(conn, &since_str)?;

    // Symptoms since last visit
    let symptoms = fetch_recent_symptoms(conn, &since_str)?;

    // Source documents since last visit
    let source_docs = fetch_source_documents(conn, &since_str)?;

    Ok(PrepData {
        professional_name: prof_name,
        professional_specialty: prof_specialty.unwrap_or_default(),
        appointment_date,
        since_date,
        medications,
        med_changes,
        labs,
        symptoms,
        source_docs,
    })
}

fn fetch_active_medications(conn: &Connection) -> Result<Vec<ActiveMedication>, DatabaseError> {
    let mut stmt = conn.prepare(
        "SELECT m.generic_name, m.dose, m.frequency, COALESCE(p.name, 'Unknown'), m.start_date
         FROM medications m
         LEFT JOIN professionals p ON m.prescriber_id = p.id
         WHERE m.status = 'active'
         ORDER BY m.start_date DESC",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(ActiveMedication {
            name: row.get(0)?,
            dose: row.get(1)?,
            frequency: row.get(2)?,
            prescriber_name: row.get(3)?,
            start_date: row.get::<_, Option<String>>(4)?.unwrap_or_default(),
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>().map_err(DatabaseError::from)
}

fn fetch_medication_changes(
    conn: &Connection,
    since_date: &str,
) -> Result<Vec<MedChange>, DatabaseError> {
    // New medications started since last visit
    let mut new_meds: Vec<MedChange> = Vec::new();
    {
        let mut stmt = conn.prepare(
            "SELECT generic_name, dose, start_date
             FROM medications
             WHERE start_date >= ?1
             ORDER BY start_date DESC",
        )?;
        let rows = stmt.query_map(params![since_date], |row| {
            let name: String = row.get(0)?;
            let dose: String = row.get(1)?;
            let date: String = row.get::<_, Option<String>>(2)?.unwrap_or_default();
            Ok(MedChange {
                medication_name: name.clone(),
                old_dose: None,
                new_dose: dose.clone(),
                change_date: date.clone(),
                change_type: "started".into(),
            })
        })?;
        for row in rows {
            new_meds.push(row?);
        }
    }

    // Dose changes since last visit
    {
        let mut stmt = conn.prepare(
            "SELECT m.generic_name, dc.old_dose, dc.new_dose, dc.change_date
             FROM dose_changes dc
             JOIN medications m ON dc.medication_id = m.id
             WHERE dc.change_date >= ?1
             ORDER BY dc.change_date DESC",
        )?;
        let rows = stmt.query_map(params![since_date], |row| {
            Ok(MedChange {
                medication_name: row.get(0)?,
                old_dose: row.get(1)?,
                new_dose: row.get(2)?,
                change_date: row.get(3)?,
                change_type: "dose_changed".into(),
            })
        })?;
        for row in rows {
            new_meds.push(row?);
        }
    }

    Ok(new_meds)
}

fn fetch_recent_labs(
    conn: &Connection,
    since_date: &str,
) -> Result<Vec<RecentLab>, DatabaseError> {
    let mut stmt = conn.prepare(
        "SELECT test_name, COALESCE(CAST(value AS TEXT), value_text, ''),
                COALESCE(unit, ''), COALESCE(CAST(reference_range_low AS TEXT), ''),
                COALESCE(CAST(reference_range_high AS TEXT), ''), abnormal_flag, collection_date
         FROM lab_results
         WHERE collection_date >= ?1
         ORDER BY collection_date DESC",
    )?;

    let rows = stmt.query_map(params![since_date], |row| {
        Ok(RecentLab {
            test_name: row.get(0)?,
            value: row.get(1)?,
            unit: row.get(2)?,
            range_low: row.get(3)?,
            range_high: row.get(4)?,
            abnormal_flag: row.get(5)?,
            collection_date: row.get(6)?,
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>().map_err(DatabaseError::from)
}

fn fetch_recent_symptoms(
    conn: &Connection,
    since_date: &str,
) -> Result<Vec<RecentSymptom>, DatabaseError> {
    let mut stmt = conn.prepare(
        "SELECT specific, category, severity, onset_date, still_active, duration
         FROM symptoms
         WHERE onset_date >= ?1
         ORDER BY severity DESC, onset_date DESC",
    )?;

    let rows = stmt.query_map(params![since_date], |row| {
        Ok(RecentSymptom {
            specific: row.get(0)?,
            category: row.get(1)?,
            severity: row.get(2)?,
            onset_date: row.get(3)?,
            still_active: row.get(4)?,
            duration: row.get(5)?,
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>().map_err(DatabaseError::from)
}

fn fetch_source_documents(
    conn: &Connection,
    since_date: &str,
) -> Result<Vec<SourceDoc>, DatabaseError> {
    let mut stmt = conn.prepare(
        "SELECT d.type, d.ingestion_date, COALESCE(p.name, 'Unknown')
         FROM documents d
         LEFT JOIN professionals p ON 1=0
         WHERE d.ingestion_date >= ?1
         ORDER BY d.ingestion_date DESC
         LIMIT 20",
    )?;

    let rows = stmt.query_map(params![since_date], |row| {
        Ok(SourceDoc {
            doc_type: row.get(0)?,
            date: row.get(1)?,
            professional: row.get(2)?,
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>().map_err(DatabaseError::from)
}

// ─── Copy builders ────────────────────────────────────────────────────────────

/// Builds the professional copy from assembled data (template-based, no LLM).
fn build_professional_copy(data: &PrepData) -> ProfessionalCopy {
    let header = ProfessionalHeader {
        title: "COHEARA PATIENT SUMMARY".into(),
        date: data.appointment_date.to_string(),
        professional: format!(
            "For: {} ({})",
            data.professional_name,
            if data.professional_specialty.is_empty() { "Specialist" } else { &data.professional_specialty }
        ),
        disclaimer: "AI-generated from patient-loaded documents. Not clinical advice.".into(),
    };

    let since_str = data.since_date.to_string();

    let current_medications = data.medications.iter().map(|m| {
        let is_recent = m.start_date >= since_str
            || data.med_changes.iter().any(|c| c.medication_name == m.name);
        MedicationSummary {
            name: m.name.clone(),
            dose: m.dose.clone(),
            frequency: m.frequency.clone(),
            prescriber: m.prescriber_name.clone(),
            start_date: m.start_date.clone(),
            is_recent_change: is_recent,
        }
    }).collect();

    let changes_since_last_visit = data.med_changes.iter().map(|c| {
        let desc = match c.change_type.as_str() {
            "started" => format!("New: {} {} ({})", c.medication_name, c.new_dose, c.change_date),
            "dose_changed" => format!(
                "Changed: {} {} → {} ({})",
                c.medication_name,
                c.old_dose.as_deref().unwrap_or("?"),
                c.new_dose,
                c.change_date
            ),
            _ => format!("{}: {} ({})", c.change_type, c.medication_name, c.change_date),
        };
        ChangeSummary {
            description: desc,
            date: c.change_date.clone(),
            change_type: c.change_type.clone(),
        }
    }).collect();

    let lab_results = data.labs.iter().map(|l| {
        let range = if l.range_low.is_empty() && l.range_high.is_empty() {
            "N/A".into()
        } else {
            format!("{}-{}", l.range_low, l.range_high)
        };
        LabSummary {
            test_name: l.test_name.clone(),
            value: l.value.clone(),
            unit: l.unit.clone(),
            reference_range: range,
            abnormal_flag: l.abnormal_flag.clone(),
            date: l.collection_date.clone(),
        }
    }).collect();

    let patient_reported_symptoms = data.symptoms.iter().map(|s| {
        SymptomSummary {
            description: format!(
                "{} — {}{}",
                s.specific,
                s.category,
                if s.still_active { " (still active)" } else { "" }
            ),
            severity: s.severity,
            onset_date: s.onset_date.clone(),
            duration: s.duration.clone(),
        }
    }).collect();

    let source_documents = data.source_docs.iter().map(|d| {
        DocumentReference {
            document_type: d.doc_type.clone(),
            date: d.date.clone(),
            professional: d.professional.clone(),
        }
    }).collect();

    ProfessionalCopy {
        header,
        current_medications,
        changes_since_last_visit,
        lab_results,
        patient_reported_symptoms,
        // Observations deferred — coherence_observations table not in SQLite
        observations_for_discussion: Vec::new(),
        source_documents,
        disclaimer: "This summary is AI-generated from patient-loaded documents. \
                     It is not a clinical record and should not replace professional assessment."
            .into(),
    }
}

/// Builds the patient copy using template-based questions (no LLM).
/// When MedGemma integration is available, replace with LLM-generated questions.
fn build_patient_copy(data: &PrepData) -> PatientCopy {
    let title = format!(
        "Questions for {} — {}",
        data.professional_name,
        data.appointment_date.format("%B %-d, %Y")
    );

    // Priority items from critical lab results
    let priority_items: Vec<PrepItem> = data.labs.iter()
        .filter(|l| l.abnormal_flag == "critical_low" || l.abnormal_flag == "critical_high")
        .map(|l| PrepItem {
            text: format!(
                "Your {} result ({} {}) needs prompt attention. Please discuss this with your doctor.",
                l.test_name, l.value, l.unit
            ),
            source: format!("Lab report from {}", l.collection_date),
            priority: "Critical".into(),
        })
        .collect();

    // Template-based questions from patient data
    let mut questions: Vec<PrepQuestion> = Vec::new();

    // Q1: Medication changes
    if !data.med_changes.is_empty() {
        let names: Vec<&str> = data.med_changes.iter()
            .map(|c| c.medication_name.as_str())
            .collect();
        questions.push(PrepQuestion {
            question: format!(
                "My records show changes to my medications ({}). Are these working as expected?",
                names.join(", ")
            ),
            context: "Medication changes since last visit should be reviewed".into(),
            relevance_score: 0.95,
        });
    }

    // Q2: Active symptoms
    let active_symptoms: Vec<&RecentSymptom> = data.symptoms.iter()
        .filter(|s| s.still_active)
        .collect();
    if !active_symptoms.is_empty() {
        let descs: Vec<String> = active_symptoms.iter()
            .map(|s| s.specific.clone())
            .collect();
        questions.push(PrepQuestion {
            question: format!(
                "I've been experiencing {} — should I be concerned?",
                descs.join(" and ")
            ),
            context: "Active symptoms the doctor should know about".into(),
            relevance_score: 0.9,
        });
    }

    // Q3: Abnormal labs
    let abnormal_labs: Vec<&RecentLab> = data.labs.iter()
        .filter(|l| l.abnormal_flag != "normal")
        .collect();
    if !abnormal_labs.is_empty() {
        let names: Vec<&str> = abnormal_labs.iter()
            .map(|l| l.test_name.as_str())
            .collect();
        questions.push(PrepQuestion {
            question: format!(
                "My {} result{} flagged as abnormal. What does this mean for me?",
                names.join(" and "),
                if names.len() > 1 { "s were" } else { " was" }
            ),
            context: "Abnormal lab values warrant discussion".into(),
            relevance_score: 0.85,
        });
    }

    // Q4: Multiple prescribers
    let prescriber_names: std::collections::HashSet<&str> = data.medications.iter()
        .map(|m| m.prescriber_name.as_str())
        .filter(|n| *n != "Unknown")
        .collect();
    if prescriber_names.len() > 1 {
        questions.push(PrepQuestion {
            question: "I'm taking medications from different doctors. \
                      Should they know about each other's prescriptions?"
                .into(),
            context: "Multiple prescribers increases interaction risk".into(),
            relevance_score: 0.8,
        });
    }

    // Q5: General follow-up
    if questions.len() < 5 {
        questions.push(PrepQuestion {
            question: "Is there anything from my records that you'd like to discuss or follow up on?"
                .into(),
            context: "Open-ended question ensures nothing is missed".into(),
            relevance_score: 0.5,
        });
    }

    // Sort by relevance
    questions.sort_by(|a, b| b.relevance_score.partial_cmp(&a.relevance_score).unwrap_or(std::cmp::Ordering::Equal));
    questions.truncate(5);

    // Symptoms to mention
    let symptoms_to_mention: Vec<SymptomMention> = data.symptoms.iter().map(|s| {
        SymptomMention {
            description: format!(
                "{} — {} — since {}{}",
                s.specific,
                severity_label(s.severity),
                s.onset_date,
                if s.still_active { " (still active)" } else { "" }
            ),
            severity: s.severity,
            onset_date: s.onset_date.clone(),
            still_active: s.still_active,
        }
    }).collect();

    // Medication changes
    let medication_changes: Vec<MedicationChange> = data.med_changes.iter().map(|c| {
        let desc = match c.change_type.as_str() {
            "started" => format!("Started {} {} on {}", c.medication_name, c.new_dose, c.change_date),
            "dose_changed" => format!(
                "{} dose changed from {} to {} on {}",
                c.medication_name,
                c.old_dose.as_deref().unwrap_or("?"),
                c.new_dose,
                c.change_date
            ),
            _ => format!("{} {} on {}", c.medication_name, c.change_type, c.change_date),
        };
        MedicationChange {
            description: desc,
            change_type: c.change_type.clone(),
            date: c.change_date.clone(),
        }
    }).collect();

    PatientCopy {
        title,
        priority_items,
        questions,
        symptoms_to_mention,
        medication_changes,
        reminder: "Bring this to your appointment.".into(),
    }
}

fn severity_label(severity: u8) -> &'static str {
    match severity {
        1 => "minimal",
        2 => "mild",
        3 => "moderate",
        4 => "severe",
        5 => "very severe",
        _ => "unknown",
    }
}

/// Generates the full appointment prep.
pub fn prepare_appointment_prep(
    conn: &Connection,
    professional_id: &str,
    appointment_date: NaiveDate,
    appointment_id: &str,
) -> Result<AppointmentPrep, DatabaseError> {
    let data = assemble_prep_data(conn, professional_id, appointment_date)?;

    let patient_copy = build_patient_copy(&data);
    let professional_copy = build_professional_copy(&data);

    mark_prep_generated(conn, appointment_id)?;
    update_last_seen(conn, professional_id, &appointment_date)?;

    let now: NaiveDateTime = Local::now().naive_local();

    Ok(AppointmentPrep {
        appointment_id: appointment_id.into(),
        professional_name: data.professional_name,
        professional_specialty: data.professional_specialty,
        appointment_date: appointment_date.to_string(),
        patient_copy,
        professional_copy,
        generated_at: now.to_string(),
    })
}

// ─── Post-appointment notes ───────────────────────────────────────────────────

/// Saves post-appointment notes as JSON in the appointments table.
pub fn save_post_notes(
    conn: &Connection,
    notes: &PostAppointmentNotes,
) -> Result<(), DatabaseError> {
    let notes_json = serde_json::to_string(notes)
        .map_err(|e| DatabaseError::ConstraintViolation(format!("JSON serialization: {e}")))?;

    let changed = conn.execute(
        "UPDATE appointments SET post_notes = ?1, type = 'completed' WHERE id = ?2",
        params![notes_json, notes.appointment_id],
    )?;

    if changed == 0 {
        return Err(DatabaseError::NotFound {
            entity_type: "Appointment".into(),
            id: notes.appointment_id.clone(),
        });
    }
    Ok(())
}

// ─── Appointment history ──────────────────────────────────────────────────────

/// Lists all appointments with professional info, ordered by date DESC.
pub fn list_appointments(conn: &Connection) -> Result<Vec<StoredAppointment>, DatabaseError> {
    let mut stmt = conn.prepare(
        "SELECT a.id, p.name, COALESCE(p.specialty, ''), a.date, a.type,
                a.pre_summary_generated, (a.post_notes IS NOT NULL) as has_notes
         FROM appointments a
         JOIN professionals p ON a.professional_id = p.id
         ORDER BY a.date DESC",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(StoredAppointment {
            id: row.get(0)?,
            professional_name: row.get(1)?,
            professional_specialty: row.get(2)?,
            date: row.get(3)?,
            appointment_type: row.get(4)?,
            prep_generated: row.get(5)?,
            has_post_notes: row.get(6)?,
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>().map_err(DatabaseError::from)
}

// ─── PDF generation ───────────────────────────────────────────────────────────

use printpdf::*;
use std::io::BufWriter;
use std::path::{Path, PathBuf};

/// Generates a PDF from the patient copy. Returns PDF bytes.
pub fn generate_patient_pdf(copy: &PatientCopy) -> Result<Vec<u8>, DatabaseError> {
    let (doc, page1, layer1) = PdfDocument::new(&copy.title, Mm(210.0), Mm(297.0), "Layer 1");
    let layer = doc.get_page(page1).get_layer(layer1);
    let font = doc.add_builtin_font(BuiltinFont::Helvetica)
        .map_err(|e| DatabaseError::ConstraintViolation(format!("PDF font error: {e}")))?;
    let bold = doc.add_builtin_font(BuiltinFont::HelveticaBold)
        .map_err(|e| DatabaseError::ConstraintViolation(format!("PDF font error: {e}")))?;

    let mut y = Mm(280.0);

    // Title
    layer.use_text(&copy.title, 14.0, Mm(20.0), y, &bold);
    y -= Mm(10.0);

    // Priority items
    if !copy.priority_items.is_empty() {
        layer.use_text("PRIORITY:", 11.0, Mm(20.0), y, &bold);
        y -= Mm(6.0);
        for item in &copy.priority_items {
            let text = format!("  {} ({})", item.text, item.source);
            for line in wrap_text(&text, 80) {
                layer.use_text(&line, 9.0, Mm(25.0), y, &font);
                y -= Mm(4.5);
            }
            y -= Mm(2.0);
        }
        y -= Mm(4.0);
    }

    // Questions
    layer.use_text("YOUR QUESTIONS:", 11.0, Mm(20.0), y, &bold);
    y -= Mm(6.0);
    for (i, q) in copy.questions.iter().enumerate() {
        let text = format!("  {}. {}", i + 1, q.question);
        for line in wrap_text(&text, 80) {
            layer.use_text(&line, 9.0, Mm(25.0), y, &font);
            y -= Mm(4.5);
        }
        y -= Mm(3.0);
    }

    // Symptoms to mention
    if !copy.symptoms_to_mention.is_empty() {
        y -= Mm(4.0);
        layer.use_text("SYMPTOMS TO MENTION:", 11.0, Mm(20.0), y, &bold);
        y -= Mm(6.0);
        for s in &copy.symptoms_to_mention {
            let text = format!("  · {}", s.description);
            layer.use_text(&text, 9.0, Mm(25.0), y, &font);
            y -= Mm(4.5);
        }
    }

    // Medication changes
    if !copy.medication_changes.is_empty() {
        y -= Mm(4.0);
        layer.use_text("MEDICATION CHANGES:", 11.0, Mm(20.0), y, &bold);
        y -= Mm(6.0);
        for mc in &copy.medication_changes {
            let text = format!("  · {}", mc.description);
            layer.use_text(&text, 9.0, Mm(25.0), y, &font);
            y -= Mm(4.5);
        }
    }

    // Reminder
    y -= Mm(8.0);
    layer.use_text(&copy.reminder, 10.0, Mm(20.0), y, &bold);

    let mut buf = BufWriter::new(Vec::new());
    doc.save(&mut buf)
        .map_err(|e| DatabaseError::ConstraintViolation(format!("PDF save error: {e}")))?;
    buf.into_inner()
        .map_err(|e| DatabaseError::ConstraintViolation(format!("PDF buffer error: {e}")))
}

/// Generates a PDF from the professional copy. Returns PDF bytes.
pub fn generate_professional_pdf(copy: &ProfessionalCopy) -> Result<Vec<u8>, DatabaseError> {
    let (doc, page1, layer1) = PdfDocument::new(
        &copy.header.title, Mm(210.0), Mm(297.0), "Layer 1",
    );
    let layer = doc.get_page(page1).get_layer(layer1);
    let font = doc.add_builtin_font(BuiltinFont::Helvetica)
        .map_err(|e| DatabaseError::ConstraintViolation(format!("PDF font error: {e}")))?;
    let bold = doc.add_builtin_font(BuiltinFont::HelveticaBold)
        .map_err(|e| DatabaseError::ConstraintViolation(format!("PDF font error: {e}")))?;
    let courier = doc.add_builtin_font(BuiltinFont::Courier)
        .map_err(|e| DatabaseError::ConstraintViolation(format!("PDF font error: {e}")))?;

    let mut y = Mm(280.0);

    // Header
    layer.use_text(&copy.header.title, 14.0, Mm(20.0), y, &bold);
    y -= Mm(6.0);
    layer.use_text(format!("Date: {}", copy.header.date), 9.0, Mm(20.0), y, &font);
    y -= Mm(4.5);
    layer.use_text(&copy.header.professional, 9.0, Mm(20.0), y, &font);
    y -= Mm(4.5);
    layer.use_text(&copy.header.disclaimer, 8.0, Mm(20.0), y, &font);
    y -= Mm(8.0);

    // Current Medications
    if !copy.current_medications.is_empty() {
        layer.use_text("CURRENT MEDICATIONS:", 11.0, Mm(20.0), y, &bold);
        y -= Mm(6.0);
        for m in &copy.current_medications {
            let flag = if m.is_recent_change { " [CHANGED]" } else { "" };
            let text = format!(
                "  {} {} — {} — {}{}",
                m.name, m.dose, m.frequency, m.prescriber, flag
            );
            layer.use_text(&text, 8.0, Mm(25.0), y, &courier);
            y -= Mm(4.0);
        }
        y -= Mm(4.0);
    }

    // Changes since last visit
    if !copy.changes_since_last_visit.is_empty() {
        layer.use_text("CHANGES SINCE LAST VISIT:", 11.0, Mm(20.0), y, &bold);
        y -= Mm(6.0);
        for c in &copy.changes_since_last_visit {
            let text = format!("  · {}", c.description);
            layer.use_text(&text, 8.0, Mm(25.0), y, &courier);
            y -= Mm(4.0);
        }
        y -= Mm(4.0);
    }

    // Lab results
    if !copy.lab_results.is_empty() {
        layer.use_text("LAB RESULTS:", 11.0, Mm(20.0), y, &bold);
        y -= Mm(6.0);
        for l in &copy.lab_results {
            let text = format!(
                "  {}: {} {} (ref: {}) [{}] — {}",
                l.test_name, l.value, l.unit, l.reference_range,
                l.abnormal_flag.to_uppercase(), l.date
            );
            layer.use_text(&text, 8.0, Mm(25.0), y, &courier);
            y -= Mm(4.0);
        }
        y -= Mm(4.0);
    }

    // Patient-reported symptoms
    if !copy.patient_reported_symptoms.is_empty() {
        layer.use_text("PATIENT-REPORTED SYMPTOMS:", 11.0, Mm(20.0), y, &bold);
        y -= Mm(6.0);
        for s in &copy.patient_reported_symptoms {
            let text = format!(
                "  · {} — severity {}/5 — onset {}",
                s.description, s.severity, s.onset_date
            );
            layer.use_text(&text, 8.0, Mm(25.0), y, &courier);
            y -= Mm(4.0);
        }
        y -= Mm(4.0);
    }

    // Source documents
    if !copy.source_documents.is_empty() {
        layer.use_text("SOURCE DOCUMENTS:", 11.0, Mm(20.0), y, &bold);
        y -= Mm(6.0);
        for d in &copy.source_documents {
            let text = format!("  · {} — {} — {}", d.document_type, d.professional, d.date);
            layer.use_text(&text, 8.0, Mm(25.0), y, &courier);
            y -= Mm(4.0);
        }
        y -= Mm(4.0);
    }

    // Disclaimer
    y -= Mm(4.0);
    for line in wrap_text(&copy.disclaimer, 90) {
        layer.use_text(&line, 7.0, Mm(20.0), y, &font);
        y -= Mm(3.5);
    }

    let mut buf = BufWriter::new(Vec::new());
    doc.save(&mut buf)
        .map_err(|e| DatabaseError::ConstraintViolation(format!("PDF save error: {e}")))?;
    buf.into_inner()
        .map_err(|e| DatabaseError::ConstraintViolation(format!("PDF buffer error: {e}")))
}

/// Saves PDF bytes to the profile exports directory.
pub fn export_pdf_to_file(
    pdf_bytes: &[u8],
    filename: &str,
    db_path: &Path,
) -> Result<PathBuf, DatabaseError> {
    // db_path is profile_dir/database/coheara.db
    // exports_dir is profile_dir/exports/
    let profile_dir = db_path
        .parent() // database/
        .and_then(|p| p.parent()) // profile_dir/
        .ok_or_else(|| DatabaseError::ConstraintViolation("Cannot determine profile directory".into()))?;

    let exports_dir = profile_dir.join("exports");
    std::fs::create_dir_all(&exports_dir)
        .map_err(|e| DatabaseError::ConstraintViolation(format!("Cannot create exports dir: {e}")))?;

    let path = exports_dir.join(filename);
    std::fs::write(&path, pdf_bytes)
        .map_err(|e| DatabaseError::ConstraintViolation(format!("Cannot write PDF: {e}")))?;

    Ok(path)
}

/// Simple word-wrap helper for PDF text rendering.
fn wrap_text(text: &str, max_chars: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();

    for word in text.split_whitespace() {
        if current.len() + word.len() + 1 > max_chars && !current.is_empty() {
            lines.push(current.clone());
            current.clear();
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(word);
    }
    if !current.is_empty() {
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::sqlite::open_memory_database;

    fn setup_db() -> Connection {
        let conn = open_memory_database().expect("open_memory_database");
        seed_test_data(&conn);
        conn
    }

    fn seed_test_data(conn: &Connection) {
        // Professional
        conn.execute(
            "INSERT INTO professionals (id, name, specialty, institution, last_seen_date)
             VALUES ('prof-1', 'Dr. Chen', 'GP', 'City Clinic', '2025-12-15')",
            [],
        ).unwrap();

        // Document (required FK for medications)
        conn.execute(
            "INSERT OR IGNORE INTO documents (id, type, title, ingestion_date, source_file)
             VALUES ('doc-seed', 'prescription', 'Test Rx', '2025-01-01', 'test.pdf')",
            [],
        ).unwrap();

        // Active medication
        conn.execute(
            "INSERT INTO medications (id, generic_name, dose, frequency, frequency_type, prescriber_id,
             start_date, status, document_id)
             VALUES ('med-1', 'Metformin', '500mg', '2x daily', 'scheduled', 'prof-1',
                     '2026-01-20', 'active', 'doc-seed')",
            [],
        ).unwrap();

        conn.execute(
            "INSERT INTO medications (id, generic_name, dose, frequency, frequency_type, prescriber_id,
             start_date, status, document_id)
             VALUES ('med-2', 'Lisinopril', '20mg', '1x daily', 'scheduled', 'prof-1',
                     '2025-06-01', 'active', 'doc-seed')",
            [],
        ).unwrap();

        // Dose change
        conn.execute(
            "INSERT INTO dose_changes (id, medication_id, old_dose, new_dose, change_date, reason)
             VALUES ('dc-1', 'med-2', '10mg', '20mg', '2026-01-15', 'Adjustment')",
            [],
        ).unwrap();

        // Lab result
        conn.execute(
            "INSERT INTO lab_results (id, test_name, value, unit, reference_range_low,
             reference_range_high, abnormal_flag, collection_date, document_id)
             VALUES ('lab-1', 'Potassium', 5.8, 'mEq/L', 3.5, 5.0, 'critical_high',
                     '2026-01-10', 'doc-seed')",
            [],
        ).unwrap();

        conn.execute(
            "INSERT INTO lab_results (id, test_name, value, unit, reference_range_low,
             reference_range_high, abnormal_flag, collection_date, document_id)
             VALUES ('lab-2', 'Creatinine', 1.1, 'mg/dL', 0.7, 1.3, 'normal',
                     '2026-01-10', 'doc-seed')",
            [],
        ).unwrap();

        // Symptom
        conn.execute(
            "INSERT INTO symptoms (id, category, specific, severity, onset_date,
             recorded_date, still_active, source)
             VALUES ('sym-1', 'Neurological', 'Headache', 3, '2026-01-25',
                     '2026-01-25', 1, 'patient_reported')",
            [],
        ).unwrap();

        // Previous appointment (completed)
        conn.execute(
            "INSERT INTO appointments (id, professional_id, date, type, pre_summary_generated)
             VALUES ('appt-old', 'prof-1', '2025-12-15', 'completed', 1)",
            [],
        ).unwrap();
    }

    #[test]
    fn test_list_professionals() {
        let conn = setup_db();
        let profs = list_professionals(&conn).unwrap();
        assert_eq!(profs.len(), 1);
        assert_eq!(profs[0].name, "Dr. Chen");
        assert_eq!(profs[0].specialty.as_deref(), Some("GP"));
    }

    #[test]
    fn test_create_professional() {
        let conn = setup_db();
        let new_prof = NewProfessional {
            name: "Dr. Moreau".into(),
            specialty: "Cardiologist".into(),
            institution: Some("Heart Center".into()),
        };
        let id = create_professional(&conn, &new_prof).unwrap();
        assert!(!id.is_empty());

        let profs = list_professionals(&conn).unwrap();
        assert_eq!(profs.len(), 2);
    }

    #[test]
    fn test_create_appointment() {
        let conn = setup_db();
        let date = NaiveDate::from_ymd_opt(2026, 2, 20).unwrap();
        let id = create_appointment(&conn, "prof-1", &date).unwrap();
        assert!(!id.is_empty());

        let appts = list_appointments(&conn).unwrap();
        // old appointment + new one
        assert_eq!(appts.len(), 2);
    }

    #[test]
    fn test_assemble_prep_data() {
        let conn = setup_db();
        let date = NaiveDate::from_ymd_opt(2026, 2, 20).unwrap();
        let data = assemble_prep_data(&conn, "prof-1", date).unwrap();

        assert_eq!(data.professional_name, "Dr. Chen");
        assert_eq!(data.professional_specialty, "GP");
        // Since date should be 2025-12-15 (last completed visit)
        assert_eq!(data.since_date, NaiveDate::from_ymd_opt(2025, 12, 15).unwrap());
        // Metformin started after since_date
        assert!(!data.med_changes.is_empty());
        // Both active medications
        assert_eq!(data.medications.len(), 2);
        // Labs after since_date
        assert_eq!(data.labs.len(), 2);
        // Symptom after since_date
        assert_eq!(data.symptoms.len(), 1);
    }

    #[test]
    fn test_assemble_prep_data_first_visit() {
        let conn = setup_db();
        // New professional with no previous visits
        let new_prof = NewProfessional {
            name: "Dr. New".into(),
            specialty: "Neurologist".into(),
            institution: None,
        };
        let prof_id = create_professional(&conn, &new_prof).unwrap();
        let date = NaiveDate::from_ymd_opt(2026, 2, 20).unwrap();
        let data = assemble_prep_data(&conn, &prof_id, date).unwrap();

        // Since date should be 2000-01-01 (no previous visit fallback)
        assert_eq!(data.since_date, NaiveDate::from_ymd_opt(2000, 1, 1).unwrap());
        // All data should be included
        assert_eq!(data.medications.len(), 2);
    }

    #[test]
    fn test_build_professional_copy() {
        let conn = setup_db();
        let date = NaiveDate::from_ymd_opt(2026, 2, 20).unwrap();
        let data = assemble_prep_data(&conn, "prof-1", date).unwrap();
        let copy = build_professional_copy(&data);

        assert_eq!(copy.header.title, "COHEARA PATIENT SUMMARY");
        assert!(copy.header.professional.contains("Dr. Chen"));
        assert!(copy.header.professional.contains("GP"));
        assert!(!copy.disclaimer.is_empty());
        assert_eq!(copy.current_medications.len(), 2);
        // Metformin should be flagged as recent
        let metformin = copy.current_medications.iter()
            .find(|m| m.name == "Metformin").unwrap();
        assert!(metformin.is_recent_change);
        // Labs included
        assert_eq!(copy.lab_results.len(), 2);
        // Observations deferred
        assert!(copy.observations_for_discussion.is_empty());
    }

    #[test]
    fn test_professional_copy_recent_changes_flagged() {
        let conn = setup_db();
        let date = NaiveDate::from_ymd_opt(2026, 2, 20).unwrap();
        let data = assemble_prep_data(&conn, "prof-1", date).unwrap();
        let copy = build_professional_copy(&data);

        // Lisinopril had a dose change since last visit
        let lisinopril = copy.current_medications.iter()
            .find(|m| m.name == "Lisinopril").unwrap();
        assert!(lisinopril.is_recent_change);
    }

    #[test]
    fn test_professional_copy_lab_abnormal_flags() {
        let conn = setup_db();
        let date = NaiveDate::from_ymd_opt(2026, 2, 20).unwrap();
        let data = assemble_prep_data(&conn, "prof-1", date).unwrap();
        let copy = build_professional_copy(&data);

        let potassium = copy.lab_results.iter()
            .find(|l| l.test_name == "Potassium").unwrap();
        assert_eq!(potassium.abnormal_flag, "critical_high");

        let creatinine = copy.lab_results.iter()
            .find(|l| l.test_name == "Creatinine").unwrap();
        assert_eq!(creatinine.abnormal_flag, "normal");
    }

    #[test]
    fn test_build_patient_copy() {
        let conn = setup_db();
        let date = NaiveDate::from_ymd_opt(2026, 2, 20).unwrap();
        let data = assemble_prep_data(&conn, "prof-1", date).unwrap();
        let copy = build_patient_copy(&data);

        assert!(copy.title.contains("Dr. Chen"));
        assert!(copy.title.contains("February"));
        assert!(!copy.questions.is_empty());
        assert!(copy.questions.len() <= 5);
        // Critical potassium should be in priority items
        assert!(!copy.priority_items.is_empty());
        assert!(copy.priority_items[0].text.contains("Potassium"));
        // Symptom mentioned
        assert!(!copy.symptoms_to_mention.is_empty());
        // Medication changes
        assert!(!copy.medication_changes.is_empty());
        assert_eq!(copy.reminder, "Bring this to your appointment.");
    }

    #[test]
    fn test_patient_questions_include_medication_changes() {
        let conn = setup_db();
        let date = NaiveDate::from_ymd_opt(2026, 2, 20).unwrap();
        let data = assemble_prep_data(&conn, "prof-1", date).unwrap();
        let copy = build_patient_copy(&data);

        let has_med_question = copy.questions.iter()
            .any(|q| q.question.contains("medication"));
        assert!(has_med_question);
    }

    #[test]
    fn test_patient_questions_include_symptoms() {
        let conn = setup_db();
        let date = NaiveDate::from_ymd_opt(2026, 2, 20).unwrap();
        let data = assemble_prep_data(&conn, "prof-1", date).unwrap();
        let copy = build_patient_copy(&data);

        let has_symptom_question = copy.questions.iter()
            .any(|q| q.question.contains("Headache"));
        assert!(has_symptom_question);
    }

    #[test]
    fn test_save_post_notes() {
        let conn = setup_db();
        let notes = PostAppointmentNotes {
            appointment_id: "appt-old".into(),
            doctor_said: "Everything looks good.".into(),
            changes_made: "Increased Lisinopril to 30mg.".into(),
            follow_up: Some("Come back in 3 months.".into()),
            general_notes: None,
        };

        save_post_notes(&conn, &notes).unwrap();

        // Verify saved
        let (post_notes, appt_type): (Option<String>, String) = conn.query_row(
            "SELECT post_notes, type FROM appointments WHERE id = 'appt-old'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ).unwrap();

        assert!(post_notes.is_some());
        assert_eq!(appt_type, "completed");
        let saved: PostAppointmentNotes = serde_json::from_str(&post_notes.unwrap()).unwrap();
        assert_eq!(saved.doctor_said, "Everything looks good.");
    }

    #[test]
    fn test_save_post_notes_nonexistent() {
        let conn = setup_db();
        let notes = PostAppointmentNotes {
            appointment_id: "nonexistent".into(),
            doctor_said: "Test".into(),
            changes_made: "Test".into(),
            follow_up: None,
            general_notes: None,
        };

        let err = save_post_notes(&conn, &notes).unwrap_err();
        assert!(matches!(err, DatabaseError::NotFound { .. }));
    }

    #[test]
    fn test_list_appointments() {
        let conn = setup_db();
        let appts = list_appointments(&conn).unwrap();
        assert_eq!(appts.len(), 1);
        assert_eq!(appts[0].professional_name, "Dr. Chen");
        assert_eq!(appts[0].appointment_type, "completed");
        assert!(appts[0].prep_generated);
        assert!(!appts[0].has_post_notes);
    }

    #[test]
    fn test_list_appointments_ordered() {
        let conn = setup_db();
        // Add a newer appointment
        let date = NaiveDate::from_ymd_opt(2026, 2, 20).unwrap();
        create_appointment(&conn, "prof-1", &date).unwrap();

        let appts = list_appointments(&conn).unwrap();
        assert_eq!(appts.len(), 2);
        // Newer first
        assert_eq!(appts[0].date, "2026-02-20");
        assert_eq!(appts[1].date, "2025-12-15");
    }

    #[test]
    fn test_mark_prep_generated() {
        let conn = setup_db();
        let date = NaiveDate::from_ymd_opt(2026, 3, 1).unwrap();
        let id = create_appointment(&conn, "prof-1", &date).unwrap();

        mark_prep_generated(&conn, &id).unwrap();

        let generated: bool = conn.query_row(
            "SELECT pre_summary_generated FROM appointments WHERE id = ?1",
            params![id],
            |row| row.get(0),
        ).unwrap();
        assert!(generated);
    }

    #[test]
    fn test_mark_prep_generated_nonexistent() {
        let conn = setup_db();
        let err = mark_prep_generated(&conn, "no-such-id").unwrap_err();
        assert!(matches!(err, DatabaseError::NotFound { .. }));
    }

    #[test]
    fn test_prepare_appointment_prep() {
        let conn = setup_db();
        let date = NaiveDate::from_ymd_opt(2026, 2, 20).unwrap();
        let appt_id = create_appointment(&conn, "prof-1", &date).unwrap();

        let prep = prepare_appointment_prep(&conn, "prof-1", date, &appt_id).unwrap();

        assert_eq!(prep.professional_name, "Dr. Chen");
        assert_eq!(prep.appointment_date, "2026-02-20");
        assert!(!prep.patient_copy.questions.is_empty());
        assert!(!prep.professional_copy.current_medications.is_empty());

        // Verify prep marked as generated
        let generated: bool = conn.query_row(
            "SELECT pre_summary_generated FROM appointments WHERE id = ?1",
            params![appt_id],
            |row| row.get(0),
        ).unwrap();
        assert!(generated);
    }

    #[test]
    fn test_pdf_patient_generation() {
        let copy = PatientCopy {
            title: "Questions for Dr. Chen — February 20, 2026".into(),
            priority_items: vec![PrepItem {
                text: "Potassium flagged critical.".into(),
                source: "Lab Jan 10".into(),
                priority: "Critical".into(),
            }],
            questions: vec![PrepQuestion {
                question: "Should I continue taking Metformin?".into(),
                context: "Recently started".into(),
                relevance_score: 0.9,
            }],
            symptoms_to_mention: vec![SymptomMention {
                description: "Headache — moderate — since Jan 25".into(),
                severity: 3,
                onset_date: "2026-01-25".into(),
                still_active: true,
            }],
            medication_changes: vec![MedicationChange {
                description: "Started Metformin 500mg on Jan 20".into(),
                change_type: "started".into(),
                date: "2026-01-20".into(),
            }],
            reminder: "Bring this to your appointment.".into(),
        };

        let bytes = generate_patient_pdf(&copy).unwrap();
        assert!(!bytes.is_empty());
        // PDF magic bytes: %PDF
        assert_eq!(&bytes[0..4], b"%PDF");
    }

    #[test]
    fn test_pdf_professional_generation() {
        let copy = ProfessionalCopy {
            header: ProfessionalHeader {
                title: "COHEARA PATIENT SUMMARY".into(),
                date: "2026-02-20".into(),
                professional: "For: Dr. Chen (GP)".into(),
                disclaimer: "AI-generated. Not clinical advice.".into(),
            },
            current_medications: vec![MedicationSummary {
                name: "Metformin".into(),
                dose: "500mg".into(),
                frequency: "2x daily".into(),
                prescriber: "Dr. Chen".into(),
                start_date: "2026-01-20".into(),
                is_recent_change: true,
            }],
            changes_since_last_visit: vec![],
            lab_results: vec![LabSummary {
                test_name: "Potassium".into(),
                value: "5.8".into(),
                unit: "mEq/L".into(),
                reference_range: "3.5-5.0".into(),
                abnormal_flag: "critical_high".into(),
                date: "2026-01-10".into(),
            }],
            patient_reported_symptoms: vec![],
            observations_for_discussion: vec![],
            source_documents: vec![],
            disclaimer: "Not a clinical record.".into(),
        };

        let bytes = generate_professional_pdf(&copy).unwrap();
        assert!(!bytes.is_empty());
        assert_eq!(&bytes[0..4], b"%PDF");
    }

    #[test]
    fn test_export_pdf_to_file() {
        let tmp = tempfile::tempdir().unwrap();
        let db_dir = tmp.path().join("database");
        std::fs::create_dir_all(&db_dir).unwrap();
        let db_path = db_dir.join("coheara.db");

        let pdf_bytes = b"%PDF-1.4 test content";
        let path = export_pdf_to_file(pdf_bytes, "test.pdf", &db_path).unwrap();

        assert!(path.exists());
        assert_eq!(std::fs::read(&path).unwrap(), pdf_bytes);
        assert!(path.to_str().unwrap().contains("exports"));
    }

    #[test]
    fn test_medication_changes_since_last_visit() {
        let conn = setup_db();
        let changes = fetch_medication_changes(&conn, "2025-12-15").unwrap();
        // Should find: Metformin started 2026-01-20, Lisinopril dose changed 2026-01-15
        assert_eq!(changes.len(), 2);

        let started: Vec<&MedChange> = changes.iter()
            .filter(|c| c.change_type == "started")
            .collect();
        assert_eq!(started.len(), 1);
        assert_eq!(started[0].medication_name, "Metformin");

        let dose_changed: Vec<&MedChange> = changes.iter()
            .filter(|c| c.change_type == "dose_changed")
            .collect();
        assert_eq!(dose_changed.len(), 1);
        assert_eq!(dose_changed[0].medication_name, "Lisinopril");
    }

    #[test]
    fn test_create_new_professional_during_prep() {
        let conn = setup_db();
        let new_prof = NewProfessional {
            name: "Dr. Moreau".into(),
            specialty: "Cardiologist".into(),
            institution: Some("Heart Center".into()),
        };
        let prof_id = create_professional(&conn, &new_prof).unwrap();
        let date = NaiveDate::from_ymd_opt(2026, 3, 5).unwrap();
        let appt_id = create_appointment(&conn, &prof_id, &date).unwrap();
        let prep = prepare_appointment_prep(&conn, &prof_id, date, &appt_id).unwrap();

        assert_eq!(prep.professional_name, "Dr. Moreau");
        assert_eq!(prep.professional_specialty, "Cardiologist");
    }

    #[test]
    fn test_wrap_text() {
        let text = "This is a long sentence that should be wrapped at around forty characters or so.";
        let lines = wrap_text(text, 40);
        assert!(lines.len() > 1);
        for line in &lines {
            assert!(line.len() <= 45); // Allow some slack for word boundaries
        }
    }

    #[test]
    fn test_wrap_text_short() {
        let lines = wrap_text("Short", 40);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0], "Short");
    }

    #[test]
    fn test_wrap_text_empty() {
        let lines = wrap_text("", 40);
        assert_eq!(lines.len(), 1);
    }
}

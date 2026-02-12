//! L4-04: Timeline View — chronological visualization of the patient's medical journey.
//!
//! Assembles events from ALL entity tables (medications, dose_changes, lab_results,
//! symptoms, procedures, appointments, documents, diagnoses) into a unified
//! `Vec<TimelineEvent>`, sorted by date. Detects temporal correlations between
//! symptom onset and medication changes. Returns everything in a single payload.

use chrono::NaiveDate;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use crate::db::DatabaseError;

// ── Types ──────────────────────────────────────────────────────────────────

/// A single event on the timeline — unified across all entity tables.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineEvent {
    pub id: String,
    pub event_type: EventType,
    pub date: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub professional_id: Option<String>,
    pub professional_name: Option<String>,
    pub document_id: Option<String>,
    pub severity: Option<EventSeverity>,
    pub metadata: EventMetadata,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EventSeverity {
    Normal,
    Low,
    Moderate,
    High,
    Critical,
}

/// Type-specific metadata carried by each event.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum EventMetadata {
    Medication {
        generic_name: String,
        brand_name: Option<String>,
        dose: String,
        frequency: String,
        status: String,
        reason: Option<String>,
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
        severity: u8,
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
        appointment_type: String,
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

/// A correlation between two timeline events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineCorrelation {
    pub source_id: String,
    pub target_id: String,
    pub correlation_type: CorrelationType,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CorrelationType {
    SymptomAfterMedicationChange,
    SymptomAfterMedicationStart,
    SymptomResolvedAfterMedicationStop,
    LabAfterMedicationChange,
    ExplicitLink,
}

/// Filter parameters sent from frontend.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TimelineFilter {
    pub event_types: Option<Vec<EventType>>,
    pub professional_id: Option<String>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub since_appointment_id: Option<String>,
}

/// Complete timeline data — single response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineData {
    pub events: Vec<TimelineEvent>,
    pub correlations: Vec<TimelineCorrelation>,
    pub date_range: DateRange,
    pub event_counts: EventCounts,
    pub professionals: Vec<ProfessionalSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DateRange {
    pub earliest: Option<String>,
    pub latest: Option<String>,
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
    pub id: String,
    pub name: String,
    pub specialty: Option<String>,
    pub event_count: u32,
}

// ── Assembly ───────────────────────────────────────────────────────────────

/// Assembles timeline events from ALL entity tables, applies filters,
/// and returns them sorted chronologically (oldest first).
pub fn assemble_timeline_events(
    conn: &Connection,
    filter: &TimelineFilter,
) -> Result<Vec<TimelineEvent>, DatabaseError> {
    let (date_from, date_to) = resolve_date_bounds(conn, filter)?;

    let mut events: Vec<TimelineEvent> = Vec::new();

    events.extend(fetch_medication_starts(conn, &date_from, &date_to)?);
    events.extend(fetch_medication_stops(conn, &date_from, &date_to)?);
    events.extend(fetch_dose_changes(conn, &date_from, &date_to)?);
    events.extend(fetch_lab_events(conn, &date_from, &date_to)?);
    events.extend(fetch_symptom_events(conn, &date_from, &date_to)?);
    events.extend(fetch_procedure_events(conn, &date_from, &date_to)?);
    events.extend(fetch_appointment_events(conn, &date_from, &date_to)?);
    events.extend(fetch_document_events(conn, &date_from, &date_to)?);
    events.extend(fetch_diagnosis_events(conn, &date_from, &date_to)?);

    // Apply event_type filter
    if let Some(ref types) = filter.event_types {
        events.retain(|e| types.contains(&e.event_type));
    }

    // Apply professional filter
    if let Some(ref prof_id) = filter.professional_id {
        events.retain(|e| e.professional_id.as_deref() == Some(prof_id.as_str()));
    }

    // Sort chronologically
    events.sort_by(|a, b| a.date.cmp(&b.date));

    Ok(events)
}

/// Resolves effective date bounds. If "since last visit" mode,
/// date_from = 30 days before appointment date (for context).
fn resolve_date_bounds(
    conn: &Connection,
    filter: &TimelineFilter,
) -> Result<(Option<String>, Option<String>), DatabaseError> {
    let date_from = if let Some(ref appt_id) = filter.since_appointment_id {
        let appt_date: String = conn
            .query_row(
                "SELECT date FROM appointments WHERE id = ?1",
                params![appt_id],
                |row| row.get(0),
            )
            .map_err(|_| DatabaseError::NotFound {
                entity_type: "appointment".into(),
                id: appt_id.clone(),
            })?;

        // Go back 30 days for context
        if let Ok(parsed) = NaiveDate::parse_from_str(&appt_date, "%Y-%m-%d") {
            let context_start = parsed - chrono::Duration::days(30);
            Some(context_start.format("%Y-%m-%d").to_string())
        } else {
            Some(appt_date)
        }
    } else {
        filter.date_from.clone()
    };

    Ok((date_from, filter.date_to.clone()))
}

// ── Per-Table Fetch Functions ──────────────────────────────────────────────

/// Helper: builds dynamic WHERE clause with date bounds.
struct DateBoundQuery {
    clauses: Vec<String>,
    params: Vec<Box<dyn rusqlite::types::ToSql>>,
}

impl DateBoundQuery {
    fn new(
        date_column: &str,
        date_from: &Option<String>,
        date_to: &Option<String>,
    ) -> Self {
        let mut clauses = Vec::new();
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if let Some(ref d) = date_from {
            params.push(Box::new(d.clone()));
            clauses.push(format!(
                " AND {} >= ?{}",
                date_column,
                params.len()
            ));
        }
        if let Some(ref d) = date_to {
            params.push(Box::new(d.clone()));
            clauses.push(format!(
                " AND {} <= ?{}",
                date_column,
                params.len()
            ));
        }

        Self { clauses, params }
    }

    fn sql_suffix(&self) -> String {
        self.clauses.join("")
    }

    fn param_refs(&self) -> Vec<&dyn rusqlite::types::ToSql> {
        self.params.iter().map(|p| p.as_ref()).collect()
    }
}

fn fetch_medication_starts(
    conn: &Connection,
    date_from: &Option<String>,
    date_to: &Option<String>,
) -> Result<Vec<TimelineEvent>, DatabaseError> {
    let bounds = DateBoundQuery::new("m.start_date", date_from, date_to);
    let sql = format!(
        "SELECT m.id, m.generic_name, m.brand_name, m.dose, m.frequency,
                m.status, m.start_date, m.reason_start,
                m.prescriber_id, p.name AS prof_name, m.document_id
         FROM medications m
         LEFT JOIN professionals p ON m.prescriber_id = p.id
         WHERE m.start_date IS NOT NULL{}",
        bounds.sql_suffix()
    );

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(bounds.param_refs().as_slice(), |row| {
        Ok(TimelineEvent {
            id: row.get::<_, String>("id")?,
            event_type: EventType::MedicationStart,
            date: row.get::<_, String>("start_date")?,
            title: format!("Started {}", row.get::<_, String>("generic_name")?),
            subtitle: Some(row.get::<_, String>("dose")?),
            professional_id: row.get("prescriber_id")?,
            professional_name: row.get("prof_name")?,
            document_id: row.get("document_id")?,
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

    rows.collect::<Result<Vec<_>, _>>().map_err(DatabaseError::from)
}

fn fetch_medication_stops(
    conn: &Connection,
    date_from: &Option<String>,
    date_to: &Option<String>,
) -> Result<Vec<TimelineEvent>, DatabaseError> {
    let bounds = DateBoundQuery::new("m.end_date", date_from, date_to);
    let sql = format!(
        "SELECT m.id, m.generic_name, m.brand_name, m.dose, m.frequency,
                m.status, m.end_date, m.reason_stop,
                m.prescriber_id, p.name AS prof_name, m.document_id
         FROM medications m
         LEFT JOIN professionals p ON m.prescriber_id = p.id
         WHERE m.status = 'stopped' AND m.end_date IS NOT NULL{}",
        bounds.sql_suffix()
    );

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(bounds.param_refs().as_slice(), |row| {
        Ok(TimelineEvent {
            id: format!("{}-stop", row.get::<_, String>("id")?),
            event_type: EventType::MedicationStop,
            date: row.get::<_, String>("end_date")?,
            title: format!("Stopped {}", row.get::<_, String>("generic_name")?),
            subtitle: row.get("reason_stop")?,
            professional_id: row.get("prescriber_id")?,
            professional_name: row.get("prof_name")?,
            document_id: row.get("document_id")?,
            severity: None,
            metadata: EventMetadata::Medication {
                generic_name: row.get("generic_name")?,
                brand_name: row.get("brand_name")?,
                dose: row.get("dose")?,
                frequency: row.get("frequency")?,
                status: row.get("status")?,
                reason: row.get("reason_stop")?,
            },
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>().map_err(DatabaseError::from)
}

fn fetch_dose_changes(
    conn: &Connection,
    date_from: &Option<String>,
    date_to: &Option<String>,
) -> Result<Vec<TimelineEvent>, DatabaseError> {
    let bounds = DateBoundQuery::new("dc.change_date", date_from, date_to);
    let sql = format!(
        "SELECT dc.id, dc.old_dose, dc.new_dose, dc.old_frequency, dc.new_frequency,
                dc.change_date, dc.reason,
                m.generic_name,
                dc.changed_by_id, p.name AS prof_name, dc.document_id
         FROM dose_changes dc
         JOIN medications m ON dc.medication_id = m.id
         LEFT JOIN professionals p ON dc.changed_by_id = p.id
         WHERE 1=1{}",
        bounds.sql_suffix()
    );

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(bounds.param_refs().as_slice(), |row| {
        let generic: String = row.get("generic_name")?;
        let new_dose: String = row.get("new_dose")?;
        Ok(TimelineEvent {
            id: row.get::<_, String>("id")?,
            event_type: EventType::MedicationDoseChange,
            date: row.get::<_, String>("change_date")?,
            title: format!("{} dose changed", generic),
            subtitle: Some(new_dose.clone()),
            professional_id: row.get("changed_by_id")?,
            professional_name: row.get("prof_name")?,
            document_id: row.get("document_id")?,
            severity: None,
            metadata: EventMetadata::DoseChange {
                generic_name: generic,
                old_dose: row.get("old_dose")?,
                new_dose,
                old_frequency: row.get("old_frequency")?,
                new_frequency: row.get("new_frequency")?,
                reason: row.get("reason")?,
            },
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>().map_err(DatabaseError::from)
}

fn severity_from_lab_flag(flag: &str) -> EventSeverity {
    match flag {
        "normal" => EventSeverity::Normal,
        "low" => EventSeverity::Low,
        "high" => EventSeverity::High,
        "critical_low" | "critical_high" => EventSeverity::Critical,
        _ => EventSeverity::Normal,
    }
}

fn fetch_lab_events(
    conn: &Connection,
    date_from: &Option<String>,
    date_to: &Option<String>,
) -> Result<Vec<TimelineEvent>, DatabaseError> {
    let bounds = DateBoundQuery::new("l.collection_date", date_from, date_to);
    let sql = format!(
        "SELECT l.id, l.test_name, l.value, l.value_text, l.unit,
                l.reference_range_low, l.reference_range_high,
                l.abnormal_flag, l.collection_date,
                l.ordering_physician_id, p.name AS prof_name, l.document_id
         FROM lab_results l
         LEFT JOIN professionals p ON l.ordering_physician_id = p.id
         WHERE 1=1{}",
        bounds.sql_suffix()
    );

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(bounds.param_refs().as_slice(), |row| {
        let test_name: String = row.get("test_name")?;
        let flag: String = row.get("abnormal_flag")?;
        let value: Option<f64> = row.get("value")?;
        let value_text: Option<String> = row.get("value_text")?;
        let unit: Option<String> = row.get("unit")?;

        let subtitle = match (value, &value_text, &unit) {
            (Some(v), _, Some(u)) => Some(format!("{v} {u}")),
            (Some(v), _, None) => Some(format!("{v}")),
            (None, Some(t), _) => Some(t.clone()),
            _ => None,
        };

        Ok(TimelineEvent {
            id: row.get::<_, String>("id")?,
            event_type: EventType::LabResult,
            date: row.get::<_, String>("collection_date")?,
            title: test_name.clone(),
            subtitle,
            professional_id: row.get("ordering_physician_id")?,
            professional_name: row.get("prof_name")?,
            document_id: row.get("document_id")?,
            severity: Some(severity_from_lab_flag(&flag)),
            metadata: EventMetadata::Lab {
                test_name,
                value,
                value_text,
                unit,
                reference_low: row.get("reference_range_low")?,
                reference_high: row.get("reference_range_high")?,
                abnormal_flag: flag,
            },
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>().map_err(DatabaseError::from)
}

fn severity_from_symptom(sev: u8) -> EventSeverity {
    match sev {
        1 => EventSeverity::Low,
        2 => EventSeverity::Low,
        3 => EventSeverity::Moderate,
        4 => EventSeverity::High,
        5 => EventSeverity::Critical,
        _ => EventSeverity::Normal,
    }
}

fn fetch_symptom_events(
    conn: &Connection,
    date_from: &Option<String>,
    date_to: &Option<String>,
) -> Result<Vec<TimelineEvent>, DatabaseError> {
    let bounds = DateBoundQuery::new("s.onset_date", date_from, date_to);
    let sql = format!(
        "SELECT s.id, s.category, s.specific, s.severity, s.body_region,
                s.still_active, s.onset_date, s.related_medication_id
         FROM symptoms s
         WHERE 1=1{}",
        bounds.sql_suffix()
    );

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(bounds.param_refs().as_slice(), |row| {
        let category: String = row.get("category")?;
        let specific: String = row.get("specific")?;
        let sev: i32 = row.get("severity")?;
        let still_active_int: i32 = row.get("still_active")?;

        Ok(TimelineEvent {
            id: row.get::<_, String>("id")?,
            event_type: EventType::Symptom,
            date: row.get::<_, String>("onset_date")?,
            title: specific.clone(),
            subtitle: Some(category.clone()),
            professional_id: None,
            professional_name: None,
            document_id: None,
            severity: Some(severity_from_symptom(sev as u8)),
            metadata: EventMetadata::Symptom {
                category,
                specific,
                severity: sev as u8,
                body_region: row.get("body_region")?,
                still_active: still_active_int != 0,
            },
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>().map_err(DatabaseError::from)
}

fn fetch_procedure_events(
    conn: &Connection,
    date_from: &Option<String>,
    date_to: &Option<String>,
) -> Result<Vec<TimelineEvent>, DatabaseError> {
    let bounds = DateBoundQuery::new("pr.date", date_from, date_to);
    let sql = format!(
        "SELECT pr.id, pr.name, pr.date, pr.facility, pr.outcome,
                pr.follow_up_required,
                pr.performing_professional_id, p.name AS prof_name, pr.document_id
         FROM procedures pr
         LEFT JOIN professionals p ON pr.performing_professional_id = p.id
         WHERE pr.date IS NOT NULL{}",
        bounds.sql_suffix()
    );

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(bounds.param_refs().as_slice(), |row| {
        let name: String = row.get("name")?;
        let follow_up: i32 = row.get("follow_up_required")?;
        Ok(TimelineEvent {
            id: row.get::<_, String>("id")?,
            event_type: EventType::Procedure,
            date: row.get::<_, String>("date")?,
            title: name.clone(),
            subtitle: row.get("facility")?,
            professional_id: row.get("performing_professional_id")?,
            professional_name: row.get("prof_name")?,
            document_id: row.get("document_id")?,
            severity: None,
            metadata: EventMetadata::Procedure {
                name,
                facility: row.get("facility")?,
                outcome: row.get("outcome")?,
                follow_up_required: follow_up != 0,
            },
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>().map_err(DatabaseError::from)
}

fn fetch_appointment_events(
    conn: &Connection,
    date_from: &Option<String>,
    date_to: &Option<String>,
) -> Result<Vec<TimelineEvent>, DatabaseError> {
    let bounds = DateBoundQuery::new("a.date", date_from, date_to);
    let sql = format!(
        "SELECT a.id, a.date, a.type AS appt_type,
                a.professional_id, p.name AS prof_name, p.specialty
         FROM appointments a
         JOIN professionals p ON a.professional_id = p.id
         WHERE 1=1{}",
        bounds.sql_suffix()
    );

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(bounds.param_refs().as_slice(), |row| {
        let appt_type: String = row.get("appt_type")?;
        let prof_name: String = row.get("prof_name")?;
        Ok(TimelineEvent {
            id: row.get::<_, String>("id")?,
            event_type: EventType::Appointment,
            date: row.get::<_, String>("date")?,
            title: format!("{} with {}", if appt_type == "completed" { "Visit" } else { "Upcoming" }, prof_name),
            subtitle: row.get("specialty")?,
            professional_id: row.get("professional_id")?,
            professional_name: Some(prof_name),
            document_id: None,
            severity: None,
            metadata: EventMetadata::Appointment {
                appointment_type: appt_type,
                professional_specialty: row.get("specialty")?,
            },
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>().map_err(DatabaseError::from)
}

fn fetch_document_events(
    conn: &Connection,
    date_from: &Option<String>,
    date_to: &Option<String>,
) -> Result<Vec<TimelineEvent>, DatabaseError> {
    // Use document_date if available, fall back to ingestion_date
    let bounds = DateBoundQuery::new(
        "COALESCE(d.document_date, d.ingestion_date)",
        date_from,
        date_to,
    );
    let sql = format!(
        "SELECT d.id, d.type AS doc_type, d.title, d.document_date, d.ingestion_date,
                d.verified,
                d.professional_id, p.name AS prof_name
         FROM documents d
         LEFT JOIN professionals p ON d.professional_id = p.id
         WHERE 1=1{}",
        bounds.sql_suffix()
    );

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(bounds.param_refs().as_slice(), |row| {
        let doc_type: String = row.get("doc_type")?;
        let verified_int: i32 = row.get("verified")?;
        let doc_date: Option<String> = row.get("document_date")?;
        let ingest_date: String = row.get("ingestion_date")?;
        let effective_date = doc_date.unwrap_or(ingest_date);

        Ok(TimelineEvent {
            id: row.get::<_, String>("id")?,
            event_type: EventType::Document,
            date: effective_date,
            title: row.get::<_, String>("title")?,
            subtitle: Some(doc_type.replace('_', " ")),
            professional_id: row.get("professional_id")?,
            professional_name: row.get("prof_name")?,
            document_id: Some(row.get::<_, String>("id")?),
            severity: None,
            metadata: EventMetadata::Document {
                document_type: doc_type,
                verified: verified_int != 0,
            },
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>().map_err(DatabaseError::from)
}

fn fetch_diagnosis_events(
    conn: &Connection,
    date_from: &Option<String>,
    date_to: &Option<String>,
) -> Result<Vec<TimelineEvent>, DatabaseError> {
    let bounds = DateBoundQuery::new("dg.date_diagnosed", date_from, date_to);
    let sql = format!(
        "SELECT dg.id, dg.name, dg.icd_code, dg.date_diagnosed, dg.status,
                dg.diagnosing_professional_id, p.name AS prof_name, dg.document_id
         FROM diagnoses dg
         LEFT JOIN professionals p ON dg.diagnosing_professional_id = p.id
         WHERE dg.date_diagnosed IS NOT NULL{}",
        bounds.sql_suffix()
    );

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(bounds.param_refs().as_slice(), |row| {
        let name: String = row.get("name")?;
        Ok(TimelineEvent {
            id: row.get::<_, String>("id")?,
            event_type: EventType::Diagnosis,
            date: row.get::<_, String>("date_diagnosed")?,
            title: name.clone(),
            subtitle: row.get("icd_code")?,
            professional_id: row.get("diagnosing_professional_id")?,
            professional_name: row.get("prof_name")?,
            document_id: row.get("document_id")?,
            severity: None,
            metadata: EventMetadata::Diagnosis {
                name,
                icd_code: row.get("icd_code")?,
                status: row.get("status")?,
            },
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>().map_err(DatabaseError::from)
}

// ── Correlation Detection ──────────────────────────────────────────────────

const CORRELATION_WINDOW_DAYS: i64 = 14;

/// Detects temporal correlations between symptoms and medication events.
/// O(n*m) where n = symptoms, m = medication events — bounded for typical profiles.
pub fn detect_correlations(events: &[TimelineEvent]) -> Vec<TimelineCorrelation> {
    let symptoms: Vec<&TimelineEvent> = events
        .iter()
        .filter(|e| e.event_type == EventType::Symptom)
        .collect();

    let med_events: Vec<&TimelineEvent> = events
        .iter()
        .filter(|e| {
            matches!(
                e.event_type,
                EventType::MedicationStart
                    | EventType::MedicationStop
                    | EventType::MedicationDoseChange
            )
        })
        .collect();

    let mut correlations = Vec::new();

    for symptom in &symptoms {
        let symptom_date = match NaiveDate::parse_from_str(&symptom.date, "%Y-%m-%d") {
            Ok(d) => d,
            Err(_) => continue,
        };

        for med in &med_events {
            let med_date = match NaiveDate::parse_from_str(&med.date, "%Y-%m-%d") {
                Ok(d) => d,
                Err(_) => continue,
            };

            let days_diff = (symptom_date - med_date).num_days();

            // Symptom appeared AFTER medication event (within window)
            if (0..=CORRELATION_WINDOW_DAYS).contains(&days_diff) {
                let corr_type = match med.event_type {
                    EventType::MedicationStart => {
                        CorrelationType::SymptomAfterMedicationStart
                    }
                    EventType::MedicationDoseChange => {
                        CorrelationType::SymptomAfterMedicationChange
                    }
                    _ => continue,
                };

                correlations.push(TimelineCorrelation {
                    source_id: symptom.id.clone(),
                    target_id: med.id.clone(),
                    correlation_type: corr_type,
                    description: format!(
                        "{} appeared {} day(s) after {}",
                        symptom.title, days_diff, med.title,
                    ),
                });
            }
        }
    }

    correlations
}

/// Fetches explicit symptom-medication links stored via related_medication_id.
pub fn fetch_explicit_correlations(
    conn: &Connection,
) -> Result<Vec<TimelineCorrelation>, DatabaseError> {
    let mut stmt = conn.prepare(
        "SELECT s.id AS symptom_id, s.specific,
                m.id AS med_id, m.generic_name
         FROM symptoms s
         JOIN medications m ON s.related_medication_id = m.id
         WHERE s.related_medication_id IS NOT NULL",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(TimelineCorrelation {
            source_id: row.get::<_, String>("symptom_id")?,
            target_id: row.get::<_, String>("med_id")?,
            correlation_type: CorrelationType::ExplicitLink,
            description: format!(
                "{} linked to {}",
                row.get::<_, String>("specific")?,
                row.get::<_, String>("generic_name")?,
            ),
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(DatabaseError::from)
}

// ── Aggregate Queries ──────────────────────────────────────────────────────

/// Computes total event counts across all tables (unfiltered).
/// Used for filter badge counts.
pub fn compute_event_counts(conn: &Connection) -> Result<EventCounts, DatabaseError> {
    let count = |sql: &str| -> Result<u32, DatabaseError> {
        conn.query_row(sql, [], |row| row.get(0))
            .map_err(DatabaseError::from)
    };

    Ok(EventCounts {
        medications: count("SELECT COUNT(*) FROM medications WHERE start_date IS NOT NULL")?
            + count("SELECT COUNT(*) FROM dose_changes")?,
        lab_results: count("SELECT COUNT(*) FROM lab_results")?,
        symptoms: count("SELECT COUNT(*) FROM symptoms")?,
        procedures: count("SELECT COUNT(*) FROM procedures WHERE date IS NOT NULL")?,
        appointments: count("SELECT COUNT(*) FROM appointments")?,
        documents: count("SELECT COUNT(*) FROM documents")?,
        diagnoses: count("SELECT COUNT(*) FROM diagnoses WHERE date_diagnosed IS NOT NULL")?,
    })
}

/// Fetches professionals with their event counts for the filter dropdown.
pub fn fetch_professionals_with_counts(
    conn: &Connection,
) -> Result<Vec<ProfessionalSummary>, DatabaseError> {
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
         GROUP BY p.id
         HAVING event_count > 0
         ORDER BY event_count DESC",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(ProfessionalSummary {
            id: row.get::<_, String>("id")?,
            name: row.get("name")?,
            specialty: row.get("specialty")?,
            event_count: row.get("event_count")?,
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(DatabaseError::from)
}

/// Top-level assembly: assembles all timeline data in a single call.
pub fn get_timeline_data(
    conn: &Connection,
    filter: &TimelineFilter,
) -> Result<TimelineData, DatabaseError> {
    let events = assemble_timeline_events(conn, filter)?;

    let mut correlations = detect_correlations(&events);
    let explicit = fetch_explicit_correlations(conn)?;
    correlations.extend(explicit);

    // Deduplicate (same source+target pair)
    correlations.sort_by(|a, b| (&a.source_id, &a.target_id).cmp(&(&b.source_id, &b.target_id)));
    correlations.dedup_by(|a, b| a.source_id == b.source_id && a.target_id == b.target_id);

    let date_range = DateRange {
        earliest: events.first().map(|e| e.date.clone()),
        latest: events.last().map(|e| e.date.clone()),
    };

    let event_counts = compute_event_counts(conn)?;
    let professionals = fetch_professionals_with_counts(conn)?;

    Ok(TimelineData {
        events,
        correlations,
        date_range,
        event_counts,
        professionals,
    })
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_memory_database;

    fn setup_db() -> Connection {
        open_memory_database().expect("Failed to open test DB")
    }

    fn insert_professional(conn: &Connection, id: &str, name: &str, specialty: &str) {
        conn.execute(
            "INSERT INTO professionals (id, name, specialty) VALUES (?1, ?2, ?3)",
            params![id, name, specialty],
        )
        .unwrap();
    }

    fn insert_document(conn: &Connection, id: &str, title: &str, date: &str, prof_id: Option<&str>) {
        conn.execute(
            "INSERT INTO documents (id, type, title, document_date, ingestion_date, source_file, professional_id)
             VALUES (?1, 'clinical_note', ?2, ?3, ?3, 'test.pdf', ?4)",
            params![id, title, date, prof_id],
        )
        .unwrap();
    }

    fn insert_medication(
        conn: &Connection,
        id: &str,
        generic: &str,
        dose: &str,
        start: &str,
        end: Option<&str>,
        status: &str,
        doc_id: &str,
        prescriber: Option<&str>,
    ) {
        conn.execute(
            "INSERT INTO medications (id, generic_name, dose, frequency, frequency_type, status, start_date, end_date, document_id, prescriber_id)
             VALUES (?1, ?2, ?3, 'daily', 'scheduled', ?4, ?5, ?6, ?7, ?8)",
            params![id, generic, dose, status, start, end, doc_id, prescriber],
        )
        .unwrap();
    }

    // ── Assembly Tests ─────────────────────────────────────────────────

    #[test]
    fn test_assemble_empty_database() {
        let conn = setup_db();
        let filter = TimelineFilter::default();
        let events = assemble_timeline_events(&conn, &filter).unwrap();
        assert!(events.is_empty());
    }

    #[test]
    fn test_assemble_medications_start() {
        let conn = setup_db();
        insert_professional(&conn, "prof-1", "Dr. Chen", "Endocrinology");
        insert_document(&conn, "doc-1", "Prescription", "2026-01-15", Some("prof-1"));
        insert_medication(&conn, "med-1", "Metformin", "500mg", "2026-01-15", None, "active", "doc-1", Some("prof-1"));

        let filter = TimelineFilter::default();
        let events = assemble_timeline_events(&conn, &filter).unwrap();

        assert_eq!(events.len(), 2); // 1 med start + 1 document
        let med = events.iter().find(|e| e.event_type == EventType::MedicationStart).unwrap();
        assert_eq!(med.title, "Started Metformin");
        assert_eq!(med.date, "2026-01-15");
        assert_eq!(med.professional_name.as_deref(), Some("Dr. Chen"));
    }

    #[test]
    fn test_assemble_medications_stop() {
        let conn = setup_db();
        insert_document(&conn, "doc-1", "Note", "2026-01-01", None);
        insert_medication(&conn, "med-1", "Aspirin", "100mg", "2026-01-01", Some("2026-02-01"), "stopped", "doc-1", None);

        let filter = TimelineFilter::default();
        let events = assemble_timeline_events(&conn, &filter).unwrap();

        let stops: Vec<_> = events.iter().filter(|e| e.event_type == EventType::MedicationStop).collect();
        assert_eq!(stops.len(), 1);
        assert_eq!(stops[0].title, "Stopped Aspirin");
        assert_eq!(stops[0].date, "2026-02-01");
    }

    #[test]
    fn test_assemble_dose_changes() {
        let conn = setup_db();
        insert_document(&conn, "doc-1", "Note", "2026-01-01", None);
        insert_medication(&conn, "med-1", "Metformin", "500mg", "2026-01-01", None, "active", "doc-1", None);

        conn.execute(
            "INSERT INTO dose_changes (id, medication_id, old_dose, new_dose, change_date)
             VALUES ('dc-1', 'med-1', '500mg', '1000mg', '2026-02-01')",
            [],
        ).unwrap();

        let filter = TimelineFilter::default();
        let events = assemble_timeline_events(&conn, &filter).unwrap();

        let dcs: Vec<_> = events.iter().filter(|e| e.event_type == EventType::MedicationDoseChange).collect();
        assert_eq!(dcs.len(), 1);
        assert_eq!(dcs[0].title, "Metformin dose changed");
    }

    #[test]
    fn test_assemble_lab_results() {
        let conn = setup_db();
        insert_document(&conn, "doc-1", "Lab Report", "2026-01-10", None);

        conn.execute(
            "INSERT INTO lab_results (id, test_name, value, unit, abnormal_flag, collection_date, document_id)
             VALUES ('lab-1', 'HbA1c', 6.5, '%', 'high', '2026-01-10', 'doc-1')",
            [],
        ).unwrap();

        let filter = TimelineFilter::default();
        let events = assemble_timeline_events(&conn, &filter).unwrap();

        let labs: Vec<_> = events.iter().filter(|e| e.event_type == EventType::LabResult).collect();
        assert_eq!(labs.len(), 1);
        assert_eq!(labs[0].title, "HbA1c");
        assert_eq!(labs[0].severity, Some(EventSeverity::High));
    }

    #[test]
    fn test_assemble_symptoms() {
        let conn = setup_db();

        conn.execute(
            "INSERT INTO symptoms (id, category, specific, severity, onset_date, recorded_date, source)
             VALUES ('sym-1', 'Digestive', 'Nausea', 3, '2026-01-20', '2026-01-20', 'patient_reported')",
            [],
        ).unwrap();

        let filter = TimelineFilter::default();
        let events = assemble_timeline_events(&conn, &filter).unwrap();

        let syms: Vec<_> = events.iter().filter(|e| e.event_type == EventType::Symptom).collect();
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].title, "Nausea");
        assert_eq!(syms[0].severity, Some(EventSeverity::Moderate));
    }

    #[test]
    fn test_assemble_procedures() {
        let conn = setup_db();
        insert_document(&conn, "doc-1", "Report", "2026-01-05", None);

        conn.execute(
            "INSERT INTO procedures (id, name, date, facility, document_id)
             VALUES ('proc-1', 'Blood Draw', '2026-01-05', 'City Lab', 'doc-1')",
            [],
        ).unwrap();

        let filter = TimelineFilter::default();
        let events = assemble_timeline_events(&conn, &filter).unwrap();

        let procs: Vec<_> = events.iter().filter(|e| e.event_type == EventType::Procedure).collect();
        assert_eq!(procs.len(), 1);
        assert_eq!(procs[0].title, "Blood Draw");
    }

    #[test]
    fn test_assemble_appointments() {
        let conn = setup_db();
        insert_professional(&conn, "prof-1", "Dr. Smith", "Cardiology");

        conn.execute(
            "INSERT INTO appointments (id, professional_id, date, type)
             VALUES ('appt-1', 'prof-1', '2026-01-25', 'completed')",
            [],
        ).unwrap();

        let filter = TimelineFilter::default();
        let events = assemble_timeline_events(&conn, &filter).unwrap();

        let appts: Vec<_> = events.iter().filter(|e| e.event_type == EventType::Appointment).collect();
        assert_eq!(appts.len(), 1);
        assert_eq!(appts[0].title, "Visit with Dr. Smith");
    }

    #[test]
    fn test_assemble_documents() {
        let conn = setup_db();
        insert_document(&conn, "doc-1", "Lab Report", "2026-01-10", None);

        let filter = TimelineFilter::default();
        let events = assemble_timeline_events(&conn, &filter).unwrap();

        let docs: Vec<_> = events.iter().filter(|e| e.event_type == EventType::Document).collect();
        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].title, "Lab Report");
    }

    #[test]
    fn test_assemble_diagnoses() {
        let conn = setup_db();
        insert_professional(&conn, "prof-1", "Dr. Chen", "Endocrinology");
        insert_document(&conn, "doc-1", "Clinical Note", "2026-01-15", Some("prof-1"));

        conn.execute(
            "INSERT INTO diagnoses (id, name, icd_code, date_diagnosed, status, diagnosing_professional_id, document_id)
             VALUES ('dx-1', 'Type 2 Diabetes', 'E11.9', '2026-01-15', 'active', 'prof-1', 'doc-1')",
            [],
        ).unwrap();

        let filter = TimelineFilter::default();
        let events = assemble_timeline_events(&conn, &filter).unwrap();

        let dx: Vec<_> = events.iter().filter(|e| e.event_type == EventType::Diagnosis).collect();
        assert_eq!(dx.len(), 1);
        assert_eq!(dx[0].title, "Type 2 Diabetes");
    }

    #[test]
    fn test_events_sorted_by_date() {
        let conn = setup_db();
        insert_document(&conn, "doc-1", "Note", "2026-01-01", None);
        insert_medication(&conn, "med-1", "Aspirin", "100mg", "2026-02-01", None, "active", "doc-1", None);

        conn.execute(
            "INSERT INTO symptoms (id, category, specific, severity, onset_date, recorded_date, source)
             VALUES ('sym-1', 'Pain', 'Headache', 2, '2026-01-15', '2026-01-15', 'patient_reported')",
            [],
        ).unwrap();

        let filter = TimelineFilter::default();
        let events = assemble_timeline_events(&conn, &filter).unwrap();

        // Should be sorted: doc (Jan 1), symptom (Jan 15), med (Feb 1)
        assert!(events.len() >= 3);
        for i in 1..events.len() {
            assert!(events[i].date >= events[i - 1].date, "Events not sorted at index {i}");
        }
    }

    // ── Filter Tests ───────────────────────────────────────────────────

    #[test]
    fn test_filter_by_event_type() {
        let conn = setup_db();
        insert_document(&conn, "doc-1", "Note", "2026-01-01", None);
        insert_medication(&conn, "med-1", "Aspirin", "100mg", "2026-01-01", None, "active", "doc-1", None);

        conn.execute(
            "INSERT INTO symptoms (id, category, specific, severity, onset_date, recorded_date, source)
             VALUES ('sym-1', 'Pain', 'Headache', 2, '2026-01-15', '2026-01-15', 'patient_reported')",
            [],
        ).unwrap();

        let filter = TimelineFilter {
            event_types: Some(vec![EventType::Symptom]),
            ..Default::default()
        };
        let events = assemble_timeline_events(&conn, &filter).unwrap();

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, EventType::Symptom);
    }

    #[test]
    fn test_filter_by_professional() {
        let conn = setup_db();
        insert_professional(&conn, "prof-1", "Dr. Chen", "Endo");
        insert_professional(&conn, "prof-2", "Dr. Smith", "Cardio");
        insert_document(&conn, "doc-1", "Note1", "2026-01-01", Some("prof-1"));
        insert_document(&conn, "doc-2", "Note2", "2026-01-02", Some("prof-2"));
        insert_medication(&conn, "med-1", "Metformin", "500mg", "2026-01-01", None, "active", "doc-1", Some("prof-1"));
        insert_medication(&conn, "med-2", "Lisinopril", "10mg", "2026-01-02", None, "active", "doc-2", Some("prof-2"));

        let filter = TimelineFilter {
            professional_id: Some("prof-1".into()),
            ..Default::default()
        };
        let events = assemble_timeline_events(&conn, &filter).unwrap();

        // Only events with prof-1
        for ev in &events {
            assert_eq!(ev.professional_id.as_deref(), Some("prof-1"));
        }
    }

    #[test]
    fn test_filter_by_date_range() {
        let conn = setup_db();
        insert_document(&conn, "doc-1", "Note", "2026-01-01", None);
        insert_medication(&conn, "med-1", "Aspirin", "100mg", "2026-01-01", None, "active", "doc-1", None);
        insert_medication(&conn, "med-2", "Metformin", "500mg", "2026-03-01", None, "active", "doc-1", None);

        let filter = TimelineFilter {
            date_from: Some("2026-02-01".into()),
            ..Default::default()
        };
        let events = assemble_timeline_events(&conn, &filter).unwrap();

        // Only events on or after Feb 1 should be included
        for ev in &events {
            assert!(ev.date.as_str() >= "2026-02-01", "Event {} has date {} before filter", ev.title, ev.date);
        }
    }

    #[test]
    fn test_since_appointment_resolves_date() {
        let conn = setup_db();
        insert_professional(&conn, "prof-1", "Dr. Chen", "Endo");
        conn.execute(
            "INSERT INTO appointments (id, professional_id, date, type) VALUES ('appt-1', 'prof-1', '2026-01-15', 'completed')",
            [],
        ).unwrap();

        let filter = TimelineFilter {
            since_appointment_id: Some("appt-1".into()),
            ..Default::default()
        };
        let (date_from, _) = resolve_date_bounds(&conn, &filter).unwrap();

        // Should be 30 days before appointment (Dec 16, 2025)
        assert!(date_from.is_some());
        assert_eq!(date_from.unwrap(), "2025-12-16");
    }

    #[test]
    fn test_since_appointment_not_found() {
        let conn = setup_db();
        let filter = TimelineFilter {
            since_appointment_id: Some("nonexistent".into()),
            ..Default::default()
        };
        let result = resolve_date_bounds(&conn, &filter);
        assert!(result.is_err());
    }

    // ── Correlation Tests ──────────────────────────────────────────────

    #[test]
    fn test_detect_correlations_within_window() {
        let events = vec![
            TimelineEvent {
                id: "med-1".into(),
                event_type: EventType::MedicationStart,
                date: "2026-01-10".into(),
                title: "Started Metformin".into(),
                subtitle: None,
                professional_id: None,
                professional_name: None,
                document_id: None,
                severity: None,
                metadata: EventMetadata::Medication {
                    generic_name: "Metformin".into(),
                    brand_name: None,
                    dose: "500mg".into(),
                    frequency: "daily".into(),
                    status: "active".into(),
                    reason: None,
                },
            },
            TimelineEvent {
                id: "sym-1".into(),
                event_type: EventType::Symptom,
                date: "2026-01-15".into(),
                title: "Nausea".into(),
                subtitle: None,
                professional_id: None,
                professional_name: None,
                document_id: None,
                severity: Some(EventSeverity::Moderate),
                metadata: EventMetadata::Symptom {
                    category: "Digestive".into(),
                    specific: "Nausea".into(),
                    severity: 3,
                    body_region: None,
                    still_active: true,
                },
            },
        ];

        let corrs = detect_correlations(&events);
        assert_eq!(corrs.len(), 1);
        assert_eq!(corrs[0].source_id, "sym-1");
        assert_eq!(corrs[0].target_id, "med-1");
        assert_eq!(corrs[0].correlation_type, CorrelationType::SymptomAfterMedicationStart);
        assert!(corrs[0].description.contains("5 day(s)"));
    }

    #[test]
    fn test_detect_correlations_outside_window() {
        let events = vec![
            TimelineEvent {
                id: "med-1".into(),
                event_type: EventType::MedicationStart,
                date: "2026-01-01".into(),
                title: "Started Metformin".into(),
                subtitle: None,
                professional_id: None,
                professional_name: None,
                document_id: None,
                severity: None,
                metadata: EventMetadata::Medication {
                    generic_name: "Metformin".into(),
                    brand_name: None,
                    dose: "500mg".into(),
                    frequency: "daily".into(),
                    status: "active".into(),
                    reason: None,
                },
            },
            TimelineEvent {
                id: "sym-1".into(),
                event_type: EventType::Symptom,
                date: "2026-02-15".into(),
                title: "Nausea".into(),
                subtitle: None,
                professional_id: None,
                professional_name: None,
                document_id: None,
                severity: Some(EventSeverity::Moderate),
                metadata: EventMetadata::Symptom {
                    category: "Digestive".into(),
                    specific: "Nausea".into(),
                    severity: 3,
                    body_region: None,
                    still_active: true,
                },
            },
        ];

        let corrs = detect_correlations(&events);
        assert!(corrs.is_empty(), "Should not detect correlation outside 14-day window");
    }

    #[test]
    fn test_explicit_correlations() {
        let conn = setup_db();
        insert_document(&conn, "doc-1", "Note", "2026-01-01", None);
        insert_medication(&conn, "med-1", "Metformin", "500mg", "2026-01-01", None, "active", "doc-1", None);

        conn.execute(
            "INSERT INTO symptoms (id, category, specific, severity, onset_date, recorded_date, source, related_medication_id)
             VALUES ('sym-1', 'Digestive', 'Nausea', 3, '2026-01-20', '2026-01-20', 'patient_reported', 'med-1')",
            [],
        ).unwrap();

        let corrs = fetch_explicit_correlations(&conn).unwrap();
        assert_eq!(corrs.len(), 1);
        assert_eq!(corrs[0].correlation_type, CorrelationType::ExplicitLink);
        assert_eq!(corrs[0].source_id, "sym-1");
        assert_eq!(corrs[0].target_id, "med-1");
    }

    #[test]
    fn test_correlation_deduplication() {
        let conn = setup_db();
        insert_document(&conn, "doc-1", "Note", "2026-01-01", None);
        insert_medication(&conn, "med-1", "Metformin", "500mg", "2026-01-10", None, "active", "doc-1", None);

        conn.execute(
            "INSERT INTO symptoms (id, category, specific, severity, onset_date, recorded_date, source, related_medication_id)
             VALUES ('sym-1', 'Digestive', 'Nausea', 3, '2026-01-15', '2026-01-15', 'patient_reported', 'med-1')",
            [],
        ).unwrap();

        let filter = TimelineFilter::default();
        let data = get_timeline_data(&conn, &filter).unwrap();

        // Both temporal and explicit should detect the same pair — but dedup to 1
        let pair_count = data
            .correlations
            .iter()
            .filter(|c| c.source_id == "sym-1" && c.target_id == "med-1")
            .count();
        assert_eq!(pair_count, 1, "Duplicate correlations should be deduped");
    }

    // ── Aggregate Tests ────────────────────────────────────────────────

    #[test]
    fn test_event_counts_all_tables() {
        let conn = setup_db();
        insert_professional(&conn, "prof-1", "Dr. Chen", "Endo");
        insert_document(&conn, "doc-1", "Note", "2026-01-01", None);
        insert_medication(&conn, "med-1", "Aspirin", "100mg", "2026-01-01", None, "active", "doc-1", None);

        conn.execute(
            "INSERT INTO lab_results (id, test_name, abnormal_flag, collection_date, document_id)
             VALUES ('lab-1', 'CBC', 'normal', '2026-01-05', 'doc-1')",
            [],
        ).unwrap();

        conn.execute(
            "INSERT INTO symptoms (id, category, specific, severity, onset_date, recorded_date, source)
             VALUES ('sym-1', 'Pain', 'Headache', 2, '2026-01-10', '2026-01-10', 'patient_reported')",
            [],
        ).unwrap();

        conn.execute(
            "INSERT INTO appointments (id, professional_id, date, type) VALUES ('appt-1', 'prof-1', '2026-01-20', 'completed')",
            [],
        ).unwrap();

        let counts = compute_event_counts(&conn).unwrap();
        assert_eq!(counts.medications, 1); // 1 med start
        assert_eq!(counts.lab_results, 1);
        assert_eq!(counts.symptoms, 1);
        assert_eq!(counts.appointments, 1);
        assert_eq!(counts.documents, 1);
    }

    #[test]
    fn test_professionals_with_counts() {
        let conn = setup_db();
        insert_professional(&conn, "prof-1", "Dr. Chen", "Endo");
        insert_professional(&conn, "prof-2", "Dr. Idle", "Derm");
        insert_document(&conn, "doc-1", "Note", "2026-01-01", Some("prof-1"));
        insert_medication(&conn, "med-1", "Metformin", "500mg", "2026-01-01", None, "active", "doc-1", Some("prof-1"));

        let profs = fetch_professionals_with_counts(&conn).unwrap();

        // prof-1 has events (1 med + 1 doc), prof-2 has none
        assert!(profs.iter().any(|p| p.id == "prof-1"));
        assert!(!profs.iter().any(|p| p.id == "prof-2"), "Prof with 0 events should be excluded");

        let chen = profs.iter().find(|p| p.id == "prof-1").unwrap();
        assert!(chen.event_count > 0);
    }

    // ── Severity Mapping Tests ─────────────────────────────────────────

    #[test]
    fn test_severity_from_lab_flag() {
        assert_eq!(severity_from_lab_flag("normal"), EventSeverity::Normal);
        assert_eq!(severity_from_lab_flag("low"), EventSeverity::Low);
        assert_eq!(severity_from_lab_flag("high"), EventSeverity::High);
        assert_eq!(severity_from_lab_flag("critical_low"), EventSeverity::Critical);
        assert_eq!(severity_from_lab_flag("critical_high"), EventSeverity::Critical);
        assert_eq!(severity_from_lab_flag("unknown"), EventSeverity::Normal);
    }

    #[test]
    fn test_severity_from_symptom() {
        assert_eq!(severity_from_symptom(1), EventSeverity::Low);
        assert_eq!(severity_from_symptom(2), EventSeverity::Low);
        assert_eq!(severity_from_symptom(3), EventSeverity::Moderate);
        assert_eq!(severity_from_symptom(4), EventSeverity::High);
        assert_eq!(severity_from_symptom(5), EventSeverity::Critical);
    }

    // ── Type Serialization Test ────────────────────────────────────────

    #[test]
    fn test_event_type_serialization_roundtrip() {
        let types = vec![
            EventType::MedicationStart,
            EventType::MedicationStop,
            EventType::MedicationDoseChange,
            EventType::LabResult,
            EventType::Symptom,
            EventType::Procedure,
            EventType::Appointment,
            EventType::Document,
            EventType::Diagnosis,
        ];

        for t in types {
            let json = serde_json::to_string(&t).unwrap();
            let back: EventType = serde_json::from_str(&json).unwrap();
            assert_eq!(back, t);
        }
    }

    // ── Full Pipeline Test ─────────────────────────────────────────────

    #[test]
    fn test_timeline_data_structure() {
        let conn = setup_db();
        let filter = TimelineFilter::default();
        let data = get_timeline_data(&conn, &filter).unwrap();

        // Empty DB should return valid structure with empty vecs
        assert!(data.events.is_empty());
        assert!(data.correlations.is_empty());
        assert!(data.date_range.earliest.is_none());
        assert!(data.date_range.latest.is_none());
        assert_eq!(data.event_counts.medications, 0);
        assert!(data.professionals.is_empty());
    }
}

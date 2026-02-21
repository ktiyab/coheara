use rusqlite::Connection;

use crate::db::DatabaseError;
use super::types::*;

/// Helper: builds dynamic WHERE clause with date bounds.
pub(super) struct DateBoundQuery {
    clauses: Vec<String>,
    params: Vec<Box<dyn rusqlite::types::ToSql>>,
}

impl DateBoundQuery {
    pub(super) fn new(
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

    pub(super) fn sql_suffix(&self) -> String {
        self.clauses.join("")
    }

    pub(super) fn param_refs(&self) -> Vec<&dyn rusqlite::types::ToSql> {
        self.params.iter().map(|p| p.as_ref()).collect()
    }
}

pub(super) fn severity_from_lab_flag(flag: &str) -> EventSeverity {
    match flag {
        "normal" => EventSeverity::Normal,
        "low" => EventSeverity::Low,
        "high" => EventSeverity::High,
        "critical_low" | "critical_high" => EventSeverity::Critical,
        _ => EventSeverity::Normal,
    }
}

pub(super) fn severity_from_symptom(sev: u8) -> EventSeverity {
    match sev {
        1 => EventSeverity::Low,
        2 => EventSeverity::Low,
        3 => EventSeverity::Moderate,
        4 => EventSeverity::High,
        5 => EventSeverity::Critical,
        _ => EventSeverity::Normal,
    }
}

pub(super) fn fetch_medication_starts(
    conn: &Connection,
    date_from: &Option<String>,
    date_to: &Option<String>,
) -> Result<Vec<TimelineEvent>, DatabaseError> {
    let bounds = DateBoundQuery::new("m.start_date", date_from, date_to);
    let sql = format!(
        "SELECT m.id, m.generic_name, m.brand_name, m.dose, m.frequency,
                m.status, m.start_date, m.reason_start,
                m.prescriber_id, p.name AS prof_name, m.document_id,
                m.route, m.frequency_type, m.is_otc, m.condition, m.administration_instructions
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
                route: row.get("route")?,
                frequency_type: row.get("frequency_type")?,
                is_otc: Some(row.get::<_, i32>("is_otc")? != 0),
                condition: row.get("condition")?,
                administration_instructions: row.get("administration_instructions")?,
            },
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>().map_err(DatabaseError::from)
}

pub(super) fn fetch_medication_stops(
    conn: &Connection,
    date_from: &Option<String>,
    date_to: &Option<String>,
) -> Result<Vec<TimelineEvent>, DatabaseError> {
    let bounds = DateBoundQuery::new("m.end_date", date_from, date_to);
    let sql = format!(
        "SELECT m.id, m.generic_name, m.brand_name, m.dose, m.frequency,
                m.status, m.end_date, m.reason_stop,
                m.prescriber_id, p.name AS prof_name, m.document_id,
                m.route, m.frequency_type, m.is_otc, m.condition, m.administration_instructions
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
                route: row.get("route")?,
                frequency_type: row.get("frequency_type")?,
                is_otc: Some(row.get::<_, i32>("is_otc")? != 0),
                condition: row.get("condition")?,
                administration_instructions: row.get("administration_instructions")?,
            },
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>().map_err(DatabaseError::from)
}

pub(super) fn fetch_dose_changes(
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

pub(super) fn fetch_lab_events(
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

pub(super) fn fetch_symptom_events(
    conn: &Connection,
    date_from: &Option<String>,
    date_to: &Option<String>,
) -> Result<Vec<TimelineEvent>, DatabaseError> {
    let bounds = DateBoundQuery::new("s.onset_date", date_from, date_to);
    let sql = format!(
        "SELECT s.id, s.category, s.specific, s.severity, s.body_region,
                s.still_active, s.onset_date, s.related_medication_id,
                s.duration, s.character, s.aggravating, s.relieving,
                s.timing_pattern, s.resolved_date, s.notes, s.source,
                s.related_diagnosis_id
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
                duration: row.get("duration")?,
                character: row.get("character")?,
                aggravating: row.get("aggravating")?,
                relieving: row.get("relieving")?,
                timing_pattern: row.get("timing_pattern")?,
                resolved_date: row.get("resolved_date")?,
                notes: row.get("notes")?,
                source: row.get("source")?,
                related_medication_id: row.get("related_medication_id")?,
                related_diagnosis_id: row.get("related_diagnosis_id")?,
            },
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>().map_err(DatabaseError::from)
}

pub(super) fn fetch_procedure_events(
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

pub(super) fn fetch_appointment_events(
    conn: &Connection,
    date_from: &Option<String>,
    date_to: &Option<String>,
) -> Result<Vec<TimelineEvent>, DatabaseError> {
    let bounds = DateBoundQuery::new("a.date", date_from, date_to);
    let sql = format!(
        "SELECT a.id, a.date, a.type AS appt_type,
                a.professional_id, p.name AS prof_name, p.specialty,
                a.pre_summary_generated, a.post_notes
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
                pre_summary_generated: Some(row.get::<_, i32>("pre_summary_generated")? != 0),
                post_notes: row.get("post_notes")?,
            },
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>().map_err(DatabaseError::from)
}

pub(super) fn fetch_document_events(
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

pub(super) fn fetch_diagnosis_events(
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

pub(super) fn severity_from_alert_level(level: &str) -> EventSeverity {
    match level {
        "critical" => EventSeverity::Critical,
        "standard" => EventSeverity::Moderate,
        "info" => EventSeverity::Low,
        _ => EventSeverity::Low,
    }
}

fn format_alert_title(alert_type: &str) -> String {
    match alert_type {
        "conflict" => "Medication Conflict".into(),
        "gap" => "Coverage Gap".into(),
        "drift" => "Treatment Drift".into(),
        "ambiguity" => "Ambiguous Information".into(),
        "duplicate" => "Duplicate Therapy".into(),
        "allergy" => "Allergy Alert".into(),
        "dose" => "Dosage Concern".into(),
        "critical" => "Critical Alert".into(),
        "temporal" => "Timing Alert".into(),
        _ => {
            let formatted = alert_type.replace('_', " ");
            let mut chars = formatted.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => c.to_uppercase().to_string() + chars.as_str(),
            }
        }
    }
}

pub(super) fn fetch_coherence_alert_events(
    conn: &Connection,
    date_from: &Option<String>,
    date_to: &Option<String>,
    include_dismissed: bool,
) -> Result<Vec<TimelineEvent>, DatabaseError> {
    let bounds = DateBoundQuery::new("ca.detected_at", date_from, date_to);
    let dismissed_clause = if include_dismissed { "" } else { " AND ca.dismissed = 0" };
    let sql = format!(
        "SELECT ca.id, ca.alert_type, ca.severity, ca.entity_ids,
                ca.patient_message, ca.detected_at, ca.dismissed,
                ca.two_step_confirmed
         FROM coherence_alerts ca
         WHERE 1=1{dismissed_clause}{}",
        bounds.sql_suffix()
    );

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(bounds.param_refs().as_slice(), |row| {
        let alert_type: String = row.get("alert_type")?;
        let severity_str: String = row.get("severity")?;
        let entity_ids_json: Option<String> = row.get("entity_ids")?;
        let entity_ids: Vec<String> = entity_ids_json
            .and_then(|j| serde_json::from_str(&j).ok())
            .unwrap_or_default();
        let dismissed_int: i32 = row.get("dismissed")?;
        let confirmed_int: i32 = row.get("two_step_confirmed")?;
        let patient_message: Option<String> = row.get("patient_message")?;

        Ok(TimelineEvent {
            id: row.get::<_, String>("id")?,
            event_type: EventType::CoherenceAlert,
            date: row.get::<_, String>("detected_at")?,
            title: format_alert_title(&alert_type),
            subtitle: patient_message.clone(),
            professional_id: None,
            professional_name: None,
            document_id: None,
            severity: Some(severity_from_alert_level(&severity_str)),
            metadata: EventMetadata::CoherenceAlert {
                alert_type,
                severity: severity_str,
                patient_message,
                entity_ids,
                dismissed: dismissed_int != 0,
                two_step_confirmed: confirmed_int != 0,
            },
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>().map_err(DatabaseError::from)
}

fn format_vital_type(vital_type: &str) -> String {
    match vital_type {
        "blood_pressure" => "Blood Pressure".into(),
        "heart_rate" => "Heart Rate".into(),
        "temperature" => "Temperature".into(),
        "weight" => "Weight".into(),
        "blood_glucose" => "Blood Glucose".into(),
        "oxygen_saturation" => "Oxygen Saturation".into(),
        _ => {
            let formatted = vital_type.replace('_', " ");
            let mut chars = formatted.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => c.to_uppercase().to_string() + chars.as_str(),
            }
        }
    }
}

pub(super) fn fetch_vital_sign_events(
    conn: &Connection,
    date_from: &Option<String>,
    date_to: &Option<String>,
) -> Result<Vec<TimelineEvent>, DatabaseError> {
    let bounds = DateBoundQuery::new("vs.recorded_at", date_from, date_to);
    let sql = format!(
        "SELECT vs.id, vs.vital_type, vs.value_primary, vs.value_secondary,
                vs.unit, vs.recorded_at, vs.notes, vs.source
         FROM vital_signs vs
         WHERE 1=1{}",
        bounds.sql_suffix()
    );

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(bounds.param_refs().as_slice(), |row| {
        let vital_type: String = row.get("vital_type")?;
        let value_primary: f64 = row.get("value_primary")?;
        let value_secondary: Option<f64> = row.get("value_secondary")?;
        let unit: String = row.get("unit")?;

        let subtitle = match value_secondary {
            Some(sec) => format!("{}/{} {}", value_primary, sec, unit),
            None => format!("{} {}", value_primary, unit),
        };

        Ok(TimelineEvent {
            id: row.get::<_, String>("id")?,
            event_type: EventType::VitalSign,
            date: row.get::<_, String>("recorded_at")?,
            title: format_vital_type(&vital_type),
            subtitle: Some(subtitle),
            professional_id: None,
            professional_name: None,
            document_id: None,
            severity: None,
            metadata: EventMetadata::VitalSign {
                vital_type,
                value_primary,
                value_secondary,
                unit,
                notes: row.get("notes")?,
                source: row.get("source")?,
            },
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>().map_err(DatabaseError::from)
}

//! L4-01: Symptom Journal — backend types and repository functions.
//!
//! View types for the symptom journal (recording, history, correlation,
//! nudge logic, category/subcategory data), plus query functions that
//! operate against the existing L0-02 symptoms table.

use chrono::{Local, NaiveDate};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::DatabaseError;
use crate::models::enums::SymptomSource;

// ═══════════════════════════════════════════
// Constants — Categories, Subcategories, Body Regions
// ═══════════════════════════════════════════

pub const CATEGORIES: &[&str] = &[
    "Pain",
    "Digestive",
    "Respiratory",
    "Neurological",
    "General",
    "Mood",
    "Skin",
    "Other",
];

pub fn subcategories_for(category: &str) -> Vec<&'static str> {
    match category {
        "Pain" => vec![
            "Headache", "Back pain", "Joint pain", "Chest pain",
            "Abdominal pain", "Muscle pain", "Neck pain", "Other",
        ],
        "Digestive" => vec![
            "Nausea", "Vomiting", "Diarrhea", "Constipation",
            "Bloating", "Heartburn", "Loss of appetite", "Other",
        ],
        "Respiratory" => vec![
            "Shortness of breath", "Cough", "Wheezing",
            "Chest tightness", "Sore throat", "Congestion", "Other",
        ],
        "Neurological" => vec![
            "Dizziness", "Numbness", "Tingling", "Tremor",
            "Memory issues", "Confusion", "Other",
        ],
        "General" => vec![
            "Fatigue", "Fever", "Chills", "Weight change",
            "Night sweats", "Swelling", "Other",
        ],
        "Mood" => vec![
            "Anxiety", "Low mood", "Irritability", "Sleep difficulty",
            "Difficulty concentrating", "Other",
        ],
        "Skin" => vec![
            "Rash", "Itching", "Bruising", "Dryness",
            "Swelling", "Color change", "Other",
        ],
        "Other" => vec!["Other"],
        _ => vec![],
    }
}

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

// ═══════════════════════════════════════════
// View types — serialised to frontend
// ═══════════════════════════════════════════

/// Input for recording a new symptom (from guided flow).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymptomEntry {
    pub category: String,
    pub specific: String,
    pub severity: u8,
    pub onset_date: String, // YYYY-MM-DD
    pub onset_time: Option<String>, // HH:MM or null
    pub body_region: Option<String>,
    pub duration: Option<String>,
    pub character: Option<String>,
    pub aggravating: Vec<String>,
    pub relieving: Vec<String>,
    pub timing_pattern: Option<String>,
    pub notes: Option<String>,
}

/// Stored symptom with joined metadata for history display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredSymptom {
    pub id: String,
    pub category: String,
    pub specific: String,
    pub severity: u8,
    pub body_region: Option<String>,
    pub duration: Option<String>,
    pub character: Option<String>,
    pub aggravating: Option<String>,
    pub relieving: Option<String>,
    pub timing_pattern: Option<String>,
    pub onset_date: String,
    pub onset_time: Option<String>,
    pub recorded_date: String,
    pub still_active: bool,
    pub resolved_date: Option<String>,
    pub related_medication_name: Option<String>,
    pub related_diagnosis_name: Option<String>,
    pub notes: Option<String>,
    pub source: String,
}

/// Temporal correlation — medication change near symptom onset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalCorrelation {
    pub medication_name: String,
    pub medication_change_date: String,
    pub days_since_change: i64,
    pub message: String,
}

/// Result of recording a symptom.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordResult {
    pub symptom_id: String,
    pub correlations: Vec<TemporalCorrelation>,
}

/// Check-in nudge decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NudgeDecision {
    pub should_nudge: bool,
    pub nudge_type: Option<String>, // "DailyCheckIn" or "PostMedicationChange"
    pub message: Option<String>,
    pub related_medication: Option<String>,
}

/// Symptom history filter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymptomFilter {
    pub category: Option<String>,
    pub severity_min: Option<u8>,
    pub severity_max: Option<u8>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub still_active: Option<bool>,
}

/// Category info with subcategories.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryInfo {
    pub name: String,
    pub subcategories: Vec<String>,
}

// ═══════════════════════════════════════════
// Repository functions
// ═══════════════════════════════════════════

/// Records a new symptom entry. Returns the generated UUID.
pub fn record_symptom(conn: &Connection, entry: &SymptomEntry) -> Result<Uuid, DatabaseError> {
    let symptom_id = Uuid::new_v4();
    let now = Local::now().naive_local().format("%Y-%m-%d %H:%M:%S").to_string();

    let aggravating = if entry.aggravating.is_empty() {
        None
    } else {
        Some(entry.aggravating.join(", "))
    };
    let relieving = if entry.relieving.is_empty() {
        None
    } else {
        Some(entry.relieving.join(", "))
    };

    conn.execute(
        "INSERT INTO symptoms (id, category, specific, severity, body_region,
         duration, character, aggravating, relieving, timing_pattern,
         onset_date, onset_time, recorded_date, still_active, source, notes)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, 1, ?14, ?15)",
        params![
            symptom_id.to_string(),
            entry.category,
            entry.specific,
            entry.severity as i32,
            entry.body_region,
            entry.duration,
            entry.character,
            aggravating,
            relieving,
            entry.timing_pattern,
            entry.onset_date,
            entry.onset_time,
            now,
            SymptomSource::PatientReported.as_str(),
            entry.notes,
        ],
    )?;

    Ok(symptom_id)
}

/// Fetches symptom history with optional filters, joined with medication/diagnosis names.
pub fn fetch_symptoms_filtered(
    conn: &Connection,
    filter: &Option<SymptomFilter>,
) -> Result<Vec<StoredSymptom>, DatabaseError> {
    let mut sql = String::from(
        "SELECT s.id, s.category, s.specific, s.severity, s.body_region,
                s.duration, s.character, s.aggravating, s.relieving, s.timing_pattern,
                s.onset_date, s.onset_time, s.recorded_date, s.still_active,
                s.resolved_date, s.notes, s.source,
                m.generic_name AS med_name,
                d.name AS diag_name
         FROM symptoms s
         LEFT JOIN medications m ON s.related_medication_id = m.id
         LEFT JOIN diagnoses d ON s.related_diagnosis_id = d.id
         WHERE 1=1"
    );

    let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut param_idx = 1u32;

    if let Some(ref f) = filter {
        if let Some(ref cat) = f.category {
            sql.push_str(&format!(" AND s.category = ?{param_idx}"));
            params_vec.push(Box::new(cat.clone()));
            param_idx += 1;
        }
        if let Some(min) = f.severity_min {
            sql.push_str(&format!(" AND s.severity >= ?{param_idx}"));
            params_vec.push(Box::new(min as i32));
            param_idx += 1;
        }
        if let Some(max) = f.severity_max {
            sql.push_str(&format!(" AND s.severity <= ?{param_idx}"));
            params_vec.push(Box::new(max as i32));
            param_idx += 1;
        }
        if let Some(ref from) = f.date_from {
            sql.push_str(&format!(" AND s.onset_date >= ?{param_idx}"));
            params_vec.push(Box::new(from.clone()));
            param_idx += 1;
        }
        if let Some(ref to) = f.date_to {
            sql.push_str(&format!(" AND s.onset_date <= ?{param_idx}"));
            params_vec.push(Box::new(to.clone()));
            param_idx += 1;
        }
        if let Some(active) = f.still_active {
            sql.push_str(&format!(" AND s.still_active = ?{param_idx}"));
            params_vec.push(Box::new(active as i32));
            param_idx += 1;
        }
    }
    let _ = param_idx; // suppress unused warning

    sql.push_str(" ORDER BY s.recorded_date DESC");

    let param_refs: Vec<&dyn rusqlite::types::ToSql> =
        params_vec.iter().map(|p| p.as_ref()).collect();

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(param_refs.as_slice(), |row| {
        let still_active_int: i32 = row.get(13)?;
        Ok(StoredSymptom {
            id: row.get(0)?,
            category: row.get(1)?,
            specific: row.get(2)?,
            severity: row.get::<_, i32>(3)? as u8,
            body_region: row.get(4)?,
            duration: row.get(5)?,
            character: row.get(6)?,
            aggravating: row.get(7)?,
            relieving: row.get(8)?,
            timing_pattern: row.get(9)?,
            onset_date: row.get(10)?,
            onset_time: row.get(11)?,
            recorded_date: row.get(12)?,
            still_active: still_active_int != 0,
            resolved_date: row.get(14)?,
            notes: row.get(15)?,
            source: row.get(16)?,
            related_medication_name: row.get(17)?,
            related_diagnosis_name: row.get(18)?,
        })
    })?;

    let mut symptoms = Vec::new();
    for row in rows {
        symptoms.push(row?);
    }
    Ok(symptoms)
}

/// Marks a symptom as resolved (no longer active).
pub fn resolve_symptom(conn: &Connection, symptom_id: &str) -> Result<(), DatabaseError> {
    let today = Local::now().date_naive().to_string();
    let updated = conn.execute(
        "UPDATE symptoms SET still_active = 0, resolved_date = ?1 WHERE id = ?2",
        params![today, symptom_id],
    )?;
    if updated == 0 {
        return Err(DatabaseError::NotFound {
            entity_type: "Symptom".into(),
            id: symptom_id.into(),
        });
    }
    Ok(())
}

/// Hard-deletes a symptom entry.
pub fn delete_symptom(conn: &Connection, symptom_id: &str) -> Result<(), DatabaseError> {
    let deleted = conn.execute(
        "DELETE FROM symptoms WHERE id = ?1",
        params![symptom_id],
    )?;
    if deleted == 0 {
        return Err(DatabaseError::NotFound {
            entity_type: "Symptom".into(),
            id: symptom_id.into(),
        });
    }
    Ok(())
}

/// Detects medication changes within 14 days of symptom onset date.
pub fn detect_temporal_correlation(
    conn: &Connection,
    onset_date_str: &str,
) -> Result<Vec<TemporalCorrelation>, DatabaseError> {
    let onset = NaiveDate::parse_from_str(onset_date_str, "%Y-%m-%d")
        .map_err(|e| DatabaseError::ConstraintViolation(format!("Invalid onset date: {e}")))?;
    let window_start = onset - chrono::Duration::days(14);

    let mut correlations = Vec::new();

    // 1. New medications started within the 14-day window
    {
        let mut stmt = conn.prepare(
            "SELECT m.generic_name, m.start_date
             FROM medications m
             WHERE m.start_date BETWEEN ?1 AND ?2
             ORDER BY m.start_date DESC"
        )?;

        let rows = stmt.query_map(
            params![window_start.to_string(), onset_date_str],
            |row| {
                let name: String = row.get(0)?;
                let start_date_str: String = row.get(1)?;
                Ok((name, start_date_str))
            },
        )?;

        for row in rows {
            let (name, start_date_str) = row?;
            if let Ok(start_date) = NaiveDate::parse_from_str(&start_date_str, "%Y-%m-%d") {
                let days = (onset - start_date).num_days();
                correlations.push(TemporalCorrelation {
                    medication_name: name.clone(),
                    medication_change_date: start_date_str,
                    days_since_change: days,
                    message: format!(
                        "You started {} on {}. If you think this might be related, mention it to your doctor at your next visit.",
                        name,
                        start_date.format("%B %d")
                    ),
                });
            }
        }
    }

    // 2. Dose changes within the 14-day window
    {
        let mut stmt = conn.prepare(
            "SELECT m.generic_name, dc.change_date
             FROM dose_changes dc
             JOIN medications m ON dc.medication_id = m.id
             WHERE dc.change_date BETWEEN ?1 AND ?2
             ORDER BY dc.change_date DESC"
        )?;

        let rows = stmt.query_map(
            params![window_start.to_string(), onset_date_str],
            |row| {
                let name: String = row.get(0)?;
                let change_date_str: String = row.get(1)?;
                Ok((name, change_date_str))
            },
        )?;

        for row in rows {
            let (name, change_date_str) = row?;
            if let Ok(change_date) = NaiveDate::parse_from_str(&change_date_str, "%Y-%m-%d") {
                let days = (onset - change_date).num_days();
                correlations.push(TemporalCorrelation {
                    medication_name: name.clone(),
                    medication_change_date: change_date_str,
                    days_since_change: days,
                    message: format!(
                        "Your dose of {} was changed on {}. If you think this might be related, mention it to your doctor at your next visit.",
                        name,
                        change_date.format("%B %d")
                    ),
                });
            }
        }
    }

    correlations.sort_by_key(|c| c.days_since_change);
    Ok(correlations)
}

/// Determines whether to show a check-in nudge.
pub fn check_nudge(conn: &Connection) -> Result<NudgeDecision, DatabaseError> {
    let today = Local::now().date_naive();
    let seven_days_ago = (today - chrono::Duration::days(7)).to_string();

    // 1. Post-medication-change nudge (higher priority)
    let recent_med: Option<(String, String)> = conn
        .query_row(
            "SELECT m.generic_name, m.start_date
             FROM medications m
             WHERE m.start_date >= ?1
             AND m.status = 'active'
             ORDER BY m.start_date DESC
             LIMIT 1",
            params![seven_days_ago],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .ok();

    if let Some((med_name, start_date_str)) = recent_med {
        // Check if there's already a symptom entry since this medication started
        let has_entry: bool = conn.query_row(
            "SELECT COUNT(*) > 0 FROM symptoms WHERE recorded_date >= ?1",
            params![start_date_str],
            |row| row.get(0),
        )?;

        if !has_entry {
            if let Ok(start_date) = NaiveDate::parse_from_str(&start_date_str, "%Y-%m-%d") {
                return Ok(NudgeDecision {
                    should_nudge: true,
                    nudge_type: Some("PostMedicationChange".into()),
                    message: Some(format!(
                        "You started {} on {}. Over the next few days, would you like to track how you're feeling? This can help your doctor understand how you're responding.",
                        med_name,
                        start_date.format("%B %d")
                    )),
                    related_medication: Some(med_name),
                });
            }
        }
    }

    // 2. Daily check-in nudge (3 days no entry + active symptoms)
    let last_entry_date: Option<String> = conn
        .query_row(
            "SELECT MAX(DATE(recorded_date)) FROM symptoms",
            [],
            |row| row.get(0),
        )
        .ok()
        .flatten();

    let active_symptoms: i64 = conn.query_row(
        "SELECT COUNT(*) FROM symptoms WHERE still_active = 1",
        [],
        |row| row.get(0),
    )?;

    if let Some(ref last_str) = last_entry_date {
        if let Ok(last_date) = NaiveDate::parse_from_str(last_str, "%Y-%m-%d") {
            let days_since = (today - last_date).num_days();
            if days_since >= 3 && active_symptoms > 0 {
                return Ok(NudgeDecision {
                    should_nudge: true,
                    nudge_type: Some("DailyCheckIn".into()),
                    message: Some(
                        "It's been a few days \u{2014} would you like to note how you're feeling?"
                            .into(),
                    ),
                    related_medication: None,
                });
            }
        }
    }

    Ok(NudgeDecision {
        should_nudge: false,
        nudge_type: None,
        message: None,
        related_medication: None,
    })
}

/// Returns all symptom categories with their subcategories.
pub fn get_symptom_categories() -> Vec<CategoryInfo> {
    CATEGORIES
        .iter()
        .map(|&cat| CategoryInfo {
            name: cat.to_string(),
            subcategories: subcategories_for(cat)
                .iter()
                .map(|s| s.to_string())
                .collect(),
        })
        .collect()
}

// ═══════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::sqlite::open_memory_database;

    fn test_db() -> Connection {
        open_memory_database().expect("in-memory DB")
    }

    fn make_entry(category: &str, specific: &str, severity: u8) -> SymptomEntry {
        SymptomEntry {
            category: category.into(),
            specific: specific.into(),
            severity,
            onset_date: "2025-01-15".into(),
            onset_time: None,
            body_region: None,
            duration: None,
            character: None,
            aggravating: vec![],
            relieving: vec![],
            timing_pattern: None,
            notes: None,
        }
    }

    fn seed_medication(conn: &Connection, name: &str, start_date: &str) {
        // Need a document first (medications require document_id)
        conn.execute(
            "INSERT OR IGNORE INTO documents (id, type, title, ingestion_date, source_file)
             VALUES ('doc-seed', 'other', 'Test Document', '2025-01-01', 'test.pdf')",
            [],
        ).ok();
        conn.execute(
            "INSERT INTO medications (id, generic_name, dose, frequency, frequency_type, route, start_date, status, is_otc, document_id)
             VALUES (?1, ?2, '10mg', 'once daily', 'scheduled', 'oral', ?3, 'active', 0, 'doc-seed')",
            params![Uuid::new_v4().to_string(), name, start_date],
        ).expect("seed medication");
    }

    fn seed_dose_change(conn: &Connection, med_name: &str, change_date: &str) {
        // Find medication ID
        let med_id: String = conn
            .query_row(
                "SELECT id FROM medications WHERE generic_name = ?1",
                params![med_name],
                |row| row.get(0),
            )
            .expect("medication exists");
        conn.execute(
            "INSERT INTO dose_changes (id, medication_id, new_dose, change_date)
             VALUES (?1, ?2, '20mg', ?3)",
            params![Uuid::new_v4().to_string(), med_id, change_date],
        )
        .expect("seed dose change");
    }

    // ───────────────────────────────────────
    // record_symptom tests
    // ───────────────────────────────────────

    #[test]
    fn record_basic_symptom() {
        let conn = test_db();
        let entry = make_entry("Pain", "Headache", 3);
        let id = record_symptom(&conn, &entry).unwrap();

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM symptoms", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);

        let stored_cat: String = conn
            .query_row(
                "SELECT category FROM symptoms WHERE id = ?1",
                params![id.to_string()],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(stored_cat, "Pain");
    }

    #[test]
    fn record_symptom_with_expanded_fields() {
        let conn = test_db();
        let entry = SymptomEntry {
            category: "Pain".into(),
            specific: "Headache".into(),
            severity: 4,
            onset_date: "2025-01-15".into(),
            onset_time: Some("09:00".into()),
            body_region: Some("head".into()),
            duration: Some("Hours".into()),
            character: Some("Throbbing".into()),
            aggravating: vec!["Stress".into(), "Screen time".into()],
            relieving: vec!["Rest".into()],
            timing_pattern: Some("Morning".into()),
            notes: Some("Occurs after screen time".into()),
        };

        let id = record_symptom(&conn, &entry).unwrap();

        let (agg, rel): (Option<String>, Option<String>) = conn
            .query_row(
                "SELECT aggravating, relieving FROM symptoms WHERE id = ?1",
                params![id.to_string()],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(agg.unwrap(), "Stress, Screen time");
        assert_eq!(rel.unwrap(), "Rest");
    }

    #[test]
    fn record_symptom_empty_aggravating_stores_null() {
        let conn = test_db();
        let entry = make_entry("General", "Fatigue", 2);
        let id = record_symptom(&conn, &entry).unwrap();

        let agg: Option<String> = conn
            .query_row(
                "SELECT aggravating FROM symptoms WHERE id = ?1",
                params![id.to_string()],
                |r| r.get(0),
            )
            .unwrap();
        assert!(agg.is_none());
    }

    // ───────────────────────────────────────
    // fetch_symptoms_filtered tests
    // ───────────────────────────────────────

    #[test]
    fn fetch_all_symptoms() {
        let conn = test_db();
        record_symptom(&conn, &make_entry("Pain", "Headache", 3)).unwrap();
        record_symptom(&conn, &make_entry("Digestive", "Nausea", 2)).unwrap();

        let symptoms = fetch_symptoms_filtered(&conn, &None).unwrap();
        assert_eq!(symptoms.len(), 2);
    }

    #[test]
    fn fetch_symptoms_filter_by_category() {
        let conn = test_db();
        record_symptom(&conn, &make_entry("Pain", "Headache", 3)).unwrap();
        record_symptom(&conn, &make_entry("Digestive", "Nausea", 2)).unwrap();
        record_symptom(&conn, &make_entry("Pain", "Back pain", 4)).unwrap();

        let filter = Some(SymptomFilter {
            category: Some("Pain".into()),
            severity_min: None,
            severity_max: None,
            date_from: None,
            date_to: None,
            still_active: None,
        });
        let symptoms = fetch_symptoms_filtered(&conn, &filter).unwrap();
        assert_eq!(symptoms.len(), 2);
        assert!(symptoms.iter().all(|s| s.category == "Pain"));
    }

    #[test]
    fn fetch_symptoms_filter_by_severity_range() {
        let conn = test_db();
        record_symptom(&conn, &make_entry("Pain", "Headache", 1)).unwrap();
        record_symptom(&conn, &make_entry("Pain", "Back pain", 3)).unwrap();
        record_symptom(&conn, &make_entry("Pain", "Chest pain", 5)).unwrap();

        let filter = Some(SymptomFilter {
            category: None,
            severity_min: Some(2),
            severity_max: Some(4),
            date_from: None,
            date_to: None,
            still_active: None,
        });
        let symptoms = fetch_symptoms_filtered(&conn, &filter).unwrap();
        assert_eq!(symptoms.len(), 1);
        assert_eq!(symptoms[0].severity, 3);
    }

    #[test]
    fn fetch_symptoms_filter_active_only() {
        let conn = test_db();
        let id1 = record_symptom(&conn, &make_entry("Pain", "Headache", 3)).unwrap();
        record_symptom(&conn, &make_entry("Digestive", "Nausea", 2)).unwrap();
        resolve_symptom(&conn, &id1.to_string()).unwrap();

        let filter = Some(SymptomFilter {
            category: None,
            severity_min: None,
            severity_max: None,
            date_from: None,
            date_to: None,
            still_active: Some(true),
        });
        let symptoms = fetch_symptoms_filtered(&conn, &filter).unwrap();
        assert_eq!(symptoms.len(), 1);
        assert_eq!(symptoms[0].specific, "Nausea");
    }

    #[test]
    fn fetch_empty_database() {
        let conn = test_db();
        let symptoms = fetch_symptoms_filtered(&conn, &None).unwrap();
        assert!(symptoms.is_empty());
    }

    // ───────────────────────────────────────
    // resolve_symptom tests
    // ───────────────────────────────────────

    #[test]
    fn resolve_sets_inactive_and_date() {
        let conn = test_db();
        let id = record_symptom(&conn, &make_entry("Pain", "Headache", 3)).unwrap();
        resolve_symptom(&conn, &id.to_string()).unwrap();

        let (active, resolved): (i32, Option<String>) = conn
            .query_row(
                "SELECT still_active, resolved_date FROM symptoms WHERE id = ?1",
                params![id.to_string()],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(active, 0);
        assert!(resolved.is_some());
    }

    #[test]
    fn resolve_nonexistent_returns_not_found() {
        let conn = test_db();
        let result = resolve_symptom(&conn, "nonexistent-id");
        assert!(result.is_err());
    }

    // ───────────────────────────────────────
    // delete_symptom tests
    // ───────────────────────────────────────

    #[test]
    fn delete_removes_from_db() {
        let conn = test_db();
        let id = record_symptom(&conn, &make_entry("Pain", "Headache", 3)).unwrap();
        delete_symptom(&conn, &id.to_string()).unwrap();

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM symptoms", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn delete_nonexistent_returns_not_found() {
        let conn = test_db();
        let result = delete_symptom(&conn, "nonexistent-id");
        assert!(result.is_err());
    }

    // ───────────────────────────────────────
    // temporal correlation tests
    // ───────────────────────────────────────

    #[test]
    fn correlation_detects_recent_medication() {
        let conn = test_db();
        seed_medication(&conn, "Metformin", "2025-01-10");

        let correlations = detect_temporal_correlation(&conn, "2025-01-15").unwrap();
        assert_eq!(correlations.len(), 1);
        assert_eq!(correlations[0].medication_name, "Metformin");
        assert_eq!(correlations[0].days_since_change, 5);
    }

    #[test]
    fn correlation_no_match_outside_window() {
        let conn = test_db();
        seed_medication(&conn, "Metformin", "2024-12-01");

        let correlations = detect_temporal_correlation(&conn, "2025-01-15").unwrap();
        assert!(correlations.is_empty());
    }

    #[test]
    fn correlation_detects_dose_change() {
        let conn = test_db();
        seed_medication(&conn, "Lisinopril", "2024-06-01");
        seed_dose_change(&conn, "Lisinopril", "2025-01-12");

        let correlations = detect_temporal_correlation(&conn, "2025-01-15").unwrap();
        assert_eq!(correlations.len(), 1);
        assert!(correlations[0].message.contains("dose"));
    }

    #[test]
    fn correlation_none_when_empty_db() {
        let conn = test_db();
        let correlations = detect_temporal_correlation(&conn, "2025-01-15").unwrap();
        assert!(correlations.is_empty());
    }

    // ───────────────────────────────────────
    // nudge tests
    // ───────────────────────────────────────

    #[test]
    fn nudge_no_data_returns_no_nudge() {
        let conn = test_db();
        let nudge = check_nudge(&conn).unwrap();
        assert!(!nudge.should_nudge);
    }

    #[test]
    fn nudge_post_medication_when_no_recent_entry() {
        let conn = test_db();
        let today = Local::now().date_naive();
        let recent = (today - chrono::Duration::days(2)).to_string();
        seed_medication(&conn, "Metformin", &recent);

        let nudge = check_nudge(&conn).unwrap();
        assert!(nudge.should_nudge);
        assert_eq!(nudge.nudge_type.as_deref(), Some("PostMedicationChange"));
        assert!(nudge.related_medication.is_some());
    }

    #[test]
    fn nudge_suppressed_when_entry_exists_since_medication() {
        let conn = test_db();
        let today = Local::now().date_naive();
        let recent = (today - chrono::Duration::days(2)).to_string();
        seed_medication(&conn, "Metformin", &recent);

        // Record a symptom after the medication start
        record_symptom(&conn, &make_entry("General", "Fatigue", 2)).unwrap();

        let nudge = check_nudge(&conn).unwrap();
        // Should not nudge for PostMedicationChange since entry exists
        // May or may not nudge for DailyCheckIn depending on timing
        assert!(nudge.nudge_type.as_deref() != Some("PostMedicationChange"));
    }

    // ───────────────────────────────────────
    // categories tests
    // ───────────────────────────────────────

    #[test]
    fn categories_returns_all_eight() {
        let cats = get_symptom_categories();
        assert_eq!(cats.len(), 8);
        assert_eq!(cats[0].name, "Pain");
        assert!(!cats[0].subcategories.is_empty());
    }

    #[test]
    fn subcategories_for_pain_has_other() {
        let subs = subcategories_for("Pain");
        assert!(subs.contains(&"Other"));
        assert!(subs.contains(&"Headache"));
    }

    #[test]
    fn subcategories_for_unknown_is_empty() {
        let subs = subcategories_for("Unknown");
        assert!(subs.is_empty());
    }

    #[test]
    fn body_regions_has_24_entries() {
        assert_eq!(BODY_REGIONS.len(), 24);
    }
}

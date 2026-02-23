//! M0-04: Sync Engine — version-based delta sync between desktop and phone.
//!
//! The sync engine keeps the phone's cache current without mirroring the desktop
//! database. Each entity type has a monotonic version counter. When the phone
//! connects, it sends its known versions; the desktop returns only what changed.
//!
//! Six entity types: medications, labs, timeline, alerts, appointments, profile.
//!
//! Journal entries flow phone → desktop (piggybacked on sync requests).

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::db::DatabaseError;

// ═══════════════════════════════════════════════════════════════════════════
// Sync Version Types
// ═══════════════════════════════════════════════════════════════════════════

/// Version counters for all 6 entity types.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncVersions {
    pub medications: i64,
    pub labs: i64,
    pub timeline: i64,
    pub alerts: i64,
    pub appointments: i64,
    pub profile: i64,
}

/// Sync request from phone.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncRequest {
    pub versions: SyncVersions,
    #[serde(default)]
    pub journal_entries: Vec<MobileJournalEntry>,
}

/// Sync response to phone. Fields are `None` if that entity type hasn't changed.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub medications: Option<Vec<CachedMedication>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labs: Option<Vec<CachedLabResult>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeline: Option<Vec<CachedTimelineEvent>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alerts: Option<Vec<CachedAlert>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub appointment: Option<CachedAppointment>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile: Option<CachedProfile>,
    pub versions: SyncVersions,
    pub synced_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub journal_sync: Option<JournalSyncResult>,
}

// ═══════════════════════════════════════════════════════════════════════════
// Cached Entity Types (curated payloads for phone)
// ═══════════════════════════════════════════════════════════════════════════

/// Curated medication for phone cache.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CachedMedication {
    pub id: String,
    pub generic_name: String,
    pub brand_name: Option<String>,
    pub dose: String,
    pub frequency: String,
    pub route: String,
    pub status: String,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub prescriber_name: Option<String>,
    pub condition: Option<String>,
    pub is_otc: bool,
}

/// Curated lab result for phone cache.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CachedLabResult {
    pub id: String,
    pub test_name: String,
    pub value: Option<f64>,
    pub value_text: Option<String>,
    pub unit: Option<String>,
    pub reference_range_low: Option<f64>,
    pub reference_range_high: Option<f64>,
    pub abnormal_flag: String,
    pub collection_date: String,
    pub is_abnormal: bool,
    /// Trend vs. prior result of the same test: "up", "down", "stable", or null.
    pub trend_direction: Option<String>,
}

/// Curated timeline event for phone cache.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CachedTimelineEvent {
    pub id: String,
    pub event_type: String,
    pub category: String,
    pub description: String,
    pub severity: Option<i32>,
    pub date: String,
    pub still_active: bool,
}

/// Curated alert for phone cache (matches phone `CachedAlert` type).
///
/// Currently populated from `dismissed_alerts` table only (all have `dismissed: true`).
/// Active (non-dismissed) alerts require coherence engine persistence (RS-L2-03-001).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CachedAlert {
    pub id: String,
    pub title: String,
    pub description: String,
    pub severity: String,
    pub created_at: String,
    pub dismissed: bool,
}

/// Curated appointment for phone cache.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CachedAppointment {
    pub id: String,
    pub professional_name: String,
    pub professional_specialty: Option<String>,
    pub date: String,
    pub appointment_type: String,
    pub prep_available: bool,
}

/// Curated profile summary for phone cache.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CachedProfile {
    pub profile_name: String,
    pub total_documents: u32,
    pub extraction_accuracy: f64,
    pub allergies: Vec<CachedAllergy>,
}

/// Allergy summary within profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CachedAllergy {
    pub allergen: String,
    pub severity: String,
    pub verified: bool,
}

// ═══════════════════════════════════════════════════════════════════════════
// Journal Sync Types (phone → desktop)
// ═══════════════════════════════════════════════════════════════════════════

/// Journal entry from phone (piggybacked on sync request).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MobileJournalEntry {
    pub id: String,
    pub severity: i32,
    pub body_location: Option<String>,
    pub free_text: Option<String>,
    pub activity_context: Option<String>,
    pub symptom_chip: Option<String>,
    pub created_at: String,
}

/// Result of processing piggybacked journal entries.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JournalSyncResult {
    pub synced_ids: Vec<String>,
    pub rejected_ids: Vec<String>,
    pub correlations: Vec<JournalCorrelation>,
}

/// Medication-symptom correlation found during journal sync.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JournalCorrelation {
    pub entry_id: String,
    pub medication_name: String,
    pub days_since_change: i64,
    pub message: String,
}

// ═══════════════════════════════════════════════════════════════════════════
// Version Counter Functions
// ═══════════════════════════════════════════════════════════════════════════

/// Get current sync versions from the database.
pub fn get_sync_versions(conn: &Connection) -> Result<SyncVersions, DatabaseError> {
    let mut versions = SyncVersions::default();

    let mut stmt = conn.prepare(
        "SELECT entity_type, version FROM sync_versions",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
    })?;

    for row in rows {
        let (entity_type, version) = row?;
        match entity_type.as_str() {
            "medications" => versions.medications = version,
            "labs" => versions.labs = version,
            "timeline" => versions.timeline = version,
            "alerts" => versions.alerts = version,
            "appointments" => versions.appointments = version,
            "profile" => versions.profile = version,
            _ => {}
        }
    }

    Ok(versions)
}

/// Check which entity types have changed between phone versions and desktop versions.
/// Returns (has_any_changes, changed_types_list).
pub fn diff_versions(phone: &SyncVersions, desktop: &SyncVersions) -> Vec<String> {
    let mut changed = Vec::new();
    if phone.medications < desktop.medications {
        changed.push("medications".to_string());
    }
    if phone.labs < desktop.labs {
        changed.push("labs".to_string());
    }
    if phone.timeline < desktop.timeline {
        changed.push("timeline".to_string());
    }
    if phone.alerts < desktop.alerts {
        changed.push("alerts".to_string());
    }
    if phone.appointments < desktop.appointments {
        changed.push("appointments".to_string());
    }
    if phone.profile < desktop.profile {
        changed.push("profile".to_string());
    }
    changed
}

// ═══════════════════════════════════════════════════════════════════════════
// Payload Assembly Functions
// ═══════════════════════════════════════════════════════════════════════════

/// Assemble curated medication payload (all active + recently discontinued).
pub fn assemble_medications(conn: &Connection) -> Result<Vec<CachedMedication>, DatabaseError> {
    let mut stmt = conn.prepare(
        "SELECT m.id, m.generic_name, m.brand_name, m.dose, m.frequency, m.route,
                m.status, m.start_date, m.end_date, m.condition, m.is_otc,
                p.name AS prescriber_name
         FROM medications m
         LEFT JOIN professionals p ON m.prescriber_id = p.id
         WHERE m.status = 'active'
            OR (m.status = 'stopped' AND m.end_date >= date('now', '-6 months'))
         ORDER BY m.status ASC, m.generic_name ASC",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(CachedMedication {
            id: row.get(0)?,
            generic_name: row.get(1)?,
            brand_name: row.get(2)?,
            dose: row.get(3)?,
            frequency: row.get(4)?,
            route: row.get(5)?,
            status: row.get(6)?,
            start_date: row.get(7)?,
            end_date: row.get(8)?,
            condition: row.get(9)?,
            is_otc: row.get::<_, i32>(10)? != 0,
            prescriber_name: row.get(11)?,
        })
    })?;

    rows.map(|r| r.map_err(DatabaseError::from)).collect()
}

/// Assemble recent lab results with abnormal flag and trend direction.
pub fn assemble_recent_labs(
    conn: &Connection,
    limit: u32,
) -> Result<Vec<CachedLabResult>, DatabaseError> {
    // Subquery computes trend by comparing each result's value to the prior
    // result of the same test type (by collection_date).
    let mut stmt = conn.prepare(
        "SELECT lr.id, lr.test_name, lr.value, lr.value_text, lr.unit,
                lr.reference_range_low, lr.reference_range_high, lr.abnormal_flag,
                lr.collection_date,
                (SELECT prev.value FROM lab_results prev
                 WHERE prev.test_name = lr.test_name
                   AND prev.collection_date < lr.collection_date
                   AND prev.value IS NOT NULL
                 ORDER BY prev.collection_date DESC
                 LIMIT 1) AS prev_value
         FROM lab_results lr
         ORDER BY lr.collection_date DESC
         LIMIT ?1",
    )?;

    let rows = stmt.query_map(params![limit], |row| {
        let abnormal_flag: String = row.get(7)?;
        let is_abnormal = abnormal_flag != "normal";
        let current_value: Option<f64> = row.get(2)?;
        let prev_value: Option<f64> = row.get(9)?;

        let trend_direction = match (current_value, prev_value) {
            (Some(curr), Some(prev)) => {
                let diff = (curr - prev).abs();
                let threshold = prev.abs() * 0.01; // 1% tolerance for "stable"
                if diff <= threshold {
                    Some("stable".to_string())
                } else if curr > prev {
                    Some("up".to_string())
                } else {
                    Some("down".to_string())
                }
            }
            _ => None,
        };

        Ok(CachedLabResult {
            id: row.get(0)?,
            test_name: row.get(1)?,
            value: current_value,
            value_text: row.get(3)?,
            unit: row.get(4)?,
            reference_range_low: row.get(5)?,
            reference_range_high: row.get(6)?,
            abnormal_flag,
            collection_date: row.get(8)?,
            is_abnormal,
            trend_direction,
        })
    })?;

    rows.map(|r| r.map_err(DatabaseError::from)).collect()
}

/// Assemble recent timeline events (symptoms/journal entries).
pub fn assemble_recent_timeline(
    conn: &Connection,
    limit: u32,
) -> Result<Vec<CachedTimelineEvent>, DatabaseError> {
    let mut stmt = conn.prepare(
        "SELECT id, category, specific, severity, onset_date, still_active
         FROM symptoms
         ORDER BY onset_date DESC
         LIMIT ?1",
    )?;

    let rows = stmt.query_map(params![limit], |row| {
        let category: String = row.get(1)?;
        let specific: String = row.get(2)?;
        Ok(CachedTimelineEvent {
            id: row.get(0)?,
            event_type: "journal".to_string(),
            category: category.clone(),
            description: specific,
            severity: row.get(3)?,
            date: row.get(4)?,
            still_active: row.get::<_, i32>(5)? != 0,
        })
    })?;

    rows.map(|r| r.map_err(DatabaseError::from)).collect()
}

/// Assemble alerts for phone cache.
///
/// Returns both active coherence alerts and dismissed alerts for the phone cache.
/// Active alerts come from `coherence_alerts` (WHERE dismissed = 0).
/// Dismissed alerts come from `dismissed_alerts` (legacy table).
/// Active alerts sort first (by severity desc, then detected_at desc).
///
/// IMP-001: Previously returned only dismissed alerts — phone missed active health alerts.
pub fn assemble_alerts(conn: &Connection) -> Result<Vec<CachedAlert>, DatabaseError> {
    let mut alerts = Vec::new();

    // 1. Active alerts from coherence_alerts (IMP-001 fix)
    if table_exists(conn, "coherence_alerts") {
        let mut stmt = conn.prepare(
            "SELECT id, alert_type, severity, patient_message, detected_at
             FROM coherence_alerts
             WHERE dismissed = 0
             ORDER BY
                 CASE severity WHEN 'critical' THEN 0 WHEN 'standard' THEN 1 ELSE 2 END,
                 detected_at DESC",
        )?;

        let active_rows = stmt.query_map([], |row| {
            let id: String = row.get(0)?;
            let alert_type: String = row.get(1)?;
            let severity: String = row.get(2)?;
            let patient_message: String = row.get(3)?;
            let detected_at: String = row.get(4)?;
            Ok(CachedAlert {
                id,
                title: format_alert_title(&alert_type),
                description: patient_message,
                severity,
                created_at: detected_at,
                dismissed: false,
            })
        })?;

        for row in active_rows {
            alerts.push(row.map_err(DatabaseError::from)?);
        }
    }

    // 2. Dismissed alerts from dismissed_alerts (legacy, kept for history)
    let mut stmt = conn.prepare(
        "SELECT id, alert_type, reason, dismissed_date
         FROM dismissed_alerts
         ORDER BY dismissed_date DESC",
    )?;

    let dismissed_rows = stmt.query_map([], |row| {
        let id: String = row.get(0)?;
        let alert_type: String = row.get(1)?;
        let reason: Option<String> = row.get(2)?;
        let dismissed_date: String = row.get(3)?;
        let severity = match alert_type.as_str() {
            "critical" | "emergency" => "critical",
            "conflict" | "contradiction" => "warning",
            _ => "info",
        };
        Ok(CachedAlert {
            id,
            title: format_alert_title(&alert_type),
            description: reason.unwrap_or_default(),
            severity: severity.to_string(),
            created_at: dismissed_date,
            dismissed: true,
        })
    })?;

    for row in dismissed_rows {
        alerts.push(row.map_err(DatabaseError::from)?);
    }

    Ok(alerts)
}

/// Check if a table exists (graceful for DB versions without coherence_alerts).
fn table_exists(conn: &Connection, name: &str) -> bool {
    conn.prepare(&format!(
        "SELECT 1 FROM sqlite_master WHERE type='table' AND name='{name}'"
    ))
    .and_then(|mut s| s.query_row([], |_| Ok(())))
    .is_ok()
}

/// Format alert type string into a human-readable title.
fn format_alert_title(alert_type: &str) -> String {
    match alert_type {
        "conflict" => "Medication Conflict".to_string(),
        "contradiction" => "Data Contradiction".to_string(),
        "critical" => "Critical Alert".to_string(),
        "emergency" => "Emergency Alert".to_string(),
        "duplicate" => "Duplicate Entry".to_string(),
        "gap" => "Coverage Gap".to_string(),
        "trend" => "Trend Alert".to_string(),
        other => other.replace('_', " "),
    }
}

/// Assemble next appointment (within 7 days).
pub fn assemble_next_appointment(
    conn: &Connection,
) -> Result<Option<CachedAppointment>, DatabaseError> {
    let result = conn.query_row(
        "SELECT a.id, p.name, p.specialty, a.date, a.type, a.pre_summary_generated
         FROM appointments a
         JOIN professionals p ON a.professional_id = p.id
         WHERE a.type = 'upcoming'
           AND a.date >= date('now')
           AND a.date <= date('now', '+7 days')
         ORDER BY a.date ASC
         LIMIT 1",
        [],
        |row| {
            Ok(CachedAppointment {
                id: row.get(0)?,
                professional_name: row.get(1)?,
                professional_specialty: row.get(2)?,
                date: row.get(3)?,
                appointment_type: row.get(4)?,
                prep_available: row.get::<_, i32>(5)? != 0,
            })
        },
    );

    match result {
        Ok(appt) => Ok(Some(appt)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Assemble profile summary (name, trust metrics, allergies).
pub fn assemble_profile_summary(
    conn: &Connection,
    profile_name: &str,
) -> Result<CachedProfile, DatabaseError> {
    let (total_documents, extraction_accuracy): (u32, f64) = conn.query_row(
        "SELECT total_documents, extraction_accuracy FROM profile_trust WHERE id = 1",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;

    let mut stmt = conn.prepare(
        "SELECT allergen, severity, verified FROM allergies",
    )?;

    let valid_severities = ["mild", "moderate", "severe", "life_threatening"];
    let mut allergies: Vec<CachedAllergy> = Vec::new();
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i32>(2)?,
        ))
    })?;
    for row_result in rows {
        match row_result {
            Ok((allergen, severity, verified_int)) => {
                if !valid_severities.contains(&severity.as_str()) {
                    tracing::warn!(
                        allergen = %allergen,
                        severity = %severity,
                        "Skipping allergy with invalid severity during sync"
                    );
                    continue;
                }
                allergies.push(CachedAllergy {
                    allergen,
                    severity,
                    verified: verified_int != 0,
                });
            }
            Err(e) => {
                tracing::warn!(error = %e, "Failed to read allergy row during sync");
            }
        }
    }

    Ok(CachedProfile {
        profile_name: profile_name.to_string(),
        total_documents,
        extraction_accuracy,
        allergies,
    })
}

// ═══════════════════════════════════════════════════════════════════════════
// Journal Sync Processing (phone → desktop)
// ═══════════════════════════════════════════════════════════════════════════

/// Validate a journal entry's fields before database insertion (RS-M0-04-D04).
fn validate_journal_entry(entry: &MobileJournalEntry) -> Result<(), &'static str> {
    if !(1..=5).contains(&entry.severity) {
        return Err("severity out of range (must be 1-5)");
    }
    if uuid::Uuid::parse_str(&entry.id).is_err() {
        return Err("invalid UUID format for id");
    }
    // Accept YYYY-MM-DD or YYYY-MM-DDTHH:MM:SS date formats
    if chrono::NaiveDate::parse_from_str(&entry.created_at, "%Y-%m-%d").is_err()
        && chrono::NaiveDateTime::parse_from_str(&entry.created_at, "%Y-%m-%dT%H:%M:%S").is_err()
    {
        return Err("invalid date format for created_at");
    }
    Ok(())
}

/// Process journal entries piggybacked on sync request.
///
/// Validates each entry before insertion (RS-M0-04-D04). Invalid entries are
/// rejected with a warning log rather than silently inserted or dropped.
/// Uses INSERT OR IGNORE for idempotency (duplicate UUIDs are silently skipped).
/// After inserting, checks for medication-symptom temporal correlations.
pub fn process_journal_sync(
    conn: &Connection,
    entries: &[MobileJournalEntry],
) -> Result<JournalSyncResult, DatabaseError> {
    let mut synced_ids = Vec::new();
    let mut rejected_ids = Vec::new();
    let mut correlations = Vec::new();

    for entry in entries {
        if let Err(reason) = validate_journal_entry(entry) {
            tracing::warn!(
                entry_id = %entry.id,
                reason = reason,
                "Rejecting invalid journal entry from phone"
            );
            rejected_ids.push(entry.id.clone());
            continue;
        }

        // INSERT OR IGNORE for idempotency
        let rows_changed = conn.execute(
            "INSERT OR IGNORE INTO symptoms
             (id, category, specific, severity, body_region, onset_date, recorded_date, source, notes)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'patient_reported', ?8)",
            params![
                entry.id,
                entry.symptom_chip.as_deref().unwrap_or("general"),
                entry.free_text.as_deref().unwrap_or(""),
                entry.severity,
                entry.body_location,
                entry.created_at,
                entry.created_at,
                entry.activity_context,
            ],
        )?;

        synced_ids.push(entry.id.clone());

        // Only check correlations for newly inserted entries
        if rows_changed > 0 {
            if let Ok(corrs) = find_medication_correlations(conn, entry) {
                correlations.extend(corrs);
            }
        }
    }

    Ok(JournalSyncResult {
        synced_ids,
        rejected_ids,
        correlations,
    })
}

/// Find temporal correlations between a journal entry and recent medication changes.
///
/// Checks for dose changes within the last 14 days before the symptom onset.
fn find_medication_correlations(
    conn: &Connection,
    entry: &MobileJournalEntry,
) -> Result<Vec<JournalCorrelation>, DatabaseError> {
    let onset_date = entry.created_at.split('T').next().unwrap_or(&entry.created_at);

    let mut stmt = conn.prepare(
        "SELECT m.generic_name, dc.change_date,
                julianday(?1) - julianday(dc.change_date) AS days_diff
         FROM dose_changes dc
         JOIN medications m ON dc.medication_id = m.id
         WHERE dc.change_date >= date(?1, '-14 days')
           AND dc.change_date <= ?1
         ORDER BY dc.change_date DESC",
    )?;

    let rows = stmt.query_map(params![onset_date], |row| {
        let med_name: String = row.get(0)?;
        let days: f64 = row.get(2)?;
        let days_since = days.round() as i64;
        Ok(JournalCorrelation {
            entry_id: entry.id.clone(),
            medication_name: med_name.clone(),
            days_since_change: days_since,
            message: format!(
                "Your {} dose was changed {} day(s) ago. This symptom may be related.",
                med_name, days_since
            ),
        })
    })?;

    rows.map(|r| r.map_err(DatabaseError::from)).collect()
}

// ═══════════════════════════════════════════════════════════════════════════
// Full Sync Orchestration
// ═══════════════════════════════════════════════════════════════════════════

/// Build the complete sync response by comparing versions and assembling changed payloads.
///
/// Returns `None` if nothing changed and no journal entries were submitted.
pub fn build_sync_response(
    conn: &Connection,
    request: &SyncRequest,
    profile_name: &str,
) -> Result<Option<SyncResponse>, DatabaseError> {
    let current = get_sync_versions(conn)?;
    let changed = diff_versions(&request.versions, &current);

    // Process journal entries (always, even if nothing else changed)
    let journal_sync = if !request.journal_entries.is_empty() {
        Some(process_journal_sync(conn, &request.journal_entries)?)
    } else {
        None
    };

    // If nothing changed and no journal entries, return None (caller sends 204)
    if changed.is_empty() && journal_sync.is_none() {
        return Ok(None);
    }

    let mut response = SyncResponse {
        versions: current.clone(),
        synced_at: chrono::Utc::now().to_rfc3339(),
        journal_sync,
        ..Default::default()
    };

    for entity_type in &changed {
        match entity_type.as_str() {
            "medications" => {
                response.medications = Some(assemble_medications(conn)?);
            }
            "labs" => {
                response.labs = Some(assemble_recent_labs(conn, 10)?);
            }
            "timeline" => {
                response.timeline = Some(assemble_recent_timeline(conn, 30)?);
            }
            "alerts" => {
                response.alerts = Some(assemble_alerts(conn)?);
            }
            "appointments" => {
                response.appointment = assemble_next_appointment(conn)?;
            }
            "profile" => {
                response.profile = Some(assemble_profile_summary(conn, profile_name)?);
            }
            _ => {}
        }
    }

    Ok(Some(response))
}

// ═══════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::sqlite::open_memory_database;
    use uuid::Uuid;

    fn test_db() -> Connection {
        open_memory_database().unwrap()
    }

    fn insert_doc(conn: &Connection) -> String {
        let id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO documents (id, type, title, ingestion_date, source_file, verified)
             VALUES (?1, 'prescription', 'Test', datetime('now'), '/tmp/test.pdf', 0)",
            params![id],
        )
        .unwrap();
        id
    }

    fn insert_professional(conn: &Connection) -> String {
        let id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO professionals (id, name, specialty) VALUES (?1, 'Dr. Smith', 'GP')",
            params![id],
        )
        .unwrap();
        id
    }

    // -----------------------------------------------------------------------
    // Version counters
    // -----------------------------------------------------------------------

    #[test]
    fn sync_versions_initialized_to_zero() {
        let conn = test_db();
        let versions = get_sync_versions(&conn).unwrap();
        assert_eq!(versions, SyncVersions::default());
    }

    #[test]
    fn medication_insert_increments_version() {
        let conn = test_db();
        let doc_id = insert_doc(&conn);
        let med_id = Uuid::new_v4().to_string();

        conn.execute(
            "INSERT INTO medications (id, generic_name, dose, frequency, frequency_type, status, document_id)
             VALUES (?1, 'Metformin', '500mg', 'twice daily', 'scheduled', 'active', ?2)",
            params![med_id, doc_id],
        )
        .unwrap();

        let versions = get_sync_versions(&conn).unwrap();
        assert_eq!(versions.medications, 1);
        assert_eq!(versions.labs, 0);
    }

    #[test]
    fn medication_update_increments_version() {
        let conn = test_db();
        let doc_id = insert_doc(&conn);
        let med_id = Uuid::new_v4().to_string();

        conn.execute(
            "INSERT INTO medications (id, generic_name, dose, frequency, frequency_type, status, document_id)
             VALUES (?1, 'Metformin', '500mg', 'daily', 'scheduled', 'active', ?2)",
            params![med_id, doc_id],
        )
        .unwrap();

        conn.execute(
            "UPDATE medications SET dose = '1000mg' WHERE id = ?1",
            params![med_id],
        )
        .unwrap();

        let versions = get_sync_versions(&conn).unwrap();
        assert_eq!(versions.medications, 2); // insert + update
    }

    #[test]
    fn medication_delete_increments_version() {
        let conn = test_db();
        let doc_id = insert_doc(&conn);
        let med_id = Uuid::new_v4().to_string();

        conn.execute(
            "INSERT INTO medications (id, generic_name, dose, frequency, frequency_type, status, document_id)
             VALUES (?1, 'Metformin', '500mg', 'daily', 'scheduled', 'active', ?2)",
            params![med_id, doc_id],
        )
        .unwrap();

        conn.execute("DELETE FROM medications WHERE id = ?1", params![med_id])
            .unwrap();

        let versions = get_sync_versions(&conn).unwrap();
        assert_eq!(versions.medications, 2); // insert + delete
    }

    #[test]
    fn lab_insert_increments_labs_version() {
        let conn = test_db();
        let doc_id = insert_doc(&conn);

        conn.execute(
            "INSERT INTO lab_results (id, test_name, abnormal_flag, collection_date, document_id)
             VALUES (?1, 'HbA1c', 'normal', '2026-01-15', ?2)",
            params![Uuid::new_v4().to_string(), doc_id],
        )
        .unwrap();

        let versions = get_sync_versions(&conn).unwrap();
        assert_eq!(versions.labs, 1);
        assert_eq!(versions.medications, 0);
    }

    #[test]
    fn symptom_insert_increments_timeline_version() {
        let conn = test_db();

        conn.execute(
            "INSERT INTO symptoms (id, category, specific, severity, onset_date, recorded_date, source)
             VALUES (?1, 'pain', 'headache', 3, '2026-01-15', '2026-01-15', 'patient_reported')",
            params![Uuid::new_v4().to_string()],
        )
        .unwrap();

        let versions = get_sync_versions(&conn).unwrap();
        assert_eq!(versions.timeline, 1);
    }

    #[test]
    fn alert_insert_increments_alerts_version() {
        let conn = test_db();

        conn.execute(
            "INSERT INTO dismissed_alerts (id, alert_type, entity_ids, dismissed_date, dismissed_by)
             VALUES (?1, 'conflict', 'med1,med2', datetime('now'), 'patient')",
            params![Uuid::new_v4().to_string()],
        )
        .unwrap();

        let versions = get_sync_versions(&conn).unwrap();
        assert_eq!(versions.alerts, 1);
    }

    #[test]
    fn appointment_insert_increments_appointments_version() {
        let conn = test_db();
        let prof_id = insert_professional(&conn);

        conn.execute(
            "INSERT INTO appointments (id, professional_id, date, type)
             VALUES (?1, ?2, date('now', '+3 days'), 'upcoming')",
            params![Uuid::new_v4().to_string(), prof_id],
        )
        .unwrap();

        let versions = get_sync_versions(&conn).unwrap();
        assert_eq!(versions.appointments, 1);
    }

    #[test]
    fn allergy_insert_increments_profile_version() {
        let conn = test_db();

        conn.execute(
            "INSERT INTO allergies (id, allergen, severity, source, verified)
             VALUES (?1, 'Penicillin', 'severe', 'patient_reported', 1)",
            params![Uuid::new_v4().to_string()],
        )
        .unwrap();

        let versions = get_sync_versions(&conn).unwrap();
        assert_eq!(versions.profile, 1);
    }

    #[test]
    fn profile_trust_update_increments_profile_version() {
        let conn = test_db();

        conn.execute(
            "UPDATE profile_trust SET total_documents = 1, last_updated = datetime('now') WHERE id = 1",
            [],
        )
        .unwrap();

        let versions = get_sync_versions(&conn).unwrap();
        assert_eq!(versions.profile, 1);
    }

    // -----------------------------------------------------------------------
    // diff_versions
    // -----------------------------------------------------------------------

    #[test]
    fn diff_versions_no_changes() {
        let phone = SyncVersions {
            medications: 5,
            labs: 3,
            timeline: 10,
            alerts: 2,
            appointments: 1,
            profile: 4,
        };
        let desktop = phone.clone();
        assert!(diff_versions(&phone, &desktop).is_empty());
    }

    #[test]
    fn diff_versions_single_change() {
        let phone = SyncVersions {
            medications: 5,
            ..Default::default()
        };
        let desktop = SyncVersions {
            medications: 6,
            ..Default::default()
        };
        let changed = diff_versions(&phone, &desktop);
        assert_eq!(changed, vec!["medications"]);
    }

    #[test]
    fn diff_versions_multiple_changes() {
        let phone = SyncVersions::default();
        let desktop = SyncVersions {
            medications: 3,
            labs: 2,
            timeline: 0,
            alerts: 0,
            appointments: 1,
            profile: 0,
        };
        let changed = diff_versions(&phone, &desktop);
        assert_eq!(changed.len(), 3);
        assert!(changed.contains(&"medications".to_string()));
        assert!(changed.contains(&"labs".to_string()));
        assert!(changed.contains(&"appointments".to_string()));
    }

    #[test]
    fn diff_versions_full_sync_all_zero() {
        let phone = SyncVersions::default();
        let desktop = SyncVersions {
            medications: 1,
            labs: 1,
            timeline: 1,
            alerts: 1,
            appointments: 1,
            profile: 1,
        };
        let changed = diff_versions(&phone, &desktop);
        assert_eq!(changed.len(), 6);
    }

    // -----------------------------------------------------------------------
    // Payload assembly
    // -----------------------------------------------------------------------

    #[test]
    fn assemble_medications_empty() {
        let conn = test_db();
        let meds = assemble_medications(&conn).unwrap();
        assert!(meds.is_empty());
    }

    #[test]
    fn assemble_medications_active_only() {
        let conn = test_db();
        let doc_id = insert_doc(&conn);

        conn.execute(
            "INSERT INTO medications (id, generic_name, dose, frequency, frequency_type, status, document_id)
             VALUES (?1, 'Metformin', '500mg', 'twice daily', 'scheduled', 'active', ?2)",
            params![Uuid::new_v4().to_string(), doc_id],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO medications (id, generic_name, dose, frequency, frequency_type, status, document_id)
             VALUES (?1, 'Amoxicillin', '250mg', 'thrice daily', 'scheduled', 'stopped', ?2)",
            params![Uuid::new_v4().to_string(), doc_id],
        )
        .unwrap();

        let meds = assemble_medications(&conn).unwrap();
        // Only active (stopped has no end_date so not within 6 months)
        assert_eq!(meds.len(), 1);
        assert_eq!(meds[0].generic_name, "Metformin");
    }

    #[test]
    fn assemble_medications_with_prescriber() {
        let conn = test_db();
        let doc_id = insert_doc(&conn);
        let prof_id = insert_professional(&conn);

        conn.execute(
            "INSERT INTO medications (id, generic_name, dose, frequency, frequency_type, status, prescriber_id, document_id)
             VALUES (?1, 'Metformin', '500mg', 'daily', 'scheduled', 'active', ?2, ?3)",
            params![Uuid::new_v4().to_string(), prof_id, doc_id],
        )
        .unwrap();

        let meds = assemble_medications(&conn).unwrap();
        assert_eq!(meds[0].prescriber_name.as_deref(), Some("Dr. Smith"));
    }

    #[test]
    fn assemble_recent_labs_ordered_by_date() {
        let conn = test_db();
        let doc_id = insert_doc(&conn);

        conn.execute(
            "INSERT INTO lab_results (id, test_name, abnormal_flag, collection_date, document_id)
             VALUES (?1, 'HbA1c', 'normal', '2026-01-10', ?2)",
            params![Uuid::new_v4().to_string(), doc_id],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO lab_results (id, test_name, value, unit, reference_range_low, reference_range_high, abnormal_flag, collection_date, document_id)
             VALUES (?1, 'Potassium', 6.5, 'mmol/L', 3.5, 5.0, 'critical_high', '2026-01-15', ?2)",
            params![Uuid::new_v4().to_string(), doc_id],
        )
        .unwrap();

        let labs = assemble_recent_labs(&conn, 10).unwrap();
        assert_eq!(labs.len(), 2);
        // Most recent first
        assert_eq!(labs[0].test_name, "Potassium");
        assert!(labs[0].is_abnormal);
        assert_eq!(labs[1].test_name, "HbA1c");
        assert!(!labs[1].is_abnormal);
    }

    #[test]
    fn assemble_recent_labs_respects_limit() {
        let conn = test_db();
        let doc_id = insert_doc(&conn);

        for i in 0..5 {
            conn.execute(
                "INSERT INTO lab_results (id, test_name, abnormal_flag, collection_date, document_id)
                 VALUES (?1, ?2, 'normal', ?3, ?4)",
                params![
                    Uuid::new_v4().to_string(),
                    format!("Test{i}"),
                    format!("2026-01-{:02}", i + 1),
                    doc_id
                ],
            )
            .unwrap();
        }

        let labs = assemble_recent_labs(&conn, 3).unwrap();
        assert_eq!(labs.len(), 3);
    }

    #[test]
    fn assemble_recent_labs_computes_trend_direction() {
        let conn = test_db();
        let doc_id = insert_doc(&conn);

        // Insert two HbA1c results: 7.0 then 6.5 (trending down)
        conn.execute(
            "INSERT INTO lab_results (id, test_name, value, unit, abnormal_flag, collection_date, document_id)
             VALUES (?1, 'HbA1c', 7.0, '%', 'high', '2026-01-01', ?2)",
            params![Uuid::new_v4().to_string(), doc_id],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO lab_results (id, test_name, value, unit, abnormal_flag, collection_date, document_id)
             VALUES (?1, 'HbA1c', 6.5, '%', 'high', '2026-01-15', ?2)",
            params![Uuid::new_v4().to_string(), doc_id],
        )
        .unwrap();

        // Insert one Potassium result (no prior — no trend)
        conn.execute(
            "INSERT INTO lab_results (id, test_name, value, unit, abnormal_flag, collection_date, document_id)
             VALUES (?1, 'Potassium', 4.5, 'mmol/L', 'normal', '2026-01-10', ?2)",
            params![Uuid::new_v4().to_string(), doc_id],
        )
        .unwrap();

        let labs = assemble_recent_labs(&conn, 10).unwrap();
        // Most recent first: HbA1c(Jan15) → Potassium(Jan10) → HbA1c(Jan01)
        assert_eq!(labs[0].test_name, "HbA1c");
        assert_eq!(labs[0].trend_direction.as_deref(), Some("down"));

        assert_eq!(labs[1].test_name, "Potassium");
        assert!(labs[1].trend_direction.is_none()); // No prior result

        assert_eq!(labs[2].test_name, "HbA1c");
        assert!(labs[2].trend_direction.is_none()); // First result, no prior
    }

    #[test]
    fn assemble_recent_timeline_from_symptoms() {
        let conn = test_db();

        conn.execute(
            "INSERT INTO symptoms (id, category, specific, severity, onset_date, recorded_date, source)
             VALUES (?1, 'pain', 'headache', 3, '2026-01-15', '2026-01-15', 'patient_reported')",
            params![Uuid::new_v4().to_string()],
        )
        .unwrap();

        let events = assemble_recent_timeline(&conn, 30).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, "journal");
        assert_eq!(events[0].category, "pain");
        assert_eq!(events[0].description, "headache");
    }

    #[test]
    fn assemble_alerts_maps_dismissed_to_phone_format() {
        let conn = test_db();

        conn.execute(
            "INSERT INTO dismissed_alerts (id, alert_type, entity_ids, dismissed_date, reason, dismissed_by)
             VALUES (?1, 'conflict', 'med1,med2', datetime('now'), 'Discussed with doctor', 'patient')",
            params![Uuid::new_v4().to_string()],
        )
        .unwrap();

        let alerts = assemble_alerts(&conn).unwrap();
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].title, "Medication Conflict");
        assert_eq!(alerts[0].description, "Discussed with doctor");
        assert_eq!(alerts[0].severity, "warning");
        assert!(alerts[0].dismissed);
    }

    // -----------------------------------------------------------------------
    // IMP-001: Active alerts from coherence_alerts
    // -----------------------------------------------------------------------

    #[test]
    fn assemble_alerts_returns_active_from_coherence_alerts() {
        let conn = test_db();
        let alert_id = Uuid::new_v4().to_string();

        conn.execute(
            "INSERT INTO coherence_alerts (id, alert_type, severity, entity_ids, source_document_ids,
             patient_message, detail_json, detected_at, dismissed)
             VALUES (?1, 'conflict', 'critical', '[]', '[]', 'Metformin conflicts with Glyburide', '{}', datetime('now'), 0)",
            params![alert_id],
        )
        .unwrap();

        let alerts = assemble_alerts(&conn).unwrap();
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].title, "Medication Conflict");
        assert_eq!(alerts[0].description, "Metformin conflicts with Glyburide");
        assert_eq!(alerts[0].severity, "critical");
        assert!(!alerts[0].dismissed);
    }

    #[test]
    fn assemble_alerts_excludes_dismissed_coherence_alerts() {
        let conn = test_db();

        // Active alert
        conn.execute(
            "INSERT INTO coherence_alerts (id, alert_type, severity, entity_ids, source_document_ids,
             patient_message, detail_json, detected_at, dismissed)
             VALUES (?1, 'gap', 'standard', '[]', '[]', 'Missing lab result', '{}', datetime('now'), 0)",
            params![Uuid::new_v4().to_string()],
        )
        .unwrap();

        // Dismissed alert (should NOT appear as active)
        conn.execute(
            "INSERT INTO coherence_alerts (id, alert_type, severity, entity_ids, source_document_ids,
             patient_message, detail_json, detected_at, dismissed)
             VALUES (?1, 'duplicate', 'info', '[]', '[]', 'Duplicate entry', '{}', datetime('now'), 1)",
            params![Uuid::new_v4().to_string()],
        )
        .unwrap();

        let alerts = assemble_alerts(&conn).unwrap();
        // Only the active alert (dismissed coherence_alerts excluded, no dismissed_alerts entries)
        assert_eq!(alerts.len(), 1);
        assert!(!alerts[0].dismissed);
        assert_eq!(alerts[0].description, "Missing lab result");
    }

    #[test]
    fn assemble_alerts_mixed_active_and_dismissed() {
        let conn = test_db();

        // Active alert in coherence_alerts
        conn.execute(
            "INSERT INTO coherence_alerts (id, alert_type, severity, entity_ids, source_document_ids,
             patient_message, detail_json, detected_at, dismissed)
             VALUES (?1, 'critical', 'critical', '[]', '[]', 'Dangerous interaction', '{}', datetime('now'), 0)",
            params![Uuid::new_v4().to_string()],
        )
        .unwrap();

        // Dismissed alert in dismissed_alerts
        conn.execute(
            "INSERT INTO dismissed_alerts (id, alert_type, entity_ids, dismissed_date, reason, dismissed_by)
             VALUES (?1, 'conflict', 'med1', datetime('now'), 'Resolved', 'patient')",
            params![Uuid::new_v4().to_string()],
        )
        .unwrap();

        let alerts = assemble_alerts(&conn).unwrap();
        assert_eq!(alerts.len(), 2);

        // Active comes first (from coherence_alerts)
        assert!(!alerts[0].dismissed);
        assert_eq!(alerts[0].description, "Dangerous interaction");

        // Dismissed second (from dismissed_alerts)
        assert!(alerts[1].dismissed);
        assert_eq!(alerts[1].description, "Resolved");
    }

    #[test]
    fn assemble_alerts_severity_ordering_critical_first() {
        let conn = test_db();

        // Insert standard severity first (chronologically earlier)
        conn.execute(
            "INSERT INTO coherence_alerts (id, alert_type, severity, entity_ids, source_document_ids,
             patient_message, detail_json, detected_at, dismissed)
             VALUES (?1, 'gap', 'standard', '[]', '[]', 'Standard alert', '{}', datetime('now', '-1 hour'), 0)",
            params![Uuid::new_v4().to_string()],
        )
        .unwrap();

        // Insert critical severity second
        conn.execute(
            "INSERT INTO coherence_alerts (id, alert_type, severity, entity_ids, source_document_ids,
             patient_message, detail_json, detected_at, dismissed)
             VALUES (?1, 'critical', 'critical', '[]', '[]', 'Critical alert', '{}', datetime('now'), 0)",
            params![Uuid::new_v4().to_string()],
        )
        .unwrap();

        // Insert info severity
        conn.execute(
            "INSERT INTO coherence_alerts (id, alert_type, severity, entity_ids, source_document_ids,
             patient_message, detail_json, detected_at, dismissed)
             VALUES (?1, 'duplicate', 'info', '[]', '[]', 'Info alert', '{}', datetime('now'), 0)",
            params![Uuid::new_v4().to_string()],
        )
        .unwrap();

        let alerts = assemble_alerts(&conn).unwrap();
        assert_eq!(alerts.len(), 3);
        // Critical sorts first regardless of insert order
        assert_eq!(alerts[0].severity, "critical");
        assert_eq!(alerts[1].severity, "standard");
        assert_eq!(alerts[2].severity, "info");
    }

    #[test]
    fn assemble_alerts_empty_when_no_alerts_exist() {
        let conn = test_db();
        let alerts = assemble_alerts(&conn).unwrap();
        assert!(alerts.is_empty());
    }

    // -----------------------------------------------------------------------
    // Appointments
    // -----------------------------------------------------------------------

    #[test]
    fn assemble_next_appointment_within_7_days() {
        let conn = test_db();
        let prof_id = insert_professional(&conn);

        conn.execute(
            "INSERT INTO appointments (id, professional_id, date, type)
             VALUES (?1, ?2, date('now', '+3 days'), 'upcoming')",
            params![Uuid::new_v4().to_string(), prof_id],
        )
        .unwrap();

        let appt = assemble_next_appointment(&conn).unwrap();
        assert!(appt.is_some());
        let appt = appt.unwrap();
        assert_eq!(appt.professional_name, "Dr. Smith");
    }

    #[test]
    fn assemble_next_appointment_none_if_far() {
        let conn = test_db();
        let prof_id = insert_professional(&conn);

        conn.execute(
            "INSERT INTO appointments (id, professional_id, date, type)
             VALUES (?1, ?2, date('now', '+30 days'), 'upcoming')",
            params![Uuid::new_v4().to_string(), prof_id],
        )
        .unwrap();

        let appt = assemble_next_appointment(&conn).unwrap();
        assert!(appt.is_none());
    }

    #[test]
    fn assemble_profile_summary_with_allergies() {
        let conn = test_db();

        conn.execute(
            "INSERT INTO allergies (id, allergen, severity, source, verified)
             VALUES (?1, 'Penicillin', 'severe', 'patient_reported', 1)",
            params![Uuid::new_v4().to_string()],
        )
        .unwrap();

        let profile = assemble_profile_summary(&conn, "Léa").unwrap();
        assert_eq!(profile.profile_name, "Léa");
        assert_eq!(profile.allergies.len(), 1);
        assert_eq!(profile.allergies[0].allergen, "Penicillin");
        assert!(profile.allergies[0].verified);
    }

    // -----------------------------------------------------------------------
    // Journal sync
    // -----------------------------------------------------------------------

    #[test]
    fn journal_sync_inserts_entries() {
        let conn = test_db();
        let entries = vec![MobileJournalEntry {
            id: Uuid::new_v4().to_string(),
            severity: 5,
            body_location: Some("head".to_string()),
            free_text: Some("Dizzy after walking".to_string()),
            activity_context: Some("Walking back from class".to_string()),
            symptom_chip: Some("dizzy".to_string()),
            created_at: "2026-01-15".to_string(),
        }];

        let result = process_journal_sync(&conn, &entries).unwrap();
        assert_eq!(result.synced_ids.len(), 1);

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM symptoms", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn journal_sync_idempotent() {
        let conn = test_db();
        let id = Uuid::new_v4().to_string();
        let entries = vec![MobileJournalEntry {
            id: id.clone(),
            severity: 3,
            body_location: None,
            free_text: Some("Headache".to_string()),
            activity_context: None,
            symptom_chip: Some("pain".to_string()),
            created_at: "2026-01-15".to_string(),
        }];

        // First sync
        let result1 = process_journal_sync(&conn, &entries).unwrap();
        assert_eq!(result1.synced_ids.len(), 1);

        // Second sync (same ID) — should not duplicate
        let result2 = process_journal_sync(&conn, &entries).unwrap();
        assert_eq!(result2.synced_ids.len(), 1);

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM symptoms", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn journal_sync_multiple_entries() {
        let conn = test_db();
        let entries = vec![
            MobileJournalEntry {
                id: Uuid::new_v4().to_string(),
                severity: 3,
                body_location: Some("head".to_string()),
                free_text: Some("Headache".to_string()),
                activity_context: None,
                symptom_chip: Some("pain".to_string()),
                created_at: "2026-01-15".to_string(),
            },
            MobileJournalEntry {
                id: Uuid::new_v4().to_string(),
                severity: 5,
                body_location: Some("chest".to_string()),
                free_text: Some("Chest tightness".to_string()),
                activity_context: Some("After exercise".to_string()),
                symptom_chip: Some("discomfort".to_string()),
                created_at: "2026-01-16".to_string(),
            },
        ];

        let result = process_journal_sync(&conn, &entries).unwrap();
        assert_eq!(result.synced_ids.len(), 2);

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM symptoms", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn journal_sync_with_correlation() {
        let conn = test_db();
        let doc_id = insert_doc(&conn);
        let med_id = Uuid::new_v4().to_string();

        // Insert a medication
        conn.execute(
            "INSERT INTO medications (id, generic_name, dose, frequency, frequency_type, status, document_id)
             VALUES (?1, 'Metformin', '500mg', 'daily', 'scheduled', 'active', ?2)",
            params![med_id, doc_id],
        )
        .unwrap();

        // Insert a dose change (today)
        conn.execute(
            "INSERT INTO dose_changes (id, medication_id, new_dose, change_date)
             VALUES (?1, ?2, '1000mg', date('now'))",
            params![Uuid::new_v4().to_string(), med_id],
        )
        .unwrap();

        // Sync a journal entry for today
        let entry_id = Uuid::new_v4().to_string();
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        let entries = vec![MobileJournalEntry {
            id: entry_id.clone(),
            severity: 4,
            body_location: Some("stomach".to_string()),
            free_text: Some("Nausea".to_string()),
            activity_context: None,
            symptom_chip: Some("nausea".to_string()),
            created_at: today,
        }];

        let result = process_journal_sync(&conn, &entries).unwrap();
        assert_eq!(result.synced_ids.len(), 1);
        assert!(!result.correlations.is_empty());
        assert_eq!(result.correlations[0].medication_name, "Metformin");
    }

    // -----------------------------------------------------------------------
    // Journal validation (RS-M0-04-D04)
    // -----------------------------------------------------------------------

    #[test]
    fn journal_rejects_severity_out_of_range() {
        let conn = test_db();
        let entries = vec![MobileJournalEntry {
            id: Uuid::new_v4().to_string(),
            severity: 0,
            body_location: None,
            free_text: Some("Bad severity".to_string()),
            activity_context: None,
            symptom_chip: Some("pain".to_string()),
            created_at: "2026-01-15".to_string(),
        }];

        let result = process_journal_sync(&conn, &entries).unwrap();
        assert!(result.synced_ids.is_empty());
        assert_eq!(result.rejected_ids.len(), 1);

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM symptoms", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn journal_rejects_severity_above_five() {
        let conn = test_db();
        let entries = vec![MobileJournalEntry {
            id: Uuid::new_v4().to_string(),
            severity: 6,
            body_location: None,
            free_text: Some("Too high".to_string()),
            activity_context: None,
            symptom_chip: Some("pain".to_string()),
            created_at: "2026-01-15".to_string(),
        }];

        let result = process_journal_sync(&conn, &entries).unwrap();
        assert!(result.synced_ids.is_empty());
        assert_eq!(result.rejected_ids.len(), 1);
    }

    #[test]
    fn journal_rejects_invalid_uuid() {
        let conn = test_db();
        let entries = vec![MobileJournalEntry {
            id: "not-a-uuid".to_string(),
            severity: 3,
            body_location: None,
            free_text: Some("Bad id".to_string()),
            activity_context: None,
            symptom_chip: None,
            created_at: "2026-01-15".to_string(),
        }];

        let result = process_journal_sync(&conn, &entries).unwrap();
        assert!(result.synced_ids.is_empty());
        assert_eq!(result.rejected_ids.len(), 1);
    }

    #[test]
    fn journal_rejects_invalid_date() {
        let conn = test_db();
        let entries = vec![MobileJournalEntry {
            id: Uuid::new_v4().to_string(),
            severity: 3,
            body_location: None,
            free_text: Some("Bad date".to_string()),
            activity_context: None,
            symptom_chip: None,
            created_at: "not-a-date".to_string(),
        }];

        let result = process_journal_sync(&conn, &entries).unwrap();
        assert!(result.synced_ids.is_empty());
        assert_eq!(result.rejected_ids.len(), 1);
    }

    #[test]
    fn journal_mixed_valid_and_invalid() {
        let conn = test_db();
        let entries = vec![
            MobileJournalEntry {
                id: Uuid::new_v4().to_string(),
                severity: 3,
                body_location: None,
                free_text: Some("Valid".to_string()),
                activity_context: None,
                symptom_chip: Some("pain".to_string()),
                created_at: "2026-01-15".to_string(),
            },
            MobileJournalEntry {
                id: Uuid::new_v4().to_string(),
                severity: 99,
                body_location: None,
                free_text: Some("Invalid severity".to_string()),
                activity_context: None,
                symptom_chip: None,
                created_at: "2026-01-15".to_string(),
            },
        ];

        let result = process_journal_sync(&conn, &entries).unwrap();
        assert_eq!(result.synced_ids.len(), 1);
        assert_eq!(result.rejected_ids.len(), 1);

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM symptoms", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn journal_accepts_datetime_format() {
        let conn = test_db();
        let entries = vec![MobileJournalEntry {
            id: Uuid::new_v4().to_string(),
            severity: 2,
            body_location: None,
            free_text: Some("Datetime format".to_string()),
            activity_context: None,
            symptom_chip: Some("general".to_string()),
            created_at: "2026-01-15T14:30:00".to_string(),
        }];

        let result = process_journal_sync(&conn, &entries).unwrap();
        assert_eq!(result.synced_ids.len(), 1);
        assert!(result.rejected_ids.is_empty());
    }

    // -----------------------------------------------------------------------
    // Full sync orchestration
    // -----------------------------------------------------------------------

    #[test]
    fn build_sync_no_changes_returns_none() {
        let conn = test_db();
        let request = SyncRequest {
            versions: SyncVersions::default(),
            journal_entries: vec![],
        };

        let response = build_sync_response(&conn, &request, "Léa").unwrap();
        assert!(response.is_none());
    }

    #[test]
    fn build_sync_with_medication_change() {
        let conn = test_db();
        let doc_id = insert_doc(&conn);

        conn.execute(
            "INSERT INTO medications (id, generic_name, dose, frequency, frequency_type, status, document_id)
             VALUES (?1, 'Metformin', '500mg', 'daily', 'scheduled', 'active', ?2)",
            params![Uuid::new_v4().to_string(), doc_id],
        )
        .unwrap();

        let request = SyncRequest {
            versions: SyncVersions::default(), // phone has version 0, desktop has 1
            journal_entries: vec![],
        };

        let response = build_sync_response(&conn, &request, "Léa").unwrap();
        assert!(response.is_some());
        let resp = response.unwrap();
        assert!(resp.medications.is_some());
        assert_eq!(resp.medications.unwrap().len(), 1);
        assert!(resp.labs.is_none());
        assert!(resp.timeline.is_none());
        assert_eq!(resp.versions.medications, 1);
    }

    #[test]
    fn build_sync_multiple_types_changed() {
        let conn = test_db();
        let doc_id = insert_doc(&conn);

        // Insert medication
        conn.execute(
            "INSERT INTO medications (id, generic_name, dose, frequency, frequency_type, status, document_id)
             VALUES (?1, 'Metformin', '500mg', 'daily', 'scheduled', 'active', ?2)",
            params![Uuid::new_v4().to_string(), doc_id],
        )
        .unwrap();

        // Insert lab
        conn.execute(
            "INSERT INTO lab_results (id, test_name, abnormal_flag, collection_date, document_id)
             VALUES (?1, 'HbA1c', 'normal', '2026-01-15', ?2)",
            params![Uuid::new_v4().to_string(), doc_id],
        )
        .unwrap();

        let request = SyncRequest {
            versions: SyncVersions::default(),
            journal_entries: vec![],
        };

        let response = build_sync_response(&conn, &request, "Léa").unwrap();
        assert!(response.is_some());
        let resp = response.unwrap();
        assert!(resp.medications.is_some());
        assert!(resp.labs.is_some());
        assert!(resp.timeline.is_none());
    }

    #[test]
    fn build_sync_phone_up_to_date() {
        let conn = test_db();
        let doc_id = insert_doc(&conn);

        conn.execute(
            "INSERT INTO medications (id, generic_name, dose, frequency, frequency_type, status, document_id)
             VALUES (?1, 'Metformin', '500mg', 'daily', 'scheduled', 'active', ?2)",
            params![Uuid::new_v4().to_string(), doc_id],
        )
        .unwrap();

        // Phone already has version 1
        let request = SyncRequest {
            versions: SyncVersions {
                medications: 1,
                ..Default::default()
            },
            journal_entries: vec![],
        };

        let response = build_sync_response(&conn, &request, "Léa").unwrap();
        assert!(response.is_none());
    }

    #[test]
    fn build_sync_journal_only() {
        let conn = test_db();

        let request = SyncRequest {
            versions: SyncVersions::default(),
            journal_entries: vec![MobileJournalEntry {
                id: Uuid::new_v4().to_string(),
                severity: 3,
                body_location: None,
                free_text: Some("Headache".to_string()),
                activity_context: None,
                symptom_chip: Some("pain".to_string()),
                created_at: "2026-01-15".to_string(),
            }],
        };

        let response = build_sync_response(&conn, &request, "Léa").unwrap();
        assert!(response.is_some());
        let resp = response.unwrap();
        assert!(resp.journal_sync.is_some());
        assert_eq!(resp.journal_sync.unwrap().synced_ids.len(), 1);
        // Medications should also be returned because journal insert bumped timeline version
        // and phone has version 0
    }

    #[test]
    fn build_sync_full_resync_all_zeros() {
        let conn = test_db();
        let doc_id = insert_doc(&conn);
        let prof_id = insert_professional(&conn);

        // Insert data in multiple entity types
        conn.execute(
            "INSERT INTO medications (id, generic_name, dose, frequency, frequency_type, status, document_id)
             VALUES (?1, 'Metformin', '500mg', 'daily', 'scheduled', 'active', ?2)",
            params![Uuid::new_v4().to_string(), doc_id],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO lab_results (id, test_name, abnormal_flag, collection_date, document_id)
             VALUES (?1, 'HbA1c', 'normal', '2026-01-15', ?2)",
            params![Uuid::new_v4().to_string(), doc_id],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO allergies (id, allergen, severity, source, verified)
             VALUES (?1, 'Penicillin', 'severe', 'patient_reported', 1)",
            params![Uuid::new_v4().to_string()],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO appointments (id, professional_id, date, type)
             VALUES (?1, ?2, date('now', '+3 days'), 'upcoming')",
            params![Uuid::new_v4().to_string(), prof_id],
        )
        .unwrap();

        // Full resync (all versions = 0)
        let request = SyncRequest {
            versions: SyncVersions::default(),
            journal_entries: vec![],
        };

        let response = build_sync_response(&conn, &request, "Léa").unwrap();
        assert!(response.is_some());
        let resp = response.unwrap();
        assert!(resp.medications.is_some());
        assert!(resp.labs.is_some());
        assert!(resp.profile.is_some());
        assert!(resp.appointment.is_some());

        let profile = resp.profile.unwrap();
        assert_eq!(profile.profile_name, "Léa");
        assert_eq!(profile.allergies.len(), 1);
    }

    // -----------------------------------------------------------------------
    // Serialization
    // -----------------------------------------------------------------------

    #[test]
    fn sync_request_deserializes() {
        let json = r#"{
            "versions": {
                "medications": 5,
                "labs": 3,
                "timeline": 10,
                "alerts": 2,
                "appointments": 1,
                "profile": 4
            }
        }"#;

        let req: SyncRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.versions.medications, 5);
        assert!(req.journal_entries.is_empty());
    }

    #[test]
    fn sync_request_deserializes_camel_case() {
        // Phone sends camelCase — verify it deserializes correctly (CA-03)
        let json = r#"{
            "versions": {
                "medications": 7,
                "labs": 1,
                "timeline": 0,
                "alerts": 0,
                "appointments": 0,
                "profile": 2
            },
            "journalEntries": []
        }"#;

        let req: SyncRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.versions.medications, 7);
        assert!(req.journal_entries.is_empty());
    }

    #[test]
    fn sync_request_with_journal_deserializes() {
        // Phone sends camelCase (CA-03)
        let json = r#"{
            "versions": { "medications": 0, "labs": 0, "timeline": 0, "alerts": 0, "appointments": 0, "profile": 0 },
            "journalEntries": [
                {
                    "id": "abc-123",
                    "severity": 6,
                    "bodyLocation": "head",
                    "freeText": "Dizzy",
                    "activityContext": null,
                    "symptomChip": "dizzy",
                    "createdAt": "2026-02-12T14:15:00+01:00"
                }
            ]
        }"#;

        let req: SyncRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.journal_entries.len(), 1);
        assert_eq!(req.journal_entries[0].severity, 6);
    }

    #[test]
    fn sync_response_skips_none_fields() {
        let resp = SyncResponse {
            versions: SyncVersions {
                medications: 5,
                ..Default::default()
            },
            synced_at: "2026-02-12T00:00:00Z".to_string(),
            medications: Some(vec![]),
            ..Default::default()
        };

        let json_value: serde_json::Value = serde_json::to_value(&resp).unwrap();
        let obj = json_value.as_object().unwrap();
        // medications is present as top-level key (Some(vec![]))
        assert!(obj.contains_key("medications"));
        // labs, timeline, alerts, appointment, profile, journalSync are omitted (None)
        assert!(!obj.contains_key("labs"));
        assert!(!obj.contains_key("timeline"));
        assert!(!obj.contains_key("alerts"));
        assert!(!obj.contains_key("appointment"));
        assert!(!obj.contains_key("profile"));
        assert!(!obj.contains_key("journalSync")); // camelCase (CA-03)
        // versions always present; synced_at → syncedAt
        assert!(obj.contains_key("versions"));
        assert!(obj.contains_key("syncedAt")); // camelCase (CA-03)
    }

    // -----------------------------------------------------------------------
    // Edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn dose_change_bumps_medications_version() {
        let conn = test_db();
        let doc_id = insert_doc(&conn);
        let med_id = Uuid::new_v4().to_string();

        conn.execute(
            "INSERT INTO medications (id, generic_name, dose, frequency, frequency_type, status, document_id)
             VALUES (?1, 'Metformin', '500mg', 'daily', 'scheduled', 'active', ?2)",
            params![med_id, doc_id],
        )
        .unwrap();

        let v1 = get_sync_versions(&conn).unwrap();

        conn.execute(
            "INSERT INTO dose_changes (id, medication_id, new_dose, change_date)
             VALUES (?1, ?2, '1000mg', date('now'))",
            params![Uuid::new_v4().to_string(), med_id],
        )
        .unwrap();

        let v2 = get_sync_versions(&conn).unwrap();
        assert!(v2.medications > v1.medications);
    }

    #[test]
    fn empty_journal_entries_not_included_in_response() {
        let conn = test_db();
        let doc_id = insert_doc(&conn);

        conn.execute(
            "INSERT INTO medications (id, generic_name, dose, frequency, frequency_type, status, document_id)
             VALUES (?1, 'Metformin', '500mg', 'daily', 'scheduled', 'active', ?2)",
            params![Uuid::new_v4().to_string(), doc_id],
        )
        .unwrap();

        let request = SyncRequest {
            versions: SyncVersions::default(),
            journal_entries: vec![],
        };

        let response = build_sync_response(&conn, &request, "Léa").unwrap();
        assert!(response.is_some());
        let resp = response.unwrap();
        assert!(resp.journal_sync.is_none());
    }

    // -----------------------------------------------------------------------
    // IMP-019: Reliability under real data
    // -----------------------------------------------------------------------

    #[test]
    fn journal_sync_large_batch_50_entries() {
        let conn = test_db();
        let entries: Vec<MobileJournalEntry> = (0..50)
            .map(|i| MobileJournalEntry {
                id: Uuid::new_v4().to_string(),
                severity: (i % 5) + 1,
                body_location: Some(format!("location_{i}")),
                free_text: Some(format!("Symptom description for entry {i}")),
                activity_context: if i % 3 == 0 {
                    Some(format!("Activity context {i}"))
                } else {
                    None
                },
                symptom_chip: Some(format!("chip_{}", i % 10)),
                created_at: format!("2026-01-{:02}", (i % 28) + 1),
            })
            .collect();

        let result = process_journal_sync(&conn, &entries).unwrap();
        assert_eq!(result.synced_ids.len(), 50);
        assert!(result.rejected_ids.is_empty());

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM symptoms", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 50);
    }

    #[test]
    fn journal_sync_special_characters_in_text() {
        let conn = test_db();
        let entries = vec![
            MobileJournalEntry {
                id: Uuid::new_v4().to_string(),
                severity: 3,
                body_location: Some("côté gauche".to_string()),
                free_text: Some("J'ai mal à l'estomac — «douleur» 'intense'".to_string()),
                activity_context: Some("Après le déjeuner; 100% sûr".to_string()),
                symptom_chip: Some("douleur".to_string()),
                created_at: "2026-01-15".to_string(),
            },
            MobileJournalEntry {
                id: Uuid::new_v4().to_string(),
                severity: 2,
                body_location: Some("头".to_string()),
                free_text: Some("头痛很厉害".to_string()),
                activity_context: None,
                symptom_chip: Some("headache".to_string()),
                created_at: "2026-01-16".to_string(),
            },
        ];

        let result = process_journal_sync(&conn, &entries).unwrap();
        assert_eq!(result.synced_ids.len(), 2);
        assert!(result.rejected_ids.is_empty());

        // Verify data stored correctly
        let text: String = conn
            .query_row(
                "SELECT specific FROM symptoms WHERE id = ?1",
                params![entries[0].id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(text, "J'ai mal à l'estomac — «douleur» 'intense'");
    }

    #[test]
    fn journal_sync_empty_optional_fields() {
        let conn = test_db();
        let entries = vec![MobileJournalEntry {
            id: Uuid::new_v4().to_string(),
            severity: 1,
            body_location: None,
            free_text: None,
            activity_context: None,
            symptom_chip: None,
            created_at: "2026-01-15".to_string(),
        }];

        let result = process_journal_sync(&conn, &entries).unwrap();
        assert_eq!(result.synced_ids.len(), 1);
    }

    #[test]
    fn sync_versions_high_numbers() {
        let phone = SyncVersions {
            medications: 999_999,
            labs: 500_000,
            timeline: 1_000_000,
            alerts: 0,
            appointments: 42,
            profile: 100,
        };
        let desktop = SyncVersions {
            medications: 1_000_000,
            labs: 500_000,
            timeline: 1_000_000,
            alerts: 1,
            appointments: 42,
            profile: 100,
        };
        let changed = diff_versions(&phone, &desktop);
        assert_eq!(changed.len(), 2);
        assert!(changed.contains(&"medications".to_string()));
        assert!(changed.contains(&"alerts".to_string()));
    }

    #[test]
    fn full_sync_all_entity_types_serializes_under_1mb() {
        let conn = test_db();
        let doc_id = insert_doc(&conn);
        let prof_id = insert_professional(&conn);

        // Insert realistic volume of data
        for i in 0..20 {
            conn.execute(
                "INSERT INTO medications (id, generic_name, dose, frequency, frequency_type, status, document_id, prescriber_id)
                 VALUES (?1, ?2, ?3, 'daily', 'scheduled', 'active', ?4, ?5)",
                params![
                    Uuid::new_v4().to_string(),
                    format!("Medication_{i}"),
                    format!("{} mg", (i + 1) * 100),
                    doc_id,
                    prof_id,
                ],
            )
            .unwrap();
        }
        for i in 0..30 {
            conn.execute(
                "INSERT INTO lab_results (id, test_name, value, unit, abnormal_flag, collection_date, document_id)
                 VALUES (?1, ?2, ?3, 'mmol/L', 'normal', ?4, ?5)",
                params![
                    Uuid::new_v4().to_string(),
                    format!("Test_{}", i % 10),
                    (i as f64) * 0.5 + 3.0,
                    format!("2026-01-{:02}", (i % 28) + 1),
                    doc_id,
                ],
            )
            .unwrap();
        }
        for i in 0..10 {
            conn.execute(
                "INSERT INTO symptoms (id, category, specific, severity, onset_date, recorded_date, source)
                 VALUES (?1, 'pain', ?2, ?3, ?4, ?4, 'patient_reported')",
                params![
                    Uuid::new_v4().to_string(),
                    format!("Symptom description {i}"),
                    (i % 5) + 1,
                    format!("2026-01-{:02}", (i % 28) + 1),
                ],
            )
            .unwrap();
        }
        conn.execute(
            "INSERT INTO allergies (id, allergen, severity, source, verified)
             VALUES (?1, 'Penicillin', 'severe', 'patient_reported', 1)",
            params![Uuid::new_v4().to_string()],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO appointments (id, professional_id, date, type)
             VALUES (?1, ?2, date('now', '+3 days'), 'upcoming')",
            params![Uuid::new_v4().to_string(), prof_id],
        )
        .unwrap();

        let request = SyncRequest {
            versions: SyncVersions::default(),
            journal_entries: vec![],
        };

        let response = build_sync_response(&conn, &request, "Léa").unwrap();
        assert!(response.is_some());
        let resp = response.unwrap();

        // Verify all entity types present
        assert!(resp.medications.is_some());
        assert!(resp.labs.is_some());
        assert!(resp.timeline.is_some());
        assert!(resp.profile.is_some());
        assert!(resp.appointment.is_some());

        // Verify counts
        assert_eq!(resp.medications.as_ref().unwrap().len(), 20);
        assert_eq!(resp.labs.as_ref().unwrap().len(), 10); // limit=10
        assert_eq!(resp.timeline.as_ref().unwrap().len(), 10);

        // Verify serialized payload is reasonable size (under 1MB)
        let json = serde_json::to_string(&resp).unwrap();
        assert!(
            json.len() < 1_000_000,
            "Sync payload should be under 1MB, got {} bytes",
            json.len()
        );
    }

    #[test]
    fn journal_sync_duplicate_batch_idempotent() {
        let conn = test_db();
        let shared_id = Uuid::new_v4().to_string();

        // Same entry duplicated within a single batch
        let entries = vec![
            MobileJournalEntry {
                id: shared_id.clone(),
                severity: 3,
                body_location: None,
                free_text: Some("First attempt".to_string()),
                activity_context: None,
                symptom_chip: Some("pain".to_string()),
                created_at: "2026-01-15".to_string(),
            },
            MobileJournalEntry {
                id: shared_id.clone(),
                severity: 3,
                body_location: None,
                free_text: Some("Duplicate attempt".to_string()),
                activity_context: None,
                symptom_chip: Some("pain".to_string()),
                created_at: "2026-01-15".to_string(),
            },
        ];

        let result = process_journal_sync(&conn, &entries).unwrap();
        // Both IDs reported as synced (INSERT OR IGNORE succeeds silently)
        assert_eq!(result.synced_ids.len(), 2);

        // But only one row in the database
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM symptoms", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn assemble_profile_all_allergy_severities() {
        let conn = test_db();

        for severity in &["mild", "moderate", "severe", "life_threatening"] {
            conn.execute(
                "INSERT INTO allergies (id, allergen, severity, source, verified)
                 VALUES (?1, ?2, ?3, 'patient_reported', 1)",
                params![
                    Uuid::new_v4().to_string(),
                    format!("Allergen_{severity}"),
                    severity,
                ],
            )
            .unwrap();
        }

        let profile = assemble_profile_summary(&conn, "Test").unwrap();
        assert_eq!(profile.allergies.len(), 4);
    }

    #[test]
    fn sync_response_versions_always_current() {
        let conn = test_db();
        let doc_id = insert_doc(&conn);

        conn.execute(
            "INSERT INTO medications (id, generic_name, dose, frequency, frequency_type, status, document_id)
             VALUES (?1, 'Metformin', '500mg', 'daily', 'scheduled', 'active', ?2)",
            params![Uuid::new_v4().to_string(), doc_id],
        )
        .unwrap();

        let request = SyncRequest {
            versions: SyncVersions::default(),
            journal_entries: vec![],
        };

        let resp = build_sync_response(&conn, &request, "Léa")
            .unwrap()
            .unwrap();

        // Response versions should reflect current state, not phone state
        assert_eq!(resp.versions.medications, 1);
        assert_eq!(resp.versions.labs, 0);
    }

    #[test]
    fn assemble_medications_recently_stopped_with_end_date() {
        let conn = test_db();
        let doc_id = insert_doc(&conn);

        // Recently stopped with end_date within 6 months — should appear
        conn.execute(
            "INSERT INTO medications (id, generic_name, dose, frequency, frequency_type, status, end_date, document_id)
             VALUES (?1, 'Amoxicillin', '250mg', 'thrice daily', 'scheduled', 'stopped', date('now', '-1 month'), ?2)",
            params![Uuid::new_v4().to_string(), doc_id],
        )
        .unwrap();

        // Old stopped medication — should NOT appear
        conn.execute(
            "INSERT INTO medications (id, generic_name, dose, frequency, frequency_type, status, end_date, document_id)
             VALUES (?1, 'OldDrug', '100mg', 'daily', 'scheduled', 'stopped', '2020-01-01', ?2)",
            params![Uuid::new_v4().to_string(), doc_id],
        )
        .unwrap();

        let meds = assemble_medications(&conn).unwrap();
        assert_eq!(meds.len(), 1);
        assert_eq!(meds[0].generic_name, "Amoxicillin");
    }

    #[test]
    fn lab_trend_stable_within_threshold() {
        let conn = test_db();
        let doc_id = insert_doc(&conn);

        // Two results within 1% of each other — should be "stable"
        conn.execute(
            "INSERT INTO lab_results (id, test_name, value, unit, abnormal_flag, collection_date, document_id)
             VALUES (?1, 'Glucose', 5.0, 'mmol/L', 'normal', '2026-01-01', ?2)",
            params![Uuid::new_v4().to_string(), doc_id],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO lab_results (id, test_name, value, unit, abnormal_flag, collection_date, document_id)
             VALUES (?1, 'Glucose', 5.04, 'mmol/L', 'normal', '2026-01-15', ?2)",
            params![Uuid::new_v4().to_string(), doc_id],
        )
        .unwrap();

        let labs = assemble_recent_labs(&conn, 10).unwrap();
        let recent = &labs[0]; // Most recent (Jan 15)
        assert_eq!(recent.test_name, "Glucose");
        assert_eq!(recent.trend_direction.as_deref(), Some("stable"));
    }

    #[test]
    fn format_alert_title_covers_all_types() {
        assert_eq!(format_alert_title("conflict"), "Medication Conflict");
        assert_eq!(format_alert_title("contradiction"), "Data Contradiction");
        assert_eq!(format_alert_title("critical"), "Critical Alert");
        assert_eq!(format_alert_title("emergency"), "Emergency Alert");
        assert_eq!(format_alert_title("duplicate"), "Duplicate Entry");
        assert_eq!(format_alert_title("gap"), "Coverage Gap");
        assert_eq!(format_alert_title("trend"), "Trend Alert");
        assert_eq!(format_alert_title("some_custom_type"), "some custom type");
    }
}

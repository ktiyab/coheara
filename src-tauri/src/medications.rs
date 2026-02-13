//! L3-05: Medication List — backend types and repository functions.
//!
//! View types for the medication list screen (cards, detail, alerts,
//! dose history, tapering, compounds, aliases, OTC entry), plus all
//! query functions that operate against the existing L0-02 data model.

use chrono::NaiveDate;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::DatabaseError;

// ═══════════════════════════════════════════
// View types — serialised to frontend
// ═══════════════════════════════════════════

/// A medication card for the list view.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MedicationCard {
    pub id: Uuid,
    pub generic_name: String,
    pub brand_name: Option<String>,
    pub dose: String,
    pub frequency: String,
    pub frequency_type: String,
    pub route: String,
    pub prescriber_name: Option<String>,
    pub prescriber_specialty: Option<String>,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub status: String,
    pub reason_start: Option<String>,
    pub is_otc: bool,
    pub is_compound: bool,
    pub has_tapering: bool,
    pub dose_type: String,
    pub administration_instructions: Option<String>,
    pub condition: Option<String>,
    pub coherence_alerts: Vec<MedicationAlert>,
}

/// Coherence alert for a medication (shown inline on card).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MedicationAlert {
    pub id: Uuid,
    pub alert_type: String,
    pub severity: String,
    pub summary: String,
}

/// Full medication detail (expanded view).
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

/// Instruction entry for a medication.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MedicationInstructionView {
    pub id: Uuid,
    pub instruction: String,
    pub timing: Option<String>,
}

/// Compound ingredient display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompoundIngredientView {
    pub id: Uuid,
    pub ingredient_name: String,
    pub ingredient_dose: Option<String>,
    pub maps_to_generic: Option<String>,
}

/// Tapering step display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaperingStepView {
    pub step_number: i32,
    pub dose: String,
    pub duration_days: i32,
    pub start_date: Option<NaiveDate>,
    pub instructions: Option<String>,
    pub is_current: bool,
}

/// Dose change record.
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

/// Brand/generic alias entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MedicationAliasView {
    pub generic_name: String,
    pub brand_name: String,
    pub country: String,
    pub source: String,
}

/// Filter parameters for the medication list.
#[derive(Debug, Clone, Deserialize)]
pub struct MedicationListFilter {
    pub status: Option<String>,
    pub prescriber_id: Option<String>,
    pub search_query: Option<String>,
    pub include_otc: bool,
}

/// Data for the medication list screen header.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MedicationListData {
    pub medications: Vec<MedicationCard>,
    pub total_active: u32,
    pub total_paused: u32,
    pub total_stopped: u32,
    pub prescribers: Vec<PrescriberOption>,
}

/// Prescriber option for the filter dropdown.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrescriberOption {
    pub id: Uuid,
    pub name: String,
    pub specialty: Option<String>,
    pub medication_count: u32,
}

/// OTC medication entry input.
#[derive(Debug, Clone, Deserialize)]
pub struct OtcMedicationInput {
    pub name: String,
    pub dose: String,
    pub frequency: String,
    pub route: String,
    pub reason: Option<String>,
    pub start_date: Option<String>,
    pub instructions: Option<String>,
}

/// Alias search result (for autocomplete).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AliasSearchResult {
    pub generic_name: String,
    pub brand_names: Vec<String>,
    pub source: String,
}

// ═══════════════════════════════════════════
// Repository functions
// ═══════════════════════════════════════════

/// Fetch medications with dynamic filters, joined with prescriber info.
/// Sorted: active first, then paused, then stopped; within group by start_date DESC.
pub fn fetch_medications_filtered(
    conn: &Connection,
    filter: &MedicationListFilter,
) -> Result<Vec<MedicationCard>, DatabaseError> {
    let mut sql = String::from(
        "SELECT m.id, m.generic_name, m.brand_name, m.dose, m.frequency,
                m.frequency_type, m.route, m.status, m.start_date, m.end_date,
                m.reason_start, m.is_otc, m.is_compound, m.dose_type,
                m.administration_instructions, m.condition,
                p.name AS prescriber_name, p.specialty AS prescriber_specialty
         FROM medications m
         LEFT JOIN professionals p ON m.prescriber_id = p.id
         WHERE 1=1",
    );

    let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut param_idx = 1;

    if let Some(status) = &filter.status {
        sql.push_str(&format!(" AND m.status = ?{param_idx}"));
        params_vec.push(Box::new(status.clone()));
        param_idx += 1;
    }

    if let Some(prescriber_id) = &filter.prescriber_id {
        sql.push_str(&format!(" AND m.prescriber_id = ?{param_idx}"));
        params_vec.push(Box::new(prescriber_id.clone()));
        param_idx += 1;
    }

    if !filter.include_otc {
        sql.push_str(" AND m.is_otc = 0");
    }

    if let Some(query) = &filter.search_query {
        if !query.trim().is_empty() {
            let pattern = format!("%{}%", query.trim());
            sql.push_str(&format!(
                " AND (m.generic_name LIKE ?{p} COLLATE NOCASE
                   OR m.brand_name LIKE ?{p} COLLATE NOCASE
                   OR m.condition LIKE ?{p} COLLATE NOCASE)",
                p = param_idx
            ));
            params_vec.push(Box::new(pattern));
            // param_idx incremented but not used after this
        }
    }

    sql.push_str(
        " ORDER BY CASE m.status
            WHEN 'active' THEN 1
            WHEN 'paused' THEN 2
            WHEN 'stopped' THEN 3
          END ASC,
          m.start_date DESC",
    );

    let params_refs: Vec<&dyn rusqlite::types::ToSql> =
        params_vec.iter().map(|p| p.as_ref()).collect();

    let mut stmt = conn.prepare(&sql)?;
    let cards = stmt
        .query_map(params_refs.as_slice(), |row| {
            Ok(MedicationCard {
                id: row
                    .get::<_, String>(0)?
                    .parse()
                    .unwrap_or_else(|_| Uuid::nil()),
                generic_name: row.get(1)?,
                brand_name: row.get(2)?,
                dose: row.get(3)?,
                frequency: row.get(4)?,
                frequency_type: row.get(5)?,
                route: row.get(6)?,
                status: row.get(7)?,
                start_date: row
                    .get::<_, Option<String>>(8)?
                    .and_then(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok()),
                end_date: row
                    .get::<_, Option<String>>(9)?
                    .and_then(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok()),
                reason_start: row.get(10)?,
                is_otc: row.get::<_, i32>(11)? != 0,
                is_compound: row.get::<_, i32>(12)? != 0,
                has_tapering: false, // enriched after query
                dose_type: row.get(13)?,
                administration_instructions: row.get(14)?,
                condition: row.get(15)?,
                prescriber_name: row.get(16)?,
                prescriber_specialty: row.get(17)?,
                coherence_alerts: Vec::new(), // enriched after query
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(cards)
}

/// Fetch medication counts by status for the tab bar.
pub fn fetch_medication_status_counts(
    conn: &Connection,
) -> Result<(u32, u32, u32), DatabaseError> {
    let active: u32 = conn.query_row(
        "SELECT COUNT(*) FROM medications WHERE status = 'active'",
        [],
        |row| row.get(0),
    )?;
    let paused: u32 = conn.query_row(
        "SELECT COUNT(*) FROM medications WHERE status = 'paused'",
        [],
        |row| row.get(0),
    )?;
    let stopped: u32 = conn.query_row(
        "SELECT COUNT(*) FROM medications WHERE status = 'stopped'",
        [],
        |row| row.get(0),
    )?;
    Ok((active, paused, stopped))
}

/// Fetch prescribers who have medications, with counts.
pub fn fetch_prescriber_options(
    conn: &Connection,
) -> Result<Vec<PrescriberOption>, DatabaseError> {
    let mut stmt = conn.prepare(
        "SELECT p.id, p.name, p.specialty, COUNT(m.id) AS med_count
         FROM professionals p
         INNER JOIN medications m ON m.prescriber_id = p.id
         GROUP BY p.id
         ORDER BY med_count DESC, p.name ASC",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok(PrescriberOption {
                id: row
                    .get::<_, String>(0)?
                    .parse()
                    .unwrap_or_else(|_| Uuid::nil()),
                name: row.get(1)?,
                specialty: row.get(2)?,
                medication_count: row.get(3)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Fetch a single medication card by ID, enriched with tapering flag and alerts.
pub fn fetch_single_medication_card(
    conn: &Connection,
    medication_id: &Uuid,
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
                id: row
                    .get::<_, String>(0)?
                    .parse()
                    .unwrap_or_else(|_| Uuid::nil()),
                generic_name: row.get(1)?,
                brand_name: row.get(2)?,
                dose: row.get(3)?,
                frequency: row.get(4)?,
                frequency_type: row.get(5)?,
                route: row.get(6)?,
                status: row.get(7)?,
                start_date: row
                    .get::<_, Option<String>>(8)?
                    .and_then(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok()),
                end_date: row
                    .get::<_, Option<String>>(9)?
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
            card.has_tapering = conn
                .query_row(
                    "SELECT COUNT(*) FROM tapering_schedules WHERE medication_id = ?1",
                    params![medication_id.to_string()],
                    |row| row.get::<_, u32>(0),
                )
                .unwrap_or(0)
                > 0;

            card.coherence_alerts =
                fetch_medication_alerts(conn, medication_id).unwrap_or_default();

            Ok(Some(card))
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(DatabaseError::from(e)),
    }
}

/// Enrich a list of medication cards with tapering flags and coherence alerts.
pub fn enrich_medication_cards(
    conn: &Connection,
    cards: Vec<MedicationCard>,
) -> Vec<MedicationCard> {
    cards
        .into_iter()
        .map(|mut card| {
            card.has_tapering = conn
                .query_row(
                    "SELECT COUNT(*) FROM tapering_schedules WHERE medication_id = ?1",
                    params![card.id.to_string()],
                    |row| row.get::<_, u32>(0),
                )
                .unwrap_or(0)
                > 0;

            card.coherence_alerts =
                fetch_medication_alerts(conn, &card.id).unwrap_or_default();

            card
        })
        .collect()
}

/// Fetch medication instructions for a given medication.
pub fn fetch_medication_instructions(
    conn: &Connection,
    medication_id: &Uuid,
) -> Result<Vec<MedicationInstructionView>, DatabaseError> {
    let mut stmt = conn.prepare(
        "SELECT id, instruction, timing
         FROM medication_instructions
         WHERE medication_id = ?1
         ORDER BY id ASC",
    )?;
    let rows = stmt
        .query_map(params![medication_id.to_string()], |row| {
            Ok(MedicationInstructionView {
                id: row
                    .get::<_, String>(0)?
                    .parse()
                    .unwrap_or_else(|_| Uuid::nil()),
                instruction: row.get(1)?,
                timing: row.get(2)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Fetch compound ingredients for a medication.
pub fn fetch_compound_ingredients(
    conn: &Connection,
    medication_id: &Uuid,
) -> Result<Vec<CompoundIngredientView>, DatabaseError> {
    let mut stmt = conn.prepare(
        "SELECT id, ingredient_name, ingredient_dose, maps_to_generic
         FROM compound_ingredients
         WHERE medication_id = ?1
         ORDER BY ingredient_name ASC",
    )?;
    let rows = stmt
        .query_map(params![medication_id.to_string()], |row| {
            Ok(CompoundIngredientView {
                id: row
                    .get::<_, String>(0)?
                    .parse()
                    .unwrap_or_else(|_| Uuid::nil()),
                ingredient_name: row.get(1)?,
                ingredient_dose: row.get(2)?,
                maps_to_generic: row.get(3)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Fetch tapering steps with current-step computation.
pub fn fetch_tapering_steps(
    conn: &Connection,
    medication_id: &Uuid,
) -> Result<Vec<TaperingStepView>, DatabaseError> {
    let today = chrono::Local::now().date_naive();

    let mut stmt = conn.prepare(
        "SELECT step_number, dose, duration_days, start_date
         FROM tapering_schedules
         WHERE medication_id = ?1
         ORDER BY step_number ASC",
    )?;
    let mut steps: Vec<TaperingStepView> = stmt
        .query_map(params![medication_id.to_string()], |row| {
            let start_date = row
                .get::<_, Option<String>>(3)?
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

    // Compute which step is current based on dates.
    let mut found_current = false;
    for step in steps.iter_mut() {
        if let Some(start) = step.start_date {
            let end = start + chrono::Duration::days(i64::from(step.duration_days));
            if !found_current && today >= start && today < end {
                step.is_current = true;
                found_current = true;
            }
        }
    }
    // Fallback: mark first step as current if no date matched.
    if !found_current {
        if let Some(first) = steps.first_mut() {
            first.is_current = true;
        }
    }

    Ok(steps)
}

/// Fetch dose change history ordered chronologically.
pub fn fetch_dose_history(
    conn: &Connection,
    medication_id: &Uuid,
) -> Result<Vec<DoseChangeView>, DatabaseError> {
    let mut stmt = conn.prepare(
        "SELECT dc.id, dc.old_dose, dc.new_dose, dc.old_frequency,
                dc.new_frequency, dc.change_date, dc.reason,
                p.name AS changed_by_name,
                d.title AS document_title
         FROM dose_changes dc
         LEFT JOIN professionals p ON dc.changed_by_id = p.id
         LEFT JOIN documents d ON dc.document_id = d.id
         WHERE dc.medication_id = ?1
         ORDER BY dc.change_date ASC",
    )?;
    let rows = stmt
        .query_map(params![medication_id.to_string()], |row| {
            Ok(DoseChangeView {
                id: row
                    .get::<_, String>(0)?
                    .parse()
                    .unwrap_or_else(|_| Uuid::nil()),
                old_dose: row.get(1)?,
                new_dose: row.get(2)?,
                old_frequency: row.get(3)?,
                new_frequency: row.get(4)?,
                change_date: NaiveDate::parse_from_str(
                    &row.get::<_, String>(5)?,
                    "%Y-%m-%d",
                )
                .unwrap_or_else(|_| chrono::Local::now().date_naive()),
                reason: row.get(6)?,
                changed_by_name: row.get(7)?,
                document_title: row.get(8)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Fetch known aliases for a medication's generic name.
pub fn fetch_medication_aliases(
    conn: &Connection,
    generic_name: &str,
) -> Result<Vec<MedicationAliasView>, DatabaseError> {
    let mut stmt = conn.prepare(
        "SELECT generic_name, brand_name, country, source
         FROM medication_aliases
         WHERE generic_name = ?1 COLLATE NOCASE
         ORDER BY source ASC, brand_name ASC",
    )?;
    let rows = stmt
        .query_map(params![generic_name], |row| {
            Ok(MedicationAliasView {
                generic_name: row.get(0)?,
                brand_name: row.get(1)?,
                country: row.get(2)?,
                source: row.get(3)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Search medication aliases by partial name (for autocomplete).
pub fn search_medication_aliases(
    conn: &Connection,
    query: &str,
    limit: u32,
) -> Result<Vec<AliasSearchResult>, DatabaseError> {
    let pattern = format!("%{}%", query);
    let mut stmt = conn.prepare(
        "SELECT generic_name, GROUP_CONCAT(brand_name, ', '), source
         FROM medication_aliases
         WHERE generic_name LIKE ?1 COLLATE NOCASE
            OR brand_name LIKE ?1 COLLATE NOCASE
         GROUP BY generic_name
         ORDER BY generic_name ASC
         LIMIT ?2",
    )?;
    let rows = stmt
        .query_map(params![pattern, limit], |row| {
            let brand_str: String = row.get::<_, Option<String>>(1)?.unwrap_or_default();
            Ok(AliasSearchResult {
                generic_name: row.get(0)?,
                brand_names: if brand_str.is_empty() {
                    Vec::new()
                } else {
                    brand_str.split(", ").map(String::from).collect()
                },
                source: row.get(2)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Fetch coherence alerts for a specific medication.
///
/// Queries `coherence_alerts` table (migration 004) for non-dismissed alerts
/// whose entity_ids JSON array contains this medication's UUID.
pub fn fetch_medication_alerts(
    conn: &Connection,
    medication_id: &Uuid,
) -> Result<Vec<MedicationAlert>, DatabaseError> {
    let med_id_str = medication_id.to_string();
    let mut stmt = conn.prepare(
        "SELECT id, alert_type, severity, patient_message
         FROM coherence_alerts
         WHERE dismissed = 0
           AND entity_ids LIKE ?1
         ORDER BY CASE severity
           WHEN 'critical' THEN 0
           WHEN 'standard' THEN 1
           ELSE 2
         END",
    )?;
    let rows = stmt
        .query_map(params![format!("%{med_id_str}%")], |row| {
            Ok(MedicationAlert {
                id: row
                    .get::<_, String>(0)?
                    .parse()
                    .unwrap_or_else(|_| Uuid::nil()),
                alert_type: row.get(1)?,
                severity: row.get(2)?,
                summary: row.get(3)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Get or create the synthetic "patient-reported" document for OTC entries.
pub fn get_or_create_patient_reported_document(
    conn: &Connection,
) -> Result<Uuid, DatabaseError> {
    let existing: Option<String> = conn
        .query_row(
            "SELECT id FROM documents WHERE type = 'other' AND title = 'Patient-reported medications'",
            [],
            |row| row.get(0),
        )
        .ok();

    if let Some(id_str) = existing {
        return id_str
            .parse()
            .map_err(|e| DatabaseError::InvalidEnum {
                field: "document_id".into(),
                value: format!("{e}"),
            });
    }

    let doc_id = Uuid::new_v4();
    conn.execute(
        "INSERT INTO documents (id, type, title, ingestion_date, source_file, verified)
         VALUES (?1, 'other', 'Patient-reported medications', datetime('now'), 'patient-reported', 1)",
        params![doc_id.to_string()],
    )?;

    Ok(doc_id)
}

/// Fetch source document info for a medication.
pub fn fetch_source_document(
    conn: &Connection,
    medication_id: &Uuid,
) -> (Option<String>, Option<NaiveDate>) {
    conn.query_row(
        "SELECT d.title, d.document_date
         FROM documents d
         INNER JOIN medications m ON m.document_id = d.id
         WHERE m.id = ?1",
        params![medication_id.to_string()],
        |row| {
            let title: Option<String> = row.get(0)?;
            let date_str: Option<String> = row.get(1)?;
            let date =
                date_str.and_then(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok());
            Ok((title, date))
        },
    )
    .unwrap_or((None, None))
}

// ═══════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::sqlite::open_memory_database;

    /// Helper: insert a test medication and return its ID.
    fn insert_test_medication(
        conn: &Connection,
        generic_name: &str,
        status: &str,
        is_otc: bool,
        prescriber_id: Option<&str>,
        document_id: &str,
    ) -> Uuid {
        let id = Uuid::new_v4();
        conn.execute(
            "INSERT INTO medications (
                id, generic_name, brand_name, dose, frequency, frequency_type,
                route, prescriber_id, start_date, end_date, reason_start,
                reason_stop, is_otc, status, administration_instructions,
                max_daily_dose, condition, dose_type, is_compound, document_id
            ) VALUES (
                ?1, ?2, NULL, '500mg', 'Twice daily', 'scheduled',
                'oral', ?3, '2025-01-15', NULL, 'Test reason',
                NULL, ?4, ?5, NULL,
                NULL, NULL, 'fixed', 0, ?6
            )",
            params![
                id.to_string(),
                generic_name,
                prescriber_id,
                is_otc as i32,
                status,
                document_id,
            ],
        )
        .expect("insert medication");
        id
    }

    /// Helper: insert a test document and return its ID.
    fn insert_test_document(conn: &Connection, title: &str) -> Uuid {
        let id = Uuid::new_v4();
        conn.execute(
            "INSERT INTO documents (id, type, title, ingestion_date, source_file, verified)
             VALUES (?1, 'prescription', ?2, datetime('now'), 'test.pdf', 1)",
            params![id.to_string(), title],
        )
        .expect("insert document");
        id
    }

    /// Helper: insert a test professional and return the ID string.
    fn insert_test_professional(
        conn: &Connection,
        name: &str,
        specialty: Option<&str>,
    ) -> String {
        let id = Uuid::new_v4();
        conn.execute(
            "INSERT INTO professionals (id, name, specialty, first_seen_date, last_seen_date)
             VALUES (?1, ?2, ?3, '2025-01-01', '2025-01-01')",
            params![id.to_string(), name, specialty],
        )
        .expect("insert professional");
        id.to_string()
    }

    #[test]
    fn fetch_filtered_returns_all_medications() {
        let conn = open_memory_database().unwrap();
        let doc_id = insert_test_document(&conn, "Rx Test");
        insert_test_medication(&conn, "Metformin", "active", false, None, &doc_id.to_string());
        insert_test_medication(&conn, "Lisinopril", "active", false, None, &doc_id.to_string());
        insert_test_medication(&conn, "Ibuprofen", "stopped", true, None, &doc_id.to_string());

        let filter = MedicationListFilter {
            status: None,
            prescriber_id: None,
            search_query: None,
            include_otc: true,
        };
        let meds = fetch_medications_filtered(&conn, &filter).unwrap();
        assert_eq!(meds.len(), 3);
        // Active medications should come first
        assert_eq!(meds[0].status, "active");
        assert_eq!(meds[1].status, "active");
        assert_eq!(meds[2].status, "stopped");
    }

    #[test]
    fn fetch_filtered_by_status() {
        let conn = open_memory_database().unwrap();
        let doc_id = insert_test_document(&conn, "Rx Test");
        insert_test_medication(&conn, "Metformin", "active", false, None, &doc_id.to_string());
        insert_test_medication(&conn, "Lisinopril", "paused", false, None, &doc_id.to_string());
        insert_test_medication(&conn, "Ibuprofen", "stopped", true, None, &doc_id.to_string());

        let filter = MedicationListFilter {
            status: Some("active".into()),
            prescriber_id: None,
            search_query: None,
            include_otc: true,
        };
        let meds = fetch_medications_filtered(&conn, &filter).unwrap();
        assert_eq!(meds.len(), 1);
        assert_eq!(meds[0].generic_name, "Metformin");
    }

    #[test]
    fn fetch_filtered_excludes_otc() {
        let conn = open_memory_database().unwrap();
        let doc_id = insert_test_document(&conn, "Rx Test");
        insert_test_medication(&conn, "Metformin", "active", false, None, &doc_id.to_string());
        insert_test_medication(&conn, "Ibuprofen", "active", true, None, &doc_id.to_string());

        let filter = MedicationListFilter {
            status: None,
            prescriber_id: None,
            search_query: None,
            include_otc: false,
        };
        let meds = fetch_medications_filtered(&conn, &filter).unwrap();
        assert_eq!(meds.len(), 1);
        assert_eq!(meds[0].generic_name, "Metformin");
    }

    #[test]
    fn fetch_filtered_by_search_query() {
        let conn = open_memory_database().unwrap();
        let doc_id = insert_test_document(&conn, "Rx Test");
        insert_test_medication(&conn, "Metformin", "active", false, None, &doc_id.to_string());
        insert_test_medication(&conn, "Lisinopril", "active", false, None, &doc_id.to_string());

        let filter = MedicationListFilter {
            status: None,
            prescriber_id: None,
            search_query: Some("metfor".into()),
            include_otc: true,
        };
        let meds = fetch_medications_filtered(&conn, &filter).unwrap();
        assert_eq!(meds.len(), 1);
        assert_eq!(meds[0].generic_name, "Metformin");
    }

    #[test]
    fn fetch_filtered_by_prescriber() {
        let conn = open_memory_database().unwrap();
        let doc_id = insert_test_document(&conn, "Rx Test");
        let prof_id = insert_test_professional(&conn, "Dr. Chen", Some("Endocrinology"));
        insert_test_medication(&conn, "Metformin", "active", false, Some(&prof_id), &doc_id.to_string());
        insert_test_medication(&conn, "Ibuprofen", "active", true, None, &doc_id.to_string());

        let filter = MedicationListFilter {
            status: None,
            prescriber_id: Some(prof_id.clone()),
            search_query: None,
            include_otc: true,
        };
        let meds = fetch_medications_filtered(&conn, &filter).unwrap();
        assert_eq!(meds.len(), 1);
        assert_eq!(meds[0].generic_name, "Metformin");
        assert_eq!(meds[0].prescriber_name.as_deref(), Some("Dr. Chen"));
        assert_eq!(meds[0].prescriber_specialty.as_deref(), Some("Endocrinology"));
    }

    #[test]
    fn status_counts_correct() {
        let conn = open_memory_database().unwrap();
        let doc_id = insert_test_document(&conn, "Rx Test");
        insert_test_medication(&conn, "A", "active", false, None, &doc_id.to_string());
        insert_test_medication(&conn, "B", "active", false, None, &doc_id.to_string());
        insert_test_medication(&conn, "C", "paused", false, None, &doc_id.to_string());
        insert_test_medication(&conn, "D", "stopped", false, None, &doc_id.to_string());

        let (active, paused, stopped) = fetch_medication_status_counts(&conn).unwrap();
        assert_eq!(active, 2);
        assert_eq!(paused, 1);
        assert_eq!(stopped, 1);
    }

    #[test]
    fn prescriber_options_with_counts() {
        let conn = open_memory_database().unwrap();
        let doc_id = insert_test_document(&conn, "Rx Test");
        let prof1 = insert_test_professional(&conn, "Dr. Chen", Some("Endo"));
        let prof2 = insert_test_professional(&conn, "Dr. Smith", None);
        insert_test_medication(&conn, "A", "active", false, Some(&prof1), &doc_id.to_string());
        insert_test_medication(&conn, "B", "active", false, Some(&prof1), &doc_id.to_string());
        insert_test_medication(&conn, "C", "active", false, Some(&prof2), &doc_id.to_string());

        let options = fetch_prescriber_options(&conn).unwrap();
        assert_eq!(options.len(), 2);
        // Dr. Chen should be first (2 meds vs 1)
        assert_eq!(options[0].name, "Dr. Chen");
        assert_eq!(options[0].medication_count, 2);
        assert_eq!(options[1].name, "Dr. Smith");
        assert_eq!(options[1].medication_count, 1);
    }

    #[test]
    fn single_medication_card_with_tapering() {
        let conn = open_memory_database().unwrap();
        let doc_id = insert_test_document(&conn, "Rx Test");
        let med_id = insert_test_medication(&conn, "Prednisone", "active", false, None, &doc_id.to_string());

        // Add tapering step
        conn.execute(
            "INSERT INTO tapering_schedules (id, medication_id, step_number, dose, duration_days)
             VALUES (?1, ?2, 1, '40mg', 7)",
            params![Uuid::new_v4().to_string(), med_id.to_string()],
        )
        .unwrap();

        let card = fetch_single_medication_card(&conn, &med_id).unwrap().unwrap();
        assert_eq!(card.generic_name, "Prednisone");
        assert!(card.has_tapering);
    }

    #[test]
    fn single_medication_card_not_found() {
        let conn = open_memory_database().unwrap();
        let card = fetch_single_medication_card(&conn, &Uuid::new_v4()).unwrap();
        assert!(card.is_none());
    }

    #[test]
    fn fetch_instructions_for_medication() {
        let conn = open_memory_database().unwrap();
        let doc_id = insert_test_document(&conn, "Rx");
        let med_id = insert_test_medication(&conn, "Metformin", "active", false, None, &doc_id.to_string());

        let instr_id = Uuid::new_v4();
        conn.execute(
            "INSERT INTO medication_instructions (id, medication_id, instruction, timing)
             VALUES (?1, ?2, 'Take with food', 'with meals')",
            params![instr_id.to_string(), med_id.to_string()],
        )
        .unwrap();

        let instructions = fetch_medication_instructions(&conn, &med_id).unwrap();
        assert_eq!(instructions.len(), 1);
        assert_eq!(instructions[0].instruction, "Take with food");
        assert_eq!(instructions[0].timing.as_deref(), Some("with meals"));
    }

    #[test]
    fn fetch_compound_ingredients_for_medication() {
        let conn = open_memory_database().unwrap();
        let doc_id = insert_test_document(&conn, "Rx");
        let med_id = insert_test_medication(&conn, "Compound cream", "active", false, None, &doc_id.to_string());

        let ing1 = Uuid::new_v4();
        let ing2 = Uuid::new_v4();
        conn.execute(
            "INSERT INTO compound_ingredients (id, medication_id, ingredient_name, ingredient_dose, maps_to_generic)
             VALUES (?1, ?2, 'Lidocaine', '2%', NULL)",
            params![ing1.to_string(), med_id.to_string()],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO compound_ingredients (id, medication_id, ingredient_name, ingredient_dose, maps_to_generic)
             VALUES (?1, ?2, 'Gabapentin', '6%', 'Gabapentin')",
            params![ing2.to_string(), med_id.to_string()],
        )
        .unwrap();

        let ingredients = fetch_compound_ingredients(&conn, &med_id).unwrap();
        assert_eq!(ingredients.len(), 2);
        // Sorted by name ASC
        assert_eq!(ingredients[0].ingredient_name, "Gabapentin");
        assert_eq!(ingredients[1].ingredient_name, "Lidocaine");
    }

    #[test]
    fn fetch_tapering_steps_marks_current() {
        let conn = open_memory_database().unwrap();
        let doc_id = insert_test_document(&conn, "Rx");
        let med_id = insert_test_medication(&conn, "Prednisone", "active", false, None, &doc_id.to_string());

        let today = chrono::Local::now().date_naive();
        let past = today - chrono::Duration::days(14);
        let recent = today - chrono::Duration::days(3);

        conn.execute(
            "INSERT INTO tapering_schedules (id, medication_id, step_number, dose, duration_days, start_date)
             VALUES (?1, ?2, 1, '40mg', 7, ?3)",
            params![Uuid::new_v4().to_string(), med_id.to_string(), past.to_string()],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO tapering_schedules (id, medication_id, step_number, dose, duration_days, start_date)
             VALUES (?1, ?2, 2, '30mg', 7, ?3)",
            params![Uuid::new_v4().to_string(), med_id.to_string(), recent.to_string()],
        )
        .unwrap();

        let steps = fetch_tapering_steps(&conn, &med_id).unwrap();
        assert_eq!(steps.len(), 2);
        assert!(!steps[0].is_current); // past step ended
        assert!(steps[1].is_current); // current step
    }

    #[test]
    fn fetch_dose_history_ordered() {
        let conn = open_memory_database().unwrap();
        let doc_id = insert_test_document(&conn, "Rx");
        let med_id = insert_test_medication(&conn, "Metformin", "active", false, None, &doc_id.to_string());

        conn.execute(
            "INSERT INTO dose_changes (id, medication_id, old_dose, new_dose, change_date)
             VALUES (?1, ?2, NULL, '250mg', '2025-01-15')",
            params![Uuid::new_v4().to_string(), med_id.to_string()],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO dose_changes (id, medication_id, old_dose, new_dose, old_frequency, new_frequency, change_date, reason)
             VALUES (?1, ?2, '250mg', '500mg', 'Once daily', 'Twice daily', '2025-02-01', 'Dose increase')",
            params![Uuid::new_v4().to_string(), med_id.to_string()],
        )
        .unwrap();

        let history = fetch_dose_history(&conn, &med_id).unwrap();
        assert_eq!(history.len(), 2);
        assert!(history[0].old_dose.is_none()); // first entry
        assert_eq!(history[0].new_dose, "250mg");
        assert_eq!(history[1].old_dose.as_deref(), Some("250mg"));
        assert_eq!(history[1].new_dose, "500mg");
        assert_eq!(history[1].reason.as_deref(), Some("Dose increase"));
    }

    #[test]
    fn fetch_aliases_for_medication() {
        let conn = open_memory_database().unwrap();

        conn.execute(
            "INSERT INTO medication_aliases (generic_name, brand_name, country, source)
             VALUES ('Metformin', 'Glucophage', 'US', 'bundled')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO medication_aliases (generic_name, brand_name, country, source)
             VALUES ('Metformin', 'Fortamet', 'US', 'bundled')",
            [],
        )
        .unwrap();

        let aliases = fetch_medication_aliases(&conn, "Metformin").unwrap();
        assert_eq!(aliases.len(), 2);
        assert_eq!(aliases[0].brand_name, "Fortamet"); // sorted by brand_name
        assert_eq!(aliases[1].brand_name, "Glucophage");
    }

    #[test]
    fn search_aliases_partial_match() {
        let conn = open_memory_database().unwrap();

        conn.execute(
            "INSERT INTO medication_aliases (generic_name, brand_name, country, source)
             VALUES ('Metformin', 'Glucophage', 'US', 'bundled')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO medication_aliases (generic_name, brand_name, country, source)
             VALUES ('Ibuprofen', 'Advil', 'US', 'bundled')",
            [],
        )
        .unwrap();

        let results = search_medication_aliases(&conn, "met", 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].generic_name, "Metformin");
        assert!(results[0].brand_names.contains(&"Glucophage".to_string()));
    }

    #[test]
    fn search_aliases_by_brand_name() {
        let conn = open_memory_database().unwrap();

        conn.execute(
            "INSERT INTO medication_aliases (generic_name, brand_name, country, source)
             VALUES ('Ibuprofen', 'Advil', 'US', 'bundled')",
            [],
        )
        .unwrap();

        let results = search_medication_aliases(&conn, "adv", 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].generic_name, "Ibuprofen");
    }

    #[test]
    fn medication_alerts_returns_empty_when_no_alerts() {
        let conn = open_memory_database().unwrap();
        let alerts = fetch_medication_alerts(&conn, &Uuid::new_v4()).unwrap();
        assert!(alerts.is_empty());
    }

    #[test]
    fn medication_alerts_returns_matching_alerts() {
        let conn = open_memory_database().unwrap();
        let med_id = Uuid::new_v4();
        let alert_id = Uuid::new_v4();

        // Insert a coherence alert referencing this medication
        conn.execute(
            "INSERT INTO coherence_alerts (id, alert_type, severity, entity_ids,
             source_document_ids, patient_message, detail_json, detected_at)
             VALUES (?1, 'conflict', 'standard', ?2, '[]', 'Conflicting doses found', '{}', datetime('now'))",
            params![alert_id.to_string(), format!("[\"{}\"]", med_id)],
        )
        .unwrap();

        let alerts = fetch_medication_alerts(&conn, &med_id).unwrap();
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].id, alert_id);
        assert_eq!(alerts[0].alert_type, "conflict");
        assert_eq!(alerts[0].severity, "standard");
        assert_eq!(alerts[0].summary, "Conflicting doses found");
    }

    #[test]
    fn medication_alerts_excludes_dismissed() {
        let conn = open_memory_database().unwrap();
        let med_id = Uuid::new_v4();
        let alert_id = Uuid::new_v4();

        // Insert a dismissed alert
        conn.execute(
            "INSERT INTO coherence_alerts (id, alert_type, severity, entity_ids,
             source_document_ids, patient_message, detail_json, detected_at, dismissed)
             VALUES (?1, 'dose', 'critical', ?2, '[]', 'High dose', '{}', datetime('now'), 1)",
            params![alert_id.to_string(), format!("[\"{}\"]", med_id)],
        )
        .unwrap();

        let alerts = fetch_medication_alerts(&conn, &med_id).unwrap();
        assert!(alerts.is_empty());
    }

    #[test]
    fn get_or_create_patient_reported_document_creates() {
        let conn = open_memory_database().unwrap();
        let doc_id1 = get_or_create_patient_reported_document(&conn).unwrap();
        let doc_id2 = get_or_create_patient_reported_document(&conn).unwrap();
        // Should return the same document on second call
        assert_eq!(doc_id1, doc_id2);
    }

    #[test]
    fn source_document_info() {
        let conn = open_memory_database().unwrap();
        let doc_id = insert_test_document(&conn, "Prescription Jan 2025");
        let med_id = insert_test_medication(&conn, "Metformin", "active", false, None, &doc_id.to_string());

        let (title, _date) = fetch_source_document(&conn, &med_id);
        assert_eq!(title.as_deref(), Some("Prescription Jan 2025"));
    }

    #[test]
    fn enrich_cards_sets_tapering_flag() {
        let conn = open_memory_database().unwrap();
        let doc_id = insert_test_document(&conn, "Rx");
        let med_id = insert_test_medication(&conn, "Prednisone", "active", false, None, &doc_id.to_string());

        conn.execute(
            "INSERT INTO tapering_schedules (id, medication_id, step_number, dose, duration_days)
             VALUES (?1, ?2, 1, '40mg', 7)",
            params![Uuid::new_v4().to_string(), med_id.to_string()],
        )
        .unwrap();

        let filter = MedicationListFilter {
            status: None,
            prescriber_id: None,
            search_query: None,
            include_otc: true,
        };
        let cards = fetch_medications_filtered(&conn, &filter).unwrap();
        assert!(!cards[0].has_tapering); // not yet enriched

        let enriched = enrich_medication_cards(&conn, cards);
        assert!(enriched[0].has_tapering); // now enriched
    }

    #[test]
    fn empty_database_returns_no_medications() {
        let conn = open_memory_database().unwrap();
        let filter = MedicationListFilter {
            status: None,
            prescriber_id: None,
            search_query: None,
            include_otc: true,
        };
        let meds = fetch_medications_filtered(&conn, &filter).unwrap();
        assert!(meds.is_empty());

        let (active, paused, stopped) = fetch_medication_status_counts(&conn).unwrap();
        assert_eq!(active, 0);
        assert_eq!(paused, 0);
        assert_eq!(stopped, 0);

        let options = fetch_prescriber_options(&conn).unwrap();
        assert!(options.is_empty());
    }
}

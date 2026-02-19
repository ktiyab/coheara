use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::TrustError;

/// Critical lab alert surfaced to the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CriticalLabAlert {
    pub id: String,
    pub test_name: String,
    pub value: String,
    pub unit: String,
    pub reference_range: String,
    pub abnormal_flag: String,
    pub lab_date: String,
    pub document_id: String,
    pub detected_at: String,
    pub dismissed: bool,
}

/// 2-step dismissal request for critical alerts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CriticalDismissRequest {
    pub alert_id: String,
    pub step: DismissStep,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DismissStep {
    AskConfirmation,
    ConfirmDismissal { reason: String },
}

/// Fetch all critical lab results that have NOT been dismissed.
pub fn fetch_critical_alerts(conn: &Connection) -> Result<Vec<CriticalLabAlert>, TrustError> {
    // Get dismissed alert entity_ids for type 'critical'
    let dismissed_lab_ids: std::collections::HashSet<String> = {
        let mut stmt = conn.prepare(
            "SELECT entity_ids FROM dismissed_alerts WHERE alert_type = 'critical'",
        )?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        let mut set = std::collections::HashSet::new();
        for row in rows {
            let ids_json = row?;
            // entity_ids is JSON array of IDs
            if let Ok(ids) = serde_json::from_str::<Vec<String>>(&ids_json) {
                for id in ids {
                    set.insert(id);
                }
            } else {
                // Fallback: treat as single ID string
                set.insert(ids_json);
            }
        }
        set
    };

    let mut stmt = conn.prepare(
        "SELECT id, test_name, value, value_text, unit,
                reference_range_low, reference_range_high,
                abnormal_flag, collection_date, document_id
         FROM lab_results
         WHERE abnormal_flag IN ('critical_low', 'critical_high')",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<f64>>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, Option<String>>(4)?,
            row.get::<_, Option<f64>>(5)?,
            row.get::<_, Option<f64>>(6)?,
            row.get::<_, String>(7)?,
            row.get::<_, String>(8)?,
            row.get::<_, String>(9)?,
        ))
    })?;

    let mut alerts = Vec::new();
    for row in rows {
        let (id, test_name, value, value_text, unit, ref_low, ref_high, flag, date, doc_id) =
            row?;

        if dismissed_lab_ids.contains(&id) {
            continue;
        }

        let value_str = value
            .map(|v| format!("{v}"))
            .or(value_text)
            .unwrap_or_else(|| "N/A".into());

        let unit_str = unit.unwrap_or_default();

        let range_str = match (ref_low, ref_high) {
            (Some(lo), Some(hi)) => format!("{lo} — {hi} {unit_str}"),
            _ => "Not available".into(),
        };

        alerts.push(CriticalLabAlert {
            id: id.clone(),
            test_name,
            value: value_str,
            unit: unit_str,
            reference_range: range_str,
            abnormal_flag: flag,
            lab_date: date,
            document_id: doc_id,
            detected_at: chrono::Local::now().naive_local().to_string(),
            dismissed: false,
        });
    }

    Ok(alerts)
}

/// Handle critical alert dismissal (2-step process).
pub fn dismiss_critical_alert(
    conn: &Connection,
    request: &CriticalDismissRequest,
) -> Result<(), TrustError> {
    match &request.step {
        DismissStep::AskConfirmation => {
            // Step 1: Validate the alert exists and is critical
            let exists: bool = conn.query_row(
                "SELECT COUNT(*) > 0 FROM lab_results
                 WHERE id = ?1 AND abnormal_flag IN ('critical_low', 'critical_high')",
                params![request.alert_id],
                |row| row.get(0),
            )?;
            if !exists {
                return Err(TrustError::NotFound("Critical alert not found".into()));
            }
            Ok(())
        }
        DismissStep::ConfirmDismissal { reason } => {
            if reason.is_empty() {
                return Err(TrustError::Validation(
                    "Reason required to dismiss critical alert".into(),
                ));
            }

            // Verify alert exists
            let exists: bool = conn.query_row(
                "SELECT COUNT(*) > 0 FROM lab_results
                 WHERE id = ?1 AND abnormal_flag IN ('critical_low', 'critical_high')",
                params![request.alert_id],
                |row| row.get(0),
            )?;
            if !exists {
                return Err(TrustError::NotFound("Critical alert not found".into()));
            }

            // Store dismissal record
            let dismiss_id = Uuid::new_v4().to_string();
            let entity_ids_json =
                serde_json::to_string(&vec![&request.alert_id])?;

            conn.execute(
                "INSERT INTO dismissed_alerts
                 (id, alert_type, entity_ids, dismissed_date, reason, dismissed_by)
                 VALUES (?1, 'critical', ?2, datetime('now'), ?3, 'patient')",
                params![dismiss_id, entity_ids_json, reason],
            )?;

            tracing::info!(
                alert_id = %request.alert_id,
                "Critical alert dismissed with 2-step confirmation"
            );

            Ok(())
        }
    }
}

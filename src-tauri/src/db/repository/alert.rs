use std::str::FromStr;

use chrono::NaiveDateTime;
use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::db::DatabaseError;
use crate::intelligence::types::{AlertDetail, AlertSeverity, CoherenceAlert};
use crate::models::*;
use crate::models::enums::*;

pub fn get_dismissed_alerts(conn: &Connection) -> Result<Vec<DismissedAlert>, DatabaseError> {
    let mut stmt = conn.prepare(
        "SELECT id, alert_type, entity_ids, dismissed_date, reason, dismissed_by
         FROM dismissed_alerts",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, Option<String>>(4)?,
            row.get::<_, String>(5)?,
        ))
    })?;

    let mut alerts = Vec::new();
    for row in rows {
        let (id, alert_type, entity_ids, dismissed_date, reason, dismissed_by) = row?;
        alerts.push(DismissedAlert {
            id: Uuid::parse_str(&id)
                .map_err(|e| DatabaseError::ConstraintViolation(e.to_string()))?,
            alert_type: AlertType::from_str(&alert_type)?,
            entity_ids,
            dismissed_date: NaiveDateTime::parse_from_str(&dismissed_date, "%Y-%m-%d %H:%M:%S")
                .unwrap_or_default(),
            reason,
            dismissed_by: DismissedBy::from_str(&dismissed_by)?,
        });
    }
    Ok(alerts)
}

/// Build dismissed alert keys set from dismissed_alerts table.
pub fn get_dismissed_alert_keys(conn: &Connection) -> Result<std::collections::HashSet<(String, String)>, DatabaseError> {
    let alerts = get_dismissed_alerts(conn)?;
    let mut keys = std::collections::HashSet::new();
    for alert in alerts {
        keys.insert((alert.alert_type.as_str().to_string(), alert.entity_ids));
    }
    Ok(keys)
}

/// Insert a coherence alert into the database.
pub fn insert_coherence_alert(
    conn: &Connection,
    alert: &CoherenceAlert,
) -> Result<(), DatabaseError> {
    let entity_ids_json =
        serde_json::to_string(&alert.entity_ids).unwrap_or_else(|_| "[]".to_string());
    let source_doc_ids_json =
        serde_json::to_string(&alert.source_document_ids).unwrap_or_else(|_| "[]".to_string());
    let detail_json =
        serde_json::to_string(&alert.detail).unwrap_or_else(|_| "{}".to_string());

    let (dismissed_date, dismiss_reason, dismissed_by, two_step) =
        match &alert.dismissal {
            Some(d) => (
                Some(d.dismissed_date.format("%Y-%m-%d %H:%M:%S").to_string()),
                Some(d.reason.clone()),
                Some(d.dismissed_by.as_str().to_string()),
                d.two_step_confirmed,
            ),
            None => (None, None, None, false),
        };

    conn.execute(
        "INSERT OR REPLACE INTO coherence_alerts
         (id, alert_type, severity, entity_ids, source_document_ids,
          patient_message, detail_json, detected_at, surfaced, dismissed,
          dismissed_date, dismiss_reason, dismissed_by, two_step_confirmed)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
        params![
            alert.id.to_string(),
            alert.alert_type.as_str(),
            alert.severity.as_str(),
            entity_ids_json,
            source_doc_ids_json,
            alert.patient_message,
            detail_json,
            alert.detected_at.format("%Y-%m-%d %H:%M:%S").to_string(),
            alert.surfaced as i32,
            alert.dismissed as i32,
            dismissed_date,
            dismiss_reason,
            dismissed_by,
            two_step as i32,
        ],
    )?;
    Ok(())
}

/// Load all non-dismissed coherence alerts from the database.
pub fn load_active_coherence_alerts(
    conn: &Connection,
) -> Result<Vec<CoherenceAlert>, DatabaseError> {
    let mut stmt = conn.prepare(
        "SELECT id, alert_type, severity, entity_ids, source_document_ids,
                patient_message, detail_json, detected_at, surfaced, dismissed,
                dismissed_date, dismiss_reason, dismissed_by, two_step_confirmed
         FROM coherence_alerts WHERE dismissed = 0",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, String>(5)?,
            row.get::<_, String>(6)?,
            row.get::<_, String>(7)?,
            row.get::<_, i32>(8)?,
            row.get::<_, i32>(9)?,
        ))
    })?;

    let mut alerts = Vec::new();
    for row in rows {
        let (
            id_str, alert_type_str, severity_str, entity_ids_json,
            source_doc_ids_json, patient_message, detail_json,
            detected_at_str, surfaced, dismissed,
        ) = row?;

        let id = Uuid::parse_str(&id_str)
            .map_err(|e| DatabaseError::ConstraintViolation(e.to_string()))?;
        let alert_type = AlertType::from_str(&alert_type_str)?;
        let severity: AlertSeverity = severity_str
            .parse()
            .map_err(|e: String| DatabaseError::ConstraintViolation(e))?;
        let entity_ids: Vec<Uuid> = serde_json::from_str(&entity_ids_json)
            .unwrap_or_default();
        let source_document_ids: Vec<Uuid> =
            serde_json::from_str(&source_doc_ids_json).unwrap_or_default();
        let detail: AlertDetail = serde_json::from_str(&detail_json)
            .map_err(|e| DatabaseError::ConstraintViolation(format!("Invalid alert detail: {e}")))?;
        let detected_at =
            NaiveDateTime::parse_from_str(&detected_at_str, "%Y-%m-%d %H:%M:%S")
                .unwrap_or_default();

        alerts.push(CoherenceAlert {
            id,
            alert_type,
            severity,
            entity_ids,
            source_document_ids,
            patient_message,
            detail,
            detected_at,
            surfaced: surfaced != 0,
            dismissed: dismissed != 0,
            dismissal: None,
        });
    }
    Ok(alerts)
}

/// Mark a coherence alert as dismissed in the database.
pub fn dismiss_coherence_alert(
    conn: &Connection,
    alert_id: &Uuid,
    reason: &str,
    dismissed_by: &DismissedBy,
    two_step_confirmed: bool,
) -> Result<(), DatabaseError> {
    let now = chrono::Local::now().naive_local();
    conn.execute(
        "UPDATE coherence_alerts SET dismissed = 1, dismissed_date = ?1,
         dismiss_reason = ?2, dismissed_by = ?3, two_step_confirmed = ?4
         WHERE id = ?5",
        params![
            now.format("%Y-%m-%d %H:%M:%S").to_string(),
            reason,
            dismissed_by.as_str(),
            two_step_confirmed as i32,
            alert_id.to_string(),
        ],
    )?;
    Ok(())
}

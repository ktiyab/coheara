use rusqlite::{params, Connection};

use crate::db::DatabaseError;
use crate::pipeline::structuring::preferences::{
    ModelQuality, PreferenceSource, StoredModelPreference,
};

/// Get the stored model preference (singleton row, id=1).
pub fn get_model_preference(conn: &Connection) -> Result<StoredModelPreference, DatabaseError> {
    let mut stmt = conn.prepare(
        "SELECT active_model, model_quality, set_at, set_by FROM model_preferences WHERE id = 1",
    )?;
    stmt.query_row([], |row| {
        Ok(StoredModelPreference {
            active_model: row.get(0)?,
            model_quality: row
                .get::<_, String>(1)?
                .parse()
                .unwrap_or(ModelQuality::Unknown),
            set_at: row.get(2)?,
            set_by: row
                .get::<_, String>(3)?
                .parse()
                .unwrap_or(PreferenceSource::User),
        })
    })
    .map_err(DatabaseError::from)
}

/// Set the active model preference.
///
/// SEC-L6-13: Model name must be validated BEFORE calling this function.
pub fn set_model_preference(
    conn: &Connection,
    model_name: &str,
    quality: &ModelQuality,
    source: &PreferenceSource,
) -> Result<(), DatabaseError> {
    conn.execute(
        "UPDATE model_preferences SET
         active_model = ?1,
         model_quality = ?2,
         set_at = datetime('now'),
         set_by = ?3
         WHERE id = 1",
        params![model_name, quality.to_string(), source.to_string()],
    )?;
    Ok(())
}

/// Clear the active model preference (revert to fallback resolution).
pub fn clear_model_preference(conn: &Connection) -> Result<(), DatabaseError> {
    conn.execute(
        "UPDATE model_preferences SET
         active_model = NULL,
         model_quality = 'unknown',
         set_at = datetime('now'),
         set_by = 'user'
         WHERE id = 1",
        [],
    )?;
    Ok(())
}

// ──────────────────────────────────────────────
// R3: OCR model preference (role-based)
// ──────────────────────────────────────────────

/// Get the stored OCR model preference.
///
/// R3: Separate from `active_model` — enables different models for
/// text generation (MedGemma) vs vision OCR (DeepSeek-OCR).
/// Returns `None` if no explicit OCR model is set (uses fallback chain).
pub fn get_ocr_model_preference(conn: &Connection) -> Result<Option<String>, DatabaseError> {
    let mut stmt =
        conn.prepare("SELECT active_ocr_model FROM model_preferences WHERE id = 1")?;
    match stmt.query_row([], |row| row.get::<_, Option<String>>(0)) {
        Ok(val) => Ok(val),
        Err(e) => Err(DatabaseError::from(e)),
    }
}

/// Set the active OCR model preference.
///
/// R3: SEC-L6-13: Model name must be validated BEFORE calling this function.
pub fn set_ocr_model_preference(
    conn: &Connection,
    model_name: &str,
) -> Result<(), DatabaseError> {
    conn.execute(
        "UPDATE model_preferences SET active_ocr_model = ?1 WHERE id = 1",
        params![model_name],
    )?;
    Ok(())
}

/// Clear the OCR model preference (revert to fallback chain).
pub fn clear_ocr_model_preference(conn: &Connection) -> Result<(), DatabaseError> {
    conn.execute(
        "UPDATE model_preferences SET active_ocr_model = NULL WHERE id = 1",
        [],
    )?;
    Ok(())
}

/// Get a user preference by key. Returns None if not set.
///
/// SEC-L6-16: Key validation happens at the IPC layer, not here.
pub fn get_user_preference(
    conn: &Connection,
    key: &str,
) -> Result<Option<String>, DatabaseError> {
    let mut stmt = conn.prepare("SELECT value FROM user_preferences WHERE key = ?1")?;
    match stmt.query_row([key], |row| row.get::<_, String>(0)) {
        Ok(val) => Ok(Some(val)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(DatabaseError::from(e)),
    }
}

/// Set a user preference (upsert).
pub fn set_user_preference(
    conn: &Connection,
    key: &str,
    value: &str,
) -> Result<(), DatabaseError> {
    conn.execute(
        "INSERT INTO user_preferences (key, value, updated_at)
         VALUES (?1, ?2, datetime('now'))
         ON CONFLICT(key) DO UPDATE SET value = ?2, updated_at = datetime('now')",
        params![key, value],
    )?;
    Ok(())
}

/// Delete a user preference.
pub fn delete_user_preference(conn: &Connection, key: &str) -> Result<(), DatabaseError> {
    conn.execute("DELETE FROM user_preferences WHERE key = ?1", [key])?;
    Ok(())
}

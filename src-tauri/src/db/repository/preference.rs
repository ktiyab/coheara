use std::collections::HashMap;

use rusqlite::{params, Connection};

use crate::db::DatabaseError;
use crate::pipeline::structuring::ollama_types::CapabilityTag;
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
// OCR model preference (role-based, modular)
// ──────────────────────────────────────────────

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

// ──────────────────────────────────────────────
// CT-01: Model capability tags
// ──────────────────────────────────────────────

/// Get all capability tags for a model.
pub fn get_model_tags(
    conn: &Connection,
    model_name: &str,
) -> Result<Vec<CapabilityTag>, DatabaseError> {
    let mut stmt = conn.prepare(
        "SELECT tag FROM model_capability_tags WHERE model_name = ?1 ORDER BY tag",
    )?;
    let tags = stmt
        .query_map([model_name], |row| row.get::<_, String>(0))?
        .filter_map(|r| r.ok())
        .filter_map(|s| s.parse::<CapabilityTag>().ok())
        .collect();
    Ok(tags)
}

/// Replace all tags for a model (transactional).
pub fn set_model_tags(
    conn: &Connection,
    model_name: &str,
    tags: &[CapabilityTag],
) -> Result<(), DatabaseError> {
    conn.execute(
        "DELETE FROM model_capability_tags WHERE model_name = ?1",
        [model_name],
    )?;
    for tag in tags {
        conn.execute(
            "INSERT INTO model_capability_tags (model_name, tag) VALUES (?1, ?2)",
            params![model_name, tag.as_str()],
        )?;
    }
    Ok(())
}

/// Add a single tag to a model (idempotent).
pub fn add_model_tag(
    conn: &Connection,
    model_name: &str,
    tag: &CapabilityTag,
) -> Result<(), DatabaseError> {
    conn.execute(
        "INSERT OR IGNORE INTO model_capability_tags (model_name, tag) VALUES (?1, ?2)",
        params![model_name, tag.as_str()],
    )?;
    Ok(())
}

/// Remove a single tag from a model.
pub fn remove_model_tag(
    conn: &Connection,
    model_name: &str,
    tag: &CapabilityTag,
) -> Result<(), DatabaseError> {
    conn.execute(
        "DELETE FROM model_capability_tags WHERE model_name = ?1 AND tag = ?2",
        params![model_name, tag.as_str()],
    )?;
    Ok(())
}

/// Delete all tags for a model (cleanup on model deletion).
pub fn delete_model_tags(conn: &Connection, model_name: &str) -> Result<(), DatabaseError> {
    conn.execute(
        "DELETE FROM model_capability_tags WHERE model_name = ?1",
        [model_name],
    )?;
    Ok(())
}

/// Get all model names that have a specific tag.
pub fn get_models_with_tag(
    conn: &Connection,
    tag: &CapabilityTag,
) -> Result<Vec<String>, DatabaseError> {
    let mut stmt = conn.prepare(
        "SELECT model_name FROM model_capability_tags WHERE tag = ?1 ORDER BY model_name",
    )?;
    let names = stmt
        .query_map([tag.as_str()], |row| row.get::<_, String>(0))?
        .filter_map(|r| r.ok())
        .collect();
    Ok(names)
}

/// Get all tags for all models (bulk load for frontend).
pub fn get_all_model_tags(
    conn: &Connection,
) -> Result<HashMap<String, Vec<CapabilityTag>>, DatabaseError> {
    let mut stmt = conn.prepare(
        "SELECT model_name, tag FROM model_capability_tags ORDER BY model_name, tag",
    )?;
    let mut map: HashMap<String, Vec<CapabilityTag>> = HashMap::new();
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    for row in rows {
        let (name, tag_str) = row?;
        if let Ok(tag) = tag_str.parse::<CapabilityTag>() {
            map.entry(name).or_default().push(tag);
        }
    }
    Ok(map)
}

// ──────────────────────────────────────────────
// CT-01: Model enabled/disabled flag
// ──────────────────────────────────────────────

/// Check if a model is enabled for pipeline use.
///
/// Missing row = enabled (default on). Only explicitly disabled models return false.
pub fn is_model_enabled(conn: &Connection, model_name: &str) -> Result<bool, DatabaseError> {
    let mut stmt =
        conn.prepare("SELECT enabled FROM model_enabled WHERE model_name = ?1")?;
    match stmt.query_row([model_name], |row| row.get::<_, i32>(0)) {
        Ok(val) => Ok(val != 0),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(true),
        Err(e) => Err(DatabaseError::from(e)),
    }
}

/// Set a model's enabled state (upsert).
pub fn set_model_enabled(
    conn: &Connection,
    model_name: &str,
    enabled: bool,
) -> Result<(), DatabaseError> {
    conn.execute(
        "INSERT INTO model_enabled (model_name, enabled, updated_at)
         VALUES (?1, ?2, datetime('now'))
         ON CONFLICT(model_name) DO UPDATE SET enabled = ?2, updated_at = datetime('now')",
        params![model_name, enabled as i32],
    )?;
    Ok(())
}

/// Get all explicitly disabled model names.
pub fn get_disabled_models(conn: &Connection) -> Result<Vec<String>, DatabaseError> {
    let mut stmt =
        conn.prepare("SELECT model_name FROM model_enabled WHERE enabled = 0 ORDER BY model_name")?;
    let names = stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .filter_map(|r| r.ok())
        .collect();
    Ok(names)
}

/// Delete enabled state for a model (cleanup on model deletion).
pub fn delete_model_enabled(conn: &Connection, model_name: &str) -> Result<(), DatabaseError> {
    conn.execute(
        "DELETE FROM model_enabled WHERE model_name = ?1",
        [model_name],
    )?;
    Ok(())
}

// ──────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::sqlite::open_memory_database;

    fn setup_db() -> Connection {
        open_memory_database().expect("in-memory DB should open")
    }

    #[test]
    fn get_tags_empty_for_unknown_model() {
        let conn = setup_db();
        let tags = get_model_tags(&conn, "nonexistent:latest").unwrap();
        assert!(tags.is_empty());
    }

    #[test]
    fn set_and_get_model_tags() {
        let conn = setup_db();
        let tags = vec![CapabilityTag::Vision, CapabilityTag::Medical, CapabilityTag::Png];
        set_model_tags(&conn, "medgemma:4b", &tags).unwrap();

        let result = get_model_tags(&conn, "medgemma:4b").unwrap();
        assert_eq!(result.len(), 3);
        assert!(result.contains(&CapabilityTag::Vision));
        assert!(result.contains(&CapabilityTag::Medical));
        assert!(result.contains(&CapabilityTag::Png));
    }

    #[test]
    fn set_tags_replaces_existing() {
        let conn = setup_db();
        set_model_tags(&conn, "model:v1", &[CapabilityTag::Vision, CapabilityTag::Txt]).unwrap();
        set_model_tags(&conn, "model:v1", &[CapabilityTag::Txt]).unwrap();

        let result = get_model_tags(&conn, "model:v1").unwrap();
        assert_eq!(result, vec![CapabilityTag::Txt]);
    }

    #[test]
    fn add_tag_is_idempotent() {
        let conn = setup_db();
        add_model_tag(&conn, "llama3:8b", &CapabilityTag::Txt).unwrap();
        add_model_tag(&conn, "llama3:8b", &CapabilityTag::Txt).unwrap();

        let result = get_model_tags(&conn, "llama3:8b").unwrap();
        assert_eq!(result, vec![CapabilityTag::Txt]);
    }

    #[test]
    fn remove_tag_works() {
        let conn = setup_db();
        set_model_tags(&conn, "model:v1", &[CapabilityTag::Vision, CapabilityTag::Txt]).unwrap();
        remove_model_tag(&conn, "model:v1", &CapabilityTag::Vision).unwrap();

        let result = get_model_tags(&conn, "model:v1").unwrap();
        assert_eq!(result, vec![CapabilityTag::Txt]);
    }

    #[test]
    fn delete_model_tags_cleans_all() {
        let conn = setup_db();
        set_model_tags(&conn, "model:v1", &[CapabilityTag::Vision, CapabilityTag::Medical]).unwrap();
        delete_model_tags(&conn, "model:v1").unwrap();

        let result = get_model_tags(&conn, "model:v1").unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn get_models_with_tag_finds_correct_models() {
        let conn = setup_db();
        set_model_tags(&conn, "medgemma:4b", &[CapabilityTag::Vision, CapabilityTag::Medical]).unwrap();
        set_model_tags(&conn, "llama3:8b", &[CapabilityTag::Txt]).unwrap();
        set_model_tags(&conn, "llava:13b", &[CapabilityTag::Vision, CapabilityTag::Png]).unwrap();

        let vision_models = get_models_with_tag(&conn, &CapabilityTag::Vision).unwrap();
        assert_eq!(vision_models.len(), 2);
        assert!(vision_models.contains(&"medgemma:4b".to_string()));
        assert!(vision_models.contains(&"llava:13b".to_string()));

        let txt_models = get_models_with_tag(&conn, &CapabilityTag::Txt).unwrap();
        assert_eq!(txt_models, vec!["llama3:8b".to_string()]);
    }

    #[test]
    fn get_all_model_tags_bulk_load() {
        let conn = setup_db();
        set_model_tags(&conn, "medgemma:4b", &[CapabilityTag::Vision, CapabilityTag::Medical]).unwrap();
        set_model_tags(&conn, "llama3:8b", &[CapabilityTag::Txt]).unwrap();

        let all = get_all_model_tags(&conn).unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all["medgemma:4b"].len(), 2);
        assert_eq!(all["llama3:8b"], vec![CapabilityTag::Txt]);
    }

    // ── Model enabled/disabled tests ──────────────────────────

    #[test]
    fn unknown_model_is_enabled_by_default() {
        let conn = setup_db();
        assert!(is_model_enabled(&conn, "unknown:latest").unwrap());
    }

    #[test]
    fn disable_and_reenable_model() {
        let conn = setup_db();
        set_model_enabled(&conn, "llava:13b", false).unwrap();
        assert!(!is_model_enabled(&conn, "llava:13b").unwrap());

        set_model_enabled(&conn, "llava:13b", true).unwrap();
        assert!(is_model_enabled(&conn, "llava:13b").unwrap());
    }

    #[test]
    fn get_disabled_models_returns_only_disabled() {
        let conn = setup_db();
        set_model_enabled(&conn, "llava:13b", false).unwrap();
        set_model_enabled(&conn, "medgemma:4b", true).unwrap();
        set_model_enabled(&conn, "llama3:8b", false).unwrap();

        let disabled = get_disabled_models(&conn).unwrap();
        assert_eq!(disabled, vec!["llama3:8b", "llava:13b"]);
    }

    #[test]
    fn delete_model_enabled_cleans_up() {
        let conn = setup_db();
        set_model_enabled(&conn, "llava:13b", false).unwrap();
        assert!(!is_model_enabled(&conn, "llava:13b").unwrap());

        delete_model_enabled(&conn, "llava:13b").unwrap();
        // After deletion, defaults back to enabled
        assert!(is_model_enabled(&conn, "llava:13b").unwrap());
    }
}

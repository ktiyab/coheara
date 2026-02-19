use chrono::NaiveDateTime;
use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::db::DatabaseError;
use crate::models::{CachedExplanation, ExplanationEntityType};

/// Insert or replace a cached explanation.
pub fn upsert_cached_explanation(
    conn: &Connection,
    ce: &CachedExplanation,
) -> Result<(), DatabaseError> {
    conn.execute(
        "INSERT INTO cached_explanations (id, entity_type, entity_id, explanation_text, language, model_version, created_at, invalidated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT(entity_type, entity_id, language) DO UPDATE SET
           explanation_text = excluded.explanation_text,
           model_version = excluded.model_version,
           created_at = excluded.created_at,
           invalidated_at = NULL",
        params![
            ce.id.to_string(),
            ce.entity_type.as_str(),
            ce.entity_id.to_string(),
            ce.explanation_text,
            ce.language,
            ce.model_version,
            ce.created_at.format("%Y-%m-%d %H:%M:%S").to_string(),
            ce.invalidated_at.map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string()),
        ],
    )?;
    Ok(())
}

/// Get a valid (non-invalidated) cached explanation for an entity.
pub fn get_cached_explanation(
    conn: &Connection,
    entity_type: &ExplanationEntityType,
    entity_id: &Uuid,
    language: &str,
) -> Result<Option<CachedExplanation>, DatabaseError> {
    let mut stmt = conn.prepare(
        "SELECT id, entity_type, entity_id, explanation_text, language, model_version, created_at, invalidated_at
         FROM cached_explanations
         WHERE entity_type = ?1 AND entity_id = ?2 AND language = ?3 AND invalidated_at IS NULL
         LIMIT 1",
    )?;
    let mut rows = stmt.query_map(
        params![entity_type.as_str(), entity_id.to_string(), language],
        row_to_cached_explanation,
    )?;
    match rows.next() {
        Some(row) => Ok(Some(row?)),
        None => Ok(None),
    }
}

/// Invalidate all cached explanations for an entity (e.g., when data changes).
pub fn invalidate_cached_explanations(
    conn: &Connection,
    entity_type: &ExplanationEntityType,
    entity_id: &Uuid,
) -> Result<u64, DatabaseError> {
    let affected = conn.execute(
        "UPDATE cached_explanations SET invalidated_at = datetime('now')
         WHERE entity_type = ?1 AND entity_id = ?2 AND invalidated_at IS NULL",
        params![entity_type.as_str(), entity_id.to_string()],
    )?;
    Ok(affected as u64)
}

fn row_to_cached_explanation(row: &rusqlite::Row) -> Result<CachedExplanation, rusqlite::Error> {
    let id_str: String = row.get(0)?;
    let type_str: String = row.get(1)?;
    let entity_str: String = row.get(2)?;
    let created_str: String = row.get(6)?;
    let invalidated_str: Option<String> = row.get(7)?;

    Ok(CachedExplanation {
        id: Uuid::parse_str(&id_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?,
        entity_type: ExplanationEntityType::from_str(&type_str)
            .unwrap_or(ExplanationEntityType::Document),
        entity_id: Uuid::parse_str(&entity_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(2, rusqlite::types::Type::Text, Box::new(e))
        })?,
        explanation_text: row.get(3)?,
        language: row.get(4)?,
        model_version: row.get(5)?,
        created_at: NaiveDateTime::parse_from_str(&created_str, "%Y-%m-%d %H:%M:%S")
            .unwrap_or_default(),
        invalidated_at: invalidated_str
            .and_then(|s| NaiveDateTime::parse_from_str(&s, "%Y-%m-%d %H:%M:%S").ok()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::sqlite::open_memory_database;

    fn test_db() -> Connection {
        open_memory_database().unwrap()
    }

    fn make_explanation(entity_type: ExplanationEntityType, entity_id: Uuid) -> CachedExplanation {
        CachedExplanation {
            id: Uuid::new_v4(),
            entity_type,
            entity_id,
            explanation_text: "This lab result shows normal potassium levels.".into(),
            language: "en".into(),
            model_version: Some("medgemma-4b-v1.5".into()),
            created_at: chrono::Local::now().naive_local(),
            invalidated_at: None,
        }
    }

    #[test]
    fn insert_and_retrieve() {
        let conn = test_db();
        let entity_id = Uuid::new_v4();
        let ce = make_explanation(ExplanationEntityType::LabResult, entity_id);
        upsert_cached_explanation(&conn, &ce).unwrap();

        let result = get_cached_explanation(
            &conn,
            &ExplanationEntityType::LabResult,
            &entity_id,
            "en",
        )
        .unwrap();
        assert!(result.is_some());
        assert_eq!(
            result.unwrap().explanation_text,
            "This lab result shows normal potassium levels."
        );
    }

    #[test]
    fn upsert_replaces_existing() {
        let conn = test_db();
        let entity_id = Uuid::new_v4();
        let mut ce = make_explanation(ExplanationEntityType::Medication, entity_id);
        upsert_cached_explanation(&conn, &ce).unwrap();

        ce.id = Uuid::new_v4();
        ce.explanation_text = "Updated explanation.".into();
        upsert_cached_explanation(&conn, &ce).unwrap();

        let result = get_cached_explanation(
            &conn,
            &ExplanationEntityType::Medication,
            &entity_id,
            "en",
        )
        .unwrap()
        .unwrap();
        assert_eq!(result.explanation_text, "Updated explanation.");
    }

    #[test]
    fn invalidation_hides_from_retrieval() {
        let conn = test_db();
        let entity_id = Uuid::new_v4();
        let ce = make_explanation(ExplanationEntityType::Diagnosis, entity_id);
        upsert_cached_explanation(&conn, &ce).unwrap();

        let affected =
            invalidate_cached_explanations(&conn, &ExplanationEntityType::Diagnosis, &entity_id)
                .unwrap();
        assert_eq!(affected, 1);

        let result = get_cached_explanation(
            &conn,
            &ExplanationEntityType::Diagnosis,
            &entity_id,
            "en",
        )
        .unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn missing_returns_none() {
        let conn = test_db();
        let result = get_cached_explanation(
            &conn,
            &ExplanationEntityType::LabResult,
            &Uuid::new_v4(),
            "en",
        )
        .unwrap();
        assert!(result.is_none());
    }
}

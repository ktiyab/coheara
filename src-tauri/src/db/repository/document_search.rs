use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::db::DatabaseError;

/// A document search result from FTS5 (Spec 46: Caregiver).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DocumentSearchResult {
    pub document_id: Uuid,
    pub title: String,
    pub professional_name: Option<String>,
    pub snippet: String,
    pub rank: f64,
}

/// Index a document in the FTS5 search table.
pub fn index_document_for_search(
    conn: &Connection,
    doc_id: &Uuid,
    title: &str,
    professional_name: Option<&str>,
    content_summary: Option<&str>,
) -> Result<(), DatabaseError> {
    // Use rowid from documents table for content sync
    let rowid: Option<i64> = conn
        .query_row(
            "SELECT rowid FROM documents WHERE id = ?1",
            params![doc_id.to_string()],
            |row| row.get(0),
        )
        .ok();

    let rowid = match rowid {
        Some(r) => r,
        None => {
            return Err(DatabaseError::NotFound {
                entity_type: "document".into(),
                id: doc_id.to_string(),
            })
        }
    };

    // Delete existing entry if any, then insert
    conn.execute(
        "DELETE FROM documents_fts WHERE rowid = ?1",
        params![rowid],
    )?;
    conn.execute(
        "INSERT INTO documents_fts(rowid, title, professional_name, content_summary)
         VALUES (?1, ?2, ?3, ?4)",
        params![
            rowid,
            title,
            professional_name.unwrap_or(""),
            content_summary.unwrap_or(""),
        ],
    )?;
    Ok(())
}

/// Search documents using FTS5 full-text search.
///
/// Returns results ranked by relevance (BM25).
/// Optionally filter by document type.
pub fn search_documents_fts(
    conn: &Connection,
    query: &str,
    doc_type_filter: Option<&str>,
    limit: usize,
) -> Result<Vec<DocumentSearchResult>, DatabaseError> {
    if query.trim().is_empty() {
        return Ok(Vec::new());
    }

    // Sanitize query for FTS5: escape special characters
    let sanitized = sanitize_fts_query(query);

    let sql = if doc_type_filter.is_some() {
        "SELECT d.id, d.title, p.name, snippet(documents_fts, 2, '<b>', '</b>', '...', 32), rank
         FROM documents_fts
         JOIN documents d ON d.rowid = documents_fts.rowid
         LEFT JOIN professionals p ON d.professional_id = p.id
         WHERE documents_fts MATCH ?1 AND d.type = ?2
         ORDER BY rank
         LIMIT ?3"
    } else {
        "SELECT d.id, d.title, p.name, snippet(documents_fts, 2, '<b>', '</b>', '...', 32), rank
         FROM documents_fts
         JOIN documents d ON d.rowid = documents_fts.rowid
         LEFT JOIN professionals p ON d.professional_id = p.id
         WHERE documents_fts MATCH ?1
         ORDER BY rank
         LIMIT ?2"
    };

    let mut stmt = conn.prepare(sql)?;

    let rows = if let Some(dtype) = doc_type_filter {
        stmt.query_map(params![sanitized, dtype, limit as i64], row_to_search_result)?
    } else {
        stmt.query_map(params![sanitized, limit as i64], row_to_search_result)?
    };

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(DatabaseError::from)
}

/// Remove a document from the FTS5 index.
pub fn remove_document_from_search(
    conn: &Connection,
    doc_id: &Uuid,
) -> Result<(), DatabaseError> {
    let rowid: Option<i64> = conn
        .query_row(
            "SELECT rowid FROM documents WHERE id = ?1",
            params![doc_id.to_string()],
            |row| row.get(0),
        )
        .ok();

    if let Some(rowid) = rowid {
        conn.execute(
            "DELETE FROM documents_fts WHERE rowid = ?1",
            params![rowid],
        )?;
    }
    Ok(())
}

/// Sanitize a search query for FTS5.
/// Escapes special characters and wraps terms for prefix matching.
fn sanitize_fts_query(query: &str) -> String {
    // Remove FTS5 operators that could cause syntax errors
    let cleaned: String = query
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace() || *c == '-' || *c == '\'')
        .collect();

    // Split into terms and add prefix matching with *
    cleaned
        .split_whitespace()
        .filter(|w| !w.is_empty())
        .map(|w| format!("\"{w}\"*"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn row_to_search_result(row: &rusqlite::Row) -> Result<DocumentSearchResult, rusqlite::Error> {
    let id_str: String = row.get(0)?;
    Ok(DocumentSearchResult {
        document_id: Uuid::parse_str(&id_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?,
        title: row.get(1)?,
        professional_name: row.get(2)?,
        snippet: row.get(3)?,
        rank: row.get(4)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_fts_removes_operators() {
        assert_eq!(sanitize_fts_query("test AND query"), "\"test\"* \"AND\"* \"query\"*");
        assert_eq!(sanitize_fts_query("lab results"), "\"lab\"* \"results\"*");
    }

    #[test]
    fn sanitize_fts_handles_empty() {
        assert_eq!(sanitize_fts_query(""), "");
        assert_eq!(sanitize_fts_query("   "), "");
    }

    #[test]
    fn sanitize_fts_strips_special_chars() {
        assert_eq!(sanitize_fts_query("test(query)"), "\"testquery\"*");
        assert_eq!(sanitize_fts_query("\"quoted\""), "\"quoted\"*");
    }
}

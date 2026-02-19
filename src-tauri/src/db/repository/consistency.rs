use rusqlite::Connection;

use crate::db::DatabaseError;
use super::profile_trust::recalculate_profile_trust;

/// A single consistency issue detected by the checker.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ConsistencyIssue {
    pub category: String,
    pub severity: String,
    pub description: String,
    pub document_id: Option<String>,
}

/// Result of a consistency check across all data integrity dimensions.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ConsistencyReport {
    pub issues: Vec<ConsistencyIssue>,
    pub documents_checked: i64,
    pub trust_drift_detected: bool,
}

/// Run a full consistency check across the database.
///
/// Detects:
/// - Documents stuck in transient pipeline states (Extracting/Structuring)
/// - Documents marked confirmed/verified but missing entities and vector chunks
/// - Orphaned vector chunks referencing non-existent documents
/// - Trust count drift (profile_trust vs actual document counts)
pub fn check_consistency(conn: &Connection) -> Result<ConsistencyReport, DatabaseError> {
    let mut issues = Vec::new();

    // 1. Documents stuck in transient states (Extracting/Structuring)
    let stuck_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM documents WHERE pipeline_status IN ('extracting', 'structuring')",
        [],
        |row| row.get(0),
    )?;
    if stuck_count > 0 {
        let mut stmt = conn.prepare(
            "SELECT id, pipeline_status FROM documents
             WHERE pipeline_status IN ('extracting', 'structuring')"
        )?;
        let rows: Vec<(String, String)> = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?.filter_map(|r| r.ok()).collect();
        drop(stmt);

        for (id, status) in rows {
            issues.push(ConsistencyIssue {
                category: "stuck_pipeline".into(),
                severity: "high".into(),
                description: format!("Document stuck in '{status}' state"),
                document_id: Some(id),
            });
        }
    }

    // 2. Confirmed documents with no vector chunks (should have been stored)
    let confirmed_no_chunks: i64 = conn.query_row(
        "SELECT COUNT(*) FROM documents d
         WHERE d.pipeline_status = 'confirmed'
         AND NOT EXISTS (SELECT 1 FROM vector_chunks vc WHERE vc.document_id = d.id)",
        [],
        |row| row.get(0),
    )?;
    if confirmed_no_chunks > 0 {
        let mut stmt = conn.prepare(
            "SELECT d.id FROM documents d
             WHERE d.pipeline_status = 'confirmed'
             AND NOT EXISTS (SELECT 1 FROM vector_chunks vc WHERE vc.document_id = d.id)"
        )?;
        let ids: Vec<String> = stmt.query_map([], |row| {
            row.get::<_, String>(0)
        })?.filter_map(|r| r.ok()).collect();
        drop(stmt);

        for id in ids {
            issues.push(ConsistencyIssue {
                category: "missing_chunks".into(),
                severity: "medium".into(),
                description: "Confirmed document has no vector chunks".into(),
                document_id: Some(id),
            });
        }
    }

    // 3. Orphaned vector chunks (document_id not in documents)
    let orphaned_chunks: i64 = conn.query_row(
        "SELECT COUNT(*) FROM vector_chunks vc
         WHERE NOT EXISTS (SELECT 1 FROM documents d WHERE d.id = vc.document_id)",
        [],
        |row| row.get(0),
    )?;
    if orphaned_chunks > 0 {
        issues.push(ConsistencyIssue {
            category: "orphaned_chunks".into(),
            severity: "low".into(),
            description: format!("{orphaned_chunks} orphaned vector chunks without documents"),
            document_id: None,
        });
    }

    // 4. Trust count drift
    let actual_total: i64 = conn.query_row(
        "SELECT COUNT(*) FROM documents", [], |row| row.get(0),
    )?;
    let actual_verified: i64 = conn.query_row(
        "SELECT COUNT(*) FROM documents WHERE verified = 1", [], |row| row.get(0),
    )?;
    let (stored_total, stored_verified): (i64, i64) = conn.query_row(
        "SELECT total_documents, documents_verified FROM profile_trust WHERE id = 1",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;

    let trust_drift = actual_total != stored_total || actual_verified != stored_verified;
    if trust_drift {
        issues.push(ConsistencyIssue {
            category: "trust_drift".into(),
            severity: "medium".into(),
            description: format!(
                "Trust counts drifted: stored total={stored_total}/verified={stored_verified}, \
                 actual total={actual_total}/verified={actual_verified}"
            ),
            document_id: None,
        });
    }

    let documents_checked = actual_total;

    Ok(ConsistencyReport {
        issues,
        documents_checked,
        trust_drift_detected: trust_drift,
    })
}

/// Auto-repair consistency issues that can be safely fixed.
///
/// Currently repairs:
/// - Trust count drift -> recalculates from actual data
/// - Stuck pipeline states -> resets to Failed
///
/// Returns the number of issues repaired.
pub fn repair_consistency(conn: &Connection) -> Result<usize, DatabaseError> {
    let mut repaired = 0;

    // Repair trust drift
    let actual_total: i64 = conn.query_row(
        "SELECT COUNT(*) FROM documents", [], |row| row.get(0),
    )?;
    let actual_verified: i64 = conn.query_row(
        "SELECT COUNT(*) FROM documents WHERE verified = 1", [], |row| row.get(0),
    )?;
    let (stored_total, stored_verified): (i64, i64) = conn.query_row(
        "SELECT total_documents, documents_verified FROM profile_trust WHERE id = 1",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;

    if actual_total != stored_total || actual_verified != stored_verified {
        recalculate_profile_trust(conn)?;
        tracing::info!(
            actual_total, actual_verified, stored_total, stored_verified,
            "Repaired trust count drift"
        );
        repaired += 1;
    }

    // Repair stuck pipeline states -> Failed
    let stuck_fixed = conn.execute(
        "UPDATE documents SET pipeline_status = 'failed'
         WHERE pipeline_status IN ('extracting', 'structuring')",
        [],
    )?;
    if stuck_fixed > 0 {
        tracing::info!(count = stuck_fixed, "Repaired stuck pipeline documents -> Failed");
        repaired += stuck_fixed;
    }

    Ok(repaired)
}

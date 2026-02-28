//! BTL-10 C9: Rule-based connection extraction between entities in a document.
//!
//! After entities are stored, this module analyzes relationships using
//! simple text matching (no LLM needed). Connections are written to the
//! `entity_connections` table.
//!
//! Rules:
//! - Medication with `reason` field → match against diagnosis names → PrescribedFor
//! - Lab result with abnormal flag → match test name against diagnosis keywords → EvidencesFor
//! - Referral with `reason` → match against diagnosis names → FollowUpTo
//! - Medication name → check against allergy allergen → ContraindicatedBy

use rusqlite::Connection;
use uuid::Uuid;

use crate::db::repository;
use crate::models::entity_connection::{EntityConnection, EntityType, RelationshipType};

use super::StorageError;

/// An entity reference collected during storage, used for matching.
#[derive(Debug, Clone)]
pub struct StoredEntity {
    pub entity_type: EntityType,
    pub id: Uuid,
    /// Primary identifier for matching (e.g., medication name, diagnosis name, allergen).
    pub name: String,
    /// Optional secondary text for matching (e.g., medication reason, referral reason).
    pub reason: Option<String>,
    /// Whether this lab result has an abnormal flag.
    pub is_abnormal: bool,
}

/// Extract and store connections between entities in a document.
///
/// Returns the number of connections created.
pub fn extract_connections(
    conn: &Connection,
    document_id: &Uuid,
    entities: &[StoredEntity],
) -> Result<usize, StorageError> {
    // Clear any existing connections for idempotent reprocessing.
    repository::delete_connections_for_document(conn, document_id)?;

    let medications: Vec<&StoredEntity> = entities
        .iter()
        .filter(|e| e.entity_type == EntityType::Medication)
        .collect();
    let diagnoses: Vec<&StoredEntity> = entities
        .iter()
        .filter(|e| e.entity_type == EntityType::Diagnosis)
        .collect();
    let allergies: Vec<&StoredEntity> = entities
        .iter()
        .filter(|e| e.entity_type == EntityType::Allergy)
        .collect();
    let labs: Vec<&StoredEntity> = entities
        .iter()
        .filter(|e| e.entity_type == EntityType::LabResult)
        .collect();
    let referrals: Vec<&StoredEntity> = entities
        .iter()
        .filter(|e| e.entity_type == EntityType::Referral)
        .collect();

    let mut count = 0;

    // Rule 1: Medication.reason → Diagnosis.name → PrescribedFor
    for med in &medications {
        if let Some(reason) = &med.reason {
            for diag in &diagnoses {
                if fuzzy_match(reason, &diag.name) {
                    insert_connection(
                        conn,
                        document_id,
                        med,
                        diag,
                        RelationshipType::PrescribedFor,
                        match_confidence(reason, &diag.name),
                    )?;
                    count += 1;
                }
            }
        }
    }

    // Rule 2: Abnormal lab result → Diagnosis → EvidencesFor
    for lab in &labs {
        if lab.is_abnormal {
            for diag in &diagnoses {
                if fuzzy_match(&lab.name, &diag.name) {
                    insert_connection(
                        conn,
                        document_id,
                        lab,
                        diag,
                        RelationshipType::EvidencesFor,
                        match_confidence(&lab.name, &diag.name),
                    )?;
                    count += 1;
                }
            }
        }
    }

    // Rule 3: Referral.reason → Diagnosis.name → FollowUpTo
    for referral in &referrals {
        if let Some(reason) = &referral.reason {
            for diag in &diagnoses {
                if fuzzy_match(reason, &diag.name) {
                    insert_connection(
                        conn,
                        document_id,
                        referral,
                        diag,
                        RelationshipType::FollowUpTo,
                        match_confidence(reason, &diag.name),
                    )?;
                    count += 1;
                }
            }
        }
    }

    // Rule 4: Medication name → Allergy allergen → ContraindicatedBy
    for med in &medications {
        for allergy in &allergies {
            if fuzzy_match(&med.name, &allergy.name) {
                insert_connection(
                    conn,
                    document_id,
                    med,
                    allergy,
                    RelationshipType::ContraindicatedBy,
                    match_confidence(&med.name, &allergy.name),
                )?;
                count += 1;
            }
        }
    }

    Ok(count)
}

fn insert_connection(
    conn: &Connection,
    document_id: &Uuid,
    source: &StoredEntity,
    target: &StoredEntity,
    relationship: RelationshipType,
    confidence: f64,
) -> Result<(), StorageError> {
    let connection = EntityConnection {
        id: Uuid::new_v4(),
        source_type: source.entity_type.clone(),
        source_id: source.id,
        target_type: target.entity_type.clone(),
        target_id: target.id,
        relationship_type: relationship,
        confidence,
        document_id: *document_id,
        created_at: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
    };
    repository::insert_entity_connection(conn, &connection)?;
    Ok(())
}

/// Case-insensitive substring match between two text fields.
fn fuzzy_match(text: &str, target: &str) -> bool {
    let text_lower = text.to_lowercase();
    let target_lower = target.to_lowercase();

    // Exact match
    if text_lower == target_lower {
        return true;
    }

    // Substring: target contained in text, or text contained in target
    // Only if the shorter string is at least 4 characters (avoid false positives).
    let shorter = if text_lower.len() < target_lower.len() {
        &text_lower
    } else {
        &target_lower
    };

    if shorter.len() >= 4 {
        text_lower.contains(&target_lower) || target_lower.contains(&text_lower)
    } else {
        false
    }
}

/// Returns 1.0 for exact matches, 0.7 for fuzzy/substring matches.
fn match_confidence(text: &str, target: &str) -> f64 {
    if text.to_lowercase() == target.to_lowercase() {
        1.0
    } else {
        0.7
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::sqlite::open_memory_database;
    use crate::models::*;
    use crate::models::enums::*;

    fn test_db() -> Connection {
        open_memory_database().unwrap()
    }

    fn make_document(conn: &Connection) -> Uuid {
        let id = Uuid::new_v4();
        repository::insert_document(
            conn,
            &Document {
                id,
                doc_type: DocumentType::LabResult,
                title: "Test".into(),
                document_date: None,
                ingestion_date: chrono::Local::now().naive_local(),
                professional_id: None,
                source_file: "/test.jpg".into(),
                markdown_file: None,
                ocr_confidence: Some(0.9),
                verified: false,
                source_deleted: false,
                perceptual_hash: None,
                notes: None,
                pipeline_status: PipelineStatus::Imported,
            },
        )
        .unwrap();
        id
    }

    fn med(name: &str, reason: Option<&str>) -> StoredEntity {
        StoredEntity {
            entity_type: EntityType::Medication,
            id: Uuid::new_v4(),
            name: name.into(),
            reason: reason.map(String::from),
            is_abnormal: false,
        }
    }

    fn diag(name: &str) -> StoredEntity {
        StoredEntity {
            entity_type: EntityType::Diagnosis,
            id: Uuid::new_v4(),
            name: name.into(),
            reason: None,
            is_abnormal: false,
        }
    }

    fn allergy(allergen: &str) -> StoredEntity {
        StoredEntity {
            entity_type: EntityType::Allergy,
            id: Uuid::new_v4(),
            name: allergen.into(),
            reason: None,
            is_abnormal: false,
        }
    }

    fn lab(name: &str, abnormal: bool) -> StoredEntity {
        StoredEntity {
            entity_type: EntityType::LabResult,
            id: Uuid::new_v4(),
            name: name.into(),
            reason: None,
            is_abnormal: abnormal,
        }
    }

    fn referral(reason: Option<&str>) -> StoredEntity {
        StoredEntity {
            entity_type: EntityType::Referral,
            id: Uuid::new_v4(),
            name: "Dr. Specialist".into(),
            reason: reason.map(String::from),
            is_abnormal: false,
        }
    }

    #[test]
    fn no_entities_no_connections() {
        let conn = test_db();
        let doc_id = make_document(&conn);
        let count = extract_connections(&conn, &doc_id, &[]).unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn medication_prescribed_for_diagnosis() {
        let conn = test_db();
        let doc_id = make_document(&conn);
        let entities = vec![
            med("Metformin", Some("Type 2 Diabetes")),
            diag("Type 2 Diabetes"),
        ];
        let count = extract_connections(&conn, &doc_id, &entities).unwrap();
        assert_eq!(count, 1);

        let connections = repository::get_connections_for_document(&conn, &doc_id).unwrap();
        assert_eq!(connections.len(), 1);
        assert_eq!(connections[0].relationship_type, RelationshipType::PrescribedFor);
        assert_eq!(connections[0].confidence, 1.0);
    }

    #[test]
    fn medication_no_reason_no_connection() {
        let conn = test_db();
        let doc_id = make_document(&conn);
        let entities = vec![med("Aspirin", None), diag("Migraine")];
        let count = extract_connections(&conn, &doc_id, &entities).unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn fuzzy_match_substring() {
        let conn = test_db();
        let doc_id = make_document(&conn);
        let entities = vec![
            med("Levothyroxine", Some("Hypothyroidism")),
            diag("Hypothyroidism (primary)"),
        ];
        let count = extract_connections(&conn, &doc_id, &entities).unwrap();
        assert_eq!(count, 1);

        let connections = repository::get_connections_for_document(&conn, &doc_id).unwrap();
        assert_eq!(connections[0].confidence, 0.7); // Fuzzy match
    }

    #[test]
    fn abnormal_lab_evidences_diagnosis() {
        let conn = test_db();
        let doc_id = make_document(&conn);
        let entities = vec![
            lab("TSH", true),
            diag("Hypothyroidism"),
        ];
        // TSH and Hypothyroidism don't substring-match, so no connection
        let count = extract_connections(&conn, &doc_id, &entities).unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn abnormal_lab_with_matching_diagnosis() {
        let conn = test_db();
        let doc_id = make_document(&conn);
        let entities = vec![
            lab("Glycémie", true),
            diag("Hyperglycémie"),
        ];
        // "Glycémie" is a substring of "Hyperglycémie"
        let count = extract_connections(&conn, &doc_id, &entities).unwrap();
        assert_eq!(count, 1);

        let connections = repository::get_connections_for_document(&conn, &doc_id).unwrap();
        assert_eq!(connections[0].relationship_type, RelationshipType::EvidencesFor);
    }

    #[test]
    fn normal_lab_no_connection() {
        let conn = test_db();
        let doc_id = make_document(&conn);
        let entities = vec![
            lab("Glycémie", false), // Normal
            diag("Hyperglycémie"),
        ];
        let count = extract_connections(&conn, &doc_id, &entities).unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn referral_follows_diagnosis() {
        let conn = test_db();
        let doc_id = make_document(&conn);
        let entities = vec![
            referral(Some("Hypothyroidism follow-up")),
            diag("Hypothyroidism"),
        ];
        let count = extract_connections(&conn, &doc_id, &entities).unwrap();
        assert_eq!(count, 1);

        let connections = repository::get_connections_for_document(&conn, &doc_id).unwrap();
        assert_eq!(connections[0].relationship_type, RelationshipType::FollowUpTo);
    }

    #[test]
    fn medication_contraindicated_by_allergy() {
        let conn = test_db();
        let doc_id = make_document(&conn);
        let entities = vec![
            med("Amoxicillin", None),
            allergy("Amoxicillin"),
        ];
        let count = extract_connections(&conn, &doc_id, &entities).unwrap();
        assert_eq!(count, 1);

        let connections = repository::get_connections_for_document(&conn, &doc_id).unwrap();
        assert_eq!(connections[0].relationship_type, RelationshipType::ContraindicatedBy);
        assert_eq!(connections[0].confidence, 1.0);
    }

    #[test]
    fn no_false_positive_on_unrelated_entities() {
        let conn = test_db();
        let doc_id = make_document(&conn);
        let entities = vec![
            med("Paracetamol", Some("Headache")),
            diag("Type 2 Diabetes"),
            allergy("Penicillin"),
            lab("HbA1c", true),
        ];
        // Paracetamol reason "Headache" doesn't match "Type 2 Diabetes"
        // Paracetamol name doesn't match "Penicillin"
        // HbA1c doesn't substring-match "Type 2 Diabetes"
        let count = extract_connections(&conn, &doc_id, &entities).unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn idempotent_reprocessing() {
        let conn = test_db();
        let doc_id = make_document(&conn);
        let entities = vec![
            med("Metformin", Some("Diabetes")),
            diag("Diabetes"),
        ];

        let count1 = extract_connections(&conn, &doc_id, &entities).unwrap();
        assert_eq!(count1, 1);

        // Run again — should clear and re-insert, not double
        let count2 = extract_connections(&conn, &doc_id, &entities).unwrap();
        assert_eq!(count2, 1);

        let connections = repository::get_connections_for_document(&conn, &doc_id).unwrap();
        assert_eq!(connections.len(), 1);
    }

    #[test]
    fn short_names_do_not_match() {
        // Names shorter than 4 chars should not fuzzy match
        assert!(!fuzzy_match("ASA", "Aspirin"));
        assert!(!fuzzy_match("TSH", "Thyroid"));
    }

    #[test]
    fn case_insensitive_matching() {
        assert!(fuzzy_match("metformin", "METFORMIN"));
        assert!(fuzzy_match("Type 2 Diabetes", "type 2 diabetes"));
    }
}

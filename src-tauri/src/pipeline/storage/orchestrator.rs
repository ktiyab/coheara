use std::path::Path;

use rusqlite::Connection;

use super::StorageError;
use super::entity_store;
use super::markdown_store;
use super::types::{
    Chunker, EmbeddingModel, StoragePipeline, StorageResult, StorageWarning, VectorStore,
};
use crate::crypto::ProfileSession;
use crate::db::{repository, sqlite};
use crate::pipeline::structuring::types::StructuringResult;

/// Orchestrates the full storage pipeline:
/// chunk → embed → vector store → entity store → markdown → update document.
pub struct DocumentStoragePipeline<C: Chunker, E: EmbeddingModel, V: VectorStore> {
    chunker: C,
    embedder: E,
    vector_store: V,
    profiles_dir: std::path::PathBuf,
}

impl<C: Chunker, E: EmbeddingModel, V: VectorStore> DocumentStoragePipeline<C, E, V> {
    pub fn new(
        chunker: C,
        embedder: E,
        vector_store: V,
        profiles_dir: &Path,
    ) -> Self {
        Self {
            chunker,
            embedder,
            vector_store,
            profiles_dir: profiles_dir.to_path_buf(),
        }
    }
}

impl<C: Chunker, E: EmbeddingModel, V: VectorStore> StoragePipeline
    for DocumentStoragePipeline<C, E, V>
{
    fn store(
        &self,
        structuring_result: &StructuringResult,
        session: &ProfileSession,
    ) -> Result<StorageResult, StorageError> {
        let mut warnings: Vec<StorageWarning> = Vec::new();
        let document_id = structuring_result.document_id;

        // Step 1: Chunk the structured markdown
        let chunks = self.chunker.chunk(&structuring_result.structured_markdown);
        if chunks.is_empty() {
            return Err(StorageError::EmptyChunks);
        }

        // Step 2: Generate embeddings for each chunk
        let texts: Vec<&str> = chunks.iter().map(|c| c.content.as_str()).collect();
        let embeddings = match self.embedder.embed_batch(&texts) {
            Ok(embs) => embs,
            Err(e) => {
                tracing::warn!("Embedding batch failed, falling back to individual: {e}");
                let mut embs = Vec::with_capacity(chunks.len());
                for (i, chunk) in chunks.iter().enumerate() {
                    match self.embedder.embed(&chunk.content) {
                        Ok(emb) => embs.push(emb),
                        Err(_) => {
                            warnings.push(StorageWarning::EmbeddingFailed { chunk_index: i });
                            embs.push(vec![0.0; self.embedder.dimension()]);
                        }
                    }
                }
                embs
            }
        };

        // Step 3: Store chunks + embeddings in vector store
        let doc_type_str = structuring_result.document_type.as_str();
        let doc_date_str = structuring_result
            .document_date
            .map(|d| d.to_string());
        let professional_name = structuring_result
            .professional
            .as_ref()
            .map(|p| p.name.clone());

        let chunks_stored = self.vector_store.store_chunks(
            &chunks,
            &embeddings,
            &document_id,
            doc_type_str,
            doc_date_str.as_deref(),
            professional_name.as_deref(),
            Some(session),
        )?;

        // Step 4: Open database connection and store entities
        let conn = open_profile_db(session)?;
        let (entity_counts, entity_warnings) =
            entity_store::store_entities(&conn, structuring_result)?;
        warnings.extend(entity_warnings);

        // Step 5: Save encrypted markdown
        let markdown_path = markdown_store::save_encrypted_markdown(
            session,
            &self.profiles_dir,
            &document_id,
            &structuring_result.structured_markdown,
        )?;

        // Step 6: Update document record with results
        let professional_id = structuring_result
            .professional
            .as_ref()
            .and_then(|p| {
                repository::find_or_create_professional(&conn, &p.name, p.specialty.as_deref())
                    .ok()
                    .map(|prof| prof.id)
            });

        if let Ok(Some(mut doc)) = repository::get_document(&conn, &document_id) {
            doc.doc_type = structuring_result.document_type.clone();
            doc.document_date = structuring_result.document_date;
            doc.professional_id = professional_id;
            doc.markdown_file = Some(markdown_path);
            let _ = repository::update_document(&conn, &doc);
        }

        // Step 7: Update profile trust
        let _ = repository::update_profile_trust_verified(&conn);

        Ok(StorageResult {
            document_id,
            chunks_stored,
            entities_stored: entity_counts,
            document_type: structuring_result.document_type.clone(),
            professional_id,
            warnings,
        })
    }
}

fn open_profile_db(session: &ProfileSession) -> Result<Connection, StorageError> {
    sqlite::open_database(session.db_path()).map_err(StorageError::Database)
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    use crate::crypto::profile;
    use crate::models::enums::DocumentType;
    use crate::pipeline::storage::chunker::MedicalChunker;
    use crate::pipeline::storage::embedder::MockEmbedder;
    use crate::pipeline::storage::vectordb::InMemoryVectorStore;
    use crate::pipeline::structuring::types::*;

    fn test_session() -> (tempfile::TempDir, ProfileSession) {
        let dir = tempfile::tempdir().unwrap();
        let (info, _phrase) =
            profile::create_profile(dir.path(), "StorageTest", "test_pass_123", None).unwrap();
        let session = profile::open_profile(dir.path(), &info.id, "test_pass_123").unwrap();
        (dir, session)
    }

    fn make_pipeline(
        profiles_dir: &Path,
    ) -> DocumentStoragePipeline<MedicalChunker, MockEmbedder, InMemoryVectorStore> {
        DocumentStoragePipeline::new(
            MedicalChunker::new(),
            MockEmbedder::new(),
            InMemoryVectorStore::new(),
            profiles_dir,
        )
    }

    fn make_structuring_result(document_id: Uuid) -> StructuringResult {
        StructuringResult {
            document_id,
            document_type: DocumentType::Prescription,
            document_date: Some(chrono::NaiveDate::from_ymd_opt(2024, 1, 15).unwrap()),
            professional: Some(ExtractedProfessional {
                name: "Dr. Pipeline".into(),
                specialty: Some("GP".into()),
                institution: None,
            }),
            structured_markdown: "## Medications\n\nMetformin 500mg twice daily for type 2 diabetes management.\n\n## Lab Results\n\nHbA1c: 7.2% (target < 7.0%)\n\n## Instructions\n\nFollow up in 3 months for repeat HbA1c.".into(),
            extracted_entities: ExtractedEntities {
                medications: vec![ExtractedMedication {
                    generic_name: Some("Metformin".into()),
                    brand_name: Some("Glucophage".into()),
                    dose: "500mg".into(),
                    frequency: "twice daily".into(),
                    frequency_type: "scheduled".into(),
                    route: "oral".into(),
                    reason: Some("Type 2 diabetes".into()),
                    instructions: vec!["Take with food".into()],
                    is_compound: false,
                    compound_ingredients: vec![],
                    tapering_steps: vec![],
                    max_daily_dose: Some("2000mg".into()),
                    condition: None,
                    confidence: 0.9,
                }],
                lab_results: vec![ExtractedLabResult {
                    test_name: "HbA1c".into(),
                    test_code: None,
                    value: Some(7.2),
                    value_text: None,
                    unit: Some("%".into()),
                    reference_range_low: Some(4.0),
                    reference_range_high: Some(5.6),
                    abnormal_flag: Some("high".into()),
                    collection_date: Some("2024-01-15".into()),
                    confidence: 0.95,
                }],
                diagnoses: vec![ExtractedDiagnosis {
                    name: "Type 2 Diabetes".into(),
                    icd_code: Some("E11".into()),
                    date: Some("2024-01-15".into()),
                    status: "active".into(),
                    confidence: 0.9,
                }],
                allergies: vec![],
                procedures: vec![],
                referrals: vec![],
                instructions: vec![ExtractedInstruction {
                    text: "Follow up in 3 months".into(),
                    category: "follow_up".into(),
                }],
            },
            structuring_confidence: 0.87,
            markdown_file_path: None,
        }
    }

    fn setup_document(conn: &Connection, document_id: &Uuid) {
        repository::insert_document(
            conn,
            &crate::models::Document {
                id: *document_id,
                doc_type: DocumentType::Other,
                title: "Test Document".into(),
                document_date: None,
                ingestion_date: chrono::Local::now().naive_local(),
                professional_id: None,
                source_file: "/test/source.jpg".into(),
                markdown_file: None,
                ocr_confidence: Some(0.92),
                verified: false,
                source_deleted: false,
                perceptual_hash: Some("testhash".into()),
                notes: None,
            },
        )
        .unwrap();
    }

    #[test]
    fn full_pipeline_stores_all_components() {
        let (dir, session) = test_session();
        let pipeline = make_pipeline(dir.path());
        let doc_id = Uuid::new_v4();

        // Pre-create the document record in DB
        let conn = sqlite::open_database(session.db_path()).unwrap();
        setup_document(&conn, &doc_id);
        drop(conn);

        let structuring = make_structuring_result(doc_id);
        let result = pipeline.store(&structuring, &session).unwrap();

        assert_eq!(result.document_id, doc_id);
        assert!(result.chunks_stored >= 3);
        assert_eq!(result.entities_stored.medications, 1);
        assert_eq!(result.entities_stored.lab_results, 1);
        assert_eq!(result.entities_stored.diagnoses, 1);
        assert_eq!(result.entities_stored.instructions, 1);
        assert_eq!(result.document_type, DocumentType::Prescription);
        assert!(result.professional_id.is_some());
    }

    #[test]
    fn pipeline_updates_document_record() {
        let (dir, session) = test_session();
        let pipeline = make_pipeline(dir.path());
        let doc_id = Uuid::new_v4();

        let conn = sqlite::open_database(session.db_path()).unwrap();
        setup_document(&conn, &doc_id);
        drop(conn);

        let structuring = make_structuring_result(doc_id);
        pipeline.store(&structuring, &session).unwrap();

        // Verify document was updated
        let conn = sqlite::open_database(session.db_path()).unwrap();
        let doc = repository::get_document(&conn, &doc_id).unwrap().unwrap();
        assert_eq!(doc.doc_type, DocumentType::Prescription);
        assert!(doc.markdown_file.is_some());
        assert!(doc.professional_id.is_some());
        assert!(doc.document_date.is_some());
    }

    #[test]
    fn pipeline_saves_encrypted_markdown() {
        let (dir, session) = test_session();
        let pipeline = make_pipeline(dir.path());
        let doc_id = Uuid::new_v4();

        let conn = sqlite::open_database(session.db_path()).unwrap();
        setup_document(&conn, &doc_id);
        drop(conn);

        let structuring = make_structuring_result(doc_id);
        pipeline.store(&structuring, &session).unwrap();

        // Verify markdown was saved
        let conn = sqlite::open_database(session.db_path()).unwrap();
        let doc = repository::get_document(&conn, &doc_id).unwrap().unwrap();
        let md_path = doc.markdown_file.unwrap();

        let decrypted =
            markdown_store::read_encrypted_markdown(&session, dir.path(), &md_path).unwrap();
        assert!(decrypted.contains("Metformin"));
        assert!(decrypted.contains("HbA1c"));
    }

    #[test]
    fn pipeline_updates_profile_trust() {
        let (dir, session) = test_session();
        let pipeline = make_pipeline(dir.path());
        let doc_id = Uuid::new_v4();

        let conn = sqlite::open_database(session.db_path()).unwrap();
        setup_document(&conn, &doc_id);
        let trust_before = repository::get_profile_trust(&conn).unwrap();
        drop(conn);

        let structuring = make_structuring_result(doc_id);
        pipeline.store(&structuring, &session).unwrap();

        let conn = sqlite::open_database(session.db_path()).unwrap();
        let trust_after = repository::get_profile_trust(&conn).unwrap();
        assert_eq!(
            trust_after.total_documents,
            trust_before.total_documents + 1
        );
    }

    #[test]
    fn empty_markdown_returns_error() {
        let (dir, session) = test_session();
        let pipeline = make_pipeline(dir.path());
        let doc_id = Uuid::new_v4();

        let structuring = StructuringResult {
            document_id: doc_id,
            document_type: DocumentType::Other,
            document_date: None,
            professional: None,
            structured_markdown: "".into(),
            extracted_entities: ExtractedEntities::default(),
            structuring_confidence: 0.5,
            markdown_file_path: None,
        };

        let result = pipeline.store(&structuring, &session);
        assert!(result.is_err());
    }

    #[test]
    fn pipeline_with_no_entities_still_stores_chunks() {
        let (dir, session) = test_session();
        let pipeline = make_pipeline(dir.path());
        let doc_id = Uuid::new_v4();

        let conn = sqlite::open_database(session.db_path()).unwrap();
        setup_document(&conn, &doc_id);
        drop(conn);

        let structuring = StructuringResult {
            document_id: doc_id,
            document_type: DocumentType::ClinicalNote,
            document_date: None,
            professional: None,
            structured_markdown: "## Notes\n\nPatient presents with general fatigue. No acute findings. Will monitor.".into(),
            extracted_entities: ExtractedEntities::default(),
            structuring_confidence: 0.7,
            markdown_file_path: None,
        };

        let result = pipeline.store(&structuring, &session).unwrap();
        assert!(result.chunks_stored >= 1);
        assert_eq!(result.entities_stored.medications, 0);
    }

    #[test]
    fn pipeline_stores_chunks_in_vector_store() {
        let (dir, session) = test_session();
        let pipeline = make_pipeline(dir.path());
        let doc_id = Uuid::new_v4();

        let conn = sqlite::open_database(session.db_path()).unwrap();
        setup_document(&conn, &doc_id);
        drop(conn);

        let structuring = make_structuring_result(doc_id);
        let result = pipeline.store(&structuring, &session).unwrap();

        // The pipeline's internal vector store received chunks
        assert!(result.chunks_stored >= 3);
        assert!(result.warnings.is_empty());
    }
}

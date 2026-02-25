pub mod import;
pub mod extraction;
pub mod structuring;
pub mod storage;
pub mod processor; // E2E-B02: Document Processing Orchestrator
pub mod diagnostic; // Pipeline diagnostic dump (auto in dev, COHEARA_DUMP_DIR in prod)
pub mod rag;
pub mod safety;
pub mod batch_extraction; // LP-01: Night Batch Extraction Pipeline
pub mod model_router; // CT-01: Tag-driven pipeline assignment

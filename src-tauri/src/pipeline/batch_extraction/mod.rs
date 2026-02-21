//! Night Batch Extraction Pipeline
//!
//! Extracts structured health data (symptoms, medications, appointments) from
//! chat conversations — running as a background batch job, never during live chat.
//!
//! ## Architecture (LP-01)
//!
//! Five modules connected by traits:
//! ```text
//! M1: Scheduler → M2: Analyzer → M3: Engine → M4: Store → M5: Review (frontend)
//! ```
//!
//! ## Design Principles
//! - EP-1: Reusable engine (chat, documents, future voice)
//! - EP-2: Domain-agnostic orchestrator
//! - EP-3: Backend zero-change (writes to existing tables via existing repo functions)
//! - EP-4: User always confirms (no auto-save)
//! - EP-5: Batch is invisible (user sees results, not processing)
//! - EP-6: Conversation-level extraction (one per domain per conversation)
//! - EP-7: Modular composition (5 independent modules)

pub mod error;
pub mod types;
pub mod traits;
pub mod analyzer;
pub mod extractors;
pub mod verifier;
pub mod store;
pub mod scheduler;
pub mod runner;
pub mod dispatch;

pub use error::ExtractionError;
pub use types::*;
pub use traits::*;
pub use analyzer::RuleBasedAnalyzer;
pub use extractors::{SymptomExtractor, MedicationExtractor, AppointmentExtractor};
pub use verifier::SemanticVerifier;
pub use store::{SqlitePendingStore, create_pending_item};
pub use scheduler::{SqliteBatchScheduler, new_batch_id};
pub use runner::{BatchRunner, run_full_batch, ConversationExtractionResult};
pub use dispatch::dispatch_item;

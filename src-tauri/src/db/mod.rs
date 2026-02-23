pub mod app_db;
pub mod sqlite;
pub mod repository;

pub use sqlite::*;
pub use repository::*;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum DatabaseError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("Entity not found: {entity_type} with id {id}")]
    NotFound { entity_type: String, id: String },

    #[error("Invalid enum value for {field}: {value}")]
    InvalidEnum { field: String, value: String },

    #[error("Migration failed at version {version}: {reason}")]
    MigrationFailed { version: i64, reason: String },

    #[error("Constraint violated: {0}")]
    ConstraintViolation(String),
}

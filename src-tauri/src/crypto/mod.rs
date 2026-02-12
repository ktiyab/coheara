pub mod keys;
pub mod encryption;
pub mod recovery;
pub mod profile;

pub use keys::*;
pub use encryption::*;
pub use recovery::*;
pub use profile::*;

use thiserror::Error;
use uuid::Uuid;

#[derive(Error, Debug)]
pub enum CryptoError {
    #[error("Encryption failed")]
    EncryptionFailed,

    #[error("Decryption failed — wrong key or corrupted data")]
    DecryptionFailed,

    #[error("Wrong password")]
    WrongPassword,

    #[error("Invalid recovery phrase")]
    InvalidRecoveryPhrase,

    #[error("Profile not found: {0}")]
    ProfileNotFound(Uuid),

    #[error("Corrupted profile data")]
    CorruptedProfile,

    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Profile already exists: {0}")]
    ProfileExists(String),

    #[error("Database error: {0}")]
    Database(#[from] crate::db::DatabaseError),
}

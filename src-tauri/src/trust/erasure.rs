use std::path::Path;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::crypto::encryption::EncryptedData;
use crate::crypto::keys::{ProfileKey, SALT_LENGTH};

use super::fs_helpers::{count_dir_contents, get_profile_name_from_dir};
use super::TrustError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErasureRequest {
    pub profile_id: String,
    pub confirmation_text: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErasureResult {
    pub profile_name: String,
    pub files_deleted: u32,
    pub bytes_erased: u64,
    pub key_zeroed: bool,
}

/// Erase a profile — validates confirmation and password, then delegates
/// to the existing `delete_profile` in crypto::profile.
pub fn erase_profile_data(
    profiles_dir: &Path,
    request: &ErasureRequest,
) -> Result<ErasureResult, TrustError> {
    // 1. Validate confirmation text
    if request.confirmation_text != "DELETE MY DATA" {
        return Err(TrustError::Validation(
            "Must type 'DELETE MY DATA' to confirm deletion".into(),
        ));
    }

    // 2. Parse profile ID
    let profile_id = Uuid::parse_str(&request.profile_id)
        .map_err(|_| TrustError::Validation("Invalid profile ID".into()))?;

    // 3. Verify password by attempting to unlock
    let profile_dir = profiles_dir.join(profile_id.to_string());
    if !profile_dir.exists() {
        return Err(TrustError::NotFound("Profile not found".into()));
    }

    let salt_path = profile_dir.join("salt.bin");
    let salt_bytes = std::fs::read(&salt_path)
        .map_err(|_| TrustError::NotFound("Profile salt not found".into()))?;
    if salt_bytes.len() != SALT_LENGTH {
        return Err(TrustError::Crypto("Invalid salt length".into()));
    }
    let mut salt = [0u8; SALT_LENGTH];
    salt.copy_from_slice(&salt_bytes);

    let key = ProfileKey::derive(&request.password, &salt);

    // Verify password against stored verification token
    let verification_path = profile_dir.join("verification.enc");
    let verification_bytes = std::fs::read(&verification_path)
        .map_err(|_| TrustError::NotFound("Profile verification data not found".into()))?;
    let verification = EncryptedData::from_bytes(&verification_bytes)
        .map_err(|_| TrustError::Crypto("Corrupted verification data".into()))?;

    let decrypted = key.decrypt(&verification);
    match &decrypted {
        Ok(plaintext) if plaintext.as_slice() == b"COHEARA_VERIFY" => {}
        _ => return Err(TrustError::Validation("Incorrect password".into())),
    }

    // 4. Get profile name from registry
    let profile_name = get_profile_name_from_dir(profiles_dir, &profile_id);

    // 5. Count files and size before deletion
    let (file_count, total_bytes) = count_dir_contents(&profile_dir);

    // 6. Delete via existing crypto::profile::delete_profile
    crate::crypto::profile::delete_profile(profiles_dir, &profile_id)
        .map_err(|e| TrustError::Crypto(e.to_string()))?;

    // Key is automatically zeroed on drop (ZeroizeOnDrop)
    drop(key);

    tracing::info!(
        profile_id = %profile_id,
        files = file_count,
        bytes = total_bytes,
        "Profile erased"
    );

    Ok(ErasureResult {
        profile_name,
        files_deleted: file_count,
        bytes_erased: total_bytes,
        key_zeroed: true,
    })
}

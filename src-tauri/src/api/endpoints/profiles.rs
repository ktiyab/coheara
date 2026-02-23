//! E6: Profile list endpoint — companion queries accessible profiles.
//!
//! `GET /api/profiles/accessible` — returns profiles this device can access.

use axum::extract::State;
use axum::{Extension, Json};
use serde::Serialize;

use crate::api::error::ApiError;
use crate::api::types::{ApiContext, DeviceContext};
use crate::crypto::profile;
use crate::db::repository::device_registry;

/// A profile the device can access — enriched with metadata.
#[derive(Debug, Clone, Serialize)]
pub struct AccessibleProfileInfo {
    pub profile_id: String,
    pub profile_name: String,
    /// "own", "managed", or "granted"
    pub relationship: String,
    pub access_level: String,
    pub color_index: Option<u8>,
    pub is_active: bool,
    pub is_unlocked: bool,
}

/// Response for `GET /api/profiles/accessible`.
#[derive(Debug, Serialize)]
pub struct AccessibleProfilesResponse {
    pub profiles: Vec<AccessibleProfileInfo>,
    pub active_profile_id: String,
}

/// `GET /api/profiles/accessible` — list profiles this device can access.
///
/// Reads from `device_profile_access` table in `app.db`, enriches with
/// profile metadata (name, color, managed_by) from disk, and marks which
/// profile is currently active on the desktop.
pub async fn accessible(
    State(ctx): State<ApiContext>,
    Extension(device): Extension<DeviceContext>,
) -> Result<Json<AccessibleProfilesResponse>, ApiError> {
    // Get active profile ID
    let active_profile_id = {
        let guard = ctx.core.read_session().map_err(ApiError::from)?;
        guard
            .as_ref()
            .map(|s| s.profile_id.to_string())
            .unwrap_or_default()
    };

    // Get cached (unlocked) profile IDs
    let unlocked_ids: Vec<String> = ctx
        .core
        .cached_profile_ids()
        .unwrap_or_default()
        .iter()
        .map(|id| id.to_string())
        .collect();

    // Read device's accessible profiles from app.db
    let access_rows = match ctx.core.open_app_db() {
        Ok(app_conn) => {
            device_registry::list_accessible_profiles(&app_conn, &device.device_id)
                .unwrap_or_default()
        }
        Err(_) => Vec::new(),
    };

    // Load profile metadata from disk
    let all_profiles = profile::list_profiles(&ctx.core.profiles_dir).unwrap_or_default();

    // Build enriched response
    let mut profiles: Vec<AccessibleProfileInfo> = Vec::new();

    for row in &access_rows {
        let meta = all_profiles.iter().find(|p| p.id.to_string() == row.profile_id);
        let (name, color, relationship) = match meta {
            Some(p) => {
                let rel = if p.id.to_string() == device.owner_profile_id.to_string() {
                    "own"
                } else if p.managed_by.as_deref() == Some(&device.device_name) {
                    // If managed_by matches the owner profile's name
                    "managed"
                } else {
                    "granted"
                };
                (p.name.clone(), p.color_index, rel)
            }
            None => (format!("Profile {}", &row.profile_id[..8.min(row.profile_id.len())]), None, "granted"),
        };

        profiles.push(AccessibleProfileInfo {
            profile_id: row.profile_id.clone(),
            profile_name: name,
            relationship: relationship.to_string(),
            access_level: row.access_level.clone(),
            color_index: color,
            is_active: row.profile_id == active_profile_id,
            is_unlocked: unlocked_ids.contains(&row.profile_id),
        });
    }

    // If no app.db entries (pre-migration), fall back to active profile only
    if profiles.is_empty() && !active_profile_id.is_empty() {
        let meta = all_profiles
            .iter()
            .find(|p| p.id.to_string() == active_profile_id);
        profiles.push(AccessibleProfileInfo {
            profile_id: active_profile_id.clone(),
            profile_name: meta.map(|p| p.name.clone()).unwrap_or_else(|| "Profile".into()),
            relationship: "own".to_string(),
            access_level: "full".to_string(),
            color_index: meta.and_then(|p| p.color_index),
            is_active: true,
            is_unlocked: true,
        });
    }

    Ok(Json(AccessibleProfilesResponse {
        profiles,
        active_profile_id,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accessible_profile_info_serializes() {
        let info = AccessibleProfileInfo {
            profile_id: "abc-123".into(),
            profile_name: "Alice".into(),
            relationship: "own".into(),
            access_level: "full".into(),
            color_index: Some(2),
            is_active: true,
            is_unlocked: true,
        };
        let json = serde_json::to_value(&info).unwrap();
        assert_eq!(json["profile_id"], "abc-123");
        assert_eq!(json["relationship"], "own");
        assert_eq!(json["is_active"], true);
        assert_eq!(json["color_index"], 2);
    }

    #[test]
    fn response_serializes_with_active_profile() {
        let response = AccessibleProfilesResponse {
            profiles: vec![AccessibleProfileInfo {
                profile_id: "p-1".into(),
                profile_name: "Bob".into(),
                relationship: "managed".into(),
                access_level: "full".into(),
                color_index: None,
                is_active: false,
                is_unlocked: false,
            }],
            active_profile_id: "p-owner".into(),
        };
        let json = serde_json::to_value(&response).unwrap();
        assert_eq!(json["active_profile_id"], "p-owner");
        assert_eq!(json["profiles"][0]["profile_name"], "Bob");
    }
}

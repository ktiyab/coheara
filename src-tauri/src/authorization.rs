//! MP-01: Profile access authorization service.
//!
//! Implements the 4-rule authorization cascade (Google Family Link pattern):
//! 1. Own profile → FULL ACCESS
//! 2. Managed profile (managed_by) → FULL ACCESS
//! 3. Explicit grant (profile_access_grants) → GRANTED LEVEL
//! 4. Device access (device_profile_access) → GRANTED LEVEL
//! 5. Default → DENY
//!
//! Default-deny, checked in order. Unidirectional: Alice grants Bob != Bob grants Alice.

use std::path::Path;

use rusqlite::Connection;
use uuid::Uuid;

use crate::crypto::profile::{list_profiles, ProfileInfo};
use crate::db::repository::device_registry;

// ═══════════════════════════════════════════════════════════
// Types
// ═══════════════════════════════════════════════════════════

/// Access level granted to a device for a profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessLevel {
    Full,
    ReadOnly,
}

impl AccessLevel {
    /// Parse from database string representation.
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "full" => Some(Self::Full),
            "read_only" => Some(Self::ReadOnly),
            _ => None,
        }
    }

    /// Database string representation.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::ReadOnly => "read_only",
        }
    }
}

/// Why access was granted (or denied) — for audit trail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccessReason {
    /// Device owner accessing their own profile.
    OwnProfile,
    /// Caregiver accessing a profile they manage.
    ManagedProfile,
    /// Explicit grant in profile_access_grants table.
    ExplicitGrant,
    /// Device-profile entry in device_profile_access table.
    DeviceAccess,
    /// No matching rule — access denied.
    Denied,
}

/// Result of an authorization check.
#[derive(Debug, Clone)]
pub struct AccessDecision {
    pub allowed: bool,
    pub level: AccessLevel,
    pub reason: AccessReason,
}

impl AccessDecision {
    fn allow(level: AccessLevel, reason: AccessReason) -> Self {
        Self {
            allowed: true,
            level,
            reason,
        }
    }

    fn deny() -> Self {
        Self {
            allowed: false,
            level: AccessLevel::ReadOnly,
            reason: AccessReason::Denied,
        }
    }
}

// ═══════════════════════════════════════════════════════════
// Error type
// ═══════════════════════════════════════════════════════════

/// Errors from authorization operations.
#[derive(Debug, thiserror::Error)]
pub enum AuthorizationError {
    #[error("Database error: {0}")]
    Database(#[from] crate::db::DatabaseError),
    #[error("Failed to load profiles: {0}")]
    ProfileLoad(String),
}

// ═══════════════════════════════════════════════════════════
// Authorization check
// ═══════════════════════════════════════════════════════════

/// Check if a device owner can access a target profile.
///
/// Implements the 4-rule cascade from MP-01 spec:
/// 1. Own profile (`owner == target`) → FULL
/// 2. Managed profile (`target.managed_by == owner.name`) → FULL
/// 3. Explicit grant in `profile_access_grants` → GRANTED LEVEL
/// 4. Device-profile in `device_profile_access` → GRANTED LEVEL
/// 5. Default → DENY
///
/// Arguments:
/// - `app_conn`: Connection to the app-level database (app.db)
/// - `profiles_dir`: Path to profiles directory (for loading profiles.json)
/// - `owner_profile_id`: The profile that owns the requesting device
/// - `target_profile_id`: The profile being accessed
/// - `device_id`: The device making the request
pub fn check_profile_access(
    app_conn: &Connection,
    profiles_dir: &Path,
    owner_profile_id: &Uuid,
    target_profile_id: &Uuid,
    device_id: &str,
) -> Result<AccessDecision, AuthorizationError> {
    // Rule 1: Own profile
    if owner_profile_id == target_profile_id {
        return Ok(AccessDecision::allow(AccessLevel::Full, AccessReason::OwnProfile));
    }

    // Rule 2: Managed profile
    if is_managed_by(profiles_dir, owner_profile_id, target_profile_id)? {
        return Ok(AccessDecision::allow(AccessLevel::Full, AccessReason::ManagedProfile));
    }

    // Rule 3: Explicit grant (user-to-user in profile_access_grants)
    let owner_str = owner_profile_id.to_string();
    let target_str = target_profile_id.to_string();
    if let Some(level_str) = device_registry::has_active_grant(app_conn, &target_str, &owner_str)? {
        if let Some(level) = AccessLevel::from_str(&level_str) {
            return Ok(AccessDecision::allow(level, AccessReason::ExplicitGrant));
        }
    }

    // Rule 4: Device access (device_profile_access)
    if let Some(level_str) =
        device_registry::has_device_profile_access(app_conn, device_id, &target_str)?
    {
        if let Some(level) = AccessLevel::from_str(&level_str) {
            return Ok(AccessDecision::allow(level, AccessReason::DeviceAccess));
        }
    }

    // Rule 5: Default deny
    Ok(AccessDecision::deny())
}

/// Check if the target profile is managed by the owner profile.
///
/// Loads profiles.json and checks: target.managed_by == owner.name
fn is_managed_by(
    profiles_dir: &Path,
    owner_profile_id: &Uuid,
    target_profile_id: &Uuid,
) -> Result<bool, AuthorizationError> {
    let profiles = list_profiles(profiles_dir)
        .map_err(|e| AuthorizationError::ProfileLoad(e.to_string()))?;

    let owner = profiles.iter().find(|p| &p.id == owner_profile_id);
    let target = profiles.iter().find(|p| &p.id == target_profile_id);

    match (owner, target) {
        (Some(owner_info), Some(target_info)) => {
            Ok(target_info.managed_by.as_deref() == Some(&owner_info.name))
        }
        _ => Ok(false),
    }
}

/// Check authorization using pre-loaded profiles (avoids repeated file I/O).
///
/// Same 4-rule cascade but accepts profiles list instead of loading from disk.
pub fn check_profile_access_with_profiles(
    app_conn: &Connection,
    profiles: &[ProfileInfo],
    owner_profile_id: &Uuid,
    target_profile_id: &Uuid,
    device_id: &str,
) -> Result<AccessDecision, AuthorizationError> {
    // Rule 1: Own profile
    if owner_profile_id == target_profile_id {
        return Ok(AccessDecision::allow(AccessLevel::Full, AccessReason::OwnProfile));
    }

    // Rule 2: Managed profile
    let owner = profiles.iter().find(|p| &p.id == owner_profile_id);
    let target = profiles.iter().find(|p| &p.id == target_profile_id);
    if let (Some(owner_info), Some(target_info)) = (owner, target) {
        if target_info.managed_by.as_deref() == Some(&owner_info.name) {
            return Ok(AccessDecision::allow(AccessLevel::Full, AccessReason::ManagedProfile));
        }
    }

    // Rule 3: Explicit grant
    let owner_str = owner_profile_id.to_string();
    let target_str = target_profile_id.to_string();
    if let Some(level_str) = device_registry::has_active_grant(app_conn, &target_str, &owner_str)? {
        if let Some(level) = AccessLevel::from_str(&level_str) {
            return Ok(AccessDecision::allow(level, AccessReason::ExplicitGrant));
        }
    }

    // Rule 4: Device access
    if let Some(level_str) =
        device_registry::has_device_profile_access(app_conn, device_id, &target_str)?
    {
        if let Some(level) = AccessLevel::from_str(&level_str) {
            return Ok(AccessDecision::allow(level, AccessReason::DeviceAccess));
        }
    }

    // Rule 5: Default deny
    Ok(AccessDecision::deny())
}

// ═══════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::app_db::open_memory_app_database;
    use crate::db::repository::device_registry::{
        grant_device_profile_access, insert_device, insert_profile_grant, DeviceRegistryRow,
        ProfileAccessGrantRow,
    };
    use chrono::NaiveDateTime;

    fn test_app_db() -> Connection {
        open_memory_app_database().unwrap()
    }

    fn sample_profiles() -> Vec<ProfileInfo> {
        let alice_id = Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();
        let bob_id = Uuid::parse_str("00000000-0000-0000-0000-000000000002").unwrap();
        let child_id = Uuid::parse_str("00000000-0000-0000-0000-000000000003").unwrap();

        vec![
            ProfileInfo {
                id: alice_id,
                name: "Alice".to_string(),
                created_at: NaiveDateTime::parse_from_str(
                    "2026-01-01 00:00:00",
                    "%Y-%m-%d %H:%M:%S",
                )
                .unwrap(),
                managed_by: None,
                password_hint: None,
                date_of_birth: None,
                color_index: Some(0),
                country: None,
                address: None,
            },
            ProfileInfo {
                id: bob_id,
                name: "Bob".to_string(),
                created_at: NaiveDateTime::parse_from_str(
                    "2026-01-01 00:00:00",
                    "%Y-%m-%d %H:%M:%S",
                )
                .unwrap(),
                managed_by: None,
                password_hint: None,
                date_of_birth: None,
                color_index: Some(1),
                country: None,
                address: None,
            },
            ProfileInfo {
                id: child_id,
                name: "Child".to_string(),
                created_at: NaiveDateTime::parse_from_str(
                    "2026-01-01 00:00:00",
                    "%Y-%m-%d %H:%M:%S",
                )
                .unwrap(),
                managed_by: Some("Alice".to_string()), // Managed by Alice
                password_hint: None,
                date_of_birth: None,
                color_index: Some(2),
                country: None,
                address: None,
            },
        ]
    }

    fn alice_id() -> Uuid {
        Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap()
    }
    fn bob_id() -> Uuid {
        Uuid::parse_str("00000000-0000-0000-0000-000000000002").unwrap()
    }
    fn child_id() -> Uuid {
        Uuid::parse_str("00000000-0000-0000-0000-000000000003").unwrap()
    }

    fn setup_device(conn: &Connection, device_id: &str, owner: &str) {
        insert_device(
            conn,
            &DeviceRegistryRow {
                device_id: device_id.to_string(),
                device_name: "Phone".to_string(),
                device_model: "iPhone".to_string(),
                owner_profile_id: owner.to_string(),
                public_key: vec![1, 2, 3],
                paired_at: "2026-01-01T00:00:00Z".to_string(),
                last_seen: "2026-01-01T00:00:00Z".to_string(),
                is_revoked: false,
            },
        )
        .unwrap();
    }

    // ── Rule 1: Own profile ──────────────────────────────

    #[test]
    fn own_profile_always_full_access() {
        let conn = test_app_db();
        let profiles = sample_profiles();

        let decision = check_profile_access_with_profiles(
            &conn,
            &profiles,
            &alice_id(),
            &alice_id(),
            "dev-1",
        )
        .unwrap();

        assert!(decision.allowed);
        assert_eq!(decision.level, AccessLevel::Full);
        assert_eq!(decision.reason, AccessReason::OwnProfile);
    }

    // ── Rule 2: Managed profile ──────────────────────────

    #[test]
    fn managed_profile_full_access() {
        let conn = test_app_db();
        let profiles = sample_profiles();

        let decision = check_profile_access_with_profiles(
            &conn,
            &profiles,
            &alice_id(),
            &child_id(),
            "dev-1",
        )
        .unwrap();

        assert!(decision.allowed);
        assert_eq!(decision.level, AccessLevel::Full);
        assert_eq!(decision.reason, AccessReason::ManagedProfile);
    }

    #[test]
    fn managed_access_is_unidirectional() {
        let conn = test_app_db();
        let profiles = sample_profiles();

        // Child cannot access Alice (managed_by is one-way)
        let decision = check_profile_access_with_profiles(
            &conn,
            &profiles,
            &child_id(),
            &alice_id(),
            "dev-1",
        )
        .unwrap();

        assert!(!decision.allowed);
        assert_eq!(decision.reason, AccessReason::Denied);
    }

    // ── Rule 3: Explicit grant ───────────────────────────

    #[test]
    fn explicit_grant_read_only() {
        let conn = test_app_db();
        let profiles = sample_profiles();

        // Bob grants Alice read-only access
        insert_profile_grant(
            &conn,
            &ProfileAccessGrantRow {
                id: "grant-1".to_string(),
                granter_profile_id: bob_id().to_string(),
                grantee_profile_id: alice_id().to_string(),
                access_level: "read_only".to_string(),
                granted_at: "2026-01-01T00:00:00Z".to_string(),
                revoked_at: None,
            },
        )
        .unwrap();

        let decision = check_profile_access_with_profiles(
            &conn,
            &profiles,
            &alice_id(),
            &bob_id(),
            "dev-1",
        )
        .unwrap();

        assert!(decision.allowed);
        assert_eq!(decision.level, AccessLevel::ReadOnly);
        assert_eq!(decision.reason, AccessReason::ExplicitGrant);
    }

    #[test]
    fn explicit_grant_is_unidirectional() {
        let conn = test_app_db();
        let profiles = sample_profiles();

        // Bob grants Alice read-only access
        insert_profile_grant(
            &conn,
            &ProfileAccessGrantRow {
                id: "grant-1".to_string(),
                granter_profile_id: bob_id().to_string(),
                grantee_profile_id: alice_id().to_string(),
                access_level: "read_only".to_string(),
                granted_at: "2026-01-01T00:00:00Z".to_string(),
                revoked_at: None,
            },
        )
        .unwrap();

        // Alice→Bob: ALLOWED (grant exists)
        let decision1 = check_profile_access_with_profiles(
            &conn,
            &profiles,
            &alice_id(),
            &bob_id(),
            "dev-1",
        )
        .unwrap();
        assert!(decision1.allowed);

        // Bob→Alice: DENIED (no reverse grant)
        let decision2 = check_profile_access_with_profiles(
            &conn,
            &profiles,
            &bob_id(),
            &alice_id(),
            "dev-1",
        )
        .unwrap();
        assert!(!decision2.allowed);
    }

    // ── Rule 4: Device access ────────────────────────────

    #[test]
    fn device_profile_access_grants_level() {
        let conn = test_app_db();
        let profiles = sample_profiles();

        setup_device(&conn, "dev-alice", &alice_id().to_string());
        grant_device_profile_access(&conn, "dev-alice", &bob_id().to_string(), "read_only")
            .unwrap();

        let decision = check_profile_access_with_profiles(
            &conn,
            &profiles,
            &alice_id(),
            &bob_id(),
            "dev-alice",
        )
        .unwrap();

        assert!(decision.allowed);
        assert_eq!(decision.level, AccessLevel::ReadOnly);
        assert_eq!(decision.reason, AccessReason::DeviceAccess);
    }

    // ── Rule 5: Default deny ─────────────────────────────

    #[test]
    fn no_relationship_is_denied() {
        let conn = test_app_db();
        let profiles = sample_profiles();

        let decision = check_profile_access_with_profiles(
            &conn,
            &profiles,
            &alice_id(),
            &bob_id(),
            "dev-1",
        )
        .unwrap();

        assert!(!decision.allowed);
        assert_eq!(decision.reason, AccessReason::Denied);
    }

    // ── Rule priority ────────────────────────────────────

    #[test]
    fn managed_takes_priority_over_grant() {
        let conn = test_app_db();
        let profiles = sample_profiles();

        // Even with a read_only grant, managed = full access
        insert_profile_grant(
            &conn,
            &ProfileAccessGrantRow {
                id: "grant-1".to_string(),
                granter_profile_id: child_id().to_string(),
                grantee_profile_id: alice_id().to_string(),
                access_level: "read_only".to_string(),
                granted_at: "2026-01-01T00:00:00Z".to_string(),
                revoked_at: None,
            },
        )
        .unwrap();

        let decision = check_profile_access_with_profiles(
            &conn,
            &profiles,
            &alice_id(),
            &child_id(),
            "dev-1",
        )
        .unwrap();

        assert!(decision.allowed);
        assert_eq!(decision.level, AccessLevel::Full);
        assert_eq!(
            decision.reason,
            AccessReason::ManagedProfile,
            "Managed should take priority over grant"
        );
    }

    // ── AccessLevel parsing ──────────────────────────────

    #[test]
    fn access_level_round_trip() {
        assert_eq!(AccessLevel::from_str("full"), Some(AccessLevel::Full));
        assert_eq!(
            AccessLevel::from_str("read_only"),
            Some(AccessLevel::ReadOnly)
        );
        assert_eq!(AccessLevel::from_str("admin"), None);
        assert_eq!(AccessLevel::Full.as_str(), "full");
        assert_eq!(AccessLevel::ReadOnly.as_str(), "read_only");
    }
}

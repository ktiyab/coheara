//! Repository functions for the app-level device registry.
//!
//! Operates on `app.db` (unencrypted, global) — NOT per-profile databases.
//! Follows the same function-based pattern as per-profile repositories.

use rusqlite::{params, Connection};

use crate::db::DatabaseError;

// ═══════════════════════════════════════════════════════════
// Device Registry CRUD
// ═══════════════════════════════════════════════════════════

/// A row from the `device_registry` table.
#[derive(Debug, Clone)]
pub struct DeviceRegistryRow {
    pub device_id: String,
    pub device_name: String,
    pub device_model: String,
    pub owner_profile_id: String,
    pub public_key: Vec<u8>,
    pub paired_at: String,
    pub last_seen: String,
    pub is_revoked: bool,
}

/// Insert a new device into the global registry.
pub fn insert_device(conn: &Connection, device: &DeviceRegistryRow) -> Result<(), DatabaseError> {
    conn.execute(
        "INSERT INTO device_registry (device_id, device_name, device_model, owner_profile_id, public_key, paired_at, last_seen, is_revoked)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            device.device_id,
            device.device_name,
            device.device_model,
            device.owner_profile_id,
            device.public_key,
            device.paired_at,
            device.last_seen,
            device.is_revoked as i32,
        ],
    )?;
    Ok(())
}

/// Get a device by its ID.
pub fn get_device(conn: &Connection, device_id: &str) -> Result<Option<DeviceRegistryRow>, DatabaseError> {
    let mut stmt = conn.prepare(
        "SELECT device_id, device_name, device_model, owner_profile_id, public_key, paired_at, last_seen, is_revoked
         FROM device_registry WHERE device_id = ?1",
    )?;

    let row = stmt
        .query_row(params![device_id], |row| {
            Ok(DeviceRegistryRow {
                device_id: row.get(0)?,
                device_name: row.get(1)?,
                device_model: row.get(2)?,
                owner_profile_id: row.get(3)?,
                public_key: row.get(4)?,
                paired_at: row.get(5)?,
                last_seen: row.get(6)?,
                is_revoked: row.get::<_, i32>(7)? != 0,
            })
        })
        .optional()?;

    Ok(row)
}

/// Revoke a device (soft-delete — preserves audit trail).
pub fn revoke_device(conn: &Connection, device_id: &str) -> Result<bool, DatabaseError> {
    let updated = conn.execute(
        "UPDATE device_registry SET is_revoked = 1 WHERE device_id = ?1",
        params![device_id],
    )?;
    Ok(updated > 0)
}

/// Update the last_seen timestamp for a device.
pub fn update_device_last_seen(conn: &Connection, device_id: &str) -> Result<(), DatabaseError> {
    conn.execute(
        "UPDATE device_registry SET last_seen = datetime('now') WHERE device_id = ?1",
        params![device_id],
    )?;
    Ok(())
}

/// List all non-revoked devices for a profile owner.
pub fn list_devices_for_owner(
    conn: &Connection,
    owner_profile_id: &str,
) -> Result<Vec<DeviceRegistryRow>, DatabaseError> {
    let mut stmt = conn.prepare(
        "SELECT device_id, device_name, device_model, owner_profile_id, public_key, paired_at, last_seen, is_revoked
         FROM device_registry WHERE owner_profile_id = ?1 AND is_revoked = 0
         ORDER BY paired_at DESC",
    )?;

    let rows = stmt
        .query_map(params![owner_profile_id], |row| {
            Ok(DeviceRegistryRow {
                device_id: row.get(0)?,
                device_name: row.get(1)?,
                device_model: row.get(2)?,
                owner_profile_id: row.get(3)?,
                public_key: row.get(4)?,
                paired_at: row.get(5)?,
                last_seen: row.get(6)?,
                is_revoked: row.get::<_, i32>(7)? != 0,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(rows)
}

// ═══════════════════════════════════════════════════════════
// Device Profile Access
// ═══════════════════════════════════════════════════════════

/// A row from the `device_profile_access` table.
#[derive(Debug, Clone)]
pub struct DeviceProfileAccessRow {
    pub device_id: String,
    pub profile_id: String,
    pub access_level: String,
    pub granted_at: String,
}

/// Grant a device access to a specific profile.
pub fn grant_device_profile_access(
    conn: &Connection,
    device_id: &str,
    profile_id: &str,
    access_level: &str,
) -> Result<(), DatabaseError> {
    conn.execute(
        "INSERT OR REPLACE INTO device_profile_access (device_id, profile_id, access_level, granted_at)
         VALUES (?1, ?2, ?3, datetime('now'))",
        params![device_id, profile_id, access_level],
    )?;
    Ok(())
}

/// Revoke a device's access to a specific profile.
pub fn revoke_device_profile_access(
    conn: &Connection,
    device_id: &str,
    profile_id: &str,
) -> Result<bool, DatabaseError> {
    let deleted = conn.execute(
        "DELETE FROM device_profile_access WHERE device_id = ?1 AND profile_id = ?2",
        params![device_id, profile_id],
    )?;
    Ok(deleted > 0)
}

/// List all profiles a device can access (non-revoked device only).
pub fn list_accessible_profiles(
    conn: &Connection,
    device_id: &str,
) -> Result<Vec<DeviceProfileAccessRow>, DatabaseError> {
    let mut stmt = conn.prepare(
        "SELECT dpa.device_id, dpa.profile_id, dpa.access_level, dpa.granted_at
         FROM device_profile_access dpa
         JOIN device_registry dr ON dr.device_id = dpa.device_id
         WHERE dpa.device_id = ?1 AND dr.is_revoked = 0
         ORDER BY dpa.granted_at",
    )?;

    let rows = stmt
        .query_map(params![device_id], |row| {
            Ok(DeviceProfileAccessRow {
                device_id: row.get(0)?,
                profile_id: row.get(1)?,
                access_level: row.get(2)?,
                granted_at: row.get(3)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(rows)
}

/// Check if a device has access to a specific profile.
pub fn has_device_profile_access(
    conn: &Connection,
    device_id: &str,
    profile_id: &str,
) -> Result<Option<String>, DatabaseError> {
    let mut stmt = conn.prepare(
        "SELECT dpa.access_level
         FROM device_profile_access dpa
         JOIN device_registry dr ON dr.device_id = dpa.device_id
         WHERE dpa.device_id = ?1 AND dpa.profile_id = ?2 AND dr.is_revoked = 0",
    )?;

    let level = stmt
        .query_row(params![device_id, profile_id], |row| {
            row.get::<_, String>(0)
        })
        .optional()?;

    Ok(level)
}

// ═══════════════════════════════════════════════════════════
// Profile Access Grants (user-to-user)
// ═══════════════════════════════════════════════════════════

/// A row from the `profile_access_grants` table.
#[derive(Debug, Clone)]
pub struct ProfileAccessGrantRow {
    pub id: String,
    pub granter_profile_id: String,
    pub grantee_profile_id: String,
    pub access_level: String,
    pub granted_at: String,
    pub revoked_at: Option<String>,
}

/// Insert a new profile access grant.
pub fn insert_profile_grant(
    conn: &Connection,
    grant: &ProfileAccessGrantRow,
) -> Result<(), DatabaseError> {
    conn.execute(
        "INSERT INTO profile_access_grants (id, granter_profile_id, grantee_profile_id, access_level, granted_at, revoked_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            grant.id,
            grant.granter_profile_id,
            grant.grantee_profile_id,
            grant.access_level,
            grant.granted_at,
            grant.revoked_at,
        ],
    )?;
    Ok(())
}

/// Revoke a profile access grant (soft-delete via revoked_at timestamp).
pub fn revoke_profile_grant(
    conn: &Connection,
    granter_id: &str,
    grantee_id: &str,
) -> Result<bool, DatabaseError> {
    let updated = conn.execute(
        "UPDATE profile_access_grants SET revoked_at = datetime('now')
         WHERE granter_profile_id = ?1 AND grantee_profile_id = ?2 AND revoked_at IS NULL",
        params![granter_id, grantee_id],
    )?;
    Ok(updated > 0)
}

/// List all active (non-revoked) grants where the given profile is the grantee.
pub fn list_grants_for_grantee(
    conn: &Connection,
    grantee_id: &str,
) -> Result<Vec<ProfileAccessGrantRow>, DatabaseError> {
    let mut stmt = conn.prepare(
        "SELECT id, granter_profile_id, grantee_profile_id, access_level, granted_at, revoked_at
         FROM profile_access_grants
         WHERE grantee_profile_id = ?1 AND revoked_at IS NULL
         ORDER BY granted_at",
    )?;

    let rows = stmt
        .query_map(params![grantee_id], |row| {
            Ok(ProfileAccessGrantRow {
                id: row.get(0)?,
                granter_profile_id: row.get(1)?,
                grantee_profile_id: row.get(2)?,
                access_level: row.get(3)?,
                granted_at: row.get(4)?,
                revoked_at: row.get(5)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(rows)
}

/// Check if an active grant exists from granter to grantee.
pub fn has_active_grant(
    conn: &Connection,
    granter_id: &str,
    grantee_id: &str,
) -> Result<Option<String>, DatabaseError> {
    let mut stmt = conn.prepare(
        "SELECT access_level FROM profile_access_grants
         WHERE granter_profile_id = ?1 AND grantee_profile_id = ?2 AND revoked_at IS NULL",
    )?;

    let level = stmt
        .query_row(params![granter_id, grantee_id], |row| {
            row.get::<_, String>(0)
        })
        .optional()?;

    Ok(level)
}

// ═══════════════════════════════════════════════════════════
// rusqlite optional helper
// ═══════════════════════════════════════════════════════════

/// Extension trait to convert NotFound into None.
trait OptionalRow<T> {
    fn optional(self) -> Result<Option<T>, rusqlite::Error>;
}

impl<T> OptionalRow<T> for Result<T, rusqlite::Error> {
    fn optional(self) -> Result<Option<T>, rusqlite::Error> {
        match self {
            Ok(val) => Ok(Some(val)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::app_db::open_memory_app_database;

    fn test_db() -> Connection {
        open_memory_app_database().unwrap()
    }

    fn sample_device(id: &str, owner: &str) -> DeviceRegistryRow {
        DeviceRegistryRow {
            device_id: id.to_string(),
            device_name: "Test Phone".to_string(),
            device_model: "iPhone 15".to_string(),
            owner_profile_id: owner.to_string(),
            public_key: vec![1, 2, 3, 4, 5],
            paired_at: "2026-02-23T10:00:00Z".to_string(),
            last_seen: "2026-02-23T10:00:00Z".to_string(),
            is_revoked: false,
        }
    }

    // ── Device registry ──────────────────────────────────────

    #[test]
    fn insert_and_get_device() {
        let conn = test_db();
        let device = sample_device("dev-1", "profile-1");
        insert_device(&conn, &device).unwrap();

        let fetched = get_device(&conn, "dev-1").unwrap().unwrap();
        assert_eq!(fetched.device_name, "Test Phone");
        assert_eq!(fetched.owner_profile_id, "profile-1");
        assert!(!fetched.is_revoked);
    }

    #[test]
    fn get_nonexistent_device_returns_none() {
        let conn = test_db();
        let result = get_device(&conn, "nonexistent").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn revoke_device_sets_flag() {
        let conn = test_db();
        insert_device(&conn, &sample_device("dev-1", "profile-1")).unwrap();

        let revoked = revoke_device(&conn, "dev-1").unwrap();
        assert!(revoked);

        let device = get_device(&conn, "dev-1").unwrap().unwrap();
        assert!(device.is_revoked);
    }

    #[test]
    fn revoke_nonexistent_device_returns_false() {
        let conn = test_db();
        let result = revoke_device(&conn, "nonexistent").unwrap();
        assert!(!result);
    }

    #[test]
    fn update_last_seen() {
        let conn = test_db();
        insert_device(&conn, &sample_device("dev-1", "profile-1")).unwrap();

        update_device_last_seen(&conn, "dev-1").unwrap();

        let device = get_device(&conn, "dev-1").unwrap().unwrap();
        // last_seen should be updated (not the original value)
        assert_ne!(device.last_seen, "2026-02-23T10:00:00Z");
    }

    #[test]
    fn list_devices_for_owner_excludes_revoked() {
        let conn = test_db();
        insert_device(&conn, &sample_device("dev-1", "profile-1")).unwrap();
        insert_device(&conn, &sample_device("dev-2", "profile-1")).unwrap();
        insert_device(&conn, &sample_device("dev-3", "profile-2")).unwrap();

        revoke_device(&conn, "dev-2").unwrap();

        let devices = list_devices_for_owner(&conn, "profile-1").unwrap();
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].device_id, "dev-1");
    }

    // ── Device profile access ────────────────────────────────

    #[test]
    fn grant_and_check_device_access() {
        let conn = test_db();
        insert_device(&conn, &sample_device("dev-1", "profile-1")).unwrap();

        grant_device_profile_access(&conn, "dev-1", "profile-1", "full").unwrap();
        grant_device_profile_access(&conn, "dev-1", "profile-2", "read_only").unwrap();

        let level1 = has_device_profile_access(&conn, "dev-1", "profile-1").unwrap();
        assert_eq!(level1.as_deref(), Some("full"));

        let level2 = has_device_profile_access(&conn, "dev-1", "profile-2").unwrap();
        assert_eq!(level2.as_deref(), Some("read_only"));

        let level3 = has_device_profile_access(&conn, "dev-1", "profile-3").unwrap();
        assert!(level3.is_none());
    }

    #[test]
    fn revoked_device_has_no_access() {
        let conn = test_db();
        insert_device(&conn, &sample_device("dev-1", "profile-1")).unwrap();
        grant_device_profile_access(&conn, "dev-1", "profile-1", "full").unwrap();

        revoke_device(&conn, "dev-1").unwrap();

        let level = has_device_profile_access(&conn, "dev-1", "profile-1").unwrap();
        assert!(level.is_none(), "Revoked device should have no access");
    }

    #[test]
    fn list_accessible_profiles_for_device() {
        let conn = test_db();
        insert_device(&conn, &sample_device("dev-1", "profile-1")).unwrap();

        grant_device_profile_access(&conn, "dev-1", "profile-1", "full").unwrap();
        grant_device_profile_access(&conn, "dev-1", "profile-2", "read_only").unwrap();

        let profiles = list_accessible_profiles(&conn, "dev-1").unwrap();
        assert_eq!(profiles.len(), 2);
    }

    #[test]
    fn revoke_device_profile_access_removes_entry() {
        let conn = test_db();
        insert_device(&conn, &sample_device("dev-1", "profile-1")).unwrap();

        grant_device_profile_access(&conn, "dev-1", "profile-1", "full").unwrap();
        let revoked = revoke_device_profile_access(&conn, "dev-1", "profile-1").unwrap();
        assert!(revoked);

        let level = has_device_profile_access(&conn, "dev-1", "profile-1").unwrap();
        assert!(level.is_none());
    }

    // ── Profile access grants ────────────────────────────────

    #[test]
    fn insert_and_check_grant() {
        let conn = test_db();
        let grant = ProfileAccessGrantRow {
            id: "grant-1".to_string(),
            granter_profile_id: "alice".to_string(),
            grantee_profile_id: "bob".to_string(),
            access_level: "read_only".to_string(),
            granted_at: "2026-02-23T10:00:00Z".to_string(),
            revoked_at: None,
        };

        insert_profile_grant(&conn, &grant).unwrap();

        let level = has_active_grant(&conn, "alice", "bob").unwrap();
        assert_eq!(level.as_deref(), Some("read_only"));
    }

    #[test]
    fn grant_is_unidirectional() {
        let conn = test_db();
        let grant = ProfileAccessGrantRow {
            id: "grant-1".to_string(),
            granter_profile_id: "alice".to_string(),
            grantee_profile_id: "bob".to_string(),
            access_level: "read_only".to_string(),
            granted_at: "2026-02-23T10:00:00Z".to_string(),
            revoked_at: None,
        };

        insert_profile_grant(&conn, &grant).unwrap();

        // alice→bob exists
        let level = has_active_grant(&conn, "alice", "bob").unwrap();
        assert!(level.is_some());

        // bob→alice does NOT exist
        let reverse = has_active_grant(&conn, "bob", "alice").unwrap();
        assert!(reverse.is_none());
    }

    #[test]
    fn revoke_grant_sets_revoked_at() {
        let conn = test_db();
        let grant = ProfileAccessGrantRow {
            id: "grant-1".to_string(),
            granter_profile_id: "alice".to_string(),
            grantee_profile_id: "bob".to_string(),
            access_level: "read_only".to_string(),
            granted_at: "2026-02-23T10:00:00Z".to_string(),
            revoked_at: None,
        };

        insert_profile_grant(&conn, &grant).unwrap();
        let revoked = revoke_profile_grant(&conn, "alice", "bob").unwrap();
        assert!(revoked);

        // Grant should no longer be active
        let level = has_active_grant(&conn, "alice", "bob").unwrap();
        assert!(level.is_none());
    }

    #[test]
    fn list_grants_for_grantee_excludes_revoked() {
        let conn = test_db();

        let grant1 = ProfileAccessGrantRow {
            id: "grant-1".to_string(),
            granter_profile_id: "alice".to_string(),
            grantee_profile_id: "bob".to_string(),
            access_level: "read_only".to_string(),
            granted_at: "2026-02-23T10:00:00Z".to_string(),
            revoked_at: None,
        };

        let grant2 = ProfileAccessGrantRow {
            id: "grant-2".to_string(),
            granter_profile_id: "charlie".to_string(),
            grantee_profile_id: "bob".to_string(),
            access_level: "full".to_string(),
            granted_at: "2026-02-23T11:00:00Z".to_string(),
            revoked_at: None,
        };

        insert_profile_grant(&conn, &grant1).unwrap();
        insert_profile_grant(&conn, &grant2).unwrap();

        // Revoke alice→bob
        revoke_profile_grant(&conn, "alice", "bob").unwrap();

        let grants = list_grants_for_grantee(&conn, "bob").unwrap();
        assert_eq!(grants.len(), 1);
        assert_eq!(grants[0].granter_profile_id, "charlie");
    }
}

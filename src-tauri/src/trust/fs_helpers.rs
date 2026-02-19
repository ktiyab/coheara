use std::path::Path;

use chrono::NaiveDateTime;
use uuid::Uuid;

use super::TrustError;

/// Calculate total size of a directory recursively.
pub fn calculate_dir_size(path: &Path) -> u64 {
    if !path.exists() {
        return 0;
    }
    walkdir(path)
}

fn walkdir(path: &Path) -> u64 {
    let mut total = 0u64;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                total += walkdir(&p);
            } else if let Ok(meta) = entry.metadata() {
                total += meta.len();
            }
        }
    }
    total
}

/// Count files and total bytes in a directory.
pub fn count_dir_contents(path: &Path) -> (u32, u64) {
    if !path.exists() {
        return (0, 0);
    }
    let mut count = 0u32;
    let mut bytes = 0u64;
    count_recursive(path, &mut count, &mut bytes);
    (count, bytes)
}

fn count_recursive(path: &Path, count: &mut u32, bytes: &mut u64) {
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                count_recursive(&p, count, bytes);
            } else if let Ok(meta) = entry.metadata() {
                *count += 1;
                *bytes += meta.len();
            }
        }
    }
}

/// Get profile name from profiles.json registry.
pub fn get_profile_name_from_dir(profiles_dir: &Path, profile_id: &Uuid) -> String {
    let registry_path = profiles_dir.join("profiles.json");
    if let Ok(data) = std::fs::read_to_string(&registry_path) {
        if let Ok(profiles) = serde_json::from_str::<Vec<serde_json::Value>>(&data) {
            for p in &profiles {
                if p.get("id").and_then(|v| v.as_str()) == Some(&profile_id.to_string()) {
                    return p
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Unknown")
                        .to_string();
                }
            }
        }
    }
    "Unknown".into()
}

/// Find the most recent .coheara-backup file in a directory.
pub fn find_latest_backup(dir: &Path) -> Result<Option<NaiveDateTime>, TrustError> {
    if !dir.exists() {
        return Ok(None);
    }

    let mut latest: Option<NaiveDateTime> = None;

    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("coheara-backup") {
                if let Ok(meta) = entry.metadata() {
                    if let Ok(modified) = meta.modified() {
                        let datetime: chrono::DateTime<chrono::Local> = modified.into();
                        let naive = datetime.naive_local();
                        if latest.is_none() || Some(naive) > latest {
                            latest = Some(naive);
                        }
                    }
                }
            }
        }
    }

    Ok(latest)
}

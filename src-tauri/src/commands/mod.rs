pub mod chat;
pub mod home;
pub mod profile;
pub mod state;

/// Health check IPC command — verifies backend is running
#[tauri::command]
pub fn health_check() -> String {
    tracing::debug!("Health check called");
    "ok".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_check_returns_ok() {
        assert_eq!(health_check(), "ok");
    }
}

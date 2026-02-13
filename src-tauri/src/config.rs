use std::path::PathBuf;

/// Application-level constants
pub const APP_NAME: &str = "Coheara";
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Get the application data directory
/// ~/Coheara/ on all platforms (user-visible, per design requirement)
pub fn app_data_dir() -> PathBuf {
    let home = dirs::home_dir().expect("Cannot determine home directory");
    home.join("Coheara")
}

/// Get the profiles directory
pub fn profiles_dir() -> PathBuf {
    app_data_dir().join("profiles")
}

/// Get the models directory (for ONNX embeddings, etc.)
pub fn models_dir() -> PathBuf {
    app_data_dir().join("models")
}

/// Get the embedding model directory (all-MiniLM-L6-v2)
pub fn embedding_model_dir() -> PathBuf {
    models_dir().join("all-MiniLM-L6-v2")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_data_dir_under_home() {
        let dir = app_data_dir();
        let home = dirs::home_dir().unwrap();
        assert!(dir.starts_with(home));
        assert!(dir.ends_with("Coheara"));
    }

    #[test]
    fn profiles_dir_under_app_data() {
        let profiles = profiles_dir();
        let app = app_data_dir();
        assert!(profiles.starts_with(app));
        assert!(profiles.ends_with("profiles"));
    }

    #[test]
    fn app_name_is_coheara() {
        assert_eq!(APP_NAME, "Coheara");
    }

    #[test]
    fn app_version_matches_cargo() {
        assert_eq!(APP_VERSION, "0.1.0");
    }
}

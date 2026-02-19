use std::path::PathBuf;

/// Application-level constants
pub const APP_NAME: &str = "Coheara";
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Returns true in development builds (debug profile).
/// Compile-time constant — zero runtime cost. Dead branches eliminated by LLVM.
pub const fn is_dev() -> bool {
    cfg!(debug_assertions)
}

/// Application data directory, isolated by environment (DAT-01).
/// Dev: ~/Coheara-dev — prevents test data from contaminating production.
/// Prod: ~/Coheara — real patient data.
pub fn app_data_dir() -> PathBuf {
    let home = dirs::home_dir().expect("Cannot determine home directory");
    if is_dev() {
        home.join("Coheara-dev")
    } else {
        home.join("Coheara")
    }
}

/// Default tracing filter per environment (OBS-01).
/// Dev: debug level — maximum visibility for iteration.
/// Prod: warn level — errors and warnings only, no data leakage.
pub fn default_log_filter() -> &'static str {
    if is_dev() {
        "coheara=debug"
    } else {
        "coheara=warn"
    }
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
    fn is_dev_reflects_debug_assertions() {
        // In test builds, debug_assertions is true
        assert!(is_dev());
    }

    #[test]
    fn app_data_dir_under_home() {
        let dir = app_data_dir();
        let home = dirs::home_dir().unwrap();
        assert!(dir.starts_with(&home));
        // In test (debug) builds, should use Coheara-dev
        assert!(dir.ends_with("Coheara-dev"));
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
        assert_eq!(APP_VERSION, "0.2.0");
    }

    #[test]
    fn default_log_filter_debug_in_dev() {
        // In test (debug) builds, should be debug level
        assert_eq!(default_log_filter(), "coheara=debug");
    }

    /// SEC-01: DevTools feature must not be in default feature set.
    /// Production builds (without --features devtools) must not include DevTools.
    #[test]
    fn devtools_feature_not_in_defaults() {
        assert!(
            !cfg!(feature = "devtools"),
            "devtools feature must not be enabled by default — SEC-01 violation"
        );
    }
}

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RecoveryStrategy {
    Retry,
    RetryWithBackoff,
    FallbackAvailable(String),
    UserActionRequired(String),
    Fatal(String),
}

/// Map an error string to a recovery strategy.
pub fn recovery_for(error: &str) -> RecoveryStrategy {
    let lower = error.to_lowercase();
    if lower.contains("encryption") || lower.contains("crypto") || lower.contains("decrypt") {
        RecoveryStrategy::Fatal(
            "Encryption error — your profile may need to be restored from backup.".into(),
        )
    } else if lower.contains("password") || lower.contains("auth") {
        RecoveryStrategy::UserActionRequired(
            "Authentication failed — please check your password.".into(),
        )
    } else if lower.contains("network") || lower.contains("connection") {
        RecoveryStrategy::UserActionRequired(
            "Check your local network connection.".into(),
        )
    } else if lower.contains("timeout") {
        RecoveryStrategy::RetryWithBackoff
    } else if lower.contains("ocr") || lower.contains("extraction") {
        RecoveryStrategy::FallbackAvailable(
            "OCR failed — you can try a clearer photo or enter information manually.".into(),
        )
    } else if lower.contains("ollama") || lower.contains("llm") || lower.contains("model") {
        RecoveryStrategy::FallbackAvailable(
            "The AI model isn't running. Start Ollama and try again.".into(),
        )
    } else {
        RecoveryStrategy::Retry
    }
}

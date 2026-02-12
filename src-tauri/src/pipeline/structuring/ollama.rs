use serde::{Deserialize, Serialize};

use super::StructuringError;
use super::types::LlmClient;

/// Preferred MedGemma models in order of preference.
const MEDGEMMA_MODELS: &[&str] = &[
    "medgemma",
    "medgemma:27b",
    "medgemma:4b",
    "medgemma:latest",
];

/// Ollama HTTP client for local LLM inference.
pub struct OllamaClient {
    base_url: String,
    client: reqwest::blocking::Client,
    timeout_secs: u64,
}

impl OllamaClient {
    /// Create a new OllamaClient pointing at a local Ollama instance.
    pub fn new(base_url: &str, timeout_secs: u64) -> Self {
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(timeout_secs))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            client,
            timeout_secs,
        }
    }

    /// Default Ollama instance at localhost:11434 with 5-minute timeout.
    pub fn default_local() -> Self {
        Self::new("http://localhost:11434", 300)
    }

    /// Find the best available MedGemma model.
    pub fn find_best_model(&self) -> Result<String, StructuringError> {
        let available = self.list_models()?;
        for preferred in MEDGEMMA_MODELS {
            if available.iter().any(|m| m.starts_with(preferred)) {
                return Ok(preferred.to_string());
            }
        }
        Err(StructuringError::NoModelAvailable)
    }
}

/// Request body for Ollama /api/generate
#[derive(Serialize)]
struct OllamaGenerateRequest<'a> {
    model: &'a str,
    prompt: &'a str,
    system: &'a str,
    stream: bool,
}

/// Response body from Ollama /api/generate
#[derive(Deserialize)]
struct OllamaGenerateResponse {
    response: String,
}

/// Response body from Ollama /api/tags
#[derive(Deserialize)]
struct OllamaTagsResponse {
    models: Vec<OllamaModel>,
}

#[derive(Deserialize)]
struct OllamaModel {
    name: String,
}

impl LlmClient for OllamaClient {
    fn generate(
        &self,
        model: &str,
        prompt: &str,
        system: &str,
    ) -> Result<String, StructuringError> {
        let url = format!("{}/api/generate", self.base_url);
        let body = OllamaGenerateRequest {
            model,
            prompt,
            system,
            stream: false,
        };

        let response = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .map_err(|e| {
                if e.is_connect() {
                    StructuringError::OllamaConnection(self.base_url.clone())
                } else if e.is_timeout() {
                    StructuringError::HttpClient(format!(
                        "Request timed out after {}s",
                        self.timeout_secs
                    ))
                } else {
                    StructuringError::HttpClient(e.to_string())
                }
            })?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().unwrap_or_default();
            return Err(StructuringError::OllamaError {
                status: status.as_u16(),
                body,
            });
        }

        let parsed: OllamaGenerateResponse = response
            .json()
            .map_err(|e| StructuringError::ResponseParsing(e.to_string()))?;

        Ok(parsed.response)
    }

    fn is_model_available(&self, model: &str) -> Result<bool, StructuringError> {
        let models = self.list_models()?;
        Ok(models.iter().any(|m| m.starts_with(model)))
    }

    fn list_models(&self) -> Result<Vec<String>, StructuringError> {
        let url = format!("{}/api/tags", self.base_url);

        let response = self.client.get(&url).send().map_err(|e| {
            if e.is_connect() {
                StructuringError::OllamaConnection(self.base_url.clone())
            } else {
                StructuringError::HttpClient(e.to_string())
            }
        })?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().unwrap_or_default();
            return Err(StructuringError::OllamaError {
                status: status.as_u16(),
                body,
            });
        }

        let parsed: OllamaTagsResponse = response
            .json()
            .map_err(|e| StructuringError::ResponseParsing(e.to_string()))?;

        Ok(parsed.models.into_iter().map(|m| m.name).collect())
    }
}

/// Mock LLM client for testing — returns a configurable response.
pub struct MockLlmClient {
    response: String,
    available_models: Vec<String>,
}

impl MockLlmClient {
    pub fn new(response: &str) -> Self {
        Self {
            response: response.to_string(),
            available_models: vec!["medgemma:latest".to_string()],
        }
    }

    pub fn with_models(mut self, models: Vec<String>) -> Self {
        self.available_models = models;
        self
    }
}

impl LlmClient for MockLlmClient {
    fn generate(
        &self,
        _model: &str,
        _prompt: &str,
        _system: &str,
    ) -> Result<String, StructuringError> {
        Ok(self.response.clone())
    }

    fn is_model_available(&self, model: &str) -> Result<bool, StructuringError> {
        Ok(self.available_models.iter().any(|m| m.starts_with(model)))
    }

    fn list_models(&self) -> Result<Vec<String>, StructuringError> {
        Ok(self.available_models.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_client_returns_configured_response() {
        let client = MockLlmClient::new("test response");
        let result = client.generate("model", "prompt", "system").unwrap();
        assert_eq!(result, "test response");
    }

    #[test]
    fn mock_client_lists_models() {
        let client = MockLlmClient::new("").with_models(vec![
            "medgemma:latest".into(),
            "llama3:8b".into(),
        ]);
        let models = client.list_models().unwrap();
        assert_eq!(models.len(), 2);
        assert!(client.is_model_available("medgemma").unwrap());
    }

    #[test]
    fn mock_client_model_not_available() {
        let client = MockLlmClient::new("").with_models(vec!["llama3:8b".into()]);
        assert!(!client.is_model_available("medgemma").unwrap());
    }

    #[test]
    fn ollama_client_constructor() {
        let client = OllamaClient::new("http://localhost:11434", 120);
        assert_eq!(client.base_url, "http://localhost:11434");
        assert_eq!(client.timeout_secs, 120);
    }

    #[test]
    fn ollama_client_trims_trailing_slash() {
        let client = OllamaClient::new("http://localhost:11434/", 60);
        assert_eq!(client.base_url, "http://localhost:11434");
    }

    #[test]
    fn default_local_uses_standard_port() {
        let client = OllamaClient::default_local();
        assert_eq!(client.base_url, "http://localhost:11434");
    }

    #[test]
    fn medgemma_model_preference_order() {
        assert_eq!(MEDGEMMA_MODELS[0], "medgemma");
        assert!(MEDGEMMA_MODELS.len() >= 3);
    }
}

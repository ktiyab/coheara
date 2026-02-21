//! Adapter bridging OllamaClient (structuring) to LlmGenerate (RAG pipeline).
//!
//! OllamaClient implements LlmClient (with model parameter).
//! DocumentRagPipeline needs LlmGenerate (without model parameter).
//! This adapter stores the model name and delegates to OllamaClient.

use super::orchestrator::LlmGenerate;
use super::RagError;
use crate::pipeline::structuring::ollama::OllamaClient;
use crate::pipeline::structuring::types::LlmClient;

/// RAG-compatible LLM generator backed by a local Ollama instance.
///
/// Wraps `OllamaClient` with a fixed model name so it satisfies
/// the `LlmGenerate` trait expected by `DocumentRagPipeline`.
pub struct OllamaRagGenerator {
    client: OllamaClient,
    model: String,
}

impl OllamaRagGenerator {
    /// Create a new generator with explicit model name.
    pub fn new(client: OllamaClient, model: String) -> Self {
        Self { client, model }
    }

    /// Create a generator with a pre-resolved model name and default local client.
    ///
    /// Use `ActiveModelResolver` to resolve the model name before calling this.
    /// Returns `None` if the model is not actually available on Ollama.
    pub fn with_resolved_model(model: String) -> Option<Self> {
        let client = OllamaClient::default_local();
        match client.is_model_available(&model) {
            Ok(true) => {
                tracing::info!(model = %model, "Ollama RAG generator: model confirmed");
                Some(Self::new(client, model))
            }
            Ok(false) => {
                tracing::debug!(model = %model, "Ollama RAG generator: model not available");
                None
            }
            Err(e) => {
                tracing::debug!(error = %e, "Ollama RAG generator: cannot reach Ollama");
                None
            }
        }
    }

    /// The model name being used.
    pub fn model(&self) -> &str {
        &self.model
    }
}

impl LlmGenerate for OllamaRagGenerator {
    fn generate(&self, system: &str, prompt: &str) -> Result<String, RagError> {
        self.client
            .generate(&self.model, prompt, system)
            .map_err(|e| RagError::OllamaConnection(e.to_string()))
    }

    fn generate_streaming(
        &self,
        system: &str,
        prompt: &str,
        token_tx: std::sync::mpsc::Sender<String>,
    ) -> Result<String, RagError> {
        self.client
            .generate_streaming(&self.model, prompt, system, token_tx)
            .map_err(|e| RagError::OllamaConnection(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify the adapter compiles and satisfies the LlmGenerate trait.
    /// (Integration with real Ollama is tested in the structuring module.)
    #[test]
    fn adapter_satisfies_llm_generate_trait() {
        fn _accepts_llm_generate<G: LlmGenerate>(_g: &G) {}

        // This is a compile-time check — we can't connect to Ollama in tests.
        // The function signature proves the trait bound is satisfied.
        let _: fn(&OllamaRagGenerator) = _accepts_llm_generate;
    }
}

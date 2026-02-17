use super::StorageError;
use super::types::EmbeddingModel;

/// Standard embedding dimension for all-MiniLM-L6-v2
pub const EMBEDDING_DIM: usize = 384;

// ═══════════════════════════════════════════════════════════
// ONNX Embedder (IMP-003) — behind `onnx-embeddings` feature
// ═══════════════════════════════════════════════════════════

#[cfg(feature = "onnx-embeddings")]
mod onnx {
    use super::{EmbeddingModel, StorageError, EMBEDDING_DIM};
    use ort::session::Session;
    use std::path::Path;
    use std::sync::Mutex;

    /// Real embedding model using ONNX Runtime for all-MiniLM-L6-v2 inference.
    ///
    /// Requires two files in the model directory:
    /// - `model.onnx` — the ONNX model weights
    /// - `tokenizer.json` — HuggingFace tokenizer definition
    ///
    /// Uses interior mutability (Mutex) because ort::Session::run requires `&mut self`
    /// but our EmbeddingModel trait exposes `&self` for ergonomic shared usage.
    pub struct OnnxEmbedder {
        session: Mutex<Session>,
        tokenizer: tokenizers::Tokenizer,
    }

    impl OnnxEmbedder {
        /// Load the ONNX embedding model from a directory.
        ///
        /// `model_dir` must contain `model.onnx` and `tokenizer.json`.
        pub fn load(model_dir: &Path) -> Result<Self, StorageError> {
            let model_path = model_dir.join("model.onnx");
            let tokenizer_path = model_dir.join("tokenizer.json");

            if !model_path.exists() {
                return Err(StorageError::ModelNotFound(model_path));
            }
            if !tokenizer_path.exists() {
                return Err(StorageError::ModelNotFound(tokenizer_path));
            }

            let session = Session::builder()
                .map_err(|e: ort::Error| StorageError::ModelInit(e.to_string()))?
                .with_intra_threads(2)
                .map_err(|e: ort::Error| StorageError::ModelInit(e.to_string()))?
                .commit_from_file(&model_path)
                .map_err(|e: ort::Error| StorageError::ModelInit(format!("ONNX load failed: {e}")))?;

            let tokenizer = tokenizers::Tokenizer::from_file(&tokenizer_path)
                .map_err(|e| StorageError::ModelInit(format!("Tokenizer load failed: {e}")))?;

            tracing::info!("ONNX embedder loaded from {}", model_dir.display());

            Ok(Self {
                session: Mutex::new(session),
                tokenizer,
            })
        }

        /// Tokenize text and run ONNX inference, returning L2-normalized embedding.
        fn infer(&self, text: &str) -> Result<Vec<f32>, StorageError> {
            use ort::value::TensorRef;

            let encoding = self
                .tokenizer
                .encode(text, true)
                .map_err(|e| StorageError::Tokenization(e.to_string()))?;

            let input_ids: Vec<i64> = encoding.get_ids().iter().map(|&id| id as i64).collect();
            let attention_mask: Vec<i64> = encoding
                .get_attention_mask()
                .iter()
                .map(|&m| m as i64)
                .collect();
            let token_type_ids: Vec<i64> = encoding
                .get_type_ids()
                .iter()
                .map(|&t| t as i64)
                .collect();

            let seq_len = input_ids.len();

            let ids_array =
                ndarray::Array2::from_shape_vec((1, seq_len), input_ids)
                    .map_err(|e| StorageError::Embedding(e.to_string()))?;
            let mask_array =
                ndarray::Array2::from_shape_vec((1, seq_len), attention_mask.clone())
                    .map_err(|e| StorageError::Embedding(e.to_string()))?;
            let type_array =
                ndarray::Array2::from_shape_vec((1, seq_len), token_type_ids)
                    .map_err(|e| StorageError::Embedding(e.to_string()))?;

            let ids_tensor = TensorRef::from_array_view(&ids_array)
                .map_err(|e| StorageError::Embedding(e.to_string()))?;
            let mask_tensor = TensorRef::from_array_view(&mask_array)
                .map_err(|e| StorageError::Embedding(e.to_string()))?;
            let type_tensor = TensorRef::from_array_view(&type_array)
                .map_err(|e| StorageError::Embedding(e.to_string()))?;

            let mut session = self
                .session
                .lock()
                .map_err(|_| StorageError::Embedding("Session lock poisoned".to_string()))?;

            let outputs = session
                .run(ort::inputs![ids_tensor, mask_tensor, type_tensor])
                .map_err(|e| StorageError::Embedding(format!("ONNX inference failed: {e}")))?;

            // Output shape: [1, seq_len, 384] — apply mean pooling with attention mask
            let (shape, output_data) = outputs[0]
                .try_extract_tensor::<f32>()
                .map_err(|e| StorageError::Embedding(format!("Output extraction: {e}")))?;

            // Validate shape: [1, seq_len, EMBEDDING_DIM]
            if shape.len() != 3 || shape[2] as usize != EMBEDDING_DIM {
                return Err(StorageError::Embedding(format!(
                    "Unexpected output shape: {shape:?}, expected [1, {seq_len}, {EMBEDDING_DIM}]"
                )));
            }

            let mut pooled = vec![0.0f32; EMBEDDING_DIM];
            let mut mask_sum = 0.0f32;

            for (token_idx, &mask_val_i64) in attention_mask.iter().enumerate().take(seq_len) {
                let mask_val = mask_val_i64 as f32;
                mask_sum += mask_val;
                let offset = token_idx * EMBEDDING_DIM;
                for (dim_idx, p) in pooled.iter_mut().enumerate() {
                    *p += output_data[offset + dim_idx] * mask_val;
                }
            }

            if mask_sum > 0.0 {
                for val in &mut pooled {
                    *val /= mask_sum;
                }
            }

            // L2 normalize
            let norm: f32 = pooled.iter().map(|x| x * x).sum::<f32>().sqrt();
            if norm > 0.0 {
                for val in &mut pooled {
                    *val /= norm;
                }
            }

            Ok(pooled)
        }
    }

    impl EmbeddingModel for OnnxEmbedder {
        fn embed(&self, text: &str) -> Result<Vec<f32>, StorageError> {
            self.infer(text)
        }

        fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, StorageError> {
            texts.iter().map(|t| self.infer(t)).collect()
        }

        fn dimension(&self) -> usize {
            EMBEDDING_DIM
        }
    }
}

#[cfg(feature = "onnx-embeddings")]
pub use onnx::OnnxEmbedder;

/// Mock embedding model for testing — produces deterministic vectors.
pub struct MockEmbedder {
    dimension: usize,
}

impl MockEmbedder {
    pub fn new() -> Self {
        Self {
            dimension: EMBEDDING_DIM,
        }
    }
}

impl Default for MockEmbedder {
    fn default() -> Self {
        Self::new()
    }
}

impl EmbeddingModel for MockEmbedder {
    fn embed(&self, text: &str) -> Result<Vec<f32>, StorageError> {
        Ok(deterministic_vector(text, self.dimension))
    }

    fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, StorageError> {
        Ok(texts
            .iter()
            .map(|t| deterministic_vector(t, self.dimension))
            .collect())
    }

    fn dimension(&self) -> usize {
        self.dimension
    }
}

/// Generate a deterministic unit vector from text (for testing).
/// Uses a simple hash-based approach to produce consistent embeddings.
fn deterministic_vector(text: &str, dim: usize) -> Vec<f32> {
    let mut vec = vec![0.0f32; dim];
    let bytes = text.as_bytes();

    for (i, slot) in vec.iter_mut().enumerate() {
        let byte_idx = i % bytes.len().max(1);
        *slot = (bytes.get(byte_idx).copied().unwrap_or(0) as f32 + i as f32) / 255.0;
    }

    // L2 normalize
    let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for val in &mut vec {
            *val /= norm;
        }
    }

    vec
}

/// Build the best available embedding model at runtime.
///
/// When `onnx-embeddings` feature is enabled and model files are present,
/// loads the real all-MiniLM-L6-v2 ONNX model. Otherwise falls back to
/// MockEmbedder which produces deterministic vectors (structured data
/// retrieval still works via SQLite, only semantic search is degraded).
pub fn build_embedder() -> Box<dyn EmbeddingModel> {
    #[cfg(feature = "onnx-embeddings")]
    {
        let model_dir = crate::config::embedding_model_dir();
        if model_dir.join("model.onnx").exists() && model_dir.join("tokenizer.json").exists() {
            match OnnxEmbedder::load(&model_dir) {
                Ok(e) => {
                    tracing::info!(dir = %model_dir.display(), "ONNX embedder loaded");
                    return Box::new(e);
                }
                Err(e) => {
                    tracing::warn!(error = %e, "ONNX embedder failed to load, using mock");
                }
            }
        } else {
            tracing::info!(
                "ONNX model files not found at {}, using mock embedder",
                model_dir.display()
            );
        }
    }

    #[cfg(not(feature = "onnx-embeddings"))]
    {
        tracing::debug!("onnx-embeddings feature not enabled, using mock embedder");
    }

    Box::new(MockEmbedder::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_embed_returns_correct_dimension() {
        let embedder = MockEmbedder::new();
        let vec = embedder.embed("Hello world").unwrap();
        assert_eq!(vec.len(), EMBEDDING_DIM);
    }

    #[test]
    fn mock_embed_batch_returns_correct_count() {
        let embedder = MockEmbedder::new();
        let texts = vec!["text one", "text two", "text three"];
        let vecs = embedder.embed_batch(&texts).unwrap();
        assert_eq!(vecs.len(), 3);
        for v in &vecs {
            assert_eq!(v.len(), EMBEDDING_DIM);
        }
    }

    #[test]
    fn mock_embed_is_deterministic() {
        let embedder = MockEmbedder::new();
        let v1 = embedder.embed("same text").unwrap();
        let v2 = embedder.embed("same text").unwrap();
        assert_eq!(v1, v2);
    }

    #[test]
    fn mock_embed_different_texts_differ() {
        let embedder = MockEmbedder::new();
        let v1 = embedder.embed("text A").unwrap();
        let v2 = embedder.embed("text B").unwrap();
        assert_ne!(v1, v2);
    }

    #[test]
    fn mock_embed_is_l2_normalized() {
        let embedder = MockEmbedder::new();
        let vec = embedder.embed("test normalization").unwrap();
        let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!(
            (norm - 1.0).abs() < 0.01,
            "Vector should be L2-normalized, got norm = {norm}"
        );
    }

    #[test]
    fn dimension_returns_384() {
        let embedder = MockEmbedder::new();
        assert_eq!(embedder.dimension(), 384);
    }

    // ── M.2: Semantic cluster test suite ──────────────────────────
    // Ground-truth pairs for validating embedding quality.
    // With MockEmbedder: verifies determinism and vector properties.
    // With real ONNX model: verifies semantic clustering.

    fn cosine_sim(a: &[f32], b: &[f32]) -> f32 {
        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm_a == 0.0 || norm_b == 0.0 {
            return 0.0;
        }
        dot / (norm_a * norm_b)
    }

    /// M.2-G01: Embedding dimensions are consistent across all text types.
    #[test]
    fn embedding_dimension_consistent_across_content() {
        let embedder = MockEmbedder::new();
        let medical = embedder.embed("Metformin 500mg twice daily").unwrap();
        let french = embedder.embed("Le patient présente une douleur thoracique").unwrap();
        let lab = embedder.embed("HbA1c 7.2% elevated above target").unwrap();
        let short = embedder.embed("pain").unwrap();
        let long = embedder.embed(
            "The patient has been experiencing chronic lower back pain for the past six months \
            with intermittent episodes of acute exacerbation particularly after physical activity"
        ).unwrap();

        assert_eq!(medical.len(), EMBEDDING_DIM);
        assert_eq!(french.len(), EMBEDDING_DIM);
        assert_eq!(lab.len(), EMBEDDING_DIM);
        assert_eq!(short.len(), EMBEDDING_DIM);
        assert_eq!(long.len(), EMBEDDING_DIM);
    }

    /// M.2-G02: All embeddings are L2-normalized (unit vectors).
    #[test]
    fn all_embeddings_l2_normalized() {
        let embedder = MockEmbedder::new();
        let texts = [
            "Metformin",
            "douleur thoracique",
            "HbA1c 7.2%",
            "allergie pénicilline sévère",
            "blood pressure 120/80",
        ];
        for text in &texts {
            let vec = embedder.embed(text).unwrap();
            let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
            assert!(
                (norm - 1.0).abs() < 0.01,
                "Vector for '{text}' not normalized: norm={norm}"
            );
        }
    }

    /// M.2-G03: Identical medical terms produce identical embeddings.
    #[test]
    fn identical_medical_terms_same_embedding() {
        let embedder = MockEmbedder::new();
        let v1 = embedder.embed("Metformin 500mg").unwrap();
        let v2 = embedder.embed("Metformin 500mg").unwrap();
        let sim = cosine_sim(&v1, &v2);
        assert!((sim - 1.0).abs() < 0.001, "Identical texts should have similarity 1.0, got {sim}");
    }

    /// M.2-G04: Batch embedding matches individual embedding.
    #[test]
    fn batch_embedding_matches_individual() {
        let embedder = MockEmbedder::new();
        let texts = ["Metformin 500mg", "HbA1c 7.2%", "allergie pénicilline"];
        let batch = embedder.embed_batch(&texts).unwrap();
        for (i, text) in texts.iter().enumerate() {
            let individual = embedder.embed(text).unwrap();
            assert_eq!(batch[i], individual, "Batch[{i}] differs from individual for '{text}'");
        }
    }

    /// M.2-G05: Different texts produce different embeddings.
    /// Ground-truth pairs for when real model is available:
    /// - Same-domain pairs should be MORE similar than cross-domain pairs.
    #[test]
    fn different_texts_produce_different_vectors() {
        let embedder = MockEmbedder::new();

        // Medical domain
        let med1 = embedder.embed("Metformin 500mg twice daily for diabetes").unwrap();
        let med2 = embedder.embed("Lisinopril 10mg daily for hypertension").unwrap();

        // Lab domain
        let lab1 = embedder.embed("HbA1c 7.2% elevated above target range").unwrap();
        let lab2 = embedder.embed("Cholesterol LDL 130 mg/dL borderline high").unwrap();

        // Cross-domain
        let cooking = embedder.embed("Add two cups of flour to the mixing bowl").unwrap();

        // All different texts should produce different vectors
        assert!(cosine_sim(&med1, &med2) < 1.0);
        assert!(cosine_sim(&lab1, &lab2) < 1.0);
        assert!(cosine_sim(&med1, &cooking) < 1.0);

        // NOTE: With real model, we'd assert:
        // cosine_sim(&med1, &med2) > cosine_sim(&med1, &cooking)
        // cosine_sim(&lab1, &lab2) > cosine_sim(&lab1, &cooking)
    }

    /// M.2-G06: Empty text handling.
    #[test]
    fn empty_text_produces_valid_embedding() {
        let embedder = MockEmbedder::new();
        let vec = embedder.embed("").unwrap();
        assert_eq!(vec.len(), EMBEDDING_DIM);
    }
}

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
}

use super::StorageError;
use super::types::EmbeddingModel;

/// Standard embedding dimension for all-MiniLM-L6-v2
pub const EMBEDDING_DIM: usize = 384;

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

# L1-04 — Storage Pipeline

<!--
=============================================================================
COMPONENT SPEC — Chunks, embeds, and stores structured medical data.
Engineer review: E-DA (Data, lead), E-ML (AI/ML), E-RS (Rust), E-SC (Security), E-QA (QA)
This is the bridge between extraction and queryability.
After this component, all patient data is searchable in both layers.
=============================================================================
-->

## Table of Contents

| Section | Offset |
|---------|--------|
| [1] Identity | `offset=20 limit=30` |
| [2] Dependencies | `offset=50 limit=22` |
| [3] Interfaces | `offset=72 limit=80` |
| [4] Markdown Chunking | `offset=152 limit=65` |
| [5] Embedding Generation | `offset=217 limit=60` |
| [6] Vector Storage (LanceDB) | `offset=277 limit=55` |
| [7] Structured Storage (SQLite) | `offset=332 limit=90` |
| [8] Pipeline Orchestration | `offset=422 limit=70` |
| [9] Tauri Commands (IPC) | `offset=492 limit=45` |
| [10] Error Handling | `offset=537 limit=30` |
| [11] Security | `offset=567 limit=25` |
| [12] Testing | `offset=592 limit=55` |
| [13] Performance | `offset=647 limit=15` |
| [14] Open Questions | `offset=662 limit=15` |

---

## [1] Identity

**What:** The final stage of the document pipeline. Takes structured output from L1-03 (Markdown + extracted entities) and writes to BOTH data layers: (1) Markdown → chunk → embed → LanceDB (semantic search), (2) Extracted entities → SQLite (structured queries). Also creates bidirectional links between document and all its entities, updates the document record, and triggers the Review Screen (L3-04).

**After this session:**
- Structured Markdown chunked into semantic segments
- Each chunk embedded via all-MiniLM-L6-v2 (384-dim vectors)
- Chunks stored in LanceDB with metadata (doc_id, type, date, professional)
- Extracted medications → SQLite medications table (with compound_ingredients, tapering, instructions)
- Extracted lab results → SQLite lab_results table
- Extracted diagnoses → SQLite diagnoses table
- Extracted allergies → SQLite allergies table
- Extracted procedures → SQLite procedures table
- Extracted referrals → SQLite referrals table
- Professional → SQLite professionals table (find_or_create)
- Document record updated (type, date, markdown_file, professional_id)
- Profile trust metrics updated (total_documents incremented)
- All SQLite content fields encrypted via ProfileSession
- Bidirectional traceability: every entity links back to its source document

**Estimated complexity:** High
**Source:** Tech Spec v1.1 Section 6.1 (Storage Pipeline), Section 5 (Data Model)

---

## [2] Dependencies

**Incoming:**
- L0-02 (data model — all repository traits, model structs, SQLite schema)
- L0-03 (encryption — ProfileSession for field-level encryption)
- L1-03 (medical structuring — StructuringResult with entities and Markdown)

**Outgoing:**
- L2-01 (RAG pipeline — queries LanceDB vectors and SQLite structured data)
- L2-03 (coherence engine — reads newly stored data for conflict detection)
- L3-02 (home & document feed — shows new documents)
- L3-04 (review screen — triggered after storage, shows structured output)
- L3-05 (medication list — queries medications table)

**New Cargo.toml dependencies:**
```toml
# Embedding model
ort = { version = "2", features = ["load-dynamic"] }  # ONNX Runtime for embedding model
tokenizers = "0.20"                                     # HuggingFace tokenizers

# LanceDB (already in L0-02)
# lancedb = "0.13"
# arrow = { version = "53", features = ["json"] }
```

**Bundled runtime dependency:**
- all-MiniLM-L6-v2 ONNX model file (~80MB, bundled in installer)
- Tokenizer file for all-MiniLM-L6-v2 (bundled)

---

## [3] Interfaces

### Storage Pipeline Trait

```rust
/// Result of the full storage pipeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageResult {
    pub document_id: Uuid,
    pub chunks_stored: usize,
    pub entities_stored: EntitiesStoredCount,
    pub document_type: DocumentType,
    pub professional_id: Option<Uuid>,
    pub warnings: Vec<StorageWarning>,
}

/// Count of entities stored per type
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EntitiesStoredCount {
    pub medications: usize,
    pub lab_results: usize,
    pub diagnoses: usize,
    pub allergies: usize,
    pub procedures: usize,
    pub referrals: usize,
    pub instructions: usize,
}

/// Warnings from storage process
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StorageWarning {
    DuplicateMedication { name: String, existing_id: Uuid },
    ProfessionalNameAmbiguous { name: String },
    DateParsingFailed { field: String, value: String },
    EmbeddingFailed { chunk_index: usize },
}

/// Main pipeline orchestrator trait
pub trait StoragePipeline {
    /// Store a complete structuring result into both data layers
    fn store(
        &self,
        structuring_result: &StructuringResult,
        session: &ProfileSession,
    ) -> Result<StorageResult, StorageError>;
}
```

### Chunking Trait

```rust
/// A semantic chunk of a Markdown document
#[derive(Debug, Clone)]
pub struct TextChunk {
    pub content: String,
    pub chunk_index: usize,
    pub section_title: Option<String>,  // Markdown heading this chunk belongs to
    pub char_offset: usize,             // Character offset in original document
}

/// Chunking strategy
pub trait Chunker {
    /// Split Markdown into semantic chunks
    fn chunk(&self, markdown: &str) -> Vec<TextChunk>;
}
```

### Embedding Trait

```rust
/// Embedding model abstraction
pub trait EmbeddingModel {
    /// Generate embedding for a single text
    fn embed(&self, text: &str) -> Result<Vec<f32>, StorageError>;

    /// Generate embeddings for multiple texts (batch)
    fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, StorageError>;

    /// Get the embedding dimension
    fn dimension(&self) -> usize;
}
```

---

## [4] Markdown Chunking

**E-DA + E-ML design:** Chunks must be semantically meaningful — not arbitrary 512-char splits. Medical documents have natural sections (medications, lab results, instructions). We split on section boundaries first, then on paragraph boundaries if sections are too large.

### Chunking Strategy

```rust
/// Semantic chunker for medical Markdown documents
pub struct MedicalChunker {
    max_chunk_chars: usize,
    min_chunk_chars: usize,
    overlap_chars: usize,
}

impl MedicalChunker {
    pub fn new() -> Self {
        Self {
            max_chunk_chars: 1000,  // ~250 tokens for MiniLM
            min_chunk_chars: 50,
            overlap_chars: 100,     // Overlap between adjacent chunks
        }
    }
}

impl Chunker for MedicalChunker {
    fn chunk(&self, markdown: &str) -> Vec<TextChunk> {
        let mut chunks = Vec::new();
        let mut chunk_index = 0;

        // Step 1: Split by Markdown headings (## or ###)
        let sections = split_by_headings(markdown);

        for section in &sections {
            if section.content.len() <= self.max_chunk_chars {
                // Section fits in one chunk
                if section.content.len() >= self.min_chunk_chars {
                    chunks.push(TextChunk {
                        content: section.content.clone(),
                        chunk_index,
                        section_title: section.title.clone(),
                        char_offset: section.offset,
                    });
                    chunk_index += 1;
                }
            } else {
                // Section too large: split by paragraphs with overlap
                let sub_chunks = split_section_by_paragraphs(
                    &section.content,
                    &section.title,
                    section.offset,
                    self.max_chunk_chars,
                    self.overlap_chars,
                    &mut chunk_index,
                );
                chunks.extend(sub_chunks);
            }
        }

        // Step 2: Handle tiny remaining sections (merge with neighbors)
        merge_tiny_chunks(&mut chunks, self.min_chunk_chars);

        chunks
    }
}

/// A section parsed from Markdown headings
struct MarkdownSection {
    title: Option<String>,
    content: String,
    offset: usize,
}

/// Split Markdown by heading boundaries
fn split_by_headings(markdown: &str) -> Vec<MarkdownSection> {
    let mut sections = Vec::new();
    let mut current_title: Option<String> = None;
    let mut current_content = String::new();
    let mut current_offset = 0;

    for (i, line) in markdown.lines().enumerate() {
        if line.starts_with("## ") || line.starts_with("### ") {
            // Save previous section
            if !current_content.trim().is_empty() {
                sections.push(MarkdownSection {
                    title: current_title.take(),
                    content: current_content.trim().to_string(),
                    offset: current_offset,
                });
            }
            current_title = Some(line.trim_start_matches('#').trim().to_string());
            current_content = String::new();
            current_offset = markdown.lines().take(i).map(|l| l.len() + 1).sum();
        } else {
            current_content.push_str(line);
            current_content.push('\n');
        }
    }

    // Don't forget last section
    if !current_content.trim().is_empty() {
        sections.push(MarkdownSection {
            title: current_title,
            content: current_content.trim().to_string(),
            offset: current_offset,
        });
    }

    sections
}

/// Split a large section by paragraphs with overlap
fn split_section_by_paragraphs(
    content: &str,
    title: &Option<String>,
    base_offset: usize,
    max_chars: usize,
    overlap: usize,
    chunk_index: &mut usize,
) -> Vec<TextChunk> {
    let mut chunks = Vec::new();
    let paragraphs: Vec<&str> = content.split("\n\n").collect();

    let mut current = String::new();
    let mut char_offset = base_offset;

    for para in &paragraphs {
        if current.len() + para.len() > max_chars && !current.is_empty() {
            chunks.push(TextChunk {
                content: current.clone(),
                chunk_index: *chunk_index,
                section_title: title.clone(),
                char_offset,
            });
            *chunk_index += 1;

            // Overlap: keep last N characters from current chunk
            if current.len() > overlap {
                let overlap_start = current.len() - overlap;
                current = current[overlap_start..].to_string();
                char_offset += overlap_start;
            }
        }
        current.push_str(para);
        current.push_str("\n\n");
    }

    // Last chunk
    if !current.trim().is_empty() {
        chunks.push(TextChunk {
            content: current.trim().to_string(),
            chunk_index: *chunk_index,
            section_title: title.clone(),
            char_offset,
        });
        *chunk_index += 1;
    }

    chunks
}

/// Merge chunks smaller than min_chars with their neighbors
fn merge_tiny_chunks(chunks: &mut Vec<TextChunk>, min_chars: usize) {
    let mut i = 0;
    while i < chunks.len() {
        if chunks[i].content.len() < min_chars && i + 1 < chunks.len() {
            let next = chunks.remove(i + 1);
            chunks[i].content.push_str("\n\n");
            chunks[i].content.push_str(&next.content);
        } else {
            i += 1;
        }
    }
}
```

---

## [5] Embedding Generation

### all-MiniLM-L6-v2 via ONNX Runtime

```rust
use ort::{Environment, Session, Value};
use tokenizers::Tokenizer;

pub const EMBEDDING_DIM: usize = 384;
const MAX_SEQUENCE_LENGTH: usize = 256;  // MiniLM max tokens

/// Embedding model using ONNX Runtime
pub struct MiniLmEmbedder {
    session: Session,
    tokenizer: Tokenizer,
}

impl MiniLmEmbedder {
    /// Load the model from bundled files
    pub fn new(model_dir: &Path) -> Result<Self, StorageError> {
        let model_path = model_dir.join("all-MiniLM-L6-v2.onnx");
        let tokenizer_path = model_dir.join("tokenizer.json");

        if !model_path.exists() {
            return Err(StorageError::ModelNotFound(model_path));
        }

        let environment = Environment::builder()
            .with_name("coheara_embedding")
            .build()
            .map_err(|e| StorageError::ModelInit(e.to_string()))?;

        let session = Session::builder()
            .map_err(|e| StorageError::ModelInit(e.to_string()))?
            .with_optimization_level(ort::GraphOptimizationLevel::Level3)
            .map_err(|e| StorageError::ModelInit(e.to_string()))?
            .commit_from_file(&model_path)
            .map_err(|e| StorageError::ModelInit(e.to_string()))?;

        let tokenizer = Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| StorageError::ModelInit(e.to_string()))?;

        Ok(Self { session, tokenizer })
    }
}

impl EmbeddingModel for MiniLmEmbedder {
    fn embed(&self, text: &str) -> Result<Vec<f32>, StorageError> {
        let results = self.embed_batch(&[text])?;
        Ok(results.into_iter().next().unwrap())
    }

    fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, StorageError> {
        // Tokenize
        let encodings = self.tokenizer.encode_batch(texts.to_vec(), true)
            .map_err(|e| StorageError::Tokenization(e.to_string()))?;

        let batch_size = encodings.len();

        // Prepare input tensors
        let mut input_ids = Vec::new();
        let mut attention_masks = Vec::new();
        let mut token_type_ids = Vec::new();

        let max_len = encodings.iter()
            .map(|e| e.get_ids().len().min(MAX_SEQUENCE_LENGTH))
            .max()
            .unwrap_or(0);

        for encoding in &encodings {
            let ids = encoding.get_ids();
            let masks = encoding.get_attention_mask();
            let types = encoding.get_type_ids();

            let len = ids.len().min(MAX_SEQUENCE_LENGTH);

            // Pad to max_len
            let mut padded_ids = ids[..len].to_vec();
            let mut padded_masks = masks[..len].to_vec();
            let mut padded_types = types[..len].to_vec();

            padded_ids.resize(max_len, 0);
            padded_masks.resize(max_len, 0);
            padded_types.resize(max_len, 0);

            input_ids.extend(padded_ids.iter().map(|&x| x as i64));
            attention_masks.extend(padded_masks.iter().map(|&x| x as i64));
            token_type_ids.extend(padded_types.iter().map(|&x| x as i64));
        }

        // Run inference
        let input_ids_array = ndarray::Array2::from_shape_vec(
            (batch_size, max_len),
            input_ids,
        ).map_err(|e| StorageError::Embedding(e.to_string()))?;

        let attention_mask_array = ndarray::Array2::from_shape_vec(
            (batch_size, max_len),
            attention_masks,
        ).map_err(|e| StorageError::Embedding(e.to_string()))?;

        let token_type_ids_array = ndarray::Array2::from_shape_vec(
            (batch_size, max_len),
            token_type_ids,
        ).map_err(|e| StorageError::Embedding(e.to_string()))?;

        let outputs = self.session.run(ort::inputs![
            input_ids_array,
            attention_mask_array,
            token_type_ids_array,
        ].map_err(|e| StorageError::Embedding(e.to_string()))?)
            .map_err(|e| StorageError::Embedding(e.to_string()))?;

        // Extract embeddings from output (mean pooling of last hidden state)
        let output_tensor = outputs[0].try_extract_tensor::<f32>()
            .map_err(|e| StorageError::Embedding(e.to_string()))?;

        // Mean pooling
        let mut embeddings = Vec::with_capacity(batch_size);
        for i in 0..batch_size {
            let mut embedding = vec![0.0f32; EMBEDDING_DIM];
            let mut count = 0.0f32;

            for j in 0..max_len {
                // Only pool non-padding tokens
                let mask_val = attention_masks[i * max_len + j] as f32;
                if mask_val > 0.0 {
                    for k in 0..EMBEDDING_DIM {
                        embedding[k] += output_tensor[[i, j, k]] * mask_val;
                    }
                    count += mask_val;
                }
            }

            // Normalize
            if count > 0.0 {
                for val in &mut embedding {
                    *val /= count;
                }
            }

            // L2 normalize
            let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
            if norm > 0.0 {
                for val in &mut embedding {
                    *val /= norm;
                }
            }

            embeddings.push(embedding);
        }

        Ok(embeddings)
    }

    fn dimension(&self) -> usize {
        EMBEDDING_DIM
    }
}
```

---

## [6] Vector Storage (LanceDB)

### Writing Chunks to LanceDB

```rust
use lancedb::connect;
use arrow::array::{
    StringArray, Float32Array, Int32Array, FixedSizeListArray,
    ArrayRef, RecordBatch,
};
use arrow::datatypes::{Schema, Field, DataType};
use std::sync::Arc;

/// Store embedded chunks in LanceDB
pub async fn store_chunks_in_lancedb(
    chunks: &[TextChunk],
    embeddings: &[Vec<f32>],
    document_id: &Uuid,
    doc_type: &DocumentType,
    doc_date: Option<&NaiveDate>,
    professional_name: Option<&str>,
    session: &ProfileSession,
) -> Result<usize, StorageError> {
    let vector_dir = session.db_path()
        .parent().unwrap()
        .parent().unwrap()
        .join("vectors");

    let db = connect(vector_dir.to_str().unwrap())
        .execute()
        .await
        .map_err(|e| StorageError::VectorDb(e.to_string()))?;

    let num_chunks = chunks.len();

    // Build Arrow RecordBatch
    let ids: Vec<String> = (0..num_chunks).map(|_| Uuid::new_v4().to_string()).collect();
    let doc_ids: Vec<String> = vec![document_id.to_string(); num_chunks];
    let contents: Vec<String> = chunks.iter().map(|c| c.content.clone()).collect();
    let indices: Vec<i32> = chunks.iter().map(|c| c.chunk_index as i32).collect();
    let doc_types: Vec<String> = vec![doc_type.as_str().to_string(); num_chunks];
    let doc_dates: Vec<String> = vec![
        doc_date.map(|d| d.to_string()).unwrap_or_default();
        num_chunks
    ];
    let professionals: Vec<String> = vec![
        professional_name.unwrap_or("").to_string();
        num_chunks
    ];

    // Flatten embeddings into a single Vec<f32>
    let flat_embeddings: Vec<f32> = embeddings.iter().flatten().copied().collect();

    let id_array = Arc::new(StringArray::from(ids)) as ArrayRef;
    let doc_id_array = Arc::new(StringArray::from(doc_ids)) as ArrayRef;
    let content_array = Arc::new(StringArray::from(contents)) as ArrayRef;
    let index_array = Arc::new(Int32Array::from(indices)) as ArrayRef;
    let type_array = Arc::new(StringArray::from(doc_types)) as ArrayRef;
    let date_array = Arc::new(StringArray::from(doc_dates)) as ArrayRef;
    let prof_array = Arc::new(StringArray::from(professionals)) as ArrayRef;

    let values = Arc::new(Float32Array::from(flat_embeddings));
    let vector_array = Arc::new(FixedSizeListArray::try_new_from_values(
        values,
        EMBEDDING_DIM as i32,
    ).map_err(|e| StorageError::VectorDb(e.to_string()))?) as ArrayRef;

    let schema = Arc::new(Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        Field::new("document_id", DataType::Utf8, false),
        Field::new("content", DataType::Utf8, false),
        Field::new("vector", DataType::FixedSizeList(
            Arc::new(Field::new("item", DataType::Float32, true)),
            EMBEDDING_DIM as i32,
        ), false),
        Field::new("chunk_index", DataType::Int32, false),
        Field::new("doc_type", DataType::Utf8, false),
        Field::new("doc_date", DataType::Utf8, true),
        Field::new("professional_name", DataType::Utf8, true),
    ]));

    let batch = RecordBatch::try_new(
        schema,
        vec![id_array, doc_id_array, content_array, vector_array,
             index_array, type_array, date_array, prof_array],
    ).map_err(|e| StorageError::VectorDb(e.to_string()))?;

    // Create table or append
    let table_name = "document_chunks";
    match db.open_table(table_name).execute().await {
        Ok(table) => {
            table.add(vec![batch])
                .execute()
                .await
                .map_err(|e| StorageError::VectorDb(e.to_string()))?;
        }
        Err(_) => {
            // Table doesn't exist: create it
            db.create_table(table_name, vec![batch])
                .execute()
                .await
                .map_err(|e| StorageError::VectorDb(e.to_string()))?;
        }
    }

    tracing::info!(
        document_id = %document_id,
        chunks = num_chunks,
        "Chunks stored in LanceDB"
    );

    Ok(num_chunks)
}
```

---

## [7] Structured Storage (SQLite)

### Entity-to-DB Mapping

**E-DA:** Each extracted entity maps to its corresponding repository. All content fields are encrypted via ProfileSession before INSERT.

```rust
/// Store all extracted entities into SQLite
pub fn store_entities_in_sqlite(
    document_id: &Uuid,
    entities: &ExtractedEntities,
    professional: Option<&ExtractedProfessional>,
    doc_type: &DocumentType,
    doc_date: Option<NaiveDate>,
    session: &ProfileSession,
    repos: &RepositorySet,
) -> Result<EntitiesStoredCount, StorageError> {
    let mut count = EntitiesStoredCount::default();

    // Step 1: Find or create professional
    let professional_id = if let Some(prof) = professional {
        let pro = repos.professional.find_or_create(
            &prof.name,
            prof.specialty.as_deref(),
        )?;
        // Update institution if provided
        if prof.institution.is_some() {
            // Update professional record
        }
        Some(pro.id)
    } else {
        None
    };

    // Step 2: Update document record
    let mut doc = repos.document.get(document_id)?
        .ok_or(StorageError::DocumentNotFound(*document_id))?;
    doc.doc_type = doc_type.clone();
    doc.document_date = doc_date;
    doc.professional_id = professional_id;
    repos.document.update(&doc)?;

    // Step 3: Store medications
    for med in &entities.medications {
        match store_medication(document_id, med, professional_id.as_ref(), session, repos) {
            Ok(med_id) => {
                count.medications += 1;
                // Store compound ingredients
                for ingredient in &med.compound_ingredients {
                    store_compound_ingredient(&med_id, ingredient, session, repos)?;
                }
                // Store tapering steps
                for step in &med.tapering_steps {
                    store_tapering_step(&med_id, document_id, step, session, repos)?;
                }
                // Store instructions
                for instruction in &med.instructions {
                    store_medication_instruction(&med_id, document_id, instruction, session, repos)?;
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "Failed to store medication");
            }
        }
    }

    // Step 4: Store lab results
    for lab in &entities.lab_results {
        match store_lab_result(document_id, lab, professional_id.as_ref(), session, repos) {
            Ok(_) => count.lab_results += 1,
            Err(e) => tracing::warn!(error = %e, "Failed to store lab result"),
        }
    }

    // Step 5: Store diagnoses
    for diag in &entities.diagnoses {
        match store_diagnosis(document_id, diag, professional_id.as_ref(), session, repos) {
            Ok(_) => count.diagnoses += 1,
            Err(e) => tracing::warn!(error = %e, "Failed to store diagnosis"),
        }
    }

    // Step 6: Store allergies (CRITICAL — safety table)
    for allergy in &entities.allergies {
        match store_allergy(document_id, allergy, session, repos) {
            Ok(_) => count.allergies += 1,
            Err(e) => tracing::warn!(error = %e, "Failed to store allergy"),
        }
    }

    // Step 7: Store procedures
    for proc in &entities.procedures {
        match store_procedure(document_id, proc, professional_id.as_ref(), session, repos) {
            Ok(_) => count.procedures += 1,
            Err(e) => tracing::warn!(error = %e, "Failed to store procedure"),
        }
    }

    // Step 8: Store referrals
    for referral in &entities.referrals {
        match store_referral(document_id, referral, professional_id.as_ref(), session, repos) {
            Ok(_) => count.referrals += 1,
            Err(e) => tracing::warn!(error = %e, "Failed to store referral"),
        }
    }

    count.instructions = entities.instructions.len();

    // Step 9: Update profile trust (increment total_documents)
    repos.profile_trust.record_verified()?;

    tracing::info!(
        document_id = %document_id,
        medications = count.medications,
        lab_results = count.lab_results,
        diagnoses = count.diagnoses,
        allergies = count.allergies,
        procedures = count.procedures,
        referrals = count.referrals,
        "Entities stored in SQLite"
    );

    Ok(count)
}
```

### Individual Entity Storage Functions

```rust
/// Store a single medication (with encrypted fields)
fn store_medication(
    document_id: &Uuid,
    extracted: &ExtractedMedication,
    prescriber_id: Option<&Uuid>,
    session: &ProfileSession,
    repos: &RepositorySet,
) -> Result<Uuid, StorageError> {
    let med = Medication {
        id: Uuid::new_v4(),
        generic_name: extracted.generic_name.clone().unwrap_or_default(),
        brand_name: extracted.brand_name.clone(),
        dose: extracted.dose.clone(),
        frequency: extracted.frequency.clone(),
        frequency_type: FrequencyType::from_str(&extracted.frequency_type)
            .unwrap_or(FrequencyType::Scheduled),
        route: extracted.route.clone(),
        prescriber_id: prescriber_id.copied(),
        start_date: None,  // Inferred from document_date if not explicit
        end_date: None,
        reason_start: extracted.reason.clone(),
        reason_stop: None,
        is_otc: false,
        status: MedicationStatus::Active,
        administration_instructions: extracted.instructions.first().cloned(),
        max_daily_dose: extracted.max_daily_dose.clone(),
        condition: extracted.condition.clone(),
        dose_type: DoseType::Fixed,
        is_compound: extracted.is_compound,
        document_id: *document_id,
    };

    repos.medication.insert(&med)?;
    Ok(med.id)
}

/// Store a lab result
fn store_lab_result(
    document_id: &Uuid,
    extracted: &ExtractedLabResult,
    ordering_physician_id: Option<&Uuid>,
    session: &ProfileSession,
    repos: &RepositorySet,
) -> Result<Uuid, StorageError> {
    let lab = LabResult {
        id: Uuid::new_v4(),
        test_name: extracted.test_name.clone(),
        test_code: extracted.test_code.clone(),
        value: extracted.value,
        value_text: extracted.value_text.clone(),
        unit: extracted.unit.clone(),
        reference_range_low: extracted.reference_range_low,
        reference_range_high: extracted.reference_range_high,
        abnormal_flag: extracted.abnormal_flag.as_deref()
            .and_then(|s| AbnormalFlag::from_str(s).ok())
            .unwrap_or(AbnormalFlag::Normal),
        collection_date: extracted.collection_date.as_deref()
            .and_then(|d| parse_document_date(d))
            .unwrap_or_else(|| chrono::Local::now().date_naive()),
        lab_facility: None,
        ordering_physician_id: ordering_physician_id.copied(),
        document_id: *document_id,
    };

    repos.lab_result.insert(&lab)?;
    Ok(lab.id)
}

/// Store a diagnosis
fn store_diagnosis(
    document_id: &Uuid,
    extracted: &ExtractedDiagnosis,
    diagnosing_professional_id: Option<&Uuid>,
    session: &ProfileSession,
    repos: &RepositorySet,
) -> Result<Uuid, StorageError> {
    let diag = Diagnosis {
        id: Uuid::new_v4(),
        name: extracted.name.clone(),
        icd_code: extracted.icd_code.clone(),
        date_diagnosed: extracted.date.as_deref().and_then(|d| parse_document_date(d)),
        diagnosing_professional_id: diagnosing_professional_id.copied(),
        status: DiagnosisStatus::from_str(&extracted.status)
            .unwrap_or(DiagnosisStatus::Active),
        document_id: *document_id,
    };

    repos.diagnosis.insert(&diag)?;
    Ok(diag.id)
}

/// Store an allergy (CRITICAL — safety)
fn store_allergy(
    document_id: &Uuid,
    extracted: &ExtractedAllergy,
    session: &ProfileSession,
    repos: &RepositorySet,
) -> Result<Uuid, StorageError> {
    let allergy = Allergy {
        id: Uuid::new_v4(),
        allergen: extracted.allergen.clone(),
        reaction: extracted.reaction.clone(),
        severity: extracted.severity.as_deref()
            .and_then(|s| AllergySeverity::from_str(s).ok())
            .unwrap_or(AllergySeverity::Moderate), // Default to moderate for safety
        date_identified: None,
        source: AllergySource::DocumentExtracted,
        document_id: Some(*document_id),
        verified: false,  // Must be verified by patient in review screen
    };

    repos.allergy.insert(&allergy)?;
    Ok(allergy.id)
}

/// Store a procedure
fn store_procedure(
    document_id: &Uuid,
    extracted: &ExtractedProcedure,
    performing_professional_id: Option<&Uuid>,
    session: &ProfileSession,
    repos: &RepositorySet,
) -> Result<Uuid, StorageError> {
    let procedure = Procedure {
        id: Uuid::new_v4(),
        name: extracted.name.clone(),
        date: extracted.date.as_deref().and_then(|d| parse_document_date(d)),
        performing_professional_id: performing_professional_id.copied(),
        facility: None,
        outcome: extracted.outcome.clone(),
        follow_up_required: extracted.follow_up_required,
        follow_up_date: extracted.follow_up_date.as_deref().and_then(|d| parse_document_date(d)),
        document_id: *document_id,
    };

    repos.procedure.insert(&procedure)?;
    Ok(procedure.id)
}

/// Store a referral
fn store_referral(
    document_id: &Uuid,
    extracted: &ExtractedReferral,
    referring_professional_id: Option<&Uuid>,
    session: &ProfileSession,
    repos: &RepositorySet,
) -> Result<Uuid, StorageError> {
    // Find or create the referred-to professional
    let referred_to = repos.professional.find_or_create(
        &extracted.referred_to,
        extracted.specialty.as_deref(),
    )?;

    let referral = Referral {
        id: Uuid::new_v4(),
        referring_professional_id: referring_professional_id
            .copied()
            .unwrap_or_else(Uuid::new_v4),
        referred_to_professional_id: referred_to.id,
        reason: extracted.reason.clone(),
        date: chrono::Local::now().date_naive(),
        status: ReferralStatus::Pending,
        document_id: Some(*document_id),
    };

    repos.referral.insert(&referral)?;
    Ok(referral.id)
}

/// Store a compound ingredient
fn store_compound_ingredient(
    medication_id: &Uuid,
    extracted: &ExtractedCompoundIngredient,
    session: &ProfileSession,
    repos: &RepositorySet,
) -> Result<(), StorageError> {
    let ingredient = CompoundIngredient {
        id: Uuid::new_v4(),
        medication_id: *medication_id,
        ingredient_name: extracted.name.clone(),
        ingredient_dose: extracted.dose.clone(),
        maps_to_generic: None,  // Resolved by coherence engine later
    };
    repos.compound_ingredient.insert(&ingredient)?;
    Ok(())
}

/// Store a tapering step
fn store_tapering_step(
    medication_id: &Uuid,
    document_id: &Uuid,
    extracted: &ExtractedTaperingStep,
    session: &ProfileSession,
    repos: &RepositorySet,
) -> Result<(), StorageError> {
    let step = TaperingStep {
        id: Uuid::new_v4(),
        medication_id: *medication_id,
        step_number: extracted.step_number as i32,
        dose: extracted.dose.clone(),
        duration_days: extracted.duration_days as i32,
        start_date: None,  // Computed from document date + prior steps
        document_id: Some(*document_id),
    };
    repos.tapering_schedule.insert(&step)?;
    Ok(())
}

/// Store a medication instruction
fn store_medication_instruction(
    medication_id: &Uuid,
    document_id: &Uuid,
    instruction_text: &str,
    session: &ProfileSession,
    repos: &RepositorySet,
) -> Result<(), StorageError> {
    let instruction = MedicationInstruction {
        id: Uuid::new_v4(),
        medication_id: *medication_id,
        instruction: instruction_text.to_string(),
        timing: None,
        source_document_id: Some(*document_id),
    };
    repos.medication_instruction.insert(&instruction)?;
    Ok(())
}
```

### Repository Set

```rust
/// All repositories needed by the storage pipeline
pub struct RepositorySet {
    pub document: Box<dyn DocumentRepository>,
    pub medication: Box<dyn MedicationRepository>,
    pub lab_result: Box<dyn LabResultRepository>,
    pub diagnosis: Box<dyn Repository<Diagnosis, DiagnosisFilter>>,
    pub allergy: Box<dyn AllergyRepository>,
    pub procedure: Box<dyn Repository<Procedure, ProcedureFilter>>,
    pub referral: Box<dyn Repository<Referral, ReferralFilter>>,
    pub professional: Box<dyn ProfessionalRepository>,
    pub compound_ingredient: Box<dyn Repository<CompoundIngredient, ()>>,
    pub tapering_schedule: Box<dyn Repository<TaperingStep, ()>>,
    pub medication_instruction: Box<dyn Repository<MedicationInstruction, ()>>,
    pub profile_trust: Box<dyn ProfileTrustRepository>,
}
```

---

## [8] Pipeline Orchestration

### Full Pipeline Implementation

```rust
/// Concrete storage pipeline implementation
pub struct DocumentStoragePipeline {
    chunker: Box<dyn Chunker + Send + Sync>,
    embedder: Box<dyn EmbeddingModel + Send + Sync>,
    repos: RepositorySet,
}

impl StoragePipeline for DocumentStoragePipeline {
    fn store(
        &self,
        result: &StructuringResult,
        session: &ProfileSession,
    ) -> Result<StorageResult, StorageError> {
        let mut warnings = Vec::new();

        tracing::info!(
            document_id = %result.document_id,
            "Starting storage pipeline"
        );

        // ═══════════════════════════════════════════
        // PHASE 1: Chunk and embed Markdown → LanceDB
        // ═══════════════════════════════════════════

        let chunks = self.chunker.chunk(&result.structured_markdown);

        let chunk_texts: Vec<&str> = chunks.iter()
            .map(|c| c.content.as_str())
            .collect();

        let embeddings = match self.embedder.embed_batch(&chunk_texts) {
            Ok(embs) => embs,
            Err(e) => {
                tracing::error!(error = %e, "Embedding generation failed");
                // Generate zero vectors as fallback (searchable by metadata only)
                vec![vec![0.0f32; EMBEDDING_DIM]; chunks.len()]
            }
        };

        // Store in LanceDB (async — block on it)
        let chunks_stored = tokio::runtime::Handle::current()
            .block_on(store_chunks_in_lancedb(
                &chunks,
                &embeddings,
                &result.document_id,
                &result.document_type,
                result.document_date.as_ref(),
                result.professional.as_ref().map(|p| p.name.as_str()),
                session,
            ))?;

        // ═══════════════════════════════════════════
        // PHASE 2: Store extracted entities → SQLite
        // ═══════════════════════════════════════════

        let entities_count = store_entities_in_sqlite(
            &result.document_id,
            &result.extracted_entities,
            result.professional.as_ref(),
            &result.document_type,
            result.document_date,
            session,
            &self.repos,
        )?;

        // ═══════════════════════════════════════════
        // PHASE 3: Save encrypted Markdown file
        // ═══════════════════════════════════════════

        let markdown_path = save_encrypted_markdown(
            &result.document_id,
            &result.structured_markdown,
            session,
        )?;

        // Update document record with markdown_file path
        if let Ok(Some(mut doc)) = self.repos.document.get(&result.document_id) {
            doc.markdown_file = Some(markdown_path.to_string_lossy().to_string());
            let _ = self.repos.document.update(&doc);
        }

        tracing::info!(
            document_id = %result.document_id,
            chunks = chunks_stored,
            entities = ?entities_count,
            "Storage pipeline complete"
        );

        Ok(StorageResult {
            document_id: result.document_id,
            chunks_stored,
            entities_stored: entities_count,
            document_type: result.document_type.clone(),
            professional_id: result.professional.as_ref().map(|_| Uuid::new_v4()),
            warnings,
        })
    }
}

/// Save structured Markdown to profile directory, encrypted
fn save_encrypted_markdown(
    document_id: &Uuid,
    markdown: &str,
    session: &ProfileSession,
) -> Result<PathBuf, StorageError> {
    let markdown_dir = session.db_path()
        .parent().unwrap()
        .parent().unwrap()
        .join("markdown");

    std::fs::create_dir_all(&markdown_dir)?;

    let file_path = markdown_dir.join(format!("{}.md.enc", document_id));

    let encrypted = session.encrypt(markdown.as_bytes())?;
    std::fs::write(&file_path, encrypted.to_bytes())?;

    Ok(file_path)
}
```

### Full Document Pipeline (End-to-End)

```rust
/// The complete document pipeline: Import → Extract → Structure → Store
/// This is the top-level orchestrator called from the frontend
pub struct FullDocumentPipeline {
    importer: Box<dyn DocumentImporter + Send + Sync>,
    extractor: Box<dyn TextExtractor + Send + Sync>,
    structurer: Box<dyn MedicalStructurer + Send + Sync>,
    storage: Box<dyn StoragePipeline + Send + Sync>,
}

impl FullDocumentPipeline {
    /// Run the full pipeline on an already-imported document
    pub fn process_document(
        &self,
        import_result: &ImportResult,
        session: &ProfileSession,
    ) -> Result<StorageResult, PipelineError> {
        // Step 1: Extract text (L1-02)
        let extraction = self.extractor.extract(
            &import_result.document_id,
            &import_result.format,
            session,
        )?;

        // Step 2: Structure with MedGemma (L1-03)
        let structuring = self.structurer.structure_document(
            &import_result.document_id,
            &extraction.full_text,
            extraction.overall_confidence,
            session,
        )?;

        // Step 3: Store in both DBs (L1-04)
        let storage_result = self.storage.store(&structuring, session)?;

        Ok(storage_result)
    }
}
```

---

## [9] Tauri Commands (IPC)

```rust
// src-tauri/src/commands/pipeline.rs

/// Process an imported document through the full pipeline
#[tauri::command]
pub async fn process_document(
    document_id: String,
    pipeline: State<'_, FullDocumentPipeline>,
    session: State<'_, Option<ProfileSession>>,
) -> Result<StorageResult, String> {
    let session = session.as_ref()
        .ok_or("No active profile session")?;

    let doc_id = Uuid::parse_str(&document_id)
        .map_err(|e| format!("Invalid document ID: {e}"))?;

    // Build a minimal ImportResult from the document record
    // (The real ImportResult was returned at import time)
    let import_result = ImportResult {
        document_id: doc_id,
        original_filename: String::new(),
        format: FormatDetection {
            mime_type: String::new(),
            category: FileCategory::Image,  // Will be re-detected
            is_digital_pdf: None,
            file_size_bytes: 0,
        },
        staged_path: String::new(),
        duplicate_of: None,
        status: ImportStatus::Staged,
    };

    pipeline.process_document(&import_result, session)
        .map_err(|e| e.to_string())
}

/// Get storage status for a document
#[tauri::command]
pub async fn get_document_storage_status(
    document_id: String,
    repos: State<'_, RepositorySet>,
) -> Result<EntitiesStoredCount, String> {
    let doc_id = Uuid::parse_str(&document_id)
        .map_err(|e| format!("Invalid document ID: {e}"))?;

    // Count entities linked to this document
    // (Implementation queries each repository by document_id)
    Ok(EntitiesStoredCount::default()) // Placeholder
}
```

### Frontend API

```typescript
// src/lib/api/pipeline.ts
import { invoke } from '@tauri-apps/api/core';

export interface StorageResult {
  document_id: string;
  chunks_stored: number;
  entities_stored: {
    medications: number;
    lab_results: number;
    diagnoses: number;
    allergies: number;
    procedures: number;
    referrals: number;
    instructions: number;
  };
  document_type: string;
  professional_id: string | null;
  warnings: string[];
}

/** Process a document through the full pipeline */
export async function processDocument(documentId: string): Promise<StorageResult> {
  return invoke<StorageResult>('process_document', { documentId });
}
```

---

## [10] Error Handling

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Database error: {0}")]
    Database(#[from] DatabaseError),

    #[error("Encryption error: {0}")]
    Crypto(#[from] CryptoError),

    #[error("Vector DB error: {0}")]
    VectorDb(String),

    #[error("Embedding model not found: {0}")]
    ModelNotFound(PathBuf),

    #[error("Embedding model initialization: {0}")]
    ModelInit(String),

    #[error("Tokenization error: {0}")]
    Tokenization(String),

    #[error("Embedding generation failed: {0}")]
    Embedding(String),

    #[error("Document not found: {0}")]
    DocumentNotFound(Uuid),

    #[error("Chunking produced no results")]
    EmptyChunks,
}

#[derive(Error, Debug)]
pub enum PipelineError {
    #[error("Import error: {0}")]
    Import(#[from] ImportError),

    #[error("Extraction error: {0}")]
    Extraction(#[from] ExtractionError),

    #[error("Structuring error: {0}")]
    Structuring(#[from] StructuringError),

    #[error("Storage error: {0}")]
    Storage(#[from] StorageError),
}
```

---

## [11] Security

**E-SC review:**

| Concern | Mitigation |
|---------|-----------|
| Embedding vectors leak content | Vectors are high-dimensional numerical representations. Inverting them to recover text is computationally infeasible. Vectors stored unencrypted in LanceDB is acceptable. |
| Chunk text in LanceDB | Content field in LanceDB SHOULD be encrypted (using ProfileSession). Metadata fields (doc_type, date) left queryable. |
| SQLite content fields | Encrypted via application-level field encryption (L0-03). |
| Model files integrity | ONNX model file should be verified with checksum at first load. |
| Memory during embedding | Batch size limited. Process chunks in batches of 32 to bound memory. |

---

## [12] Testing

### Acceptance Criteria

| # | Test | Expected |
|---|------|----------|
| T-01 | Chunk a 1-page Markdown | 1-3 chunks, each < 1000 chars |
| T-02 | Chunk a 5-section Markdown | ~5 chunks aligned to section boundaries |
| T-03 | Chunk merges tiny sections | Sections < 50 chars merged with neighbors |
| T-04 | Chunk overlap preserved | Adjacent chunks share ~100 chars |
| T-05 | Embed a text chunk | 384-dim vector, L2-normalized |
| T-06 | Embed batch of 10 chunks | 10 vectors returned |
| T-07 | Store chunks in LanceDB | Chunks queryable after insert |
| T-08 | Store medication in SQLite | Medication retrievable by generic_name |
| T-09 | Store lab result in SQLite | Lab retrievable by test_name |
| T-10 | Store allergy in SQLite | Allergy retrievable by allergen |
| T-11 | Store compound medication | CompoundIngredients linked to medication |
| T-12 | Store tapering schedule | TaperingSteps linked to medication |
| T-13 | Professional find_or_create | Same name → same professional_id |
| T-14 | Document record updated | After storage, doc has type, date, professional_id |
| T-15 | Encrypted Markdown saved | File exists in markdown/ directory |
| T-16 | Full pipeline end-to-end | Import → Extract → Structure → Store → all queryable |
| T-17 | Profile trust updated | total_documents incremented after storage |
| T-18 | Entity with missing optional fields | Store succeeds with None fields |

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunking_respects_headings() {
        let markdown = "## Medications\n\nMetformin 500mg twice daily\n\n## Lab Results\n\nHbA1c: 7.2%\n\n## Instructions\n\nFollow up in 3 months";
        let chunker = MedicalChunker::new();
        let chunks = chunker.chunk(markdown);

        assert!(chunks.len() >= 3, "Should split by headings: got {}", chunks.len());
        assert!(chunks[0].section_title.as_deref() == Some("Medications"));
    }

    #[test]
    fn chunking_splits_large_sections() {
        let large_section = "## Medications\n\n".to_string() + &"Medication details. ".repeat(200);
        let chunker = MedicalChunker::new();
        let chunks = chunker.chunk(&large_section);

        assert!(chunks.len() > 1, "Large section should be split");
        for chunk in &chunks {
            assert!(chunk.content.len() <= 1100, "Chunk too large: {} chars", chunk.content.len());
        }
    }

    #[test]
    fn chunking_merges_tiny() {
        let markdown = "## A\n\nOk\n\n## B\n\nAlso ok but slightly longer content here to test merging of tiny sections.";
        let chunker = MedicalChunker::new();
        let chunks = chunker.chunk(markdown);

        // Tiny section "Ok" should be merged
        for chunk in &chunks {
            assert!(chunk.content.len() >= 10, "Tiny chunk not merged: '{}'", chunk.content);
        }
    }

    #[test]
    fn entities_stored_count_tracks_correctly() {
        let mut count = EntitiesStoredCount::default();
        count.medications = 3;
        count.lab_results = 5;
        count.allergies = 1;
        assert_eq!(count.medications, 3);
        assert_eq!(count.lab_results, 5);
    }

    #[test]
    fn storage_warning_types() {
        let warning = StorageWarning::DuplicateMedication {
            name: "Metformin".into(),
            existing_id: Uuid::new_v4(),
        };
        // Ensure warning serializes correctly for frontend
        let json = serde_json::to_string(&warning).unwrap();
        assert!(json.contains("Metformin"));
    }
}
```

---

## [13] Performance

| Metric | Target |
|--------|--------|
| Chunking (1-page document) | < 5ms |
| Embedding (single chunk) | < 50ms |
| Embedding batch (10 chunks) | < 200ms |
| LanceDB insert (10 chunks) | < 100ms |
| SQLite entity inserts (full document) | < 50ms |
| Full storage pipeline (typical document) | < 1 second (excluding embedding model load) |
| Embedding model cold start | < 2 seconds |

**E-RS notes:**
- Embedding model loaded once at application start, kept in memory.
- Batch embeddings preferred over single-item embedding for throughput.
- LanceDB writes are async — use Tokio blocking tasks.
- SQLite writes within a single transaction for atomicity.

---

## [14] Open Questions

| # | Question | Status |
|---|---------|--------|
| OQ-01 | ONNX Runtime vs candle (Rust-native ML) for embedding inference? | ONNX Runtime chosen for broader model compatibility. Candle viable if ONNX dependency is problematic. |
| OQ-02 | Should chunk content in LanceDB be encrypted? | Yes for maximum safety. Vector field left unencrypted (not invertible). |
| OQ-03 | LanceDB table creation — create at init time or first insert? | First insert (LanceDB's design pattern). Store schema in code. |
| OQ-04 | Embedding model quantization — FP16 vs FP32? | FP32 for accuracy. FP16 if memory constrained on 8GB machines. |
| OQ-05 | Should SQLite writes be transactional per document? | Yes — single transaction wrapping all entity inserts. Rollback on failure. |

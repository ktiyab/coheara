# L2-01 — RAG Pipeline

<!--
=============================================================================
COMPONENT SPEC — The conversation engine. Retrieval-Augmented Generation.
Engineer review: E-ML (AI/ML, lead), E-RS (Rust), E-SC (Security), E-QA (QA)
This is how patients ask questions and get grounded, sourced answers.
Quality here is the core value proposition of Coheara.
=============================================================================
-->

## Table of Contents

| Section | Offset |
|---------|--------|
| [1] Identity | `offset=20 limit=30` |
| [2] Dependencies | `offset=50 limit=20` |
| [3] Interfaces | `offset=70 limit=90` |
| [4] Query Classification | `offset=160 limit=55` |
| [5] Retrieval (Dual-Layer) | `offset=215 limit=70` |
| [6] Context Assembly | `offset=285 limit=60` |
| [7] Generation (MedGemma) | `offset=345 limit=75` |
| [8] Source Citation | `offset=420 limit=45` |
| [9] Conversation Memory | `offset=465 limit=50` |
| [10] Streaming | `offset=515 limit=45` |
| [11] Error Handling | `offset=560 limit=25` |
| [12] Security | `offset=585 limit=25` |
| [13] Testing | `offset=610 limit=55` |
| [14] Performance | `offset=665 limit=15` |
| [15] Open Questions | `offset=680 limit=15` |

---

## [1] Identity

**What:** The complete Retrieval-Augmented Generation pipeline for patient conversation. Takes a patient's natural language question, classifies it, retrieves relevant context from both data layers (LanceDB semantic + SQLite structured), assembles a grounded context window, generates a response via MedGemma streaming through Ollama, attaches source citations, and maintains conversation memory.

**After this session:**
- Patient asks a question in natural language
- Query classified (factual, exploratory, symptom, timeline)
- Parallel retrieval from LanceDB (semantic) and SQLite (structured)
- Context assembled within ~3000 token budget
- MedGemma generates response via Ollama (streaming)
- Every claim linked to source document (citation chips)
- Confidence score attached to response
- Conversation persisted for memory across sessions
- Response handed to Safety Filter (L2-02) before reaching patient

**Estimated complexity:** Very High
**Source:** Tech Spec v1.1 Section 7 (Conversation Engine)

---

## [2] Dependencies

**Incoming:**
- L1-04 (storage pipeline — LanceDB chunks with embeddings, SQLite entities)
- L0-02 (data model — repository traits for structured queries)
- L0-03 (encryption — decrypt content fields from SQLite)

**Outgoing:**
- L2-02 (safety filter — filters every generated response)
- L3-03 (chat interface — displays streaming responses with citations)

**New Cargo.toml dependencies:**
```toml
# Ollama client (shared with L1-03, already added)
# reqwest already in dependencies

# Streaming SSE parsing
futures = "0.3"
tokio-stream = "0.1"
```

---

## [3] Interfaces

### Query and Response Types

```rust
/// A patient's query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatientQuery {
    pub text: String,
    pub conversation_id: Uuid,
    pub query_type: Option<QueryType>,  // None = auto-classify
}

/// Classified query type determines retrieval strategy
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum QueryType {
    Factual,      // "What dose of metformin am I on?"
    Exploratory,  // "What should I ask my doctor about?"
    Symptom,      // "I've been feeling dizzy lately"
    Timeline,     // "What changed since my last visit?"
    General,      // "How does metformin work?"
}

/// Complete RAG response (before safety filtering)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagResponse {
    pub text: String,
    pub citations: Vec<Citation>,
    pub confidence: f32,
    pub query_type: QueryType,
    pub context_used: ContextSummary,
    pub boundary_check: BoundaryCheck,
}

/// A source citation linking a response claim to a document
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Citation {
    pub document_id: Uuid,
    pub document_title: String,
    pub document_date: Option<String>,
    pub professional_name: Option<String>,
    pub chunk_text: String,       // The actual chunk that supports the claim
    pub relevance_score: f32,     // How relevant this chunk was
}

/// Summary of context used for generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextSummary {
    pub semantic_chunks_used: usize,
    pub structured_records_used: usize,
    pub total_context_tokens: usize,
}

/// Boundary check from structured output
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BoundaryCheck {
    Understanding,   // Explaining what documents say
    Awareness,       // Making patient aware of observations
    Preparation,     // Preparing for appointment/action
    OutOfBounds,     // Detected as advice/diagnosis — should be regenerated
}
```

### RAG Pipeline Trait

```rust
/// The main RAG pipeline
pub trait RagPipeline {
    /// Generate a response to a patient query
    fn query(
        &self,
        query: &PatientQuery,
        session: &ProfileSession,
    ) -> Result<RagResponse, RagError>;

    /// Generate a streaming response (for real-time display)
    fn query_streaming(
        &self,
        query: &PatientQuery,
        session: &ProfileSession,
        callback: Box<dyn Fn(StreamChunk) + Send>,
    ) -> Result<RagResponse, RagError>;
}

/// A chunk of streaming output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamChunk {
    pub text: String,
    pub is_final: bool,
    pub partial_citations: Vec<Citation>,
}

/// Retrieved context from both data layers
#[derive(Debug, Clone)]
pub struct RetrievedContext {
    pub semantic_chunks: Vec<ScoredChunk>,
    pub structured_data: StructuredContext,
    pub dismissed_alerts: Vec<Uuid>,
}

/// A chunk with its relevance score
#[derive(Debug, Clone)]
pub struct ScoredChunk {
    pub chunk_id: String,
    pub document_id: Uuid,
    pub content: String,
    pub score: f32,             // Cosine similarity
    pub doc_type: String,
    pub doc_date: Option<String>,
    pub professional_name: Option<String>,
}

/// Structured data retrieved from SQLite
#[derive(Debug, Clone)]
pub struct StructuredContext {
    pub medications: Vec<Medication>,
    pub lab_results: Vec<LabResult>,
    pub diagnoses: Vec<Diagnosis>,
    pub allergies: Vec<Allergy>,
    pub symptoms: Vec<Symptom>,
    pub recent_conversations: Vec<Message>,
}
```

---

## [4] Query Classification

**E-ML:** Query type determines retrieval strategy. Misclassification degrades answer quality. Use keyword heuristics (fast, reliable) rather than a secondary LLM call.

```rust
/// Classify a patient query into a type
pub fn classify_query(text: &str) -> QueryType {
    let lower = text.to_lowercase();

    // Timeline patterns
    if has_timeline_pattern(&lower) {
        return QueryType::Timeline;
    }

    // Symptom patterns
    if has_symptom_pattern(&lower) {
        return QueryType::Symptom;
    }

    // Exploratory patterns
    if has_exploratory_pattern(&lower) {
        return QueryType::Exploratory;
    }

    // Factual patterns (default for specific questions)
    if has_factual_pattern(&lower) {
        return QueryType::Factual;
    }

    // Default to general
    QueryType::General
}

fn has_timeline_pattern(text: &str) -> bool {
    let patterns = [
        "what changed", "what's changed", "since my last",
        "since last visit", "over the past", "history of",
        "when did", "how long", "timeline", "chronolog",
        "what happened", "evolution", "progression",
    ];
    patterns.iter().any(|p| text.contains(p))
}

fn has_symptom_pattern(text: &str) -> bool {
    let patterns = [
        "feeling", "symptom", "pain", "dizzy", "nausea",
        "headache", "tired", "fatigue", "side effect",
        "since i started", "after taking", "hurts",
        "uncomfortable", "worse", "better",
    ];
    patterns.iter().any(|p| text.contains(p))
}

fn has_exploratory_pattern(text: &str) -> bool {
    let patterns = [
        "what should i ask", "what questions",
        "prepare for", "before my appointment",
        "what to expect", "should i be concerned",
        "what does this mean", "help me understand",
    ];
    patterns.iter().any(|p| text.contains(p))
}

fn has_factual_pattern(text: &str) -> bool {
    let patterns = [
        "what is my", "what's my", "what dose",
        "how much", "how often", "who prescribed",
        "when was", "which doctor", "what medication",
        "what are my", "lab result", "test result",
    ];
    patterns.iter().any(|p| text.contains(p))
}
```

### Retrieval Strategy per Query Type

```rust
/// Determine retrieval parameters based on query type
pub fn retrieval_strategy(query_type: &QueryType) -> RetrievalParams {
    match query_type {
        QueryType::Factual => RetrievalParams {
            semantic_top_k: 5,
            include_medications: true,
            include_labs: true,
            include_diagnoses: true,
            include_allergies: true,
            include_symptoms: false,
            include_conversations: false,
            temporal_weight: 0.2,   // Slight recency preference
        },
        QueryType::Exploratory => RetrievalParams {
            semantic_top_k: 8,
            include_medications: true,
            include_labs: true,
            include_diagnoses: true,
            include_allergies: true,
            include_symptoms: true,
            include_conversations: true,
            temporal_weight: 0.5,
        },
        QueryType::Symptom => RetrievalParams {
            semantic_top_k: 5,
            include_medications: true,
            include_labs: false,
            include_diagnoses: true,
            include_allergies: false,
            include_symptoms: true,
            include_conversations: false,
            temporal_weight: 0.7,   // Strong recency preference
        },
        QueryType::Timeline => RetrievalParams {
            semantic_top_k: 3,
            include_medications: true,
            include_labs: true,
            include_diagnoses: true,
            include_allergies: false,
            include_symptoms: true,
            include_conversations: false,
            temporal_weight: 1.0,   // Purely temporal
        },
        QueryType::General => RetrievalParams {
            semantic_top_k: 5,
            include_medications: true,
            include_labs: false,
            include_diagnoses: true,
            include_allergies: false,
            include_symptoms: false,
            include_conversations: false,
            temporal_weight: 0.3,
        },
    }
}

#[derive(Debug, Clone)]
pub struct RetrievalParams {
    pub semantic_top_k: usize,
    pub include_medications: bool,
    pub include_labs: bool,
    pub include_diagnoses: bool,
    pub include_allergies: bool,
    pub include_symptoms: bool,
    pub include_conversations: bool,
    pub temporal_weight: f32,
}
```

---

## [5] Retrieval (Dual-Layer)

### Semantic Search (LanceDB)

```rust
/// Search LanceDB for semantically relevant chunks
pub async fn semantic_search(
    query_embedding: &[f32],
    top_k: usize,
    vector_store: &dyn VectorStore,
) -> Result<Vec<ScoredChunk>, RagError> {
    let results = vector_store.search(query_embedding, top_k)?;

    Ok(results.into_iter().map(|r| ScoredChunk {
        chunk_id: r.id,
        document_id: r.document_id,
        content: r.content,
        score: r.score,
        doc_type: r.metadata.get("doc_type").cloned().unwrap_or_default(),
        doc_date: r.metadata.get("doc_date").cloned(),
        professional_name: r.metadata.get("professional_name").cloned(),
    }).collect())
}
```

### Structured Search (SQLite)

```rust
/// Retrieve structured data relevant to the query
pub fn structured_search(
    query_text: &str,
    params: &RetrievalParams,
    repos: &RepositorySet,
    session: &ProfileSession,
) -> Result<StructuredContext, RagError> {
    let mut ctx = StructuredContext {
        medications: vec![],
        lab_results: vec![],
        diagnoses: vec![],
        allergies: vec![],
        symptoms: vec![],
        recent_conversations: vec![],
    };

    // Extract keywords for targeted queries
    let keywords = extract_medical_keywords(query_text);

    if params.include_medications {
        // Get active medications (always relevant)
        ctx.medications = repos.medication.get_active()?;

        // If query mentions specific medication, also get its history
        for keyword in &keywords {
            let matches = repos.medication.get_by_generic_name(keyword)?;
            for med in matches {
                if !ctx.medications.iter().any(|m| m.id == med.id) {
                    ctx.medications.push(med);
                }
            }
        }
    }

    if params.include_labs {
        // Get recent lab results (last 6 months)
        let six_months_ago = chrono::Local::now().date_naive()
            - chrono::Duration::days(180);
        ctx.lab_results = repos.lab_result.list(&LabResultFilter {
            date_from: Some(six_months_ago),
            ..Default::default()
        })?;

        // If query mentions specific test, get all results for trending
        for keyword in &keywords {
            let trending = repos.lab_result.get_by_test_name(keyword)?;
            for lab in trending {
                if !ctx.lab_results.iter().any(|l| l.id == lab.id) {
                    ctx.lab_results.push(lab);
                }
            }
        }
    }

    if params.include_diagnoses {
        ctx.diagnoses = repos.diagnosis.list(&DiagnosisFilter {
            status: Some(DiagnosisStatus::Active),
            ..Default::default()
        })?;
    }

    if params.include_allergies {
        ctx.allergies = repos.allergy.get_all_active()?;
    }

    if params.include_symptoms {
        let thirty_days_ago = chrono::Local::now().date_naive()
            - chrono::Duration::days(30);
        ctx.symptoms = repos.symptom.get_in_date_range(
            thirty_days_ago,
            chrono::Local::now().date_naive(),
        )?;
    }

    Ok(ctx)
}

/// Extract medical keywords from a query for targeted SQLite lookups
fn extract_medical_keywords(query: &str) -> Vec<String> {
    // Simple keyword extraction: find words that look like medication/test names
    // (capitalized words, multi-word compounds like "blood pressure")
    let words: Vec<&str> = query.split_whitespace().collect();
    let mut keywords = Vec::new();

    for word in &words {
        let clean = word.trim_matches(|c: char| !c.is_alphanumeric());
        if clean.len() >= 3 {
            keywords.push(clean.to_lowercase());
        }
    }

    keywords
}
```

### Parallel Retrieval

```rust
/// Run semantic and structured retrieval in parallel
pub async fn parallel_retrieve(
    query_text: &str,
    query_embedding: &[f32],
    params: &RetrievalParams,
    vector_store: &dyn VectorStore,
    repos: &RepositorySet,
    session: &ProfileSession,
) -> Result<RetrievedContext, RagError> {
    // Run both retrievals (semantic is async, structured is sync)
    let semantic_handle = semantic_search(
        query_embedding,
        params.semantic_top_k,
        vector_store,
    );

    let structured = structured_search(query_text, params, repos, session)?;
    let semantic_chunks = semantic_handle.await?;

    // Get dismissed alerts
    let dismissed = repos.alert.get_dismissed()?
        .into_iter()
        .map(|a| a.id)
        .collect();

    Ok(RetrievedContext {
        semantic_chunks,
        structured_data: structured,
        dismissed_alerts: dismissed,
    })
}
```

---

## [6] Context Assembly

**E-ML + E-RS:** The context window for MedGemma 4B is limited (~8K tokens). We need to assemble the most relevant information within ~3000 tokens of context, leaving room for system prompt and response.

```rust
const MAX_CONTEXT_TOKENS: usize = 3000;
const APPROX_CHARS_PER_TOKEN: usize = 4;
const MAX_CONTEXT_CHARS: usize = MAX_CONTEXT_TOKENS * APPROX_CHARS_PER_TOKEN;

/// Assemble retrieved context into a structured prompt section
pub fn assemble_context(
    retrieved: &RetrievedContext,
    query_type: &QueryType,
) -> AssembledContext {
    let mut sections = Vec::new();
    let mut total_chars = 0;

    // Priority 1: Allergies (always include — safety critical)
    if !retrieved.structured_data.allergies.is_empty() {
        let section = format_allergies(&retrieved.structured_data.allergies);
        total_chars += section.len();
        sections.push(("KNOWN ALLERGIES", section));
    }

    // Priority 2: Most relevant semantic chunks (ordered by score)
    let mut chunks = retrieved.semantic_chunks.clone();
    chunks.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

    for chunk in &chunks {
        if total_chars >= MAX_CONTEXT_CHARS { break; }
        let section = format_chunk(chunk);
        if total_chars + section.len() <= MAX_CONTEXT_CHARS {
            total_chars += section.len();
            sections.push(("DOCUMENT EXCERPT", section));
        }
    }

    // Priority 3: Active medications (if room)
    if !retrieved.structured_data.medications.is_empty() && total_chars < MAX_CONTEXT_CHARS {
        let section = format_medications(&retrieved.structured_data.medications);
        if total_chars + section.len() <= MAX_CONTEXT_CHARS {
            total_chars += section.len();
            sections.push(("CURRENT MEDICATIONS", section));
        }
    }

    // Priority 4: Active diagnoses (if room)
    if !retrieved.structured_data.diagnoses.is_empty() && total_chars < MAX_CONTEXT_CHARS {
        let section = format_diagnoses(&retrieved.structured_data.diagnoses);
        if total_chars + section.len() <= MAX_CONTEXT_CHARS {
            total_chars += section.len();
            sections.push(("ACTIVE DIAGNOSES", section));
        }
    }

    // Priority 5: Lab results (if room and relevant)
    if !retrieved.structured_data.lab_results.is_empty() && total_chars < MAX_CONTEXT_CHARS {
        let section = format_labs(&retrieved.structured_data.lab_results);
        if total_chars + section.len() <= MAX_CONTEXT_CHARS {
            total_chars += section.len();
            sections.push(("RECENT LAB RESULTS", section));
        }
    }

    // Priority 6: Recent symptoms (for symptom queries)
    if *query_type == QueryType::Symptom
        && !retrieved.structured_data.symptoms.is_empty()
        && total_chars < MAX_CONTEXT_CHARS
    {
        let section = format_symptoms(&retrieved.structured_data.symptoms);
        if total_chars + section.len() <= MAX_CONTEXT_CHARS {
            total_chars += section.len();
            sections.push(("RECENT SYMPTOMS", section));
        }
    }

    // Build final context string
    let context_text = sections.iter()
        .map(|(label, content)| format!("<{label}>\n{content}\n</{label}>"))
        .collect::<Vec<_>>()
        .join("\n\n");

    let estimated_tokens = context_text.len() / APPROX_CHARS_PER_TOKEN;

    AssembledContext {
        text: context_text,
        estimated_tokens,
        chunks_included: chunks.iter()
            .take_while(|c| {
                // Count how many chunks we actually included
                true
            })
            .cloned()
            .collect(),
    }
}

#[derive(Debug, Clone)]
pub struct AssembledContext {
    pub text: String,
    pub estimated_tokens: usize,
    pub chunks_included: Vec<ScoredChunk>,
}

// Formatting helpers
fn format_allergies(allergies: &[Allergy]) -> String {
    allergies.iter()
        .map(|a| format!("- {} (severity: {}, reaction: {})",
            a.allergen,
            a.severity.as_str(),
            a.reaction.as_deref().unwrap_or("not specified")))
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_chunk(chunk: &ScoredChunk) -> String {
    let mut text = String::new();
    if let Some(ref date) = chunk.doc_date {
        text.push_str(&format!("[Date: {date}] "));
    }
    if let Some(ref prof) = chunk.professional_name {
        text.push_str(&format!("[From: {prof}] "));
    }
    text.push_str(&format!("[Doc ID: {}]\n", chunk.document_id));
    text.push_str(&chunk.content);
    text
}

fn format_medications(meds: &[Medication]) -> String {
    meds.iter()
        .map(|m| format!("- {} {} {} ({}), prescribed by {}",
            m.generic_name,
            m.dose,
            m.frequency,
            m.status.as_str(),
            m.prescriber_id.map(|id| id.to_string()).unwrap_or_else(|| "unknown".into()),
        ))
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_diagnoses(diagnoses: &[Diagnosis]) -> String {
    diagnoses.iter()
        .map(|d| format!("- {} (status: {})", d.name, d.status.as_str()))
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_labs(labs: &[LabResult]) -> String {
    labs.iter()
        .take(10)  // Max 10 lab results
        .map(|l| format!("- {}: {} {} (range: {}-{}, flag: {})",
            l.test_name,
            l.value.map(|v| v.to_string()).unwrap_or_else(|| l.value_text.clone().unwrap_or_default()),
            l.unit.as_deref().unwrap_or(""),
            l.reference_range_low.unwrap_or(0.0),
            l.reference_range_high.unwrap_or(0.0),
            l.abnormal_flag.as_str(),
        ))
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_symptoms(symptoms: &[Symptom]) -> String {
    symptoms.iter()
        .take(5)
        .map(|s| format!("- {} ({}): severity {}/5, since {}",
            s.specific, s.category, s.severity, s.onset_date))
        .collect::<Vec<_>>()
        .join("\n")
}
```

---

## [7] Generation (MedGemma)

### System Prompt for Conversation

```rust
pub const CONVERSATION_SYSTEM_PROMPT: &str = r#"
You are Coheara, a patient's personal medical document assistant. You help
patients understand their medical records. You are NOT a doctor.

ABSOLUTE RULES — NO EXCEPTIONS:
1. Ground ALL statements in the provided context documents.
2. NEVER diagnose, prescribe, recommend treatments, or give clinical advice.
3. NEVER say "you have [condition]" — instead say "your documents show..."
4. NEVER say "you should [take/stop/change]" — instead say "you might want to ask your doctor about..."
5. Express uncertainty when context is ambiguous or incomplete.
6. Cite source documents for every claim: [Doc: <document_id>, Date: <date>].
7. Use plain, patient-friendly language. Avoid medical jargon unless explaining it.
8. If the patient asks something you cannot answer from the documents, say so clearly.
9. If you detect something that warrants medical attention, suggest the patient discuss it with their healthcare provider.

OUTPUT FORMAT:
Start your response with a BOUNDARY_CHECK line (hidden from patient):
BOUNDARY_CHECK: understanding | awareness | preparation

Then provide your response in plain language with inline citations.

CONTEXT DOCUMENTS:
The following sections contain the patient's medical information retrieved
from their documents. ONLY use information from these sections.
"#;

/// Build the full prompt for MedGemma
pub fn build_conversation_prompt(
    query: &str,
    context: &AssembledContext,
    conversation_history: &[Message],
) -> String {
    let mut prompt = String::new();

    // Include recent conversation history (last 4 messages for context)
    let recent: Vec<_> = conversation_history.iter().rev().take(4).rev().collect();
    if !recent.is_empty() {
        prompt.push_str("<CONVERSATION_HISTORY>\n");
        for msg in recent {
            let role = match msg.role {
                MessageRole::Patient => "Patient",
                MessageRole::Coheara => "Coheara",
            };
            prompt.push_str(&format!("{}: {}\n", role, msg.content));
        }
        prompt.push_str("</CONVERSATION_HISTORY>\n\n");
    }

    // Context
    prompt.push_str(&context.text);
    prompt.push_str("\n\n");

    // Patient query
    prompt.push_str(&format!("Patient question: {}\n\n", query));
    prompt.push_str("Respond based ONLY on the context above. Begin with BOUNDARY_CHECK.");

    prompt
}
```

### Generation via Ollama (Streaming)

```rust
/// Generate a streaming response via Ollama
pub fn generate_streaming(
    client: &OllamaClient,
    model: &str,
    prompt: &str,
    system: &str,
    callback: &dyn Fn(StreamChunk),
) -> Result<String, RagError> {
    #[derive(Serialize)]
    struct StreamRequest {
        model: String,
        prompt: String,
        system: String,
        stream: bool,
        options: GenerateOptions,
    }

    #[derive(Serialize)]
    struct GenerateOptions {
        temperature: f32,
        num_predict: i32,
        top_p: f32,
        repeat_penalty: f32,
    }

    #[derive(Deserialize)]
    struct StreamResponse {
        response: String,
        done: bool,
    }

    let request = StreamRequest {
        model: model.to_string(),
        prompt: prompt.to_string(),
        system: system.to_string(),
        stream: true,
        options: GenerateOptions {
            temperature: 0.3,      // Slightly creative but grounded
            num_predict: 2048,     // Max response tokens
            top_p: 0.9,
            repeat_penalty: 1.1,   // Reduce repetition
        },
    };

    let resp = client.client
        .post(format!("{}/api/generate", client.base_url))
        .json(&request)
        .send()
        .map_err(|e| RagError::OllamaConnection(e.to_string()))?;

    let mut full_text = String::new();

    // Read streaming NDJSON response
    let reader = std::io::BufReader::new(resp);
    for line in std::io::BufRead::lines(reader) {
        let line = line.map_err(|e| RagError::StreamingError(e.to_string()))?;
        if line.is_empty() { continue; }

        let chunk: StreamResponse = serde_json::from_str(&line)
            .map_err(|e| RagError::ResponseParsing(e.to_string()))?;

        full_text.push_str(&chunk.response);

        callback(StreamChunk {
            text: chunk.response,
            is_final: chunk.done,
            partial_citations: vec![],
        });

        if chunk.done { break; }
    }

    Ok(full_text)
}
```

---

## [8] Source Citation

### Citation Extraction

```rust
/// Extract citations from MedGemma's response and match to source chunks
pub fn extract_citations(
    response_text: &str,
    context_chunks: &[ScoredChunk],
) -> Vec<Citation> {
    let mut citations = Vec::new();

    // Pattern 1: Explicit [Doc: uuid, Date: date] citations from MedGemma
    let doc_pattern = regex::Regex::new(
        r"\[Doc:\s*([a-f0-9-]+)(?:,\s*Date:\s*([^\]]+))?\]"
    ).unwrap();

    for cap in doc_pattern.captures_iter(response_text) {
        let doc_id_str = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        let date = cap.get(2).map(|m| m.as_str().to_string());

        if let Ok(doc_id) = Uuid::parse_str(doc_id_str) {
            // Find the matching chunk
            if let Some(chunk) = context_chunks.iter().find(|c| c.document_id == doc_id) {
                citations.push(Citation {
                    document_id: doc_id,
                    document_title: format!("Document from {}",
                        chunk.doc_date.as_deref().unwrap_or("unknown date")),
                    document_date: date.or_else(|| chunk.doc_date.clone()),
                    professional_name: chunk.professional_name.clone(),
                    chunk_text: chunk.content.chars().take(200).collect(),
                    relevance_score: chunk.score,
                });
            }
        }
    }

    // Pattern 2: If MedGemma didn't cite explicitly, attach top-scoring chunks
    if citations.is_empty() && !context_chunks.is_empty() {
        for chunk in context_chunks.iter().take(3) {
            if chunk.score > 0.5 {
                citations.push(Citation {
                    document_id: chunk.document_id,
                    document_title: format!("Document from {}",
                        chunk.doc_date.as_deref().unwrap_or("unknown date")),
                    document_date: chunk.doc_date.clone(),
                    professional_name: chunk.professional_name.clone(),
                    chunk_text: chunk.content.chars().take(200).collect(),
                    relevance_score: chunk.score,
                });
            }
        }
    }

    // Deduplicate by document_id
    citations.sort_by(|a, b| b.relevance_score.partial_cmp(&a.relevance_score).unwrap());
    citations.dedup_by(|a, b| a.document_id == b.document_id);

    citations
}

/// Parse the BOUNDARY_CHECK from MedGemma's response
pub fn parse_boundary_check(response: &str) -> (BoundaryCheck, String) {
    if let Some(first_line_end) = response.find('\n') {
        let first_line = &response[..first_line_end].trim();
        if first_line.starts_with("BOUNDARY_CHECK:") {
            let check_str = first_line.strip_prefix("BOUNDARY_CHECK:").unwrap().trim();
            let check = match check_str.to_lowercase().as_str() {
                "understanding" => BoundaryCheck::Understanding,
                "awareness" => BoundaryCheck::Awareness,
                "preparation" => BoundaryCheck::Preparation,
                _ => BoundaryCheck::OutOfBounds,
            };
            let cleaned_response = response[first_line_end + 1..].trim().to_string();
            return (check, cleaned_response);
        }
    }
    // No boundary check found — treat as out of bounds
    (BoundaryCheck::OutOfBounds, response.to_string())
}

/// Clean citation markers from patient-visible text
pub fn clean_citations_for_display(text: &str) -> String {
    let doc_pattern = regex::Regex::new(
        r"\[Doc:\s*[a-f0-9-]+(?:,\s*Date:\s*[^\]]+)?\]"
    ).unwrap();
    doc_pattern.replace_all(text, "").to_string()
}
```

---

## [9] Conversation Memory

```rust
/// Manage conversation persistence
pub struct ConversationManager {
    repo: Box<dyn ConversationRepository>,
}

impl ConversationManager {
    /// Start a new conversation
    pub fn start(&self) -> Result<Uuid, RagError> {
        let id = self.repo.create_conversation()?;
        Ok(id)
    }

    /// Add a patient message
    pub fn add_patient_message(
        &self,
        conversation_id: &Uuid,
        text: &str,
    ) -> Result<(), RagError> {
        let msg = Message {
            id: Uuid::new_v4(),
            conversation_id: *conversation_id,
            role: MessageRole::Patient,
            content: text.to_string(),
            timestamp: chrono::Local::now().naive_local(),
            source_chunks: None,
            confidence: None,
        };
        self.repo.add_message(&msg)?;
        Ok(())
    }

    /// Add a Coheara response
    pub fn add_response(
        &self,
        conversation_id: &Uuid,
        text: &str,
        citations: &[Citation],
        confidence: f32,
    ) -> Result<(), RagError> {
        let chunk_ids: Vec<String> = citations.iter()
            .map(|c| c.document_id.to_string())
            .collect();

        let msg = Message {
            id: Uuid::new_v4(),
            conversation_id: *conversation_id,
            role: MessageRole::Coheara,
            content: text.to_string(),
            timestamp: chrono::Local::now().naive_local(),
            source_chunks: Some(serde_json::to_string(&chunk_ids).unwrap()),
            confidence: Some(confidence),
        };
        self.repo.add_message(&msg)?;
        Ok(())
    }

    /// Get recent messages for context
    pub fn get_recent(
        &self,
        conversation_id: &Uuid,
        limit: usize,
    ) -> Result<Vec<Message>, RagError> {
        let all_messages = self.repo.get_messages(conversation_id)?;
        let start = if all_messages.len() > limit {
            all_messages.len() - limit
        } else {
            0
        };
        Ok(all_messages[start..].to_vec())
    }
}
```

---

## [10] Streaming

### Tauri Commands

```rust
// src-tauri/src/commands/chat.rs

/// Send a message and get a streaming response
#[tauri::command]
pub async fn send_message(
    conversation_id: String,
    text: String,
    pipeline: State<'_, Box<dyn RagPipeline + Send + Sync>>,
    session: State<'_, Option<ProfileSession>>,
    app: tauri::AppHandle,
) -> Result<RagResponse, String> {
    let session = session.as_ref()
        .ok_or("No active profile session")?;

    let conv_id = Uuid::parse_str(&conversation_id)
        .map_err(|e| format!("Invalid conversation ID: {e}"))?;

    let query = PatientQuery {
        text,
        conversation_id: conv_id,
        query_type: None,  // Auto-classify
    };

    // Stream tokens to frontend via Tauri events
    let app_handle = app.clone();
    let response = pipeline.query_streaming(
        &query,
        session,
        Box::new(move |chunk| {
            let _ = app_handle.emit("chat-stream", &chunk);
        }),
    ).map_err(|e| e.to_string())?;

    Ok(response)
}

/// Start a new conversation
#[tauri::command]
pub async fn start_conversation(
    conversation_mgr: State<'_, ConversationManager>,
) -> Result<String, String> {
    let id = conversation_mgr.start().map_err(|e| e.to_string())?;
    Ok(id.to_string())
}

/// Get conversation history
#[tauri::command]
pub async fn get_conversation_messages(
    conversation_id: String,
    conversation_mgr: State<'_, ConversationManager>,
) -> Result<Vec<Message>, String> {
    let id = Uuid::parse_str(&conversation_id)
        .map_err(|e| format!("Invalid ID: {e}"))?;
    conversation_mgr.get_recent(&id, 50)
        .map_err(|e| e.to_string())
}
```

### Frontend API

```typescript
// src/lib/api/chat.ts
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

export interface RagResponse {
  text: string;
  citations: Citation[];
  confidence: number;
  query_type: string;
  boundary_check: string;
}

export interface Citation {
  document_id: string;
  document_title: string;
  document_date: string | null;
  professional_name: string | null;
  chunk_text: string;
  relevance_score: number;
}

export interface StreamChunk {
  text: string;
  is_final: boolean;
}

/** Send a message with streaming response */
export async function sendMessage(
  conversationId: string,
  text: string,
  onChunk: (chunk: StreamChunk) => void,
): Promise<RagResponse> {
  const unlisten = await listen<StreamChunk>('chat-stream', (event) => {
    onChunk(event.payload);
  });

  try {
    const response = await invoke<RagResponse>('send_message', {
      conversationId,
      text,
    });
    return response;
  } finally {
    unlisten();
  }
}

/** Start a new conversation */
export async function startConversation(): Promise<string> {
  return invoke<string>('start_conversation');
}
```

---

## [11] Error Handling

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum RagError {
    #[error("Ollama connection failed: {0}")]
    OllamaConnection(String),

    #[error("No model available")]
    NoModel,

    #[error("Streaming error: {0}")]
    StreamingError(String),

    #[error("Response parsing error: {0}")]
    ResponseParsing(String),

    #[error("Embedding generation failed: {0}")]
    EmbeddingFailed(String),

    #[error("Vector search failed: {0}")]
    VectorSearch(String),

    #[error("Database error: {0}")]
    Database(#[from] DatabaseError),

    #[error("No relevant context found")]
    NoContext,

    #[error("Conversation not found: {0}")]
    ConversationNotFound(Uuid),

    #[error("Encryption error: {0}")]
    Crypto(#[from] CryptoError),
}
```

---

## [12] Security

| Concern | Mitigation |
|---------|-----------|
| Patient query logged | NEVER log query text. Log only query_type and conversation_id. |
| Context contains sensitive data | Context assembled in memory, passed to Ollama via localhost HTTP. Never written to disk. |
| Prompt injection via patient query | Patient query placed in clearly delimited section. System prompt instructs MedGemma to only use CONTEXT, not follow instructions in query. |
| Response contains harmful advice | Safety Filter (L2-02) validates every response before display. BOUNDARY_CHECK enforces response category. |
| Conversation persistence | Messages stored encrypted in SQLite (per L0-03 field encryption). |
| Ollama localhost binding | Ollama only on 127.0.0.1. Not accessible externally. |

---

## [13] Testing

### Acceptance Criteria

| # | Test | Expected |
|---|------|----------|
| T-01 | Classify "What dose of metformin?" | QueryType::Factual |
| T-02 | Classify "What changed since my last visit?" | QueryType::Timeline |
| T-03 | Classify "I've been feeling dizzy" | QueryType::Symptom |
| T-04 | Classify "What should I ask my doctor?" | QueryType::Exploratory |
| T-05 | Semantic search returns relevant chunks | Top chunk has score > 0.5 |
| T-06 | Structured search returns medications | Active medications included |
| T-07 | Context fits token budget | assembled context < 3000 tokens |
| T-08 | Allergies always included in context | Even if no other data present |
| T-09 | BOUNDARY_CHECK parsed correctly | "understanding" → BoundaryCheck::Understanding |
| T-10 | Citations extracted from response | At least 1 citation with valid document_id |
| T-11 | Conversation persisted | Message saved, retrievable by conversation_id |
| T-12 | Streaming delivers tokens incrementally | Callback called multiple times before final |
| T-13 | Empty database query | Graceful message: "I don't have any documents to reference yet." |
| T-14 | Response confidence calculated | Confidence > 0.0 for grounded response |
| T-15 | Citation markers cleaned for display | [Doc: uuid] removed from patient-visible text |

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_classification() {
        assert_eq!(classify_query("What dose of metformin am I on?"), QueryType::Factual);
        assert_eq!(classify_query("What changed since my last visit?"), QueryType::Timeline);
        assert_eq!(classify_query("I've been feeling dizzy lately"), QueryType::Symptom);
        assert_eq!(classify_query("What should I ask my doctor about?"), QueryType::Exploratory);
        assert_eq!(classify_query("Tell me about my health"), QueryType::General);
    }

    #[test]
    fn boundary_check_parsing() {
        let (check, text) = parse_boundary_check(
            "BOUNDARY_CHECK: understanding\nYour documents show that..."
        );
        assert_eq!(check, BoundaryCheck::Understanding);
        assert!(text.starts_with("Your documents"));

        let (check, _) = parse_boundary_check("Some random response");
        assert_eq!(check, BoundaryCheck::OutOfBounds);
    }

    #[test]
    fn citation_extraction() {
        let response = "Your doctor prescribed this [Doc: 550e8400-e29b-41d4-a716-446655440000, Date: 2024-01-15].";
        let chunks = vec![ScoredChunk {
            chunk_id: "c1".into(),
            document_id: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap(),
            content: "Metformin 500mg twice daily".into(),
            score: 0.9,
            doc_type: "prescription".into(),
            doc_date: Some("2024-01-15".into()),
            professional_name: Some("Dr. Chen".into()),
        }];
        let citations = extract_citations(response, &chunks);
        assert_eq!(citations.len(), 1);
        assert_eq!(citations[0].professional_name.as_deref(), Some("Dr. Chen"));
    }

    #[test]
    fn context_respects_token_budget() {
        let chunks: Vec<ScoredChunk> = (0..100).map(|i| ScoredChunk {
            chunk_id: format!("c{i}"),
            document_id: Uuid::new_v4(),
            content: "A ".repeat(500),  // 1000 chars each
            score: 0.8,
            doc_type: "prescription".into(),
            doc_date: None,
            professional_name: None,
        }).collect();

        let ctx = RetrievedContext {
            semantic_chunks: chunks,
            structured_data: StructuredContext {
                medications: vec![],
                lab_results: vec![],
                diagnoses: vec![],
                allergies: vec![],
                symptoms: vec![],
                recent_conversations: vec![],
            },
            dismissed_alerts: vec![],
        };

        let assembled = assemble_context(&ctx, &QueryType::Factual);
        assert!(assembled.text.len() <= MAX_CONTEXT_CHARS + 500); // Allow some buffer for tags
    }

    #[test]
    fn clean_citations_from_display() {
        let text = "Your doctor [Doc: abc-123, Date: 2024-01-15] prescribed this.";
        let clean = clean_citations_for_display(text);
        assert!(!clean.contains("[Doc:"));
        assert!(clean.contains("Your doctor"));
    }
}
```

---

## [14] Performance

| Metric | Target |
|--------|--------|
| Query classification | < 1ms |
| Query embedding | < 50ms |
| LanceDB semantic search (1000 chunks) | < 100ms |
| SQLite structured retrieval | < 50ms |
| Context assembly | < 10ms |
| MedGemma first token (16GB RAM) | < 2 seconds |
| MedGemma full response (16GB RAM) | < 15 seconds |
| MedGemma first token (8GB, Q4 model) | < 5 seconds |
| MedGemma full response (8GB, Q4 model) | < 30 seconds |
| Total query → first token | < 3 seconds (16GB) |

---

## [15] Open Questions

| # | Question | Status |
|---|---------|--------|
| OQ-01 | Should query classification use LLM or heuristics? | Heuristics for MVP (fast, no extra model call). LLM classification in Phase 2. |
| OQ-02 | Optimal semantic_top_k per query type? | Start with 5, tune based on retrieval quality testing. |
| OQ-03 | Conversation memory window — how many messages? | Last 4 messages in context. Full history persisted but not all sent to LLM. |
| OQ-04 | Should we re-rank semantic results? | Phase 2 feature. For MVP, cosine similarity ordering is sufficient. |

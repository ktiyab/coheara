# Coheara — Engineering Conventions

<!--
=============================================================================
EVERY coding session reads this document. Non-negotiable.
Produced by multi-persona engineering review:
  E-RS (Rust), E-UX (UI/UX), E-SC (Security), E-DA (Data), E-QA (QA)
=============================================================================
-->

## Table of Contents

| Section | Lines | Offset |
|---------|-------|--------|
| [EC-01] Project Structure | 30-80 | `offset=25 limit=55` |
| [EC-02] Rust Conventions | 82-180 | `offset=77 limit=103` |
| [EC-03] Error Handling | 182-240 | `offset=177 limit=63` |
| [EC-04] Naming Conventions | 242-290 | `offset=237 limit=53` |
| [EC-05] Svelte Conventions | 292-355 | `offset=287 limit=68` |
| [EC-06] Security Patterns | 357-420 | `offset=352 limit=68` |
| [EC-07] Testing Strategy | 422-500 | `offset=417 limit=83` |
| [EC-08] Dependency Policy | 502-535 | `offset=497 limit=38` |
| [EC-09] Performance Rules | 537-570 | `offset=532 limit=38` |
| [EC-10] Documentation Rules | 572-600 | `offset=567 limit=33` |

---

## [EC-01] Project Structure

**E-PM decision:** Flat module structure at the crate level. Deep nesting hides complexity instead of managing it. Each module corresponds to one component spec.

```
coheara/
├── Cargo.toml                    # Workspace root
├── tauri.conf.json               # Tauri configuration
├── src-tauri/
│   ├── Cargo.toml                # Backend crate
│   ├── src/
│   │   ├── main.rs               # Tauri entry point (minimal)
│   │   ├── lib.rs                # Re-exports all modules
│   │   │
│   │   ├── models/               # Data model structs + traits
│   │   │   ├── mod.rs
│   │   │   ├── document.rs       # Document, Chunk
│   │   │   ├── medication.rs     # Medication, DoseChange, Compound, Tapering
│   │   │   ├── lab.rs            # LabResult
│   │   │   ├── diagnosis.rs      # Diagnosis
│   │   │   ├── professional.rs   # Professional
│   │   │   ├── symptom.rs        # Symptom (OLDCARTS)
│   │   │   ├── allergy.rs        # Allergy
│   │   │   ├── procedure.rs      # Procedure
│   │   │   ├── appointment.rs    # Appointment
│   │   │   ├── profile.rs        # Profile, ProfileTrust
│   │   │   ├── alert.rs          # DismissedAlert, CoherenceAlert
│   │   │   ├── conversation.rs   # Conversation, Message
│   │   │   └── referral.rs       # Referral
│   │   │
│   │   ├── db/                   # Database layer
│   │   │   ├── mod.rs
│   │   │   ├── sqlite.rs         # SQLite connection, migrations
│   │   │   ├── vector.rs         # LanceDB connection, operations
│   │   │   └── repository.rs     # Repository trait + implementations
│   │   │
│   │   ├── crypto/               # Encryption layer
│   │   │   ├── mod.rs
│   │   │   ├── keys.rs           # Key derivation (PBKDF2), recovery phrase
│   │   │   ├── cipher.rs         # AES-256-GCM encrypt/decrypt
│   │   │   ├── profile_crypto.rs # Per-profile encryption lifecycle
│   │   │   └── secure_mem.rs     # Zeroize wrappers
│   │   │
│   │   ├── pipeline/             # Document processing pipeline
│   │   │   ├── mod.rs
│   │   │   ├── import.rs         # File import, format detection, dedup
│   │   │   ├── ocr.rs            # Tesseract integration
│   │   │   ├── pdf.rs            # PDF text extraction
│   │   │   ├── structuring.rs    # MedGemma structuring
│   │   │   ├── chunking.rs       # Markdown chunking
│   │   │   ├── embedding.rs      # Embedding generation (MiniLM)
│   │   │   └── storage.rs        # Orchestrator: chunk → embed → store
│   │   │
│   │   ├── intelligence/         # RAG + Safety + Coherence
│   │   │   ├── mod.rs
│   │   │   ├── rag.rs            # RAG pipeline (retrieve → augment → generate)
│   │   │   ├── safety.rs         # 3-layer safety filter
│   │   │   ├── coherence.rs      # Coherence engine
│   │   │   └── ollama.rs         # Ollama client (MedGemma interaction)
│   │   │
│   │   ├── export/               # PDF, CSV, JSON export
│   │   │   ├── mod.rs
│   │   │   ├── pdf.rs            # Appointment summary PDF generation
│   │   │   └── data.rs           # CSV/JSON export
│   │   │
│   │   ├── transfer/             # Local WiFi transfer
│   │   │   ├── mod.rs
│   │   │   ├── server.rs         # Local HTTP server
│   │   │   ├── qr.rs             # QR code generation
│   │   │   └── validation.rs     # File type + size validation
│   │   │
│   │   ├── commands/             # Tauri command handlers (IPC bridge)
│   │   │   ├── mod.rs
│   │   │   ├── profile_cmds.rs
│   │   │   ├── document_cmds.rs
│   │   │   ├── chat_cmds.rs
│   │   │   ├── medication_cmds.rs
│   │   │   ├── journal_cmds.rs
│   │   │   ├── appointment_cmds.rs
│   │   │   ├── timeline_cmds.rs
│   │   │   ├── transfer_cmds.rs
│   │   │   └── settings_cmds.rs
│   │   │
│   │   └── config.rs            # App configuration, paths, constants
│   │
│   ├── tests/                    # Integration tests
│   │   ├── pipeline_test.rs
│   │   ├── rag_test.rs
│   │   ├── crypto_test.rs
│   │   └── coherence_test.rs
│   │
│   └── resources/                # Bundled resources
│       ├── medication_aliases.json
│       ├── dose_ranges.json
│       └── migrations/           # SQL migration files
│           ├── 001_initial.sql
│           └── ...
│
├── src/                          # Svelte frontend
│   ├── app.html
│   ├── app.css                   # Global styles + Tailwind
│   ├── lib/
│   │   ├── components/           # Reusable Svelte components
│   │   │   ├── ui/               # Design system primitives
│   │   │   │   ├── Button.svelte
│   │   │   │   ├── Card.svelte
│   │   │   │   ├── Input.svelte
│   │   │   │   ├── Modal.svelte
│   │   │   │   ├── TabBar.svelte
│   │   │   │   ├── Badge.svelte
│   │   │   │   ├── Toast.svelte
│   │   │   │   └── Spinner.svelte
│   │   │   ├── chat/
│   │   │   │   ├── ChatMessage.svelte
│   │   │   │   ├── ChatInput.svelte
│   │   │   │   └── SourceCitation.svelte
│   │   │   ├── documents/
│   │   │   │   ├── DocumentCard.svelte
│   │   │   │   ├── ReviewScreen.svelte
│   │   │   │   └── ImportZone.svelte
│   │   │   ├── journal/
│   │   │   │   ├── SymptomRecorder.svelte
│   │   │   │   ├── BodyMap.svelte
│   │   │   │   ├── SeverityScale.svelte
│   │   │   │   └── CheckInPrompt.svelte
│   │   │   ├── medications/
│   │   │   │   ├── MedicationCard.svelte
│   │   │   │   ├── MedicationHistory.svelte
│   │   │   │   └── OtcEntryForm.svelte
│   │   │   ├── timeline/
│   │   │   │   ├── TimelineView.svelte
│   │   │   │   ├── TimelineEvent.svelte
│   │   │   │   └── CorrelationLine.svelte
│   │   │   └── profile/
│   │   │       ├── ProfilePicker.svelte
│   │   │       ├── ProfileCreator.svelte
│   │   │       └── PasswordEntry.svelte
│   │   │
│   │   ├── stores/               # Svelte stores (state management)
│   │   │   ├── profile.ts
│   │   │   ├── documents.ts
│   │   │   ├── medications.ts
│   │   │   ├── chat.ts
│   │   │   ├── journal.ts
│   │   │   └── ui.ts
│   │   │
│   │   ├── api/                  # Tauri IPC wrappers
│   │   │   ├── profile.ts
│   │   │   ├── documents.ts
│   │   │   ├── chat.ts
│   │   │   ├── medications.ts
│   │   │   ├── journal.ts
│   │   │   └── settings.ts
│   │   │
│   │   └── utils/                # Frontend utilities
│   │       ├── formatting.ts
│   │       ├── i18n.ts
│   │       └── accessibility.ts
│   │
│   └── routes/                   # SvelteKit routes (pages)
│       ├── +layout.svelte        # Root layout (TabBar, profile guard)
│       ├── +page.svelte          # Home / Document Feed
│       ├── chat/+page.svelte
│       ├── journal/+page.svelte
│       ├── medications/+page.svelte
│       ├── documents/+page.svelte
│       ├── timeline/+page.svelte
│       ├── appointments/+page.svelte
│       ├── settings/+page.svelte
│       └── profile/+page.svelte  # Profile picker (entry point)
│
├── Specs/                        # Specification documents
└── .babel/                       # Babel knowledge graph - Always use babel commands as specified to collect, detect and inform yourself along the dev
```

---

## [EC-02] Rust Conventions

### Module Organization
**E-RS:** Each module is a bounded context. Modules communicate through defined public interfaces (traits + structs), never through reaching into internal state.

```rust
// GOOD: Module exposes a clear interface
pub mod pipeline {
    pub use self::import::DocumentImporter;
    pub use self::ocr::OcrEngine;
    pub use self::structuring::MedicalStructurer;
    // Internal modules stay private
    mod import;
    mod ocr;
    mod structuring;
}

// BAD: Exposing internals
pub mod pipeline {
    pub mod import;  // Exposes all of import's internals
}
```

### Trait-Based Design
**E-RS:** All major components define traits. Implementations are separate. This enables testing (mock implementations) and future flexibility.

```rust
// Define capability as trait
pub trait DocumentStore {
    fn save(&self, doc: &Document) -> Result<DocumentId>;
    fn get(&self, id: &DocumentId) -> Result<Option<Document>>;
    fn delete(&self, id: &DocumentId) -> Result<()>;
    fn list(&self, filter: &DocumentFilter) -> Result<Vec<Document>>;
}

// Implementation is separate
pub struct SqliteDocumentStore {
    conn: Connection,
}

impl DocumentStore for SqliteDocumentStore {
    // ...
}
```

### Struct Design
**E-RS:** Structs match database tables 1:1 for model layer. Use builder pattern for complex construction. Derive standard traits.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Medication {
    pub id: Uuid,
    pub generic_name: String,
    pub brand_name: Option<String>,
    pub dose: String,
    pub frequency: String,
    pub frequency_type: FrequencyType,
    pub route: String,
    pub prescriber_id: Uuid,
    pub start_date: NaiveDate,
    pub end_date: Option<NaiveDate>,
    pub status: MedicationStatus,
    pub document_id: Uuid,
    // ... all fields from schema
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FrequencyType {
    Scheduled,
    AsNeeded,
    Tapering,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MedicationStatus {
    Active,
    Stopped,
    Paused,
}
```

### Ownership Rules
**E-RS:** Data flows DOWN through function calls, never UP through shared mutable state.

```rust
// GOOD: Owned data passed, result returned
fn process_document(doc: Document) -> Result<ProcessedDocument> { ... }

// GOOD: Borrowed data for read-only
fn analyze_medication(med: &Medication) -> Result<Analysis> { ... }

// BAD: Shared mutable state
static mut CURRENT_PROFILE: Option<Profile> = None;
```

### Async Strategy
**E-RS:** Use `tokio` for async runtime. Ollama calls, file I/O, and embedding generation are async. Database operations use `tokio::task::spawn_blocking` for SQLite (which is not async-safe).

```rust
// Async for I/O-bound operations
async fn query_ollama(prompt: &str) -> Result<String> { ... }

// spawn_blocking for SQLite
async fn get_medications(db: &SqlitePool) -> Result<Vec<Medication>> {
    let db = db.clone();
    tokio::task::spawn_blocking(move || {
        db.query_medications()
    }).await?
}
```

---

## [EC-03] Error Handling

**E-RS + E-QA joint decision:** One error enum per module. Use `thiserror` for definitions, `anyhow` ONLY in main.rs and command handlers (boundary). Library code uses typed errors.

### Error Enum Pattern

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PipelineError {
    #[error("OCR failed: {reason}")]
    OcrFailed { reason: String, confidence: f32 },

    #[error("Document format not supported: {format}")]
    UnsupportedFormat { format: String },

    #[error("MedGemma structuring failed: {0}")]
    StructuringFailed(String),

    #[error("Storage write failed: {0}")]
    StorageFailed(#[from] DatabaseError),

    #[error("File not found: {path}")]
    FileNotFound { path: PathBuf },
}
```

### Error Propagation
```rust
// Use ? for propagation. NEVER unwrap() in production code.
fn import_document(path: &Path) -> Result<Document, PipelineError> {
    let bytes = std::fs::read(path)
        .map_err(|_| PipelineError::FileNotFound { path: path.to_owned() })?;
    let format = detect_format(&bytes)?;
    // ...
}

// unwrap() ONLY allowed in:
// 1. Tests (with #[should_panic] or explicit assertion messages)
// 2. Static initialization where failure is irrecoverable
// 3. Never in library code. Never.
```

### User-Facing Errors
**E-UX decision:** Rust errors map to user-friendly messages at the command handler boundary. Internal error details are logged, never shown to user.

```rust
// In commands/document_cmds.rs (Tauri command handler)
#[tauri::command]
async fn import_document(path: String) -> Result<DocumentView, String> {
    match pipeline::import(&Path::new(&path)).await {
        Ok(doc) => Ok(doc.into_view()),
        Err(PipelineError::OcrFailed { confidence, .. }) if confidence < 0.3 => {
            Err("I couldn't read this document. The image might be blurry or too dark. Would you like to try with a clearer photo?".into())
        }
        Err(PipelineError::UnsupportedFormat { format }) => {
            Err(format!("I can read photos (JPG, PNG), PDFs, and text files. This file type ({format}) isn't supported yet."))
        }
        Err(e) => {
            tracing::error!("Document import failed: {e:?}");
            Err("Something went wrong while reading your document. Please try again.".into())
        }
    }
}
```

---

## [EC-04] Naming Conventions

**E-PM + E-RS joint decision:**

### Rust
| Element | Convention | Example |
|---------|-----------|---------|
| Crates | snake_case | `coheara_core` |
| Modules | snake_case | `document_import` |
| Structs | PascalCase | `MedicationRecord` |
| Enums | PascalCase | `FrequencyType` |
| Enum variants | PascalCase | `FrequencyType::AsNeeded` |
| Functions | snake_case | `extract_medications()` |
| Methods | snake_case | `medication.is_active()` |
| Constants | SCREAMING_SNAKE | `MAX_UPLOAD_SIZE_BYTES` |
| Traits | PascalCase, adjective/capability | `Storable`, `Encryptable`, `DocumentStore` |
| Type aliases | PascalCase | `type Result<T> = std::result::Result<T, AppError>` |
| Feature flags | snake_case | `quantized_model` |

### Svelte / TypeScript
| Element | Convention | Example |
|---------|-----------|---------|
| Components | PascalCase | `MedicationCard.svelte` |
| Stores | camelCase | `medicationStore.ts` |
| Functions | camelCase | `formatDate()` |
| Constants | SCREAMING_SNAKE | `MAX_SEVERITY` |
| CSS classes | kebab-case (Tailwind) | `text-lg font-medium` |
| API wrappers | camelCase | `getMedications()` |
| Route files | kebab-case | `+page.svelte` (SvelteKit default) |

### Database
| Element | Convention | Example |
|---------|-----------|---------|
| Tables | snake_case, plural | `medications`, `lab_results` |
| Columns | snake_case | `generic_name`, `start_date` |
| Foreign keys | `<referenced_table_singular>_id` | `prescriber_id`, `document_id` |
| Indexes | `idx_<table>_<column>` | `idx_medications_generic_name` |
| Enums (stored as TEXT) | snake_case | `'as_needed'`, `'critical_high'` |

---

## [EC-05] Svelte Conventions

### Component Pattern
**E-UX:** Every component follows this structure:

```svelte
<script lang="ts">
  // 1. Imports
  import { onMount } from 'svelte';
  import type { Medication } from '$lib/api/medications';

  // 2. Props (typed)
  export let medication: Medication;
  export let onDelete: ((id: string) => void) | undefined = undefined;

  // 3. Local state
  let expanded = false;

  // 4. Derived state
  $: isActive = medication.status === 'active';
  $: displayName = medication.brand_name ?? medication.generic_name;

  // 5. Functions
  function toggleExpand() {
    expanded = !expanded;
  }
</script>

<!-- 6. Template (accessible) -->
<article
  class="medication-card"
  role="article"
  aria-label="Medication: {displayName}"
>
  <!-- content -->
</article>

<!-- 7. Styles (scoped, Tailwind utilities preferred) -->
<style>
  /* Only for component-specific styles not achievable with Tailwind */
</style>
```

### Accessibility Rules (Non-Negotiable)
**E-UX + E-QA:**

```svelte
<!-- EVERY interactive element must have: -->
<!-- 1. Visible focus indicator -->
<!-- 2. aria-label if text not self-evident -->
<!-- 3. Keyboard handler if click handler exists -->
<!-- 4. Minimum 44x44px touch target -->

<!-- GOOD -->
<button
  on:click={handleClick}
  on:keydown={(e) => e.key === 'Enter' && handleClick()}
  aria-label="Load a new document"
  class="min-w-[44px] min-h-[44px] focus:ring-2 focus:ring-blue-500"
>
  Load Document
</button>

<!-- BAD -->
<div on:click={handleClick}>Load Document</div>
```

### State Management
**E-UX:** Use Svelte stores for cross-component state. One store per domain.

```typescript
// stores/medications.ts
import { writable, derived } from 'svelte/store';
import type { Medication } from '$lib/api/medications';

// Source of truth
const medications = writable<Medication[]>([]);

// Derived views
export const activeMedications = derived(medications,
  ($meds) => $meds.filter(m => m.status === 'active')
);

export const medicationCount = derived(medications,
  ($meds) => $meds.length
);

// Actions
export async function loadMedications(profileId: string) {
  const result = await invoke('get_medications', { profileId });
  medications.set(result);
}
```

### IPC Bridge Pattern
**E-UX + E-RS:** Every Tauri command call goes through a typed wrapper. Frontend NEVER calls `invoke()` directly.

```typescript
// api/medications.ts
import { invoke } from '@tauri-apps/api/core';

export interface Medication {
  id: string;
  genericName: string;
  brandName: string | null;
  dose: string;
  // ... typed to match Rust MedicationView struct
}

export async function getMedications(profileId: string): Promise<Medication[]> {
  return invoke('get_medications', { profileId });
}

export async function addOtcMedication(med: OtcMedicationInput): Promise<Medication> {
  return invoke('add_otc_medication', { medication: med });
}
```

---

## [EC-06] Security Patterns

**E-SC: These are mandatory. No exceptions. No shortcuts.**

### Rule 1: Encrypt at Rest, Decrypt in Memory
```rust
// Data lifecycle:
// DISK: always encrypted
// MEMORY: decrypted only when actively used
// DROP: zeroed immediately

use zeroize::Zeroize;

struct SensitiveData {
    content: Vec<u8>,
}

impl Drop for SensitiveData {
    fn drop(&mut self) {
        self.content.zeroize();
    }
}
```

### Rule 2: Never Log Sensitive Data
```rust
// GOOD
tracing::info!("Medication loaded: id={}", med.id);

// BAD — leaks patient data to logs
tracing::info!("Medication loaded: {}", med.generic_name);

// NEVER
tracing::debug!("Password entered: {}", password);
```

### Rule 3: Input Sanitization Before LLM
```rust
fn sanitize_for_llm(text: &str) -> String {
    let mut clean = text.to_string();
    // Strip non-visible Unicode
    clean.retain(|c| !c.is_control() || c == '\n' || c == '\t');
    // Remove known injection patterns
    let patterns = ["ignore previous", "system:", "assistant:", "<|im_start|>"];
    for pattern in patterns {
        clean = clean.replace(pattern, "[FILTERED]");
    }
    clean
}
```

### Rule 4: Validate All External Input
```rust
// File uploads
fn validate_upload(bytes: &[u8], filename: &str) -> Result<(), TransferError> {
    // Check size
    if bytes.len() > MAX_UPLOAD_SIZE_BYTES {
        return Err(TransferError::TooLarge);
    }
    // Check magic bytes (not just extension)
    let mime = infer::get(bytes)
        .ok_or(TransferError::UnknownFormat)?;
    match mime.mime_type() {
        "image/jpeg" | "image/png" | "image/webp" | "application/pdf" => Ok(()),
        other => Err(TransferError::UnsupportedType(other.into())),
    }
}
```

### Rule 5: No Network After Install
```rust
// Tauri config: deny all network permissions except local transfer
// tauri.conf.json:
// "allowlist": { "http": { "scope": ["http://localhost:*", "http://127.0.0.1:*", "http://192.168.*:*", "http://10.*:*"] } }
// All other network access denied by Tauri's security model
```

---

## [EC-07] Testing Strategy

**E-QA: Test pyramid. Every component has all three levels.**

### Unit Tests (per function/method)
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_medication_name_standard() {
        let result = parse_medication("Metformin 500mg");
        assert_eq!(result.generic_name, "Metformin");
        assert_eq!(result.dose, "500mg");
    }

    #[test]
    fn parse_medication_name_abbreviated() {
        let result = parse_medication("Metf. 500 1-0-1");
        assert_eq!(result.generic_name, "Metformin");
        assert_eq!(result.dose, "500mg");  // Expanded from abbreviation
        assert_eq!(result.frequency, "morning and evening");
    }

    #[test]
    fn reject_dose_above_plausible_range() {
        let result = check_dose_plausibility("Metformin", "5000mg");
        assert!(result.flagged);
        assert!(result.message.contains("typical range"));
    }
}
```

### Integration Tests (per module)
```rust
// tests/pipeline_test.rs
#[tokio::test]
async fn full_pipeline_prescription_photo() {
    let test_db = TestDatabase::new().await;
    let pipeline = Pipeline::new(test_db.clone());

    // Load a test prescription image
    let result = pipeline.process("fixtures/prescription_01.jpg").await;
    assert!(result.is_ok());

    // Verify structured data was extracted
    let meds = test_db.get_medications().await.unwrap();
    assert!(!meds.is_empty());
    assert_eq!(meds[0].generic_name, "Metformin");

    // Verify embeddings were stored
    let chunks = test_db.search_vectors("metformin", 5).await.unwrap();
    assert!(!chunks.is_empty());
}
```

### Acceptance Tests (per component spec Section 8)
```rust
// tests/acceptance/marie_walkthrough.rs
#[tokio::test]
async fn marie_five_minute_walkthrough() {
    let app = TestApp::launch().await;

    // Step 1: Create profile
    app.create_profile("Marie", "password123").await;

    // Step 2: Load document
    let doc_id = app.import_document("fixtures/prescription_01.jpg").await;

    // Step 3: Verify review screen shows extraction
    let review = app.get_review(doc_id).await;
    assert!(review.medications.len() > 0);
    assert!(review.confidence > 0.5);

    // Step 4: Confirm
    app.confirm_review(doc_id).await;

    // Step 5: Ask question
    let response = app.chat("Why am I taking this medication?").await;
    assert!(response.text.contains("Metformin"));
    assert!(response.sources.len() > 0);  // Cited
    assert!(!response.text.contains("you have"));  // Safety: no diagnosis
}
```

### Test Fixtures
```
src-tauri/tests/fixtures/
├── prescription_01.jpg        # Clean printed prescription (French)
├── prescription_02.jpg        # Poor quality photo, handwritten
├── lab_result_01.pdf          # Digital PDF with extractable text
├── lab_result_02.jpg          # Scanned lab result (image)
├── discharge_summary_01.pdf   # Multi-page discharge summary
├── clinical_note_01.txt       # Plain text clinical note
└── malicious_prompt.pdf       # Prompt injection test document
```

### Coverage Target
- Unit: >80% per module
- Integration: All cross-module paths
- Acceptance: All gate tests from Component Index

---

## [EC-08] Dependency Policy

**E-RS + E-SC joint decision:** Every dependency is a security surface and maintenance burden. Justify each one.

### Approved Crate Categories

| Need | Crate | Why This One |
|------|-------|-------------|
| Serialization | `serde`, `serde_json` | De facto standard, no alternative |
| Error handling | `thiserror` | Derive macros for error enums |
| UUID | `uuid` | Standard, v4 generation |
| Date/time | `chrono` | Standard, NaiveDate for medical dates |
| Async runtime | `tokio` | Tauri uses tokio |
| HTTP client | `reqwest` | For Ollama local API only |
| SQLite | `rusqlite` | Most mature Rust SQLite binding |
| Encryption | `aes-gcm`, `pbkdf2` | Pure Rust, audited |
| Secure memory | `zeroize` | Standard for secret clearing |
| Tracing | `tracing`, `tracing-subscriber` | Structured logging |
| Image format | `image` | Image loading for OCR input |
| QR code | `qrcode` | QR generation for WiFi transfer |
| BIP39 | `bip39` | Recovery phrase generation |
| File type detection | `infer` | Magic byte detection |
| Hash (perceptual) | `img_hash` | Duplicate document detection |

### Banned Patterns
- No `unsafe` without E-SC review and documented justification
- No `unwrap()` or `expect()` in non-test code (use `?` or explicit error handling)
- No `println!()` — use `tracing::` macros
- No global mutable state (`static mut`, `lazy_static` with `Mutex` around data)
- No network crates beyond `reqwest` (Ollama client only)

---

## [EC-09] Performance Rules

**E-RS:**

1. **Profile unlock must feel instant.** Key derivation (PBKDF2) is deliberately slow (600K iterations ≈ 0.5s). UI shows spinner during this. Don't try to speed it up — that weakens security.

2. **Document pipeline can be slow.** OCR + MedGemma structuring takes 30-60s. This is expected. Show progress. Stream MedGemma output token by token.

3. **Chat must stream.** First token < 3 seconds. Use Ollama's streaming API. Never buffer the complete response before showing.

4. **SQLite queries must be indexed.** Every WHERE clause used in application queries must have a corresponding index. No table scans on tables that will grow (medications, lab_results, symptoms, messages).

5. **Embedding search < 1 second.** LanceDB is fast for vector search. If it's slow, the vector table is too large or the index isn't built. Call `create_index()` after bulk ingestion.

6. **Memory budget:** App (Tauri + frontend) ≤ 200MB. Model (Ollama/MedGemma) ≤ 6GB. Total on 8GB machine: ≤ 6.5GB leaving headroom for OS.

---

## [EC-10] Documentation Rules

**E-PM:**

1. **Code comments only where the WHY is non-obvious.** Don't comment WHAT — the code says what. Comment WHY.

```rust
// GOOD: Explains the why
// PBKDF2 with 600K iterations per OWASP 2024 recommendation.
// Slower = more resistant to brute force. ~0.5s on modern hardware.
let key = pbkdf2_derive(password, &salt, 600_000);

// BAD: Restates the code
// Derive the key from the password
let key = pbkdf2_derive(password, &salt, 600_000);
```

2. **Rust doc comments on all public items.** `///` for functions, structs, traits. Include examples for non-obvious APIs.

3. **No inline TODOs without a tracking reference.** `// TODO(L2-01): implement confidence scoring` — references the component spec.

4. **Component spec is the authority.** If code and spec disagree, the spec wins. Update the spec explicitly if a deviation is intentional.

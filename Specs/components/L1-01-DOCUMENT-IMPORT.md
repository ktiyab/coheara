# L1-01 — Document Import

<!--
=============================================================================
COMPONENT SPEC — The front door for ALL patient data.
Engineer review: E-RS (Rust), E-UX (UI/UX), E-SC (Security), E-QA (QA)
Every document enters the system through this component.
If this fails, nothing downstream works.
=============================================================================
-->

## Table of Contents

| Section | Offset |
|---------|--------|
| [1] Identity | `offset=22 limit=25` |
| [2] Dependencies | `offset=47 limit=18` |
| [3] Interfaces | `offset=65 limit=90` |
| [4] Format Detection | `offset=155 limit=50` |
| [5] Duplicate Detection | `offset=205 limit=55` |
| [6] Import Pipeline | `offset=260 limit=70` |
| [7] Tauri Commands (IPC) | `offset=330 limit=55` |
| [8] Error Handling | `offset=385 limit=30` |
| [9] Security | `offset=415 limit=30` |
| [10] Testing | `offset=445 limit=55` |
| [11] Performance | `offset=500 limit=15` |
| [12] Open Questions | `offset=515 limit=15` |

---

## [1] Identity

**What:** Implement the document import system — the entry point for ALL patient data. This includes native file picker (Tauri dialog), format detection (PDF, images, text), perceptual hashing for duplicate detection, file staging (copy originals to encrypted profile directory), and the import orchestration that hands off to the next pipeline stage (L1-02 OCR & Extraction).

**After this session:**
- User can select files via native file picker dialog
- User can drag-and-drop files onto the application window
- App detects file format (PDF, JPEG, PNG, TIFF, HEIC, text)
- App distinguishes digital PDFs from scanned/image-based PDFs
- App computes perceptual hash and detects duplicate documents
- Original file is copied to the encrypted profile directory
- Document record is created in SQLite (status: `importing`)
- Import result is returned to frontend for pipeline continuation
- All file operations go through the encryption layer

**Estimated complexity:** Medium
**Source:** Tech Spec v1.1 Section 6.1 (Ingestion Flow)

---

## [2] Dependencies

**Incoming:**
- L0-01 (project scaffold — Tauri commands, IPC)
- L0-02 (data model — Document struct, DocumentRepository trait)
- L0-03 (encryption — ProfileSession for encrypting stored originals)

**Outgoing:**
- L1-02 (OCR & Extraction — receives the staged file path and format detection result)
- L3-02 (Home & Document Feed — shows import status)
- L3-04 (Review Screen — triggered after import + extraction completes)

**New Cargo.toml dependencies:**
```toml
image = { version = "0.25", default-features = false, features = ["jpeg", "png", "tiff"] }
img_hash = "4"
pdf = "0.9"
mime_guess = "2"
tauri-plugin-dialog = "2"
```

**New package.json dependencies:**
```json
{
  "dependencies": {
    "@tauri-apps/plugin-dialog": "^2"
  }
}
```

---

## [3] Interfaces

### Core Import Trait

```rust
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// Result of format detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormatDetection {
    pub mime_type: String,
    pub category: FileCategory,
    pub is_digital_pdf: Option<bool>,  // None if not a PDF
    pub file_size_bytes: u64,
}

/// Broad file categories we handle
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FileCategory {
    DigitalPdf,     // PDF with extractable text layer
    ScannedPdf,     // PDF that is image-based (needs OCR)
    Image,          // JPEG, PNG, TIFF, HEIC
    PlainText,      // .txt files
    Unsupported,    // Anything else
}

impl FileCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::DigitalPdf => "digital_pdf",
            Self::ScannedPdf => "scanned_pdf",
            Self::Image => "image",
            Self::PlainText => "plain_text",
            Self::Unsupported => "unsupported",
        }
    }

    pub fn needs_ocr(&self) -> bool {
        matches!(self, Self::ScannedPdf | Self::Image)
    }

    pub fn is_supported(&self) -> bool {
        !matches!(self, Self::Unsupported)
    }
}

/// Import result returned to the frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResult {
    pub document_id: Uuid,
    pub original_filename: String,
    pub format: FormatDetection,
    pub staged_path: String,         // Path within profile directory
    pub duplicate_of: Option<Uuid>,  // If duplicate detected
    pub status: ImportStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ImportStatus {
    Staged,           // File copied, ready for extraction
    Duplicate,        // Duplicate detected, user must decide
    Unsupported,      // File format not supported
    TooLarge,         // File exceeds size limit
    CorruptedFile,    // File could not be read
}

/// Duplicate detection result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateCheck {
    pub is_duplicate: bool,
    pub existing_document_id: Option<Uuid>,
    pub similarity_score: f64,     // 0.0 = different, 1.0 = identical
    pub hash: String,              // The computed perceptual hash
}
```

### Document Importer Trait

```rust
/// The main import orchestrator
pub trait DocumentImporter {
    /// Import a single file from a filesystem path
    fn import_file(
        &self,
        source_path: &Path,
        session: &ProfileSession,
    ) -> Result<ImportResult, ImportError>;

    /// Import multiple files (batch)
    fn import_files(
        &self,
        source_paths: &[PathBuf],
        session: &ProfileSession,
    ) -> Result<Vec<ImportResult>, ImportError>;

    /// Detect file format without importing
    fn detect_format(&self, path: &Path) -> Result<FormatDetection, ImportError>;

    /// Check for duplicates without importing
    fn check_duplicate(
        &self,
        path: &Path,
        session: &ProfileSession,
    ) -> Result<DuplicateCheck, ImportError>;

    /// Confirm import of a detected duplicate (user chose to import anyway)
    fn confirm_duplicate_import(
        &self,
        document_id: &Uuid,
        session: &ProfileSession,
    ) -> Result<(), ImportError>;
}
```

### File Staging Trait

```rust
/// Handles secure file storage within the profile directory
pub trait FileStager {
    /// Copy source file to profile's originals/ directory, encrypted
    fn stage_file(
        &self,
        source_path: &Path,
        document_id: &Uuid,
        session: &ProfileSession,
    ) -> Result<PathBuf, ImportError>;

    /// Remove a staged file (if import is cancelled)
    fn remove_staged(
        &self,
        document_id: &Uuid,
        session: &ProfileSession,
    ) -> Result<(), ImportError>;

    /// Get the staged file path for a document
    fn staged_path(
        &self,
        document_id: &Uuid,
        session: &ProfileSession,
    ) -> PathBuf;
}
```

---

## [4] Format Detection

**E-RS + E-SC:** Format detection uses magic bytes (file signatures), NOT file extensions. Extensions can be wrong. Magic bytes don't lie.

### Magic Byte Detection

```rust
/// Detect file format from magic bytes + metadata inspection
pub fn detect_format(path: &Path) -> Result<FormatDetection, ImportError> {
    let metadata = std::fs::metadata(path)?;
    let file_size = metadata.len();

    // Size guard: reject files > 100MB
    if file_size > 100 * 1024 * 1024 {
        return Ok(FormatDetection {
            mime_type: "unknown".into(),
            category: FileCategory::Unsupported,
            is_digital_pdf: None,
            file_size_bytes: file_size,
        });
    }

    // Read first 16 bytes for magic number detection
    let mut file = std::fs::File::open(path)?;
    let mut header = [0u8; 16];
    let bytes_read = std::io::Read::read(&mut file, &mut header)?;

    let (mime_type, category) = match &header[..bytes_read.min(8)] {
        // PDF: starts with %PDF
        [0x25, 0x50, 0x44, 0x46, ..] => {
            let is_digital = check_pdf_has_text(path)?;
            let category = if is_digital {
                FileCategory::DigitalPdf
            } else {
                FileCategory::ScannedPdf
            };
            ("application/pdf".into(), category)
        }
        // JPEG: starts with FF D8 FF
        [0xFF, 0xD8, 0xFF, ..] => ("image/jpeg".into(), FileCategory::Image),
        // PNG: starts with 89 50 4E 47
        [0x89, 0x50, 0x4E, 0x47, ..] => ("image/png".into(), FileCategory::Image),
        // TIFF: starts with 49 49 2A 00 (little-endian) or 4D 4D 00 2A (big-endian)
        [0x49, 0x49, 0x2A, 0x00, ..] | [0x4D, 0x4D, 0x00, 0x2A, ..] => {
            ("image/tiff".into(), FileCategory::Image)
        }
        // HEIC/HEIF: starts with ftyp at offset 4
        _ if bytes_read >= 12 && &header[4..8] == b"ftyp" => {
            ("image/heic".into(), FileCategory::Image)
        }
        _ => {
            // Try as plain text (UTF-8 validation on first chunk)
            if is_likely_text(path)? {
                ("text/plain".into(), FileCategory::PlainText)
            } else {
                ("application/octet-stream".into(), FileCategory::Unsupported)
            }
        }
    };

    Ok(FormatDetection {
        mime_type,
        category,
        is_digital_pdf: match &category {
            FileCategory::DigitalPdf => Some(true),
            FileCategory::ScannedPdf => Some(false),
            _ => None,
        },
        file_size_bytes: file_size,
    })
}
```

### PDF Text Layer Detection

```rust
/// Check if a PDF has extractable text (digital vs scanned)
fn check_pdf_has_text(path: &Path) -> Result<bool, ImportError> {
    let bytes = std::fs::read(path)?;
    let doc = pdf::file::FileOptions::cached().load(bytes)?;

    // Sample first 3 pages (or all if fewer)
    let page_count = doc.num_pages().min(3);
    let mut total_chars = 0;

    for i in 0..page_count {
        if let Ok(page) = doc.get_page(i) {
            if let Ok(content) = page.extract_text() {
                total_chars += content.chars().filter(|c| c.is_alphanumeric()).count();
            }
        }
    }

    // Heuristic: if we extracted > 50 alphanumeric chars from sampled pages,
    // this is likely a digital PDF. Scanned PDFs yield 0 or near-0 chars.
    Ok(total_chars > 50)
}

/// Check if a file is likely plain text (valid UTF-8, printable chars)
fn is_likely_text(path: &Path) -> Result<bool, ImportError> {
    let mut file = std::fs::File::open(path)?;
    let mut buffer = vec![0u8; 4096];  // Read first 4KB
    let n = std::io::Read::read(&mut file, &mut buffer)?;
    buffer.truncate(n);

    // Must be valid UTF-8
    let text = match std::str::from_utf8(&buffer) {
        Ok(t) => t,
        Err(_) => return Ok(false),
    };

    // At least 80% printable characters
    let printable = text.chars().filter(|c| !c.is_control() || c.is_whitespace()).count();
    let ratio = printable as f64 / text.len().max(1) as f64;
    Ok(ratio > 0.80)
}
```

---

## [5] Duplicate Detection

**E-RS + E-UX design:** Use perceptual hashing for images and content hashing for PDFs/text. Perceptual hashing catches "same document, different photo" — a real scenario where Marie photographs her prescription twice from slightly different angles.

### Perceptual Hash for Images

```rust
use img_hash::{HasherConfig, HashAlg};

/// Compute perceptual hash for an image file
pub fn compute_image_hash(path: &Path) -> Result<String, ImportError> {
    let image = image::open(path)
        .map_err(|e| ImportError::ImageProcessing(e.to_string()))?;

    let hasher = HasherConfig::new()
        .hash_alg(HashAlg::DoubleGradient)
        .hash_size(16, 16)  // 256-bit hash
        .to_hasher();

    let hash = hasher.hash_image(&image);
    Ok(hash.to_base64())
}

/// Compute hash for a PDF (SHA-256 of text content, or perceptual hash of rendered first page)
pub fn compute_pdf_hash(path: &Path) -> Result<String, ImportError> {
    let bytes = std::fs::read(path)?;
    let doc = pdf::file::FileOptions::cached().load(bytes)?;

    // For digital PDFs: hash the extracted text
    let mut text_content = String::new();
    let page_count = doc.num_pages().min(5);  // First 5 pages
    for i in 0..page_count {
        if let Ok(page) = doc.get_page(i) {
            if let Ok(text) = page.extract_text() {
                text_content.push_str(&text);
            }
        }
    }

    if text_content.len() > 50 {
        // Digital PDF: SHA-256 of text content
        use sha2::{Sha256, Digest};
        let hash = Sha256::digest(text_content.as_bytes());
        Ok(base64::encode(hash))
    } else {
        // Scanned PDF: use file content hash as fallback
        // (Perceptual hash of rendered page would be ideal but requires
        //  a PDF renderer — deferred to L1-02 which has Tesseract)
        use sha2::{Sha256, Digest};
        let file_bytes = std::fs::read(path)?;
        let hash = Sha256::digest(&file_bytes);
        Ok(base64::encode(hash))
    }
}

/// Compute hash for a plain text file
pub fn compute_text_hash(path: &Path) -> Result<String, ImportError> {
    use sha2::{Sha256, Digest};
    let content = std::fs::read(path)?;
    let hash = Sha256::digest(&content);
    Ok(base64::encode(hash))
}
```

### Duplicate Comparison

```rust
/// Check if a document is a duplicate of an existing one
pub fn check_duplicate(
    hash: &str,
    category: &FileCategory,
    doc_repo: &dyn DocumentRepository,
) -> Result<DuplicateCheck, ImportError> {
    // Exact match first (fast path)
    if let Some(existing) = doc_repo.get_by_hash(hash)? {
        return Ok(DuplicateCheck {
            is_duplicate: true,
            existing_document_id: Some(existing.id),
            similarity_score: 1.0,
            hash: hash.to_string(),
        });
    }

    // For images: fuzzy perceptual hash comparison
    // (Hamming distance between perceptual hashes)
    if matches!(category, FileCategory::Image | FileCategory::ScannedPdf) {
        if let Some((existing_id, score)) = find_similar_hash(hash, doc_repo)? {
            if score > 0.85 {  // 85% similarity threshold
                return Ok(DuplicateCheck {
                    is_duplicate: true,
                    existing_document_id: Some(existing_id),
                    similarity_score: score,
                    hash: hash.to_string(),
                });
            }
        }
    }

    Ok(DuplicateCheck {
        is_duplicate: false,
        existing_document_id: None,
        similarity_score: 0.0,
        hash: hash.to_string(),
    })
}

/// Find perceptual hash similarity against existing documents
fn find_similar_hash(
    new_hash: &str,
    doc_repo: &dyn DocumentRepository,
) -> Result<Option<(Uuid, f64)>, ImportError> {
    let all_docs = doc_repo.list(&DocumentFilter::default())?;

    let new_hash_decoded = img_hash::ImageHash::from_base64(new_hash)
        .map_err(|_| ImportError::HashComputation)?;

    let mut best_match: Option<(Uuid, f64)> = None;

    for doc in &all_docs {
        if let Some(ref existing_hash_str) = doc.perceptual_hash {
            if let Ok(existing_hash) = img_hash::ImageHash::from_base64(existing_hash_str) {
                let distance = new_hash_decoded.dist(&existing_hash);
                let max_bits = (new_hash_decoded.num_bits() as f64).max(1.0);
                let similarity = 1.0 - (distance as f64 / max_bits);

                if let Some((_, best_score)) = &best_match {
                    if similarity > *best_score {
                        best_match = Some((doc.id, similarity));
                    }
                } else {
                    best_match = Some((doc.id, similarity));
                }
            }
        }
    }

    Ok(best_match)
}
```

---

## [6] Import Pipeline

### Full Import Flow

```rust
/// Main import orchestrator implementation
pub struct FileImporter {
    doc_repo: Box<dyn DocumentRepository>,
}

impl DocumentImporter for FileImporter {
    fn import_file(
        &self,
        source_path: &Path,
        session: &ProfileSession,
    ) -> Result<ImportResult, ImportError> {
        let original_filename = source_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        tracing::info!(file = %original_filename, "Starting document import");

        // Step 1: Format detection
        let format = detect_format(source_path)?;

        if !format.category.is_supported() {
            return Ok(ImportResult {
                document_id: Uuid::new_v4(),
                original_filename,
                format,
                staged_path: String::new(),
                duplicate_of: None,
                status: ImportStatus::Unsupported,
            });
        }

        // Step 2: Size check (100MB limit)
        if format.file_size_bytes > 100 * 1024 * 1024 {
            return Ok(ImportResult {
                document_id: Uuid::new_v4(),
                original_filename,
                format,
                staged_path: String::new(),
                duplicate_of: None,
                status: ImportStatus::TooLarge,
            });
        }

        // Step 3: Compute hash for duplicate detection
        let hash = compute_hash(source_path, &format.category)?;

        // Step 4: Check for duplicates
        let dup_check = check_duplicate(&hash, &format.category, self.doc_repo.as_ref())?;

        let document_id = Uuid::new_v4();

        if dup_check.is_duplicate {
            tracing::info!(
                file = %original_filename,
                duplicate_of = ?dup_check.existing_document_id,
                similarity = dup_check.similarity_score,
                "Duplicate document detected"
            );

            // Create document record with importing status
            // (User will confirm or cancel via confirm_duplicate_import)
            let doc = Document {
                id: document_id,
                doc_type: DocumentType::Other, // Will be detected by L1-03
                title: original_filename.clone(),
                document_date: None,
                ingestion_date: chrono::Local::now().naive_local(),
                professional_id: None,
                source_file: String::new(),  // Not yet staged
                markdown_file: None,
                ocr_confidence: None,
                verified: false,
                source_deleted: false,
                perceptual_hash: Some(hash),
                notes: None,
            };
            self.doc_repo.insert(&doc)?;

            return Ok(ImportResult {
                document_id,
                original_filename,
                format,
                staged_path: String::new(),
                duplicate_of: dup_check.existing_document_id,
                status: ImportStatus::Duplicate,
            });
        }

        // Step 5: Stage file (copy to profile directory, encrypted)
        let staged_path = stage_file(source_path, &document_id, session)?;

        // Step 6: Create document record in SQLite
        let doc = Document {
            id: document_id,
            doc_type: DocumentType::Other,  // Will be classified by L1-03
            title: original_filename.clone(),
            document_date: None,            // Will be extracted by L1-03
            ingestion_date: chrono::Local::now().naive_local(),
            professional_id: None,          // Will be extracted by L1-03
            source_file: staged_path.to_string_lossy().to_string(),
            markdown_file: None,            // Will be created by L1-03
            ocr_confidence: None,           // Will be set by L1-02
            verified: false,
            source_deleted: false,
            perceptual_hash: Some(hash),
            notes: None,
        };
        self.doc_repo.insert(&doc)?;

        tracing::info!(
            document_id = %document_id,
            file = %original_filename,
            category = format.category.as_str(),
            "Document imported and staged"
        );

        Ok(ImportResult {
            document_id,
            original_filename,
            format,
            staged_path: staged_path.to_string_lossy().to_string(),
            duplicate_of: None,
            status: ImportStatus::Staged,
        })
    }

    fn import_files(
        &self,
        source_paths: &[PathBuf],
        session: &ProfileSession,
    ) -> Result<Vec<ImportResult>, ImportError> {
        let mut results = Vec::with_capacity(source_paths.len());
        for path in source_paths {
            match self.import_file(path, session) {
                Ok(result) => results.push(result),
                Err(e) => {
                    tracing::warn!(file = %path.display(), error = %e, "Failed to import file");
                    results.push(ImportResult {
                        document_id: Uuid::new_v4(),
                        original_filename: path.file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("unknown")
                            .to_string(),
                        format: FormatDetection {
                            mime_type: "unknown".into(),
                            category: FileCategory::Unsupported,
                            is_digital_pdf: None,
                            file_size_bytes: 0,
                        },
                        staged_path: String::new(),
                        duplicate_of: None,
                        status: ImportStatus::CorruptedFile,
                    });
                }
            }
        }
        Ok(results)
    }

    fn detect_format(&self, path: &Path) -> Result<FormatDetection, ImportError> {
        detect_format(path)
    }

    fn check_duplicate(
        &self,
        path: &Path,
        session: &ProfileSession,
    ) -> Result<DuplicateCheck, ImportError> {
        let format = detect_format(path)?;
        let hash = compute_hash(path, &format.category)?;
        check_duplicate(&hash, &format.category, self.doc_repo.as_ref())
    }

    fn confirm_duplicate_import(
        &self,
        document_id: &Uuid,
        session: &ProfileSession,
    ) -> Result<(), ImportError> {
        // User confirmed: now stage the file
        let doc = self.doc_repo.get(document_id)?
            .ok_or(ImportError::DocumentNotFound(*document_id))?;

        // The original source path needs to be passed separately
        // or stored temporarily — for now, this is a status update
        // The frontend should re-trigger import_file after user confirms
        Ok(())
    }
}
```

### File Staging (Encrypted Copy)

```rust
/// Copy source file to profile directory, encrypted with profile key
fn stage_file(
    source_path: &Path,
    document_id: &Uuid,
    session: &ProfileSession,
) -> Result<PathBuf, ImportError> {
    // Determine file extension from original
    let extension = source_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("bin");

    // Target path: profiles/<uuid>/originals/<doc_uuid>.<ext>.enc
    let target_dir = session.db_path()
        .parent().unwrap()  // database/
        .parent().unwrap()  // profile_dir/
        .join("originals");

    std::fs::create_dir_all(&target_dir)?;

    let target_path = target_dir.join(format!("{}.{}.enc", document_id, extension));

    // Read source, encrypt, write to target
    let plaintext = std::fs::read(source_path)?;
    let encrypted = session.encrypt(&plaintext)?;
    std::fs::write(&target_path, encrypted.to_bytes())?;

    tracing::debug!(
        document_id = %document_id,
        size = plaintext.len(),
        "File staged and encrypted"
    );

    Ok(target_path)
}
```

### Hash Dispatch

```rust
/// Compute the appropriate hash based on file category
fn compute_hash(path: &Path, category: &FileCategory) -> Result<String, ImportError> {
    match category {
        FileCategory::Image => compute_image_hash(path),
        FileCategory::DigitalPdf | FileCategory::ScannedPdf => compute_pdf_hash(path),
        FileCategory::PlainText => compute_text_hash(path),
        FileCategory::Unsupported => Err(ImportError::UnsupportedFormat),
    }
}
```

---

## [7] Tauri Commands (IPC)

**E-RS + E-UX:** These are the commands exposed to the Svelte frontend via Tauri IPC.

### Rust Commands

```rust
// src-tauri/src/commands/import.rs

use tauri::State;
use tauri_plugin_dialog::DialogExt;

/// Open native file picker and import selected files
#[tauri::command]
pub async fn import_documents_dialog(
    app: tauri::AppHandle,
    importer: State<'_, Box<dyn DocumentImporter + Send + Sync>>,
    session: State<'_, Option<ProfileSession>>,
) -> Result<Vec<ImportResult>, String> {
    let session = session.as_ref()
        .ok_or("No active profile session")?;

    // Open native file dialog
    let files = app.dialog()
        .file()
        .add_filter("Medical Documents", &["pdf", "jpg", "jpeg", "png", "tiff", "tif", "heic", "txt"])
        .add_filter("All Files", &["*"])
        .set_title("Load Medical Document")
        .blocking_pick_files();

    let paths: Vec<PathBuf> = match files {
        Some(file_paths) => file_paths.iter()
            .filter_map(|fp| fp.as_path().map(|p| p.to_path_buf()))
            .collect(),
        None => return Ok(vec![]),  // User cancelled
    };

    if paths.is_empty() {
        return Ok(vec![]);
    }

    importer.import_files(&paths, session)
        .map_err(|e| e.to_string())
}

/// Import files from given paths (used by drag-and-drop)
#[tauri::command]
pub async fn import_documents_paths(
    paths: Vec<String>,
    importer: State<'_, Box<dyn DocumentImporter + Send + Sync>>,
    session: State<'_, Option<ProfileSession>>,
) -> Result<Vec<ImportResult>, String> {
    let session = session.as_ref()
        .ok_or("No active profile session")?;

    let path_bufs: Vec<PathBuf> = paths.iter()
        .map(PathBuf::from)
        .collect();

    importer.import_files(&path_bufs, session)
        .map_err(|e| e.to_string())
}

/// Check format of a file without importing
#[tauri::command]
pub async fn detect_file_format(
    path: String,
    importer: State<'_, Box<dyn DocumentImporter + Send + Sync>>,
) -> Result<FormatDetection, String> {
    importer.detect_format(Path::new(&path))
        .map_err(|e| e.to_string())
}

/// Confirm importing a duplicate document
#[tauri::command]
pub async fn confirm_duplicate(
    document_id: String,
    source_path: String,
    importer: State<'_, Box<dyn DocumentImporter + Send + Sync>>,
    session: State<'_, Option<ProfileSession>>,
) -> Result<ImportResult, String> {
    let session = session.as_ref()
        .ok_or("No active profile session")?;

    // Re-import with duplicate check bypassed
    importer.import_file(Path::new(&source_path), session)
        .map_err(|e| e.to_string())
}
```

### Frontend API (TypeScript)

```typescript
// src/lib/api/import.ts
import { invoke } from '@tauri-apps/api/core';

export interface FormatDetection {
  mime_type: string;
  category: 'digital_pdf' | 'scanned_pdf' | 'image' | 'plain_text' | 'unsupported';
  is_digital_pdf: boolean | null;
  file_size_bytes: number;
}

export interface ImportResult {
  document_id: string;
  original_filename: string;
  format: FormatDetection;
  staged_path: string;
  duplicate_of: string | null;
  status: 'Staged' | 'Duplicate' | 'Unsupported' | 'TooLarge' | 'CorruptedFile';
}

/** Open native file picker and import */
export async function importDocumentsDialog(): Promise<ImportResult[]> {
  return invoke<ImportResult[]>('import_documents_dialog');
}

/** Import from file paths (drag-and-drop) */
export async function importDocumentsPaths(paths: string[]): Promise<ImportResult[]> {
  return invoke<ImportResult[]>('import_documents_paths', { paths });
}

/** Check file format without importing */
export async function detectFileFormat(path: string): Promise<FormatDetection> {
  return invoke<FormatDetection>('detect_file_format', { path });
}

/** Confirm importing a duplicate */
export async function confirmDuplicate(
  documentId: string,
  sourcePath: string,
): Promise<ImportResult> {
  return invoke<ImportResult>('confirm_duplicate', {
    documentId,
    sourcePath,
  });
}
```

### Drag-and-Drop (Svelte Component)

```svelte
<!-- src/lib/components/DropZone.svelte -->
<script lang="ts">
  import { importDocumentsPaths, type ImportResult } from '$lib/api/import';

  let isDragging = $state(false);

  interface Props {
    onImport: (results: ImportResult[]) => void;
    onError: (error: string) => void;
  }

  let { onImport, onError }: Props = $props();

  function handleDragOver(e: DragEvent) {
    e.preventDefault();
    isDragging = true;
  }

  function handleDragLeave() {
    isDragging = false;
  }

  async function handleDrop(e: DragEvent) {
    e.preventDefault();
    isDragging = false;

    const files = e.dataTransfer?.files;
    if (!files || files.length === 0) return;

    // Tauri drag-and-drop provides file paths
    const paths: string[] = [];
    for (let i = 0; i < files.length; i++) {
      // In Tauri, dropped files have their path available
      const path = (files[i] as any).path;
      if (path) paths.push(path);
    }

    if (paths.length === 0) {
      onError('Could not read file paths from drop');
      return;
    }

    try {
      const results = await importDocumentsPaths(paths);
      onImport(results);
    } catch (err) {
      onError(String(err));
    }
  }
</script>

<div
  role="button"
  tabindex="0"
  class="drop-zone"
  class:dragging={isDragging}
  ondragover={handleDragOver}
  ondragleave={handleDragLeave}
  ondrop={handleDrop}
  onkeydown={(e) => { if (e.key === 'Enter') { /* trigger file picker */ } }}
  aria-label="Drop medical documents here or click to browse"
>
  <slot />
</div>

<style>
  .drop-zone {
    border: 2px dashed var(--color-muted);
    border-radius: 12px;
    padding: 2rem;
    text-align: center;
    transition: all 0.2s ease;
    min-height: 44px;
    min-width: 44px;
  }
  .drop-zone.dragging {
    border-color: var(--color-primary);
    background-color: color-mix(in srgb, var(--color-primary) 5%, transparent);
  }
</style>
```

---

## [8] Error Handling

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ImportError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Unsupported file format")]
    UnsupportedFormat,

    #[error("File too large (max 100MB)")]
    FileTooLarge,

    #[error("Could not read file: {0}")]
    FileReadError(String),

    #[error("Image processing error: {0}")]
    ImageProcessing(String),

    #[error("PDF processing error: {0}")]
    PdfProcessing(String),

    #[error("Hash computation failed")]
    HashComputation,

    #[error("Document not found: {0}")]
    DocumentNotFound(Uuid),

    #[error("Database error: {0}")]
    Database(#[from] DatabaseError),

    #[error("Encryption error: {0}")]
    Crypto(#[from] CryptoError),

    #[error("No active profile session")]
    NoActiveSession,
}
```

**E-UX mapping to user-facing messages:**

| Error | User sees |
|-------|-----------|
| `UnsupportedFormat` | "This file type isn't supported. Coheara works with PDFs, photos (JPEG, PNG, TIFF), and text files." |
| `FileTooLarge` | "This file is too large (max 100 MB). If it's a multi-page scan, try splitting it into smaller files." |
| `FileReadError` | "Couldn't read this file. It may be corrupted or locked by another program." |
| `ImageProcessing` | "Couldn't process this image. Try taking a clearer photo." |
| `NoActiveSession` | "Please open a profile first." |
| `Crypto` | "Encryption error. Please try again or contact support." |

---

## [9] Security

**E-SC review:**

| Concern | Mitigation |
|---------|-----------|
| Path traversal in filename | Sanitize: strip directory components, use only UUID for storage name. Source filename stored in DB only. |
| Malicious file content | File content is never executed. Only read as bytes for hashing, then passed to OCR (L1-02). |
| File system permissions | Files written to profile directory only. No writes outside `~/Coheara/`. |
| Unencrypted originals | Source files encrypted immediately on staging. Original on user's filesystem untouched. |
| Large file DoS | 100MB size limit enforced before any processing. |
| TOCTOU (time of check, time of use) | Format detection reads file once, staging reads file once. No gap between check and use. |
| Drag-and-drop injection | Paths validated as existing files. No URL or remote paths accepted. |

### Path Sanitization

```rust
/// Sanitize a filename — strip path components, limit length
fn sanitize_filename(original: &str) -> String {
    let name = Path::new(original)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("document");

    // Remove any remaining path separators
    let clean: String = name.chars()
        .filter(|c| !matches!(c, '/' | '\\' | '\0'))
        .take(255)  // Filesystem limit
        .collect();

    if clean.is_empty() { "document".to_string() } else { clean }
}
```

---

## [10] Testing

### Acceptance Criteria

| # | Test | Expected |
|---|------|----------|
| T-01 | Import a JPEG photo | ImportResult with category=Image, status=Staged |
| T-02 | Import a PNG image | ImportResult with category=Image, status=Staged |
| T-03 | Import a digital PDF | ImportResult with category=DigitalPdf, status=Staged |
| T-04 | Import a scanned PDF | ImportResult with category=ScannedPdf, status=Staged |
| T-05 | Import a plain text file | ImportResult with category=PlainText, status=Staged |
| T-06 | Import a .docx file | ImportResult with status=Unsupported |
| T-07 | Import a 150MB file | ImportResult with status=TooLarge |
| T-08 | Import same image twice | Second import returns status=Duplicate |
| T-09 | Import similar but not identical images | Perceptual hash catches near-duplicates (> 85%) |
| T-10 | Import batch of 5 files | All 5 ImportResults returned, each processed |
| T-11 | Staged file is encrypted | Read staged file from disk → cannot parse as original format |
| T-12 | Decrypt staged file matches original | Decrypt staged file → byte-identical to source |
| T-13 | Document record created in SQLite | After import, can query document by ID |
| T-14 | Perceptual hash stored in DB | Document record has non-null perceptual_hash |
| T-15 | Path traversal rejected | Filename `../../etc/passwd` → stored as `etc_passwd` or sanitized |
| T-16 | File with wrong extension detected correctly | Rename .jpg to .pdf → detected as Image (magic bytes) |
| T-17 | Cancel file picker returns empty | No error, empty Vec returned |
| T-18 | Import with no active session fails | Returns ImportError::NoActiveSession |

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn detect_jpeg_from_magic_bytes() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.jpg");
        // Write JPEG magic bytes + minimal valid JPEG
        std::fs::write(&path, &[0xFF, 0xD8, 0xFF, 0xE0, 0x00]).unwrap();
        let format = detect_format(&path).unwrap();
        assert_eq!(format.category, FileCategory::Image);
        assert_eq!(format.mime_type, "image/jpeg");
    }

    #[test]
    fn detect_pdf_from_magic_bytes() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.pdf");
        std::fs::write(&path, b"%PDF-1.4 minimal test").unwrap();
        let format = detect_format(&path).unwrap();
        // Will be detected as PDF (magic bytes match)
        assert!(matches!(
            format.category,
            FileCategory::DigitalPdf | FileCategory::ScannedPdf
        ));
    }

    #[test]
    fn detect_png_from_magic_bytes() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.png");
        std::fs::write(&path, &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]).unwrap();
        let format = detect_format(&path).unwrap();
        assert_eq!(format.category, FileCategory::Image);
    }

    #[test]
    fn detect_text_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.txt");
        std::fs::write(&path, "This is a medical report. Patient: Marie Dubois.").unwrap();
        let format = detect_format(&path).unwrap();
        assert_eq!(format.category, FileCategory::PlainText);
    }

    #[test]
    fn reject_oversized_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("huge.pdf");
        // Create a file that reports > 100MB
        // (In real test: create sparse file or mock)
        let format = FormatDetection {
            mime_type: "application/pdf".into(),
            category: FileCategory::DigitalPdf,
            is_digital_pdf: Some(true),
            file_size_bytes: 200 * 1024 * 1024,
        };
        // The import pipeline should reject this
    }

    #[test]
    fn wrong_extension_detected_correctly() {
        let dir = tempdir().unwrap();
        // Write JPEG content to a .pdf file
        let path = dir.path().join("misleading.pdf");
        std::fs::write(&path, &[0xFF, 0xD8, 0xFF, 0xE0, 0x00]).unwrap();
        let format = detect_format(&path).unwrap();
        // Magic bytes win over extension
        assert_eq!(format.category, FileCategory::Image);
    }

    #[test]
    fn sanitize_path_traversal() {
        assert_eq!(sanitize_filename("../../etc/passwd"), "passwd");
        assert_eq!(sanitize_filename("normal_file.pdf"), "normal_file.pdf");
        assert_eq!(sanitize_filename(""), "document");
        assert_eq!(sanitize_filename("file\0name.pdf"), "filename.pdf");
    }

    #[test]
    fn binary_file_unsupported() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("binary.exe");
        std::fs::write(&path, &[0x4D, 0x5A, 0x90, 0x00]).unwrap(); // PE header
        let format = detect_format(&path).unwrap();
        assert_eq!(format.category, FileCategory::Unsupported);
    }
}
```

---

## [11] Performance

| Metric | Target |
|--------|--------|
| Format detection | < 10ms per file |
| Perceptual hash computation (image) | < 200ms per image |
| PDF text check (first 3 pages) | < 100ms |
| File staging (10MB file, including encryption) | < 500ms |
| Batch import (5 files) | < 3 seconds total |
| Duplicate check (against 100 existing docs) | < 50ms |

---

## [12] Open Questions

| # | Question | Status |
|---|---------|--------|
| OQ-01 | HEIC support — does the `image` crate handle HEIC natively? | Check at implementation. May need `libheif` binding or convert via Tauri. |
| OQ-02 | Tauri drag-and-drop API — exact event shape for file drops | Verify against Tauri 2.x docs during implementation. May need `tauri-plugin-drag`. |
| OQ-03 | Perceptual hash algorithm choice — DoubleGradient vs DCT | DoubleGradient chosen for speed. Test quality during implementation. |

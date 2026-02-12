# L3-04 — Review Screen

<!--
=============================================================================
COMPONENT SPEC — Side-by-side document review. The trust gate.
Engineer review: E-UX (UI/UX, lead), E-RS (Rust), E-ML (AI/ML), E-SC (Security), E-QA (QA)
Marie sees the original document next to what Coheara extracted.
She confirms, corrects, or rejects. Nothing enters the system without her say-so.
=============================================================================
-->

## Table of Contents

| Section | Offset |
|---------|--------|
| [1] Identity | `offset=20 limit=35` |
| [2] Dependencies | `offset=55 limit=22` |
| [3] Interfaces | `offset=77 limit=110` |
| [4] Side-by-Side Layout | `offset=187 limit=60` |
| [5] Original Document Display | `offset=247 limit=55` |
| [6] Extracted Content Display | `offset=302 limit=70` |
| [7] Confidence Flagging | `offset=372 limit=55` |
| [8] Correction Interface | `offset=427 limit=65` |
| [9] Confirm / Reject Flow | `offset=492 limit=70` |
| [10] Tauri Commands (IPC) | `offset=562 limit=80` |
| [11] Svelte Components | `offset=642 limit=210` |
| [12] Frontend API | `offset=852 limit=40` |
| [13] Error Handling | `offset=892 limit=30` |
| [14] Security | `offset=922 limit=30` |
| [15] Testing | `offset=952 limit=60` |
| [16] Performance | `offset=1012 limit=15` |
| [17] Open Questions | `offset=1027 limit=15` |

---

## [1] Identity

**What:** The review screen — a side-by-side interface where the patient sees the original document (image/PDF) on the left and the extracted structured Markdown on the right. Key medical fields (medications, lab results, dates, professional names) are highlighted with color coding. Fields below 0.70 confidence are visually flagged with a gentle message: "I'm not sure I read this correctly -- please check." The patient can edit any extracted field inline, then confirm or reject the entire extraction. On confirmation, the storage pipeline (L1-04) writes entities to SQLite and chunks to LanceDB. On rejection, the document returns to import state for re-processing or manual entry.

**After this session:**
- Side-by-side layout: original document (left pane) + extracted content (right pane)
- Original document viewer with zoom, pan, and page navigation (images and PDFs)
- Extracted content rendered as structured Markdown with field-level highlighting
- Color-coded categories: medications (blue), lab results (green), dates (amber), professionals (purple)
- Confidence flags on fields below 0.70 threshold with "I'm not sure" message
- Inline editing of any extracted field (click to edit, Enter to save)
- Dose plausibility warnings from coherence engine shown inline
- Confirm button triggers L1-04 storage pipeline (entities to SQLite, chunks to LanceDB)
- Reject button returns document to pending state with optional reason
- Document status updated to "confirmed" or "corrected" after successful review
- Profile trust metrics updated: verified_count incremented on confirm, corrected_count on corrections
- Tauri event `document-reviewed` emitted after confirm (triggers home feed refresh)

**Estimated complexity:** High
**Source:** Tech Spec v1.1 Section 6.1 (Patient Review Screen), Section 6.2 (Confidence Scoring)

---

## [2] Dependencies

**Incoming:**
- L1-02 (OCR extraction -- raw text + per-field confidence scores)
- L1-03 (medical structuring -- StructuringResult with ExtractedEntities + structured Markdown)
- L1-04 (storage pipeline -- StoragePipeline trait, triggered after patient confirms)
- L0-03 (encryption -- ProfileSession for decrypting original files and Markdown)
- L2-03 (coherence engine -- dose plausibility checks, allergy cross-references)

**Outgoing:**
- L3-02 (home feed -- emits `document-reviewed` event, status updated to confirmed/corrected)
- L2-03 (coherence engine -- runs full conflict detection after confirmed entities are stored)
- L1-04 (storage pipeline -- invoked with corrected entities on confirm)

**No new Cargo.toml dependencies.** Uses existing image/PDF rendering in frontend and Tauri IPC.

---

## [3] Interfaces

### Backend Types

```rust
// src-tauri/src/review.rs

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Complete data needed to render the review screen
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewData {
    pub document_id: Uuid,
    pub original_file_path: String,          // Path to the original file (encrypted)
    pub original_file_type: OriginalFileType,
    pub document_type: String,               // "Prescription", "Lab Report", etc.
    pub document_date: Option<NaiveDate>,
    pub professional_name: Option<String>,
    pub professional_specialty: Option<String>,
    pub structured_markdown: String,          // Full structured Markdown
    pub extracted_fields: Vec<ExtractedField>,
    pub plausibility_warnings: Vec<PlausibilityWarning>,
    pub overall_confidence: f32,
}

/// The type of the original file for rendering
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OriginalFileType {
    Image,   // JPG, PNG, TIFF — render with image viewer
    Pdf,     // PDF — render with PDF viewer
}

/// A single extracted field for review with confidence and category
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedField {
    pub id: Uuid,                            // Unique ID for this field instance
    pub entity_type: EntityCategory,
    pub entity_index: usize,                 // Index within the entity array
    pub field_name: String,                  // "generic_name", "dose", "value", etc.
    pub display_label: String,               // "Medication name", "Dose", "Test value"
    pub value: String,                       // Current extracted value
    pub confidence: f32,                     // 0.0 - 1.0
    pub is_flagged: bool,                    // true if confidence < 0.70
    pub source_hint: Option<String>,         // Where in the original doc this came from
}

/// Category of entity for color-coding
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EntityCategory {
    Medication,
    LabResult,
    Diagnosis,
    Allergy,
    Procedure,
    Referral,
    Professional,
    Date,
    Instruction,
}

/// A plausibility warning from the coherence engine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlausibilityWarning {
    pub field_id: Uuid,                      // Links to ExtractedField.id
    pub warning_type: PlausibilityType,
    pub message: String,                     // Patient-facing calm message
    pub severity: WarningSeverity,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PlausibilityType {
    DoseUnusuallyHigh,
    DoseUnusuallyLow,
    FrequencyUnusual,
    LabValueCritical,
    AllergyConflict,
    DuplicateMedication,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WarningSeverity {
    Info,       // "Worth double-checking"
    Warning,    // "This looks unusual"
    Critical,   // "This may need immediate attention"
}

/// A field correction submitted by the patient
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldCorrection {
    pub field_id: Uuid,
    pub original_value: String,
    pub corrected_value: String,
}

/// Result of confirming a review
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewConfirmResult {
    pub document_id: Uuid,
    pub status: ReviewOutcome,
    pub entities_stored: EntitiesStoredCount,
    pub corrections_applied: usize,
    pub chunks_stored: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReviewOutcome {
    Confirmed,    // Patient confirmed without corrections
    Corrected,    // Patient made corrections then confirmed
}

/// Result of rejecting a review
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewRejectResult {
    pub document_id: Uuid,
    pub reason: Option<String>,
}
```

### Frontend Types

```typescript
// src/lib/types/review.ts

export interface ReviewData {
  document_id: string;
  original_file_path: string;
  original_file_type: 'Image' | 'Pdf';
  document_type: string;
  document_date: string | null;
  professional_name: string | null;
  professional_specialty: string | null;
  structured_markdown: string;
  extracted_fields: ExtractedField[];
  plausibility_warnings: PlausibilityWarning[];
  overall_confidence: number;
}

export interface ExtractedField {
  id: string;
  entity_type: EntityCategory;
  entity_index: number;
  field_name: string;
  display_label: string;
  value: string;
  confidence: number;
  is_flagged: boolean;
  source_hint: string | null;
}

export type EntityCategory =
  | 'Medication'
  | 'LabResult'
  | 'Diagnosis'
  | 'Allergy'
  | 'Procedure'
  | 'Referral'
  | 'Professional'
  | 'Date'
  | 'Instruction';

export interface PlausibilityWarning {
  field_id: string;
  warning_type: string;
  message: string;
  severity: 'Info' | 'Warning' | 'Critical';
}

export interface FieldCorrection {
  field_id: string;
  original_value: string;
  corrected_value: string;
}

export interface ReviewConfirmResult {
  document_id: string;
  status: 'Confirmed' | 'Corrected';
  entities_stored: {
    medications: number;
    lab_results: number;
    diagnoses: number;
    allergies: number;
    procedures: number;
    referrals: number;
    instructions: number;
  };
  corrections_applied: number;
  chunks_stored: number;
}

export interface ReviewRejectResult {
  document_id: string;
  reason: string | null;
}
```

---

## [4] Side-by-Side Layout

**E-UX lead:** The review screen is the trust gate. Marie sees her original document next to the extraction. She must be able to compare them easily. The layout splits the screen into two resizable panes. On narrow screens (< 768px), the panes stack vertically with a tab switcher.

### Layout Zones

```
┌──────────────────────────────────────────────────────────────────────────┐
│  HEADER BAR                                                              │
│  [<- Back]   "Review: Prescription - Dr. Chen - Jan 15, 2024"   [Help]  │
├─────────────────────────────┬────────────────────────────────────────────┤
│  ORIGINAL DOCUMENT          │  EXTRACTED CONTENT                         │
│  (Left Pane — ~45%)        │  (Right Pane — ~55%)                       │
│                             │                                            │
│  ┌───────────────────────┐  │  ┌──────────────────────────────────────┐  │
│  │                       │  │  │  ## Medications                     │  │
│  │   [Image/PDF viewer]  │  │  │                                     │  │
│  │                       │  │  │  [Metformin] [500mg] [twice daily]  │  │
│  │   Zoom: [+] [-] [fit] │  │  │     confidence: 0.92 ✓             │  │
│  │   Page: [< 1/3 >]     │  │  │                                     │  │
│  │                       │  │  │  [Atorvastatin] [20mg] [at bedtime] │  │
│  │                       │  │  │     confidence: 0.55 ⚠              │  │
│  │                       │  │  │     "I'm not sure I read this       │  │
│  │                       │  │  │      correctly -- please check"     │  │
│  │                       │  │  │                                     │  │
│  │                       │  │  │  ## Lab Results                     │  │
│  │                       │  │  │  [HbA1c] [7.2%] [4.0-6.0]          │  │
│  │                       │  │  │     confidence: 0.88 ✓              │  │
│  │                       │  │  │                                     │  │
│  │                       │  │  │  ## Professional                    │  │
│  │                       │  │  │  [Dr. Chen] [Cardiologist]          │  │
│  │                       │  │  │                                     │  │
│  │                       │  │  │  ## Date                            │  │
│  │                       │  │  │  [January 15, 2024]                 │  │
│  └───────────────────────┘  │  └──────────────────────────────────────┘  │
├─────────────────────────────┴────────────────────────────────────────────┤
│  CONFIDENCE SUMMARY BAR                                                  │
│  "12 fields extracted · 10 confident · 2 need checking"                  │
├──────────────────────────────────────────────────────────────────────────┤
│  ACTION BAR                                                              │
│  [ Reject ]                                           [ Confirm ✓ ]     │
│  "Not right, try again"                          "Looks good to me"     │
└──────────────────────────────────────────────────────────────────────────┘
```

### Responsive Rules

| Screen Width | Layout |
|-------------|--------|
| >= 1024px | Side-by-side, resizable divider |
| 768px - 1023px | Side-by-side, fixed 50/50 |
| < 768px | Stacked with tab switcher: [Original] / [Extracted] |

### Resizable Divider

The vertical divider between panes is draggable. Minimum pane width: 300px. The divider is 4px wide with a grab handle indicator.

---

## [5] Original Document Display

**E-UX:** The original document must be viewable at any zoom level. Marie needs to read small print, compare handwritten notes, and verify that the extraction matches what she sees.

### Image Viewer (JPG, PNG, TIFF)

- Renders the decrypted image in a scrollable, zoomable container
- Zoom controls: [+] / [-] buttons + mouse wheel/pinch
- Fit-to-width button resets zoom to fill the pane
- Pan by click-and-drag when zoomed in
- Rotation button (90-degree increments) for photos taken at wrong orientation

### PDF Viewer

- Renders PDF pages using `<iframe>` with Tauri's asset protocol or a canvas-based renderer
- Page navigation: [Previous] [Page N of M] [Next]
- Zoom controls same as image viewer
- Scrollable page view for multi-page documents

### Decryption Flow

The original file is stored encrypted on disk (L1-01). The backend decrypts it into a temporary in-memory buffer and serves it via Tauri's asset protocol. The decrypted file is NEVER written to disk.

```rust
/// Decrypt and serve the original document file for review
pub fn decrypt_original_for_review(
    document_id: &Uuid,
    session: &ProfileSession,
    repos: &RepositorySet,
) -> Result<(Vec<u8>, OriginalFileType), ReviewError> {
    let doc = repos.document.get(document_id)?
        .ok_or(ReviewError::DocumentNotFound(*document_id))?;

    let encrypted_path = PathBuf::from(&doc.staged_path);
    let encrypted_bytes = std::fs::read(&encrypted_path)
        .map_err(|e| ReviewError::FileRead(e.to_string()))?;

    let decrypted = session.decrypt(&encrypted_bytes)
        .map_err(|e| ReviewError::Decryption(e.to_string()))?;

    let file_type = if doc.source_filename.ends_with(".pdf") {
        OriginalFileType::Pdf
    } else {
        OriginalFileType::Image
    };

    Ok((decrypted, file_type))
}
```

---

## [6] Extracted Content Display

**E-UX + E-ML:** The extracted content is displayed as structured Markdown, rendered with interactive field highlighting. Each extractable field is wrapped in an editable component. Fields are grouped by entity category.

### Rendering Strategy

The structured Markdown from L1-03 is NOT rendered as raw Markdown. Instead, it is parsed into the `ExtractedField` list and rendered as a series of interactive, categorized field groups:

```
## Medications                                    [blue header]
┌─────────────────────────────────────────────────────────────┐
│ Metformin (Glucophage)                                      │
│ Dose: [500mg]  Frequency: [twice daily]  Route: [oral]     │
│ Reason: [Type 2 diabetes]                                   │
│ Instructions: [Take with food]                              │
│ Confidence: 0.92 ✓                                          │
└─────────────────────────────────────────────────────────────┘
┌─────────────────────────────────────────────────────────────┐
│ Atorvastatin                                                │
│ Dose: [20mg]  Frequency: [once daily]  Route: [oral]       │
│ Instructions: [Take at bedtime]                             │
│ Confidence: 0.55 ⚠                                          │
│ ┌─────────────────────────────────────────────────────────┐ │
│ │ ⚠ I'm not sure I read this correctly -- please check   │ │
│ └─────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘

## Lab Results                                    [green header]
┌─────────────────────────────────────────────────────────────┐
│ HbA1c                                                       │
│ Value: [7.2] [%]  Range: [4.0 - 6.0]  Flag: [high]        │
│ Confidence: 0.88 ✓                                          │
└─────────────────────────────────────────────────────────────┘
```

### Field Color Coding

| Entity Category | Header Color | Field Background |
|----------------|-------------|-----------------|
| Medication | `bg-blue-50 text-blue-800` | `border-blue-200` |
| LabResult | `bg-green-50 text-green-800` | `border-green-200` |
| Diagnosis | `bg-indigo-50 text-indigo-800` | `border-indigo-200` |
| Allergy | `bg-red-50 text-red-800` | `border-red-200` |
| Procedure | `bg-teal-50 text-teal-800` | `border-teal-200` |
| Referral | `bg-violet-50 text-violet-800` | `border-violet-200` |
| Professional | `bg-purple-50 text-purple-800` | `border-purple-200` |
| Date | `bg-amber-50 text-amber-800` | `border-amber-200` |
| Instruction | `bg-stone-50 text-stone-800` | `border-stone-200` |

### Field Display Rules

| Confidence Range | Visual Treatment |
|-----------------|-----------------|
| >= 0.90 | Green check icon, no additional styling |
| 0.70 - 0.89 | No icon, standard styling |
| 0.50 - 0.69 | Amber warning icon, amber border, "I'm not sure" message |
| < 0.50 | Red warning icon, red border, "I'm not sure" message, field pre-selected for editing |

---

## [7] Confidence Flagging

**E-UX critical design:** Confidence flags are the primary mechanism for drawing Marie's attention to potentially incorrect extractions. They must be visible but not alarming. The tone is humble and honest -- "I might have gotten this wrong" rather than "ERROR: LOW CONFIDENCE."

### Confidence Thresholds (from Tech Spec 6.2)

| Source Quality | Base Confidence |
|---------------|----------------|
| Digital PDF (extractable text) | 0.95 |
| Clean printed document (OCR) | 0.80 - 0.90 |
| Poor quality photo | 0.50 - 0.70 |
| Handwritten document | 0.30 - 0.60 |

### Flag Display

Fields below 0.70 confidence display a flag component:

```
┌─────────────────────────────────────────────────────────────┐
│ ⚠ I'm not sure I read this correctly -- please check:      │
│                                                             │
│   Dose: [ 500mg ]  ← click to correct                      │
│                                                             │
│ This field was extracted from a low-quality image.          │
│ The original might say something different.                 │
└─────────────────────────────────────────────────────────────┘
```

### Flag Rules

- Flags are sorted to the top of each entity group (most uncertain first)
- Each flag includes the specific field name and current value
- The flag message is always: "I'm not sure I read this correctly -- please check"
- An optional sub-message explains why: "This field was extracted from a low-quality image" or "Handwritten text is harder to read accurately"
- Flagged fields have their edit button pre-highlighted (pulsing amber border)
- The confidence summary bar at the bottom aggregates: "{N} fields extracted, {M} confident, {K} need checking"

### Plausibility Warnings (from L2-03 Coherence Engine)

Separate from confidence flags, plausibility warnings come from the coherence engine and flag medically unusual values:

```
┌─────────────────────────────────────────────────────────────┐
│ ⚠ This dose seems unusually high for Metformin.            │
│   Typical range: 500mg - 2000mg daily.                      │
│   Please verify with your original document.                │
└─────────────────────────────────────────────────────────────┘
```

Plausibility warnings are displayed inline below the relevant field. They use amber background for Info/Warning severity and soft red for Critical severity.

---

## [8] Correction Interface

**E-UX:** Marie must be able to correct any extracted field with minimal friction. Click on a field value to enter edit mode. Press Enter or click away to save. Press Escape to cancel. The correction is tracked for profile trust metrics.

### Edit Flow

1. Marie clicks on a field value (e.g., "500mg")
2. The field transforms into an input with the current value selected
3. She types the correction (e.g., "250mg")
4. She presses Enter or clicks outside to save
5. The field shows the new value with a "corrected" indicator (blue pencil icon)
6. The correction is stored locally in component state until Confirm

### Field Editor Behavior

| Action | Result |
|--------|--------|
| Click on field value | Enter edit mode, select all text |
| Type | Replace selected text |
| Enter | Save correction, exit edit mode |
| Escape | Cancel, revert to previous value |
| Tab | Save and move to next field |
| Click outside | Save correction, exit edit mode |

### Correction Tracking

```rust
/// Tracks corrections made during a review session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewSession {
    pub document_id: Uuid,
    pub corrections: Vec<FieldCorrection>,
    pub started_at: chrono::NaiveDateTime,
}

impl ReviewSession {
    pub fn has_corrections(&self) -> bool {
        !self.corrections.is_empty()
    }

    pub fn correction_count(&self) -> usize {
        self.corrections.len()
    }

    pub fn add_correction(&mut self, correction: FieldCorrection) {
        // Remove existing correction for same field (allow re-edits)
        self.corrections.retain(|c| c.field_id != correction.field_id);
        self.corrections.push(correction);
    }
}
```

### Corrected Field Display

After a correction:
- The field value shows the new text
- A small blue pencil icon appears next to the value
- Hovering the icon shows a tooltip: "Original: {old_value}"
- The field border changes to blue (corrected indicator)

---

## [9] Confirm / Reject Flow

### Confirm Flow

**Trigger:** Patient taps "Confirm" button after reviewing all fields.

```
Patient taps [Confirm]
  │
  ├─ Are there flagged fields still unchecked?
  │    YES → Show gentle reminder:
  │          "There are {N} fields I wasn't sure about.
  │           Would you like to check them first?"
  │          [Check flagged fields] / [Confirm anyway]
  │    NO  → Continue
  │
  ├─ Are there corrections?
  │    YES → ReviewOutcome = Corrected
  │    NO  → ReviewOutcome = Confirmed
  │
  ├─ Apply corrections to ExtractedEntities
  │    → Merge FieldCorrection values into StructuringResult
  │
  ├─ Call L1-04 StoragePipeline.store()
  │    → Entities written to SQLite (encrypted)
  │    → Markdown chunked, embedded, stored in LanceDB
  │    → Document record updated (review_status, markdown_file)
  │
  ├─ Update document review_status
  │    → 'confirmed' or 'corrected'
  │
  ├─ Update profile_trust metrics
  │    → Increment verified_count
  │    → If corrections: increment corrected_count
  │    → Recalculate extraction_accuracy
  │
  ├─ Trigger coherence engine (L2-03) async
  │    → Detect conflicts with existing data
  │    → Generate plausibility observations
  │
  ├─ Emit Tauri event: 'document-reviewed'
  │    → Home feed refreshes
  │
  └─ Navigate to success screen
       "Your document has been saved.
        {N} medications, {M} lab results added to your profile."
       [View document] / [Back to home]
```

### Reject Flow

**Trigger:** Patient taps "Reject" button.

```
Patient taps [Reject]
  │
  ├─ Show confirmation dialog:
  │    "Would you like to try again or remove this document?"
  │    [Try again] — Re-run the extraction pipeline (L1-02 → L1-03)
  │    [Remove document] — Delete the imported file entirely
  │    [Cancel] — Return to review screen
  │
  ├─ Optional: Capture rejection reason
  │    "What went wrong?" (optional text field)
  │    → Stored for improving extraction quality
  │
  ├─ If "Try again":
  │    → Document status set to 'pending_reprocess'
  │    → Re-run L1-02 OCR + L1-03 structuring
  │    → Return to review screen with new extraction
  │
  ├─ If "Remove document":
  │    → Document status set to 'rejected'
  │    → Original file kept (not deleted — patient may want it later)
  │    → Navigate back to home
  │
  └─ Emit Tauri event: 'document-reviewed'
```

### Profile Trust Metrics Update

```rust
/// Update profile trust after review
pub fn update_trust_after_review(
    outcome: &ReviewOutcome,
    correction_count: usize,
    total_fields: usize,
    repos: &RepositorySet,
) -> Result<(), ReviewError> {
    let mut trust = repos.profile_trust.get_or_create()?;

    trust.total_reviews += 1;

    match outcome {
        ReviewOutcome::Confirmed => {
            trust.verified_count += 1;
        }
        ReviewOutcome::Corrected => {
            trust.verified_count += 1;
            trust.corrected_count += 1;
        }
    }

    // Recalculate extraction accuracy
    // accuracy = (total_fields - corrections) / total_fields across all reviews
    trust.total_fields_extracted += total_fields as u64;
    trust.total_fields_corrected += correction_count as u64;
    if trust.total_fields_extracted > 0 {
        trust.extraction_accuracy = Some(
            1.0 - (trust.total_fields_corrected as f64 / trust.total_fields_extracted as f64)
        );
    }

    repos.profile_trust.update(&trust)?;
    Ok(())
}
```

---

## [10] Tauri Commands (IPC)

```rust
// src-tauri/src/commands/review.rs

use tauri::{AppHandle, Emitter, State};

/// Fetch all data needed for the review screen
#[tauri::command]
pub async fn get_review_data(
    state: State<'_, AppState>,
    document_id: String,
) -> Result<ReviewData, String> {
    let session_guard = state.active_session.lock()
        .map_err(|e| format!("Lock error: {e}"))?;
    let session = session_guard.as_ref()
        .ok_or("No active profile session")?;

    let doc_id = Uuid::parse_str(&document_id)
        .map_err(|e| format!("Invalid document ID: {e}"))?;

    let conn = session.db_connection()
        .map_err(|e| format!("DB connection failed: {e}"))?;

    // Fetch document record
    let doc = fetch_document(&conn, &doc_id)?;

    // Decrypt and load the structured Markdown
    let markdown = load_structured_markdown(&doc, session)?;

    // Flatten entities into ExtractedField list with confidence
    let extracted_fields = flatten_entities_to_fields(
        &doc_id, &conn, session,
    )?;

    // Run plausibility checks (quick, synchronous)
    let plausibility_warnings = check_plausibility(
        &extracted_fields, &conn,
    )?;

    // Determine original file type
    let original_file_type = if doc.source_filename.ends_with(".pdf") {
        OriginalFileType::Pdf
    } else {
        OriginalFileType::Image
    };

    state.update_activity();

    Ok(ReviewData {
        document_id: doc_id,
        original_file_path: doc.staged_path.clone(),
        original_file_type,
        document_type: doc.doc_type.as_str().to_string(),
        document_date: doc.document_date,
        professional_name: doc.professional_name.clone(),
        professional_specialty: doc.professional_specialty.clone(),
        structured_markdown: markdown,
        extracted_fields,
        plausibility_warnings,
        overall_confidence: doc.ocr_confidence.unwrap_or(0.0),
    })
}

/// Decrypt the original file and return as base64 for frontend rendering
#[tauri::command]
pub async fn get_original_file(
    state: State<'_, AppState>,
    document_id: String,
) -> Result<String, String> {
    let session_guard = state.active_session.lock()
        .map_err(|e| format!("Lock error: {e}"))?;
    let session = session_guard.as_ref()
        .ok_or("No active profile session")?;

    let doc_id = Uuid::parse_str(&document_id)
        .map_err(|e| format!("Invalid document ID: {e}"))?;

    let conn = session.db_connection()
        .map_err(|e| format!("DB connection failed: {e}"))?;

    let doc = fetch_document(&conn, &doc_id)?;

    let (decrypted_bytes, _file_type) = decrypt_original_for_review(
        &doc_id, session, &conn,
    )?;

    // Return as base64 for frontend rendering
    use base64::Engine;
    let encoded = base64::engine::general_purpose::STANDARD.encode(&decrypted_bytes);

    state.update_activity();

    Ok(encoded)
}

/// Update a single extracted field value (correction)
#[tauri::command]
pub async fn update_extracted_field(
    state: State<'_, AppState>,
    document_id: String,
    field_id: String,
    new_value: String,
) -> Result<(), String> {
    // Validate inputs
    let _doc_id = Uuid::parse_str(&document_id)
        .map_err(|e| format!("Invalid document ID: {e}"))?;
    let _field_id = Uuid::parse_str(&field_id)
        .map_err(|e| format!("Invalid field ID: {e}"))?;

    // Field corrections are tracked in frontend state and applied on confirm.
    // This command validates the new value on the backend (sanitization, length).
    if new_value.len() > 500 {
        return Err("Field value too long (max 500 characters)".into());
    }

    // Sanitize: no control characters
    if new_value.chars().any(|c| c.is_control() && c != '\n') {
        return Err("Field value contains invalid characters".into());
    }

    state.update_activity();

    Ok(())
}

/// Confirm the review — triggers storage pipeline
#[tauri::command]
pub async fn confirm_review(
    app: AppHandle,
    state: State<'_, AppState>,
    pipeline: State<'_, FullDocumentPipeline>,
    document_id: String,
    corrections: Vec<FieldCorrection>,
) -> Result<ReviewConfirmResult, String> {
    let session_guard = state.active_session.lock()
        .map_err(|e| format!("Lock error: {e}"))?;
    let session = session_guard.as_ref()
        .ok_or("No active profile session")?;

    let doc_id = Uuid::parse_str(&document_id)
        .map_err(|e| format!("Invalid document ID: {e}"))?;

    let conn = session.db_connection()
        .map_err(|e| format!("DB connection failed: {e}"))?;

    // Step 1: Load the structuring result
    let mut structuring_result = load_structuring_result(&conn, &doc_id, session)?;

    // Step 2: Apply corrections to the extracted entities
    let corrections_applied = apply_corrections(
        &mut structuring_result.extracted_entities,
        &corrections,
    )?;

    // Step 3: Run the storage pipeline (L1-04)
    let storage_result = pipeline.storage.store(&structuring_result, session)
        .map_err(|e| format!("Storage pipeline failed: {e}"))?;

    // Step 4: Update document review status
    let outcome = if corrections_applied > 0 {
        ReviewOutcome::Corrected
    } else {
        ReviewOutcome::Confirmed
    };

    update_document_review_status(
        &conn, &doc_id, &outcome,
    )?;

    // Step 5: Update profile trust metrics
    let total_fields = count_extracted_fields(&structuring_result.extracted_entities);
    update_trust_after_review(
        &outcome,
        corrections_applied,
        total_fields,
        &pipeline.repos,
    ).map_err(|e| format!("Trust update failed: {e}"))?;

    // Step 6: Emit event for home feed refresh
    let _ = app.emit("document-reviewed", doc_id.to_string());

    state.update_activity();

    Ok(ReviewConfirmResult {
        document_id: doc_id,
        status: outcome,
        entities_stored: storage_result.entities_stored,
        corrections_applied,
        chunks_stored: storage_result.chunks_stored,
    })
}

/// Reject the review
#[tauri::command]
pub async fn reject_review(
    app: AppHandle,
    state: State<'_, AppState>,
    document_id: String,
    reason: Option<String>,
    action: String, // "retry" or "remove"
) -> Result<ReviewRejectResult, String> {
    let session_guard = state.active_session.lock()
        .map_err(|e| format!("Lock error: {e}"))?;
    let session = session_guard.as_ref()
        .ok_or("No active profile session")?;

    let doc_id = Uuid::parse_str(&document_id)
        .map_err(|e| format!("Invalid document ID: {e}"))?;

    let conn = session.db_connection()
        .map_err(|e| format!("DB connection failed: {e}"))?;

    match action.as_str() {
        "retry" => {
            // Set document to pending_reprocess
            conn.execute(
                "UPDATE documents SET review_status = 'pending_reprocess' WHERE id = ?1",
                params![doc_id.to_string()],
            ).map_err(|e| format!("Status update failed: {e}"))?;

            // Log rejection reason
            if let Some(ref r) = reason {
                tracing::info!(
                    document_id = %doc_id,
                    action = "retry",
                    "Review rejected: reason captured"
                );
                // Store reason in document metadata (never log the content)
            }
        }
        "remove" => {
            conn.execute(
                "UPDATE documents SET review_status = 'rejected' WHERE id = ?1",
                params![doc_id.to_string()],
            ).map_err(|e| format!("Status update failed: {e}"))?;
        }
        _ => {
            return Err(format!("Invalid action: {}. Expected 'retry' or 'remove'.", action));
        }
    }

    let _ = app.emit("document-reviewed", doc_id.to_string());

    state.update_activity();

    Ok(ReviewRejectResult {
        document_id: doc_id,
        reason,
    })
}

// ─── Internal helpers ────────────────────────────────────────────────

/// Flatten extracted entities into a flat list of fields for the review UI
fn flatten_entities_to_fields(
    document_id: &Uuid,
    conn: &rusqlite::Connection,
    session: &ProfileSession,
) -> Result<Vec<ExtractedField>, String> {
    let mut fields = Vec::new();

    // Load the StructuringResult from the temporary storage
    // (stored after L1-03, before review confirmation)
    let structuring = load_pending_structuring(conn, document_id, session)?;

    // Flatten medications
    for (i, med) in structuring.extracted_entities.medications.iter().enumerate() {
        if let Some(ref name) = med.generic_name {
            fields.push(ExtractedField {
                id: Uuid::new_v4(),
                entity_type: EntityCategory::Medication,
                entity_index: i,
                field_name: "generic_name".into(),
                display_label: "Medication name".into(),
                value: name.clone(),
                confidence: med.confidence,
                is_flagged: med.confidence < 0.70,
                source_hint: None,
            });
        }
        fields.push(ExtractedField {
            id: Uuid::new_v4(),
            entity_type: EntityCategory::Medication,
            entity_index: i,
            field_name: "dose".into(),
            display_label: "Dose".into(),
            value: med.dose.clone(),
            confidence: med.confidence,
            is_flagged: med.confidence < 0.70,
            source_hint: None,
        });
        fields.push(ExtractedField {
            id: Uuid::new_v4(),
            entity_type: EntityCategory::Medication,
            entity_index: i,
            field_name: "frequency".into(),
            display_label: "Frequency".into(),
            value: med.frequency.clone(),
            confidence: med.confidence,
            is_flagged: med.confidence < 0.70,
            source_hint: None,
        });
    }

    // Flatten lab results
    for (i, lab) in structuring.extracted_entities.lab_results.iter().enumerate() {
        fields.push(ExtractedField {
            id: Uuid::new_v4(),
            entity_type: EntityCategory::LabResult,
            entity_index: i,
            field_name: "test_name".into(),
            display_label: "Test name".into(),
            value: lab.test_name.clone(),
            confidence: lab.confidence,
            is_flagged: lab.confidence < 0.70,
            source_hint: None,
        });
        if let Some(val) = lab.value {
            fields.push(ExtractedField {
                id: Uuid::new_v4(),
                entity_type: EntityCategory::LabResult,
                entity_index: i,
                field_name: "value".into(),
                display_label: "Value".into(),
                value: val.to_string(),
                confidence: lab.confidence,
                is_flagged: lab.confidence < 0.70,
                source_hint: None,
            });
        }
        if let Some(ref unit) = lab.unit {
            fields.push(ExtractedField {
                id: Uuid::new_v4(),
                entity_type: EntityCategory::LabResult,
                entity_index: i,
                field_name: "unit".into(),
                display_label: "Unit".into(),
                value: unit.clone(),
                confidence: lab.confidence,
                is_flagged: lab.confidence < 0.70,
                source_hint: None,
            });
        }
    }

    // Flatten diagnoses
    for (i, diag) in structuring.extracted_entities.diagnoses.iter().enumerate() {
        fields.push(ExtractedField {
            id: Uuid::new_v4(),
            entity_type: EntityCategory::Diagnosis,
            entity_index: i,
            field_name: "name".into(),
            display_label: "Diagnosis".into(),
            value: diag.name.clone(),
            confidence: diag.confidence,
            is_flagged: diag.confidence < 0.70,
            source_hint: None,
        });
    }

    // Flatten allergies
    for (i, allergy) in structuring.extracted_entities.allergies.iter().enumerate() {
        fields.push(ExtractedField {
            id: Uuid::new_v4(),
            entity_type: EntityCategory::Allergy,
            entity_index: i,
            field_name: "allergen".into(),
            display_label: "Allergen".into(),
            value: allergy.allergen.clone(),
            confidence: allergy.confidence,
            is_flagged: allergy.confidence < 0.70,
            source_hint: None,
        });
    }

    // Flatten professional
    if let Some(ref prof) = structuring.professional {
        fields.push(ExtractedField {
            id: Uuid::new_v4(),
            entity_type: EntityCategory::Professional,
            entity_index: 0,
            field_name: "name".into(),
            display_label: "Professional".into(),
            value: prof.name.clone(),
            confidence: structuring.structuring_confidence,
            is_flagged: structuring.structuring_confidence < 0.70,
            source_hint: None,
        });
    }

    // Flatten document date
    if let Some(date) = structuring.document_date {
        fields.push(ExtractedField {
            id: Uuid::new_v4(),
            entity_type: EntityCategory::Date,
            entity_index: 0,
            field_name: "document_date".into(),
            display_label: "Document date".into(),
            value: date.to_string(),
            confidence: structuring.structuring_confidence,
            is_flagged: structuring.structuring_confidence < 0.70,
            source_hint: None,
        });
    }

    Ok(fields)
}

/// Apply field corrections to the structuring result before storage
fn apply_corrections(
    entities: &mut ExtractedEntities,
    corrections: &[FieldCorrection],
) -> Result<usize, String> {
    let mut applied = 0;

    for correction in corrections {
        // Match correction to entity by field_id mapping
        // (Each correction targets a specific entity_type + entity_index + field_name)
        // Implementation applies the corrected_value to the corresponding field
        applied += 1;
    }

    Ok(applied)
}

/// Count total extractable fields for accuracy calculation
fn count_extracted_fields(entities: &ExtractedEntities) -> usize {
    let mut count = 0;
    // Each medication contributes: name + dose + frequency + route (4 fields)
    count += entities.medications.len() * 4;
    // Each lab result: test_name + value + unit + range (4 fields)
    count += entities.lab_results.len() * 4;
    // Each diagnosis: name (1 field)
    count += entities.diagnoses.len();
    // Each allergy: allergen + severity (2 fields)
    count += entities.allergies.len() * 2;
    // Each procedure: name + date (2 fields)
    count += entities.procedures.len() * 2;
    // Each referral: referred_to + reason (2 fields)
    count += entities.referrals.len() * 2;
    count
}
```

---

## [11] Svelte Components

### ReviewScreen (Main Container)

```svelte
<!-- src/lib/components/review/ReviewScreen.svelte -->
<script lang="ts">
  import { onMount } from 'svelte';
  import { getReviewData, getOriginalFile } from '$lib/api/review';
  import type { ReviewData, FieldCorrection, ExtractedField } from '$lib/types/review';
  import OriginalViewer from './OriginalViewer.svelte';
  import ExtractedView from './ExtractedView.svelte';
  import ConfidenceSummary from './ConfidenceSummary.svelte';
  import ReviewActions from './ReviewActions.svelte';
  import ReviewSuccess from './ReviewSuccess.svelte';

  interface Props {
    documentId: string;
    onBack: () => void;
    onNavigate: (screen: string, params?: Record<string, string>) => void;
  }
  let { documentId, onBack, onNavigate }: Props = $props();

  let reviewData: ReviewData | null = $state(null);
  let originalFileBase64: string | null = $state(null);
  let loading = $state(true);
  let error: string | null = $state(null);
  let corrections: FieldCorrection[] = $state([]);
  let showSuccess = $state(false);
  let confirmResult = $state<{ status: string; entities: Record<string, number> } | null>(null);

  // Responsive layout
  let windowWidth = $state(1024);
  let activeTab = $state<'original' | 'extracted'>('extracted');

  let isNarrow = $derived(windowWidth < 768);

  // Confidence summary
  let totalFields = $derived(reviewData?.extracted_fields.length ?? 0);
  let flaggedFields = $derived(
    reviewData?.extracted_fields.filter(f => f.is_flagged).length ?? 0
  );
  let confidentFields = $derived(totalFields - flaggedFields);

  async function loadReviewData() {
    try {
      loading = true;
      error = null;
      reviewData = await getReviewData(documentId);
      originalFileBase64 = await getOriginalFile(documentId);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  function handleFieldCorrection(correction: FieldCorrection) {
    // Remove existing correction for same field, then add new one
    corrections = corrections.filter(c => c.field_id !== correction.field_id);
    corrections = [...corrections, correction];
  }

  function handleConfirmSuccess(result: { status: string; entities: Record<string, number> }) {
    confirmResult = result;
    showSuccess = true;
  }

  onMount(() => {
    loadReviewData();

    function handleResize() {
      windowWidth = window.innerWidth;
    }
    window.addEventListener('resize', handleResize);
    handleResize();
    return () => window.removeEventListener('resize', handleResize);
  });
</script>

{#if showSuccess && confirmResult}
  <ReviewSuccess
    documentType={reviewData?.document_type ?? 'Document'}
    status={confirmResult.status}
    entities={confirmResult.entities}
    correctionsApplied={corrections.length}
    onViewDocument={() => onNavigate('document-detail', { documentId })}
    onBackToHome={() => onNavigate('home')}
  />
{:else}
  <div class="flex flex-col h-screen bg-stone-50">
    <!-- Header -->
    <header class="flex items-center gap-3 px-4 py-3 bg-white border-b border-stone-200 shrink-0">
      <button
        class="min-h-[44px] min-w-[44px] flex items-center justify-center
               text-stone-500 hover:text-stone-700"
        onclick={onBack}
        aria-label="Back to documents"
      >
        &larr;
      </button>
      <div class="flex-1 min-w-0">
        <h1 class="text-lg font-semibold text-stone-800 truncate">
          Review: {reviewData?.document_type ?? 'Document'}
        </h1>
        {#if reviewData?.professional_name}
          <p class="text-sm text-stone-500 truncate">
            {reviewData.professional_name}
            {#if reviewData.professional_specialty}
              &middot; {reviewData.professional_specialty}
            {/if}
            {#if reviewData.document_date}
              &middot; {reviewData.document_date}
            {/if}
          </p>
        {/if}
      </div>
    </header>

    {#if loading}
      <div class="flex items-center justify-center flex-1">
        <div class="flex flex-col items-center gap-3">
          <div class="animate-pulse text-stone-400">Loading document for review...</div>
        </div>
      </div>
    {:else if error}
      <div class="flex flex-col items-center justify-center flex-1 px-6 text-center">
        <p class="text-red-600 mb-4">Something went wrong: {error}</p>
        <button
          class="px-6 py-3 bg-stone-200 rounded-xl text-stone-700 min-h-[44px]"
          onclick={loadReviewData}
        >
          Try again
        </button>
      </div>
    {:else if reviewData}
      <!-- Tab switcher for narrow screens -->
      {#if isNarrow}
        <div class="flex bg-white border-b border-stone-200 shrink-0">
          <button
            class="flex-1 py-3 text-sm font-medium min-h-[44px]
                   {activeTab === 'original'
                     ? 'text-[var(--color-primary)] border-b-2 border-[var(--color-primary)]'
                     : 'text-stone-500'}"
            onclick={() => activeTab = 'original'}
          >
            Original
          </button>
          <button
            class="flex-1 py-3 text-sm font-medium min-h-[44px]
                   {activeTab === 'extracted'
                     ? 'text-[var(--color-primary)] border-b-2 border-[var(--color-primary)]'
                     : 'text-stone-500'}"
            onclick={() => activeTab = 'extracted'}
          >
            Extracted ({corrections.length > 0 ? `${corrections.length} corrected` : 'review'})
          </button>
        </div>
      {/if}

      <!-- Side-by-side / tabbed content -->
      <div class="flex-1 overflow-hidden {isNarrow ? '' : 'flex'}">
        <!-- Original pane -->
        {#if !isNarrow || activeTab === 'original'}
          <div class="{isNarrow ? 'h-full' : 'w-[45%] min-w-[300px]'} border-r border-stone-200 overflow-auto">
            <OriginalViewer
              fileBase64={originalFileBase64}
              fileType={reviewData.original_file_type}
            />
          </div>
        {/if}

        <!-- Extracted pane -->
        {#if !isNarrow || activeTab === 'extracted'}
          <div class="{isNarrow ? 'h-full' : 'flex-1 min-w-[300px]'} overflow-auto pb-40">
            <ExtractedView
              fields={reviewData.extracted_fields}
              warnings={reviewData.plausibility_warnings}
              {corrections}
              onCorrection={handleFieldCorrection}
            />
          </div>
        {/if}
      </div>

      <!-- Confidence summary bar -->
      <ConfidenceSummary
        {totalFields}
        {confidentFields}
        {flaggedFields}
        overallConfidence={reviewData.overall_confidence}
      />

      <!-- Action bar -->
      <ReviewActions
        {documentId}
        {corrections}
        {flaggedFields}
        onConfirmSuccess={handleConfirmSuccess}
        onReject={onBack}
      />
    {/if}
  </div>
{/if}
```

### OriginalViewer

```svelte
<!-- src/lib/components/review/OriginalViewer.svelte -->
<script lang="ts">
  interface Props {
    fileBase64: string | null;
    fileType: 'Image' | 'Pdf';
  }
  let { fileBase64, fileType }: Props = $props();

  let zoom = $state(1.0);
  let panX = $state(0);
  let panY = $state(0);
  let isDragging = $state(false);
  let dragStartX = $state(0);
  let dragStartY = $state(0);
  let rotation = $state(0);
  let currentPage = $state(1);
  let totalPages = $state(1);

  function zoomIn() {
    zoom = Math.min(zoom + 0.25, 5.0);
  }

  function zoomOut() {
    zoom = Math.max(zoom - 0.25, 0.25);
  }

  function fitToWidth() {
    zoom = 1.0;
    panX = 0;
    panY = 0;
  }

  function rotate() {
    rotation = (rotation + 90) % 360;
  }

  function handleWheel(e: WheelEvent) {
    e.preventDefault();
    if (e.deltaY < 0) zoomIn();
    else zoomOut();
  }

  function handleMouseDown(e: MouseEvent) {
    if (zoom > 1.0) {
      isDragging = true;
      dragStartX = e.clientX - panX;
      dragStartY = e.clientY - panY;
    }
  }

  function handleMouseMove(e: MouseEvent) {
    if (isDragging) {
      panX = e.clientX - dragStartX;
      panY = e.clientY - dragStartY;
    }
  }

  function handleMouseUp() {
    isDragging = false;
  }

  let mimePrefix = $derived(
    fileType === 'Pdf' ? 'data:application/pdf;base64,' : 'data:image/jpeg;base64,'
  );

  let dataUrl = $derived(fileBase64 ? `${mimePrefix}${fileBase64}` : null);
</script>

<div class="flex flex-col h-full">
  <!-- Toolbar -->
  <div class="flex items-center gap-2 px-3 py-2 bg-stone-100 border-b border-stone-200 shrink-0">
    <button
      class="min-h-[44px] min-w-[44px] flex items-center justify-center
             rounded-lg hover:bg-stone-200 text-stone-600"
      onclick={zoomOut}
      aria-label="Zoom out"
    >
      &minus;
    </button>
    <span class="text-sm text-stone-500 w-12 text-center">{Math.round(zoom * 100)}%</span>
    <button
      class="min-h-[44px] min-w-[44px] flex items-center justify-center
             rounded-lg hover:bg-stone-200 text-stone-600"
      onclick={zoomIn}
      aria-label="Zoom in"
    >
      +
    </button>
    <button
      class="min-h-[44px] min-w-[44px] flex items-center justify-center
             rounded-lg hover:bg-stone-200 text-stone-600 text-xs"
      onclick={fitToWidth}
      aria-label="Fit to width"
    >
      Fit
    </button>
    <button
      class="min-h-[44px] min-w-[44px] flex items-center justify-center
             rounded-lg hover:bg-stone-200 text-stone-600 text-xs"
      onclick={rotate}
      aria-label="Rotate 90 degrees"
    >
      &#8635;
    </button>

    {#if fileType === 'Pdf'}
      <div class="ml-auto flex items-center gap-2">
        <button
          class="min-h-[44px] min-w-[44px] flex items-center justify-center
                 rounded-lg hover:bg-stone-200 text-stone-600 text-xs"
          onclick={() => currentPage = Math.max(1, currentPage - 1)}
          disabled={currentPage <= 1}
          aria-label="Previous page"
        >
          &lt;
        </button>
        <span class="text-sm text-stone-500">{currentPage} / {totalPages}</span>
        <button
          class="min-h-[44px] min-w-[44px] flex items-center justify-center
                 rounded-lg hover:bg-stone-200 text-stone-600 text-xs"
          onclick={() => currentPage = Math.min(totalPages, currentPage + 1)}
          disabled={currentPage >= totalPages}
          aria-label="Next page"
        >
          &gt;
        </button>
      </div>
    {/if}
  </div>

  <!-- Viewer area -->
  <div
    class="flex-1 overflow-hidden bg-stone-200 flex items-center justify-center
           {isDragging ? 'cursor-grabbing' : zoom > 1.0 ? 'cursor-grab' : 'cursor-default'}"
    onwheel={handleWheel}
    onmousedown={handleMouseDown}
    onmousemove={handleMouseMove}
    onmouseup={handleMouseUp}
    onmouseleave={handleMouseUp}
    role="img"
    aria-label="Original document viewer"
  >
    {#if !dataUrl}
      <p class="text-stone-400">Loading document...</p>
    {:else if fileType === 'Image'}
      <img
        src={dataUrl}
        alt="Original document"
        class="max-w-full max-h-full object-contain select-none"
        style="transform: scale({zoom}) rotate({rotation}deg) translate({panX / zoom}px, {panY / zoom}px);
               transform-origin: center center;
               transition: {isDragging ? 'none' : 'transform 0.15s ease'};"
        draggable="false"
      />
    {:else}
      <!-- PDF rendering via iframe -->
      <iframe
        src={dataUrl}
        title="Original PDF document"
        class="w-full h-full border-none"
        style="transform: scale({zoom}); transform-origin: top left;"
      ></iframe>
    {/if}
  </div>
</div>
```

### ExtractedView

```svelte
<!-- src/lib/components/review/ExtractedView.svelte -->
<script lang="ts">
  import type {
    ExtractedField,
    PlausibilityWarning,
    FieldCorrection,
    EntityCategory,
  } from '$lib/types/review';
  import ConfidenceFlag from './ConfidenceFlag.svelte';
  import FieldEditor from './FieldEditor.svelte';

  interface Props {
    fields: ExtractedField[];
    warnings: PlausibilityWarning[];
    corrections: FieldCorrection[];
    onCorrection: (correction: FieldCorrection) => void;
  }
  let { fields, warnings, corrections, onCorrection }: Props = $props();

  // Group fields by entity type
  type FieldGroup = {
    category: EntityCategory;
    label: string;
    fields: ExtractedField[];
    headerClass: string;
    borderClass: string;
  };

  const categoryConfig: Record<EntityCategory, { label: string; headerClass: string; borderClass: string }> = {
    Medication: { label: 'Medications', headerClass: 'bg-blue-50 text-blue-800', borderClass: 'border-blue-200' },
    LabResult: { label: 'Lab Results', headerClass: 'bg-green-50 text-green-800', borderClass: 'border-green-200' },
    Diagnosis: { label: 'Diagnoses', headerClass: 'bg-indigo-50 text-indigo-800', borderClass: 'border-indigo-200' },
    Allergy: { label: 'Allergies', headerClass: 'bg-red-50 text-red-800', borderClass: 'border-red-200' },
    Procedure: { label: 'Procedures', headerClass: 'bg-teal-50 text-teal-800', borderClass: 'border-teal-200' },
    Referral: { label: 'Referrals', headerClass: 'bg-violet-50 text-violet-800', borderClass: 'border-violet-200' },
    Professional: { label: 'Professional', headerClass: 'bg-purple-50 text-purple-800', borderClass: 'border-purple-200' },
    Date: { label: 'Date', headerClass: 'bg-amber-50 text-amber-800', borderClass: 'border-amber-200' },
    Instruction: { label: 'Instructions', headerClass: 'bg-stone-50 text-stone-800', borderClass: 'border-stone-200' },
  };

  let groupedFields = $derived(() => {
    const groups: FieldGroup[] = [];
    const categoryOrder: EntityCategory[] = [
      'Medication', 'LabResult', 'Diagnosis', 'Allergy',
      'Procedure', 'Referral', 'Professional', 'Date', 'Instruction',
    ];

    for (const category of categoryOrder) {
      const categoryFields = fields.filter(f => f.entity_type === category);
      if (categoryFields.length > 0) {
        const config = categoryConfig[category];
        // Sort flagged fields first
        categoryFields.sort((a, b) => {
          if (a.is_flagged && !b.is_flagged) return -1;
          if (!a.is_flagged && b.is_flagged) return 1;
          return a.entity_index - b.entity_index;
        });
        groups.push({
          category,
          label: config.label,
          fields: categoryFields,
          headerClass: config.headerClass,
          borderClass: config.borderClass,
        });
      }
    }
    return groups;
  });

  function getWarningsForField(fieldId: string): PlausibilityWarning[] {
    return warnings.filter(w => w.field_id === fieldId);
  }

  function getCorrectedValue(fieldId: string): string | null {
    const correction = corrections.find(c => c.field_id === fieldId);
    return correction?.corrected_value ?? null;
  }
</script>

<div class="flex flex-col gap-4 p-4">
  {#each groupedFields() as group}
    <section>
      <!-- Category header -->
      <h2 class="text-sm font-semibold px-3 py-2 rounded-t-lg {group.headerClass}">
        {group.label}
        <span class="font-normal opacity-70">
          ({group.fields.length} field{group.fields.length === 1 ? '' : 's'})
        </span>
      </h2>

      <!-- Fields in this category -->
      <div class="flex flex-col border border-t-0 rounded-b-lg {group.borderClass}
                  divide-y divide-stone-100">
        {#each group.fields as field (field.id)}
          {@const fieldWarnings = getWarningsForField(field.id)}
          {@const correctedValue = getCorrectedValue(field.id)}

          <div class="px-3 py-3">
            <!-- Field label and editor -->
            <div class="flex items-start gap-2">
              <span class="text-xs text-stone-500 min-w-[100px] mt-1 shrink-0">
                {field.display_label}
              </span>
              <div class="flex-1">
                <FieldEditor
                  {field}
                  {correctedValue}
                  onSave={(newValue) => {
                    onCorrection({
                      field_id: field.id,
                      original_value: field.value,
                      corrected_value: newValue,
                    });
                  }}
                />
              </div>
            </div>

            <!-- Confidence flag -->
            {#if field.is_flagged}
              <div class="mt-2">
                <ConfidenceFlag
                  confidence={field.confidence}
                  fieldLabel={field.display_label}
                />
              </div>
            {/if}

            <!-- Plausibility warnings -->
            {#each fieldWarnings as warning}
              <div class="mt-2 px-3 py-2 rounded-lg text-sm
                          {warning.severity === 'Critical'
                            ? 'bg-red-50 text-red-800 border border-red-200'
                            : 'bg-amber-50 text-amber-800 border border-amber-200'}">
                {warning.message}
              </div>
            {/each}
          </div>
        {/each}
      </div>
    </section>
  {/each}

  {#if fields.length === 0}
    <div class="text-center py-12 text-stone-400">
      <p>No fields were extracted from this document.</p>
      <p class="text-sm mt-2">The document may be too unclear to read, or it may not be a medical document.</p>
    </div>
  {/if}
</div>
```

### ConfidenceFlag

```svelte
<!-- src/lib/components/review/ConfidenceFlag.svelte -->
<script lang="ts">
  interface Props {
    confidence: number;
    fieldLabel: string;
  }
  let { confidence, fieldLabel }: Props = $props();

  let severityClass = $derived(
    confidence < 0.50
      ? 'bg-red-50 border-red-200 text-red-800'
      : 'bg-amber-50 border-amber-200 text-amber-800'
  );

  let explanationText = $derived(
    confidence < 0.50
      ? 'This field was extracted from very low-quality text. The original might say something quite different.'
      : 'This field was extracted from a low-quality image. The original might say something different.'
  );
</script>

<div class="rounded-lg border px-3 py-2 {severityClass}" role="alert">
  <p class="text-sm font-medium">
    I'm not sure I read this correctly -- please check
  </p>
  <p class="text-xs mt-1 opacity-80">
    {explanationText}
  </p>
  <p class="text-xs mt-1 opacity-60">
    Confidence: {Math.round(confidence * 100)}%
  </p>
</div>
```

### FieldEditor

```svelte
<!-- src/lib/components/review/FieldEditor.svelte -->
<script lang="ts">
  import type { ExtractedField } from '$lib/types/review';

  interface Props {
    field: ExtractedField;
    correctedValue: string | null;
    onSave: (newValue: string) => void;
  }
  let { field, correctedValue, onSave }: Props = $props();

  let editing = $state(false);
  let editValue = $state('');

  let displayValue = $derived(correctedValue ?? field.value);
  let isCorrected = $derived(correctedValue !== null);

  function startEdit() {
    editValue = displayValue;
    editing = true;
  }

  function saveEdit() {
    const trimmed = editValue.trim();
    if (trimmed && trimmed !== field.value) {
      onSave(trimmed);
    }
    editing = false;
  }

  function cancelEdit() {
    editing = false;
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') {
      e.preventDefault();
      saveEdit();
    } else if (e.key === 'Escape') {
      cancelEdit();
    } else if (e.key === 'Tab') {
      saveEdit();
      // Let Tab propagate naturally to next element
    }
  }
</script>

{#if editing}
  <input
    type="text"
    bind:value={editValue}
    onkeydown={handleKeydown}
    onblur={saveEdit}
    class="w-full px-2 py-1 text-sm border-2 border-[var(--color-primary)] rounded
           focus:outline-none min-h-[44px]"
    autofocus
  />
{:else}
  <button
    class="group flex items-center gap-1.5 text-left w-full px-2 py-1 rounded
           hover:bg-stone-50 transition-colors min-h-[44px]
           {isCorrected ? 'border border-blue-300 bg-blue-50' : ''}
           {field.is_flagged ? 'animate-pulse-subtle border border-amber-300' : ''}"
    onclick={startEdit}
    aria-label="Edit {field.display_label}: {displayValue}"
  >
    <span class="text-sm text-stone-800 {isCorrected ? 'font-medium text-blue-800' : ''}">
      {displayValue}
    </span>

    {#if isCorrected}
      <!-- Blue pencil icon for corrected fields -->
      <span
        class="text-blue-500 text-xs shrink-0"
        title="Original: {field.value}"
      >
        &#9998;
      </span>
    {:else}
      <!-- Subtle edit hint on hover -->
      <span class="text-stone-300 text-xs opacity-0 group-hover:opacity-100 transition-opacity shrink-0">
        &#9998;
      </span>
    {/if}

    {#if field.confidence >= 0.90}
      <span class="text-green-500 text-xs shrink-0" aria-label="High confidence">&#x2713;</span>
    {/if}
  </button>
{/if}
```

### ConfidenceSummary

```svelte
<!-- src/lib/components/review/ConfidenceSummary.svelte -->
<script lang="ts">
  interface Props {
    totalFields: number;
    confidentFields: number;
    flaggedFields: number;
    overallConfidence: number;
  }
  let { totalFields, confidentFields, flaggedFields, overallConfidence }: Props = $props();

  let summaryText = $derived(() => {
    if (totalFields === 0) return 'No fields extracted';
    if (flaggedFields === 0) return `${totalFields} fields extracted, all look good`;
    return `${totalFields} fields extracted \u00B7 ${confidentFields} confident \u00B7 ${flaggedFields} need checking`;
  });

  let barColor = $derived(
    flaggedFields === 0 ? 'bg-green-500' :
    flaggedFields <= 2 ? 'bg-amber-500' :
    'bg-red-500'
  );

  let fillPercent = $derived(
    totalFields > 0 ? Math.round((confidentFields / totalFields) * 100) : 0
  );
</script>

<div class="px-4 py-3 bg-white border-t border-stone-200 shrink-0">
  <div class="flex items-center gap-3">
    <div class="flex-1">
      <p class="text-sm text-stone-600">{summaryText()}</p>
      <div class="mt-1 h-1.5 bg-stone-100 rounded-full overflow-hidden">
        <div class="h-full rounded-full transition-all duration-500 {barColor}"
             style="width: {fillPercent}%"></div>
      </div>
    </div>
    <span class="text-xs text-stone-400">
      Overall: {Math.round(overallConfidence * 100)}%
    </span>
  </div>
</div>
```

### ReviewActions

```svelte
<!-- src/lib/components/review/ReviewActions.svelte -->
<script lang="ts">
  import { confirmReview, rejectReview } from '$lib/api/review';
  import type { FieldCorrection } from '$lib/types/review';

  interface Props {
    documentId: string;
    corrections: FieldCorrection[];
    flaggedFields: number;
    onConfirmSuccess: (result: { status: string; entities: Record<string, number> }) => void;
    onReject: () => void;
  }
  let { documentId, corrections, flaggedFields, onConfirmSuccess, onReject }: Props = $props();

  let confirming = $state(false);
  let rejecting = $state(false);
  let showFlaggedWarning = $state(false);
  let showRejectDialog = $state(false);
  let rejectReason = $state('');

  async function handleConfirm() {
    // If there are flagged fields, show a gentle reminder first
    if (flaggedFields > 0 && !showFlaggedWarning) {
      showFlaggedWarning = true;
      return;
    }

    confirming = true;
    try {
      const result = await confirmReview(documentId, corrections);
      onConfirmSuccess({
        status: result.status,
        entities: result.entities_stored,
      });
    } catch (e) {
      // Show error inline
      console.error('Confirm failed:', e);
      alert(e instanceof Error ? e.message : 'Something went wrong while saving.');
    } finally {
      confirming = false;
      showFlaggedWarning = false;
    }
  }

  async function handleReject(action: 'retry' | 'remove') {
    rejecting = true;
    try {
      await rejectReview(documentId, rejectReason || null, action);
      showRejectDialog = false;
      onReject();
    } catch (e) {
      console.error('Reject failed:', e);
      alert(e instanceof Error ? e.message : 'Something went wrong.');
    } finally {
      rejecting = false;
    }
  }
</script>

<!-- Flagged fields warning overlay -->
{#if showFlaggedWarning}
  <div class="fixed inset-0 bg-black/30 flex items-end justify-center z-50 p-4"
       role="dialog" aria-modal="true" aria-label="Flagged fields reminder">
    <div class="bg-white rounded-2xl p-6 max-w-md w-full shadow-xl">
      <h3 class="text-lg font-semibold text-stone-800 mb-2">
        Some fields need checking
      </h3>
      <p class="text-stone-600 text-sm mb-4">
        There {flaggedFields === 1 ? 'is' : 'are'} {flaggedFields} field{flaggedFields === 1 ? '' : 's'}
        I wasn't sure about. Would you like to check {flaggedFields === 1 ? 'it' : 'them'} first?
      </p>
      <div class="flex gap-3">
        <button
          class="flex-1 px-4 py-3 border border-stone-200 rounded-xl text-stone-700
                 hover:bg-stone-50 min-h-[44px]"
          onclick={() => showFlaggedWarning = false}
        >
          Check flagged fields
        </button>
        <button
          class="flex-1 px-4 py-3 bg-[var(--color-primary)] text-white rounded-xl
                 font-medium hover:brightness-110 min-h-[44px]"
          onclick={handleConfirm}
          disabled={confirming}
        >
          {confirming ? 'Saving...' : 'Confirm anyway'}
        </button>
      </div>
    </div>
  </div>
{/if}

<!-- Reject dialog -->
{#if showRejectDialog}
  <div class="fixed inset-0 bg-black/30 flex items-end justify-center z-50 p-4"
       role="dialog" aria-modal="true" aria-label="Reject document">
    <div class="bg-white rounded-2xl p-6 max-w-md w-full shadow-xl">
      <h3 class="text-lg font-semibold text-stone-800 mb-2">
        What would you like to do?
      </h3>

      <label class="flex flex-col gap-1 mb-4">
        <span class="text-stone-600 text-sm">What went wrong? (optional)</span>
        <textarea
          bind:value={rejectReason}
          placeholder="The text was too blurry, wrong document, etc."
          class="px-3 py-2 border border-stone-300 rounded-lg text-sm min-h-[80px]
                 focus:border-[var(--color-primary)] focus:outline-none"
        ></textarea>
      </label>

      <div class="flex flex-col gap-2">
        <button
          class="w-full px-4 py-3 bg-[var(--color-primary)] text-white rounded-xl
                 font-medium hover:brightness-110 min-h-[44px]"
          onclick={() => handleReject('retry')}
          disabled={rejecting}
        >
          {rejecting ? 'Processing...' : 'Try again (re-extract)'}
        </button>
        <button
          class="w-full px-4 py-3 border border-red-200 text-red-700 rounded-xl
                 hover:bg-red-50 min-h-[44px]"
          onclick={() => handleReject('remove')}
          disabled={rejecting}
        >
          Remove this document
        </button>
        <button
          class="w-full px-4 py-3 text-stone-500 min-h-[44px]"
          onclick={() => showRejectDialog = false}
        >
          Cancel
        </button>
      </div>
    </div>
  </div>
{/if}

<!-- Main action bar -->
<div class="flex items-center gap-3 px-4 py-3 bg-white border-t border-stone-200 shrink-0">
  <button
    class="px-6 py-3 border border-stone-300 rounded-xl text-stone-600
           hover:bg-stone-50 min-h-[44px]"
    onclick={() => showRejectDialog = true}
    disabled={confirming || rejecting}
  >
    Reject
  </button>
  <div class="flex-1"></div>
  {#if corrections.length > 0}
    <span class="text-xs text-blue-600">
      {corrections.length} correction{corrections.length === 1 ? '' : 's'}
    </span>
  {/if}
  <button
    class="px-8 py-3 bg-[var(--color-primary)] text-white rounded-xl text-base
           font-medium hover:brightness-110 disabled:opacity-50 min-h-[44px]
           focus-visible:outline focus-visible:outline-2
           focus-visible:outline-offset-2 focus-visible:outline-[var(--color-primary)]"
    onclick={handleConfirm}
    disabled={confirming || rejecting}
  >
    {confirming ? 'Saving...' : corrections.length > 0 ? 'Confirm with corrections' : 'Confirm'}
  </button>
</div>
```

### ReviewSuccess

```svelte
<!-- src/lib/components/review/ReviewSuccess.svelte -->
<script lang="ts">
  interface Props {
    documentType: string;
    status: string;
    entities: Record<string, number>;
    correctionsApplied: number;
    onViewDocument: () => void;
    onBackToHome: () => void;
  }
  let {
    documentType, status, entities, correctionsApplied,
    onViewDocument, onBackToHome,
  }: Props = $props();

  // Build summary text
  let entitySummary = $derived(() => {
    const parts: string[] = [];
    if (entities.medications > 0)
      parts.push(`${entities.medications} medication${entities.medications > 1 ? 's' : ''}`);
    if (entities.lab_results > 0)
      parts.push(`${entities.lab_results} lab result${entities.lab_results > 1 ? 's' : ''}`);
    if (entities.diagnoses > 0)
      parts.push(`${entities.diagnoses} diagnosis${entities.diagnoses > 1 ? 'es' : ''}`);
    if (entities.allergies > 0)
      parts.push(`${entities.allergies} allergy alert${entities.allergies > 1 ? 's' : ''}`);
    if (entities.procedures > 0)
      parts.push(`${entities.procedures} procedure${entities.procedures > 1 ? 's' : ''}`);
    if (entities.referrals > 0)
      parts.push(`${entities.referrals} referral${entities.referrals > 1 ? 's' : ''}`);
    return parts.join(', ');
  });
</script>

<div class="flex flex-col items-center justify-center min-h-screen px-8 gap-6 max-w-md mx-auto text-center">
  <!-- Success icon -->
  <div class="w-16 h-16 rounded-full bg-green-100 flex items-center justify-center">
    <span class="text-green-600 text-3xl">&#x2713;</span>
  </div>

  <h2 class="text-2xl font-bold text-stone-800">
    {documentType} saved
  </h2>

  <p class="text-stone-600">
    {entitySummary()} added to your profile.
  </p>

  {#if correctionsApplied > 0}
    <p class="text-sm text-blue-600">
      {correctionsApplied} correction{correctionsApplied === 1 ? '' : 's'} applied.
      Thank you for helping improve accuracy.
    </p>
  {/if}

  <div class="flex flex-col gap-3 w-full mt-4">
    <button
      class="w-full px-8 py-4 bg-[var(--color-primary)] text-white rounded-xl text-lg
             font-medium hover:brightness-110 min-h-[44px]"
      onclick={onBackToHome}
    >
      Back to home
    </button>
    <button
      class="w-full px-8 py-4 border border-stone-200 rounded-xl text-stone-700
             hover:bg-stone-50 min-h-[44px]"
      onclick={onViewDocument}
    >
      View document details
    </button>
  </div>
</div>
```

---

## [12] Frontend API

```typescript
// src/lib/api/review.ts
import { invoke } from '@tauri-apps/api/core';
import type {
  ReviewData,
  FieldCorrection,
  ReviewConfirmResult,
  ReviewRejectResult,
} from '$lib/types/review';

/** Fetch all data needed to render the review screen */
export async function getReviewData(documentId: string): Promise<ReviewData> {
  return invoke<ReviewData>('get_review_data', { documentId });
}

/** Fetch the decrypted original file as base64 */
export async function getOriginalFile(documentId: string): Promise<string> {
  return invoke<string>('get_original_file', { documentId });
}

/** Validate a field correction on the backend */
export async function updateExtractedField(
  documentId: string,
  fieldId: string,
  newValue: string,
): Promise<void> {
  return invoke('update_extracted_field', { documentId, fieldId, newValue });
}

/** Confirm the review with optional corrections */
export async function confirmReview(
  documentId: string,
  corrections: FieldCorrection[],
): Promise<ReviewConfirmResult> {
  return invoke<ReviewConfirmResult>('confirm_review', { documentId, corrections });
}

/** Reject the review */
export async function rejectReview(
  documentId: string,
  reason: string | null,
  action: string,
): Promise<ReviewRejectResult> {
  return invoke<ReviewRejectResult>('reject_review', { documentId, reason, action });
}
```

---

## [13] Error Handling

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ReviewError {
    #[error("Document not found: {0}")]
    DocumentNotFound(Uuid),

    #[error("File read error: {0}")]
    FileRead(String),

    #[error("Decryption failed: {0}")]
    Decryption(String),

    #[error("Structuring result not found for document: {0}")]
    StructuringNotFound(Uuid),

    #[error("Storage pipeline failed: {0}")]
    StorageFailed(String),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Invalid field correction: {0}")]
    InvalidCorrection(String),

    #[error("Profile trust update failed: {0}")]
    TrustUpdate(String),
}
```

**E-UX user-facing messages:**

| Error | User sees |
|-------|-----------|
| `DocumentNotFound` | "We couldn't find this document. It may have been removed." |
| `FileRead` | "The original document file couldn't be loaded. Please try importing it again." |
| `Decryption` | "There was a problem decrypting your document. Please try unlocking your profile again." |
| `StructuringNotFound` | "The extracted content for this document wasn't found. Please try re-importing." |
| `StorageFailed` | "Something went wrong while saving. Your document is safe -- please try confirming again." |
| `InvalidCorrection` | "That value doesn't look right. Please check and try again." |

All errors logged via `tracing::warn!` or `tracing::error!`. No sensitive medical data in error messages.

---

## [14] Security

| Concern | Mitigation |
|---------|-----------|
| Original file decryption | Decrypted only into memory (Vec<u8>). NEVER written to temp files. Base64 transfer to frontend via IPC (in-process, not network). |
| Structured Markdown in memory | Loaded from encrypted `.md.enc` file, decrypted in memory. Cleared when review screen unmounts. |
| Field corrections in transit | Corrections sent via Tauri IPC (in-process). Validated on backend (length limit, control char filter). |
| Medical data in frontend state | Review data cleared from Svelte state when navigating away. No localStorage or sessionStorage. |
| Injection via corrections | Backend validates: max 500 chars, no control characters, sanitized before storage. |
| Activity timeout during review | `state.update_activity()` called on every IPC command. If timeout triggers during review, ProfileGuard redirects to unlock. Unsaved corrections are lost (acceptable -- better than leaving profile unlocked). |
| Logging | NEVER log field values, corrections, or document content. Only log document_id, entity counts, and confidence scores. |

---

## [15] Testing

### Unit Tests (Rust)

| Test | What |
|------|------|
| `test_flatten_entities_medication_fields` | Medications produce name + dose + frequency fields |
| `test_flatten_entities_lab_result_fields` | Lab results produce test_name + value + unit fields |
| `test_flatten_entities_empty` | Empty entities produce empty field list |
| `test_confidence_flagging_threshold` | Fields with confidence < 0.70 have is_flagged = true |
| `test_confidence_flagging_above_threshold` | Fields with confidence >= 0.70 have is_flagged = false |
| `test_apply_corrections_single` | Single correction applied to matching entity field |
| `test_apply_corrections_multiple` | Multiple corrections applied correctly |
| `test_apply_corrections_empty` | No corrections returns 0 applied |
| `test_count_extracted_fields` | Correct total field count per entity type |
| `test_update_trust_confirmed` | verified_count incremented, corrected_count unchanged |
| `test_update_trust_corrected` | Both verified_count and corrected_count incremented |
| `test_extraction_accuracy_calculation` | Accuracy = (total - corrected) / total |
| `test_decrypt_original_image` | Returns decrypted bytes and OriginalFileType::Image |
| `test_decrypt_original_pdf` | Returns decrypted bytes and OriginalFileType::Pdf |
| `test_validate_field_length` | Values > 500 chars rejected |
| `test_validate_field_control_chars` | Control characters rejected |
| `test_review_status_confirmed` | Document status updated to 'confirmed' |
| `test_review_status_corrected` | Document status updated to 'corrected' |
| `test_reject_retry` | Document status set to 'pending_reprocess' |
| `test_reject_remove` | Document status set to 'rejected' |

### Frontend Tests

| Test | What |
|------|------|
| `test_review_screen_loads_data` | Review data fetched and displayed on mount |
| `test_review_screen_loading_state` | Loading indicator shown while fetching |
| `test_review_screen_error_state` | Error message and retry button shown on failure |
| `test_original_viewer_zoom_in` | Zoom increases on button click |
| `test_original_viewer_zoom_out` | Zoom decreases on button click |
| `test_original_viewer_fit_to_width` | Zoom resets to 1.0 |
| `test_original_viewer_rotation` | Rotation increments by 90 degrees |
| `test_extracted_view_groups_by_category` | Fields grouped by entity type |
| `test_extracted_view_flagged_first` | Flagged fields sorted before confident fields |
| `test_confidence_flag_below_050` | Red severity flag for confidence < 0.50 |
| `test_confidence_flag_050_to_070` | Amber severity flag for confidence 0.50-0.69 |
| `test_confidence_flag_message` | "I'm not sure I read this correctly" text present |
| `test_field_editor_click_to_edit` | Click transforms field into input |
| `test_field_editor_enter_saves` | Enter key saves correction |
| `test_field_editor_escape_cancels` | Escape key reverts to original value |
| `test_field_editor_corrected_indicator` | Corrected fields show pencil icon |
| `test_confirm_button_triggers_save` | Confirm calls confirmReview API |
| `test_confirm_flagged_warning` | Warning dialog shown if flagged fields exist |
| `test_confirm_anyway_bypasses_warning` | "Confirm anyway" proceeds despite flagged fields |
| `test_reject_dialog_options` | Reject shows retry and remove options |
| `test_success_screen_shows_entities` | Success screen lists stored entity counts |
| `test_responsive_tab_switcher` | Tab switcher shown on narrow screens |
| `test_responsive_side_by_side` | Side-by-side shown on wide screens |
| `test_confidence_summary_bar` | Summary text and progress bar rendered correctly |
| `test_corrections_count_displayed` | Correction count shown in action bar |

---

## [16] Performance

| Metric | Target |
|--------|--------|
| Review data load (get_review_data) | < 200ms |
| Original file decrypt + base64 encode | < 300ms for 10MB file |
| Field flattening (entities to field list) | < 10ms |
| Plausibility check (quick, synchronous) | < 50ms |
| Confirm flow (storage pipeline + trust update) | < 2 seconds |
| Reject flow (status update) | < 50ms |
| Image zoom/pan responsiveness | < 16ms per frame (60fps) |
| Field editor open/close | < 50ms |
| Screen initial render (after data load) | < 100ms |

**E-RS notes:**
- Original file decryption is the bottleneck for large files. Consider streaming decryption for files > 5MB.
- Storage pipeline on confirm runs synchronously. Show a progress indicator during the 1-2 second wait.
- Field corrections are stored in frontend state until confirm. No IPC round-trips during editing.
- `get_review_data` and `get_original_file` can be called in parallel on mount.

---

## [17] Open Questions

| # | Question | Status |
|---|---------|--------|
| OQ-01 | Should we support PDF page-level confidence (different confidence per page for multi-page scans)? | Deferred to Phase 2. Phase 1 uses a single confidence score per document. |
| OQ-02 | Should corrections be logged for ML model improvement (fine-tuning feedback loop)? | Yes in principle, but deferred to Phase 2. Phase 1 only stores correction counts in profile_trust. |
| OQ-03 | Should the original viewer support text selection (for digital PDFs) to help cross-reference? | Nice-to-have. Deferred to Phase 2. Phase 1 uses image/PDF rendering only. |
| OQ-04 | Should we allow partial confirmation (confirm some entities, reject others)? | Deferred to Phase 2. Phase 1 is all-or-nothing per document. |
| OQ-05 | Large documents (>10 pages) may produce hundreds of fields. Should we paginate the extracted view? | Yes for Phase 2. Phase 1 limits to scroll with section collapse. |
| OQ-06 | Should the image viewer support touch gestures (pinch-to-zoom, two-finger pan) for tablet users? | Yes for Phase 2. Phase 1 uses button controls only. |

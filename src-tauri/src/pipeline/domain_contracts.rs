//! L6-10: DomainContract — declarative field-to-prompt-to-DB mapping.
//!
//! Each medical extraction domain (lab_results, medications, etc.) is
//! described by a `DomainContract`: a const data structure that declares
//! every field the SLM should extract, how to prompt for it, what type
//! to expect, and which DB column it maps to.
//!
//! The contract is the SINGLE SOURCE OF TRUTH. Prompts are generated
//! from it, not hardcoded. Response validation is derived from field
//! types. DB dispatch is guided by column mappings.
//!
//! Evidence: 08-VISION-OCR-DEGENERATION-ANALYSIS Part V §36-46
//! Problem: Three drifting catalogs (drill_fields, ExtractedEntities, DB schema)
//! Solution: One contract that all three layers consume.

use std::fmt;

// ═══════════════════════════════════════════════════════════
// Core types
// ═══════════════════════════════════════════════════════════

/// Expected type of a field value. Used for validation after extraction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldType {
    /// Free-form text (name, description, etc.)
    Text,
    /// Numeric value (lab result value, dosage amount)
    Numeric,
    /// Date string (ISO 8601 or natural language)
    Date,
    /// Boolean flag (yes/no, true/false)
    Boolean,
    /// Constrained to a fixed set of values
    Enum(&'static [&'static str]),
}

impl fmt::Display for FieldType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Text => write!(f, "text"),
            Self::Numeric => write!(f, "numeric"),
            Self::Date => write!(f, "date"),
            Self::Boolean => write!(f, "boolean"),
            Self::Enum(values) => write!(f, "enum({})", values.join("|")),
        }
    }
}

/// Descriptor for a single extractable field within a domain.
///
/// Compile-time const. Zero runtime cost. Prompts are generated
/// from `prompt_label`, validation from `field_type`.
#[derive(Debug, Clone, Copy)]
pub struct FieldDescriptor {
    /// Internal field name (key in extracted field map).
    pub name: &'static str,
    /// Human-readable label used in prompts.
    /// E.g., "result value", "low end of the normal range".
    pub prompt_label: &'static str,
    /// Expected type for response validation.
    pub field_type: FieldType,
    /// Whether missing value is an extraction failure.
    pub required: bool,
    /// Target column in the database table (for dispatch guidance).
    pub db_column: &'static str,
}

/// Contract for a medical extraction domain.
///
/// Declares everything needed to extract, validate, and store
/// entities for one medical domain (e.g., lab_results, medications).
#[derive(Debug, Clone, Copy)]
pub struct DomainContract {
    /// Domain identifier matching DB table name.
    pub domain: &'static str,
    /// Singular item label (used in drill prompts). E.g., "test".
    pub item_label: &'static str,
    /// Plural item label (used in enumerate prompts). E.g., "tests".
    pub item_label_plural: &'static str,
    /// Enumerate hint for prompts. E.g., "lab test names".
    pub enumerate_hint: &'static str,
    /// All extractable fields for this domain.
    pub fields: &'static [FieldDescriptor],
}

// ═══════════════════════════════════════════════════════════
// Input mode — C4: text vs vision prompts
// ═══════════════════════════════════════════════════════════

/// How the source content is provided to the model.
///
/// Determines prompt generation strategy:
/// - `Text`: Document text embedded in prompt (IterativeDrill on extracted text)
/// - `Vision`: Image attached to prompt (IterativeDrill on raw image)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    /// Pre-extracted text (from OCR or plain text).
    Text,
    /// Image attached to prompt (vision model reads directly).
    Vision,
}

// ═══════════════════════════════════════════════════════════
// Prompt generation
// ═══════════════════════════════════════════════════════════

impl DomainContract {
    /// Generate the MarkdownList extraction instruction.
    ///
    /// Output: "List all lab test names mentioned in this document.
    ///          For each, state: result value, unit, ..."
    pub fn markdown_list_instruction(&self) -> String {
        let field_labels: Vec<&str> = self.fields.iter().map(|f| f.prompt_label).collect();
        format!(
            "List all {} mentioned in this document. \
             For each, state: {}.",
            self.enumerate_hint,
            field_labels.join(", "),
        )
    }

    /// Generate the IterativeDrill enumerate instruction.
    ///
    /// Output: "What lab test names are mentioned in this document?
    ///          List only the names, one per line."
    pub fn enumerate_instruction(&self) -> String {
        format!(
            "What {} are mentioned in this document? \
             List only the names, one per line.",
            self.enumerate_hint,
        )
    }

    /// Generate the IterativeDrill drill instruction for one field of one item.
    ///
    /// Output: "For the test 'Hemoglobin': what is the result value?
    ///          Answer with just the value, or 'not specified'."
    pub fn drill_instruction(&self, item_name: &str, field: &FieldDescriptor) -> String {
        let type_hint = match field.field_type {
            FieldType::Numeric => " Answer with a number.",
            FieldType::Date => " Answer with a date.",
            FieldType::Boolean => " Answer yes or no.",
            FieldType::Enum(values) => {
                // Hint will be appended below
                let _ = values;
                ""
            }
            FieldType::Text => "",
        };

        let enum_hint = if let FieldType::Enum(values) = field.field_type {
            format!(" Choose from: {}.", values.join(", "))
        } else {
            String::new()
        };

        format!(
            "For the {} '{}': what is the {}? \
             Answer with just the value, or 'not specified' if not in the document.{type_hint}{enum_hint}",
            self.item_label, item_name, field.prompt_label,
        )
    }

    // ── C4: Input-mode-aware prompt generation ────────────

    /// Generate enumerate prompt for the given input mode.
    ///
    /// - `Text`: Wraps document text in XML tags (existing behavior).
    /// - `Vision`: Image-oriented prompt — no document text needed.
    ///
    /// 09-CAE: `category_context` adds document type context to vision prompts
    /// (e.g., "This is a laboratory analysis report."). When empty, the category
    /// line is omitted. The NONE instruction is always included to get a
    /// deterministic empty-case response instead of chain-of-thought reasoning.
    pub fn enumerate_prompt_for(&self, mode: InputMode, document_text: &str, category_context: &str) -> String {
        match mode {
            InputMode::Text => self.build_enumerate_prompt(document_text),
            InputMode::Vision => {
                let category_line = if category_context.is_empty() {
                    String::new()
                } else {
                    format!("{category_context}\n")
                };
                format!(
                    "{category_line}What {} are visible in this document image?\n\
                     List only the names, one per line.\n\
                     If none are visible, respond with exactly: NONE",
                    self.item_label_plural,
                )
            }
        }
    }

    /// Generate drill prompt for one field in the given input mode.
    ///
    /// - `Text`: Wraps document text + uses existing drill instruction.
    /// - `Vision`: Focused single-field question — image provides context.
    pub fn drill_prompt_for(
        &self,
        mode: InputMode,
        item_name: &str,
        field: &FieldDescriptor,
        document_text: &str,
    ) -> String {
        match mode {
            InputMode::Text => {
                let escaped = escape_xml_tags(document_text);
                let instruction = self.drill_instruction(item_name, field);
                format!("<document>\n{escaped}\n</document>\n\n{instruction}")
            }
            InputMode::Vision => {
                let type_hint = match field.field_type {
                    FieldType::Numeric => " Answer with a number.",
                    FieldType::Date => " Answer with a date.",
                    FieldType::Boolean => " Answer yes or no.",
                    FieldType::Enum(values) => {
                        let _ = values;
                        ""
                    }
                    FieldType::Text => "",
                };

                let enum_hint = if let FieldType::Enum(values) = field.field_type {
                    format!(" Choose from: {}.", values.join(", "))
                } else {
                    String::new()
                };

                format!(
                    "In this document image, for the {} '{}': what is the {}?\n\
                     Answer with just the value, or 'not specified' if not visible.{type_hint}{enum_hint}",
                    self.item_label, item_name, field.prompt_label,
                )
            }
        }
    }

    // ── Basic field accessors ──────────────────────────────

    /// Get field names as a slice of &str (for backward compat with drill_fields).
    pub fn field_names(&self) -> Vec<&'static str> {
        self.fields.iter().map(|f| f.name).collect()
    }

    /// Look up a field by name.
    pub fn field(&self, name: &str) -> Option<&FieldDescriptor> {
        self.fields.iter().find(|f| f.name == name)
    }

    /// Count of required fields.
    pub fn required_field_count(&self) -> usize {
        self.fields.iter().filter(|f| f.required).count()
    }
}

// ═══════════════════════════════════════════════════════════
// Validation
// ═══════════════════════════════════════════════════════════

impl FieldDescriptor {
    /// Check if a value is plausible for this field's type.
    ///
    /// Returns true if the value could be valid. This is a soft check —
    /// not full parsing, just plausibility. Actual parsing happens in dispatch.
    pub fn is_plausible(&self, value: &str) -> bool {
        let trimmed = value.trim();
        if trimmed.is_empty() || is_not_specified(trimmed) {
            return !self.required;
        }
        match self.field_type {
            FieldType::Text => true,
            FieldType::Numeric => {
                // Accept: "4.88", "13.3", "< 0.5", "> 100", "4,28-6,00"
                trimmed
                    .chars()
                    .any(|c| c.is_ascii_digit())
            }
            FieldType::Date => {
                // Accept: "2024-05-16", "16 May 2024", "16/05/2024"
                trimmed.len() >= 4
                    && trimmed.chars().any(|c| c.is_ascii_digit())
            }
            FieldType::Boolean => {
                let lower = trimmed.to_lowercase();
                matches!(
                    lower.as_str(),
                    "yes" | "no" | "true" | "false" | "1" | "0" | "oui" | "non"
                )
            }
            FieldType::Enum(values) => {
                let lower = trimmed.to_lowercase();
                values.iter().any(|v| v.to_lowercase() == lower)
            }
        }
    }
}

/// Check if a value means "not specified" / "N/A".
fn is_not_specified(value: &str) -> bool {
    let lower = value.to_lowercase();
    let trimmed = lower.trim();
    matches!(
        trimmed,
        "not specified"
            | "n/a"
            | "none"
            | "not mentioned"
            | "not available"
            | "not stated"
            | "not provided"
            | "non spécifié"
            | "non mentionné"
            | "non précisé"
    )
}

// ═══════════════════════════════════════════════════════════
// Domain contracts (const data)
// ═══════════════════════════════════════════════════════════

/// Abnormal flag enum values (matches DB CHECK constraint).
const ABNORMAL_FLAGS: &[&str] = &["normal", "low", "high", "critical_low", "critical_high"];

/// Medication frequency type enum values.
const FREQUENCY_TYPES: &[&str] = &["scheduled", "as_needed", "tapering"];

/// Diagnosis status enum values.
const DIAGNOSIS_STATUSES: &[&str] = &["active", "resolved", "monitoring"];

/// Allergy severity enum values.
const ALLERGY_SEVERITIES: &[&str] = &["mild", "moderate", "severe", "life_threatening"];

// ── Lab Results ─────────────────────────────────────────

pub const LAB_RESULTS: DomainContract = DomainContract {
    domain: "lab_results",
    item_label: "test",
    item_label_plural: "tests",
    enumerate_hint: "lab test names",
    fields: &[
        FieldDescriptor {
            name: "value",
            prompt_label: "result value (number)",
            field_type: FieldType::Numeric,
            required: true,
            db_column: "value",
        },
        FieldDescriptor {
            name: "unit",
            prompt_label: "unit of measurement",
            field_type: FieldType::Text,
            required: true,
            db_column: "unit",
        },
        FieldDescriptor {
            name: "reference_range_low",
            prompt_label: "low end of the normal range",
            field_type: FieldType::Numeric,
            required: false,
            db_column: "reference_range_low",
        },
        FieldDescriptor {
            name: "reference_range_high",
            prompt_label: "high end of the normal range",
            field_type: FieldType::Numeric,
            required: false,
            db_column: "reference_range_high",
        },
        FieldDescriptor {
            name: "abnormal_flag",
            prompt_label: "whether the result is normal, low, high, critical_low, or critical_high",
            field_type: FieldType::Enum(ABNORMAL_FLAGS),
            required: true,
            db_column: "abnormal_flag",
        },
        FieldDescriptor {
            name: "collection_date",
            prompt_label: "date the sample was collected",
            field_type: FieldType::Date,
            required: false,
            db_column: "collection_date",
        },
    ],
};

// ── Medications ─────────────────────────────────────────

pub const MEDICATIONS: DomainContract = DomainContract {
    domain: "medications",
    item_label: "medication",
    item_label_plural: "medications",
    enumerate_hint: "medication names",
    fields: &[
        FieldDescriptor {
            name: "dose",
            prompt_label: "dose (e.g., 500mg, 10ml)",
            field_type: FieldType::Text,
            required: true,
            db_column: "dose",
        },
        FieldDescriptor {
            name: "frequency",
            prompt_label: "how often to take it (e.g., twice daily, every 8 hours)",
            field_type: FieldType::Text,
            required: true,
            db_column: "frequency",
        },
        FieldDescriptor {
            name: "route",
            prompt_label: "route of administration (e.g., oral, injection, topical)",
            field_type: FieldType::Text,
            required: false,
            db_column: "route",
        },
        FieldDescriptor {
            name: "instructions",
            prompt_label: "administration instructions (e.g., take with food)",
            field_type: FieldType::Text,
            required: false,
            db_column: "administration_instructions",
        },
        FieldDescriptor {
            name: "frequency_type",
            prompt_label: "whether it is scheduled, as_needed, or tapering",
            field_type: FieldType::Enum(FREQUENCY_TYPES),
            required: false,
            db_column: "frequency_type",
        },
    ],
};

// ── Diagnoses ───────────────────────────────────────────

pub const DIAGNOSES: DomainContract = DomainContract {
    domain: "diagnoses",
    item_label: "diagnosis",
    item_label_plural: "diagnoses",
    enumerate_hint: "diagnosis names",
    fields: &[
        FieldDescriptor {
            name: "date",
            prompt_label: "date diagnosed",
            field_type: FieldType::Date,
            required: false,
            db_column: "date_diagnosed",
        },
        FieldDescriptor {
            name: "status",
            prompt_label: "current status (active, resolved, or monitoring)",
            field_type: FieldType::Enum(DIAGNOSIS_STATUSES),
            required: false,
            db_column: "status",
        },
    ],
};

// ── Allergies ───────────────────────────────────────────

pub const ALLERGIES: DomainContract = DomainContract {
    domain: "allergies",
    item_label: "allergy",
    item_label_plural: "allergies",
    enumerate_hint: "allergens or allergy names",
    fields: &[
        FieldDescriptor {
            name: "reaction",
            prompt_label: "allergic reaction (e.g., rash, anaphylaxis, swelling)",
            field_type: FieldType::Text,
            required: false,
            db_column: "reaction",
        },
        FieldDescriptor {
            name: "severity",
            prompt_label: "severity (mild, moderate, severe, or life_threatening)",
            field_type: FieldType::Enum(ALLERGY_SEVERITIES),
            required: false,
            db_column: "severity",
        },
    ],
};

// ── Procedures ──────────────────────────────────────────

pub const PROCEDURES: DomainContract = DomainContract {
    domain: "procedures",
    item_label: "procedure",
    item_label_plural: "procedures",
    enumerate_hint: "medical procedure names",
    fields: &[
        FieldDescriptor {
            name: "date",
            prompt_label: "date of the procedure",
            field_type: FieldType::Date,
            required: false,
            db_column: "date",
        },
        FieldDescriptor {
            name: "outcome",
            prompt_label: "outcome or result of the procedure",
            field_type: FieldType::Text,
            required: false,
            db_column: "outcome",
        },
        FieldDescriptor {
            name: "follow_up",
            prompt_label: "whether follow-up is required (yes or no)",
            field_type: FieldType::Boolean,
            required: false,
            db_column: "follow_up_required",
        },
    ],
};

// ── Referrals ───────────────────────────────────────────

pub const REFERRALS: DomainContract = DomainContract {
    domain: "referrals",
    item_label: "referral",
    item_label_plural: "referrals",
    enumerate_hint: "referral entries",
    fields: &[
        FieldDescriptor {
            name: "specialty",
            prompt_label: "medical specialty referred to",
            field_type: FieldType::Text,
            required: false,
            db_column: "referred_to_professional_id",
        },
        FieldDescriptor {
            name: "reason",
            prompt_label: "reason for the referral",
            field_type: FieldType::Text,
            required: false,
            db_column: "reason",
        },
    ],
};

// ── Instructions ────────────────────────────────────────

pub const INSTRUCTIONS: DomainContract = DomainContract {
    domain: "instructions",
    item_label: "instruction",
    item_label_plural: "instructions",
    enumerate_hint: "patient instructions or care directions",
    fields: &[
        FieldDescriptor {
            name: "category",
            prompt_label: "category (e.g., diet, activity, wound care, follow-up)",
            field_type: FieldType::Text,
            required: false,
            db_column: "category",
        },
    ],
};

// ═══════════════════════════════════════════════════════════
// Full prompt builders (document wrapping + escaping)
// ═══════════════════════════════════════════════════════════

impl DomainContract {
    /// Build complete MarkdownList user prompt with document wrapping.
    ///
    /// Includes XML-escaped document text, optional OCR confidence warning,
    /// and the domain-specific extraction instruction.
    pub fn build_markdown_list_prompt(&self, document_text: &str, ocr_confidence: f32) -> String {
        let confidence_note = confidence_warning(ocr_confidence);
        let escaped = escape_xml_tags(document_text);
        let instruction = self.markdown_list_instruction();
        format!("{confidence_note}<document>\n{escaped}\n</document>\n\n{instruction}")
    }

    /// Build complete enumerate user prompt with document wrapping.
    ///
    /// Phase 1 of IterativeDrill: list item names from the document.
    pub fn build_enumerate_prompt(&self, document_text: &str) -> String {
        let escaped = escape_xml_tags(document_text);
        let instruction = self.enumerate_instruction();
        format!("<document>\n{escaped}\n</document>\n\n{instruction}")
    }
}

/// OCR confidence warning for low-confidence documents.
fn confidence_warning(confidence: f32) -> &'static str {
    if confidence < 0.70 {
        "NOTE: This text was extracted with LOW confidence. Some characters may be misread. \
         Mark uncertain values with 'uncertain'.\n"
    } else {
        ""
    }
}

/// Escape XML-like tags in document text to prevent prompt boundary breakout.
fn escape_xml_tags(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

// ═══════════════════════════════════════════════════════════
// Lookup
// ═══════════════════════════════════════════════════════════

/// All 7 document domain contracts.
pub const ALL_DOCUMENT_CONTRACTS: &[&DomainContract] = &[
    &MEDICATIONS,
    &LAB_RESULTS,
    &DIAGNOSES,
    &ALLERGIES,
    &PROCEDURES,
    &REFERRALS,
    &INSTRUCTIONS,
];

/// Look up a contract by domain name.
pub fn contract_for_domain(domain: &str) -> Option<&'static DomainContract> {
    ALL_DOCUMENT_CONTRACTS
        .iter()
        .find(|c| c.domain == domain)
        .copied()
}

/// Look up a contract by DocumentDomain enum variant.
///
/// Maps the existing prompt_templates::DocumentDomain to contracts
/// for backward compatibility during STR-02 migration.
pub fn contract_for_document_domain(
    domain: crate::pipeline::prompt_templates::DocumentDomain,
) -> &'static DomainContract {
    use crate::pipeline::prompt_templates::DocumentDomain;
    match domain {
        DocumentDomain::Medications => &MEDICATIONS,
        DocumentDomain::LabResults => &LAB_RESULTS,
        DocumentDomain::Diagnoses => &DIAGNOSES,
        DocumentDomain::Allergies => &ALLERGIES,
        DocumentDomain::Procedures => &PROCEDURES,
        DocumentDomain::Referrals => &REFERRALS,
        DocumentDomain::Instructions => &INSTRUCTIONS,
    }
}

// ═══════════════════════════════════════════════════════════
// 09-CAE: Category-aware domain filtering
// ═══════════════════════════════════════════════════════════

use crate::pipeline::extraction::vision_classifier::UserDocumentType;
use crate::pipeline::prompt_templates::DocumentDomain;

/// 09-CAE: Return the relevant extraction domains for a user-selected document type.
///
/// Lab reports contain lab results and occasionally interpretive diagnoses.
/// Prescriptions contain medications, administration instructions, and occasionally indications.
/// Medical images are routed to MedicalImageInterpreter — they never reach IterativeDrill.
///
/// Domains not mapped to any category (Allergies, Procedures, Referrals) remain
/// available for chat extraction (NightBatch) where all 7 domains apply.
pub fn domains_for_document_type(doc_type: UserDocumentType) -> &'static [DocumentDomain] {
    match doc_type {
        UserDocumentType::LabReport => &[
            DocumentDomain::LabResults,
            DocumentDomain::Diagnoses,
        ],
        UserDocumentType::Prescription => &[
            DocumentDomain::Medications,
            DocumentDomain::Instructions,
            DocumentDomain::Diagnoses,
        ],
        UserDocumentType::MedicalImage => &[], // never reaches IterativeDrill
    }
}

/// 09-CAE: Return category context string for category-aware enumerate prompts.
///
/// Prepended to vision enumerate prompts so the model knows what document type
/// it is looking at. Empty string for legacy/fallback path.
pub fn category_context(doc_type: UserDocumentType) -> &'static str {
    match doc_type {
        UserDocumentType::LabReport => "This is a laboratory analysis report.",
        UserDocumentType::Prescription => "This is a medical prescription.",
        UserDocumentType::MedicalImage => "", // never reaches IterativeDrill
    }
}

// ═══════════════════════════════════════════════════════════
// 10-LDC: Language-aware PromptLocale
// ═══════════════════════════════════════════════════════════

/// Language-specific prompt fragments for a medical extraction domain.
///
/// Each supported language provides its own PromptLocale for each domain.
/// Prompts are written in the target language following BM-06 findings:
/// language-matched prompts eliminate translation overhead and improve
/// extraction quality by 5-23x (BM-06 evidence: 12/12 vs ~10/13 tests).
///
/// Design: prompts are DATA, not code. Adding a language means adding rows,
/// not modifying control flow. Adding a domain means adding a match arm.
#[derive(Debug, Clone, Copy)]
pub struct PromptLocale {
    /// Language code (ISO 639-1): "en", "fr", "de"
    pub lang: &'static str,
    /// Singular item label. EN: "analyte", FR: "analyse", DE: "Laborparameter"
    pub item_label: &'static str,
    /// Plural item label. EN: "analytes", FR: "analyses", DE: "Laborparameter"
    pub item_label_plural: &'static str,
    /// Vision enumerate prompt — complete, ready to send.
    pub vision_enumerate: &'static str,
    /// Vision drill prompt for value+unit. `{item}` placeholder.
    pub vision_drill_value: &'static str,
    /// Vision drill prompt for reference range. `{item}` placeholder.
    pub vision_drill_range: &'static str,
    /// NONE keyword in this language for empty-case detection.
    pub none_keyword: &'static str,
    /// System prompt for IterativeDrill — focused extraction assistant.
    pub system_prompt: &'static str,
    /// 11-SRP: Start delimiter for model's answer section.
    /// Parser extracts content between answer_start and answer_end.
    pub answer_start: &'static str,
    /// 11-SRP: End delimiter for model's answer section.
    pub answer_end: &'static str,
}

// ── Lab Results locales ────────────────────────────────────

const LAB_RESULTS_EN: PromptLocale = PromptLocale {
    lang: "en",
    item_label: "analyte",
    item_label_plural: "analytes",
    vision_enumerate:
        "What are the names of the laboratory analytes listed in this document?\n\
         One name per line.\n\
         If no analytes are visible, respond: <answer>NONE</answer>",
    vision_drill_value:
        "What is the measured value and unit for the analyte '{item}'?",
    vision_drill_range:
        "What is the reference range (normal values) for '{item}'?",
    none_keyword: "NONE",
    system_prompt:
        "You are a medical data extraction assistant.\n\
         Reason inside <think>...</think>, then give your answer inside <answer>...</answer>.\n\n\
         Example:\n\
         Question: What analytes are listed on this page?\n\
         <think>\n\
         I see a hematology section with Hemoglobin, Hematocrit, WBC count.\n\
         </think>\n\
         <answer>\n\
         Hemoglobin\n\
         Hematocrit\n\
         WBC\n\
         </answer>\n\n\
         Rules:\n\
         - Only extract what is asked\n\
         - Answer must match what you see in the document\n\
         - Close all tags",
    answer_start: "<answer>",
    answer_end: "</answer>",
};

const LAB_RESULTS_FR: PromptLocale = PromptLocale {
    lang: "fr",
    item_label: "analyse",
    item_label_plural: "analyses",
    vision_enumerate:
        "Quels sont les noms des analyses biologiques listées dans ce document ?\n\
         Un nom par ligne.\n\
         Si aucune analyse n'est visible, répondre : <answer>AUCUN</answer>",
    vision_drill_value:
        "Quelle est la valeur mesurée et l'unité pour l'analyse « {item} » ?",
    vision_drill_range:
        "Quelle est la plage de référence (valeurs normales) pour « {item} » ?",
    none_keyword: "AUCUN",
    system_prompt:
        "Tu es un assistant d'extraction médicale.\n\
         Raisonne dans <think>...</think>, puis donne ta réponse dans <answer>...</answer>.\n\n\
         Exemple :\n\
         Question : Quels sont les noms des analyses sur cette page ?\n\
         <think>\n\
         Je vois une section HEMATOLOGIE avec Hématies, Hémoglobine, Hématocrite.\n\
         </think>\n\
         <answer>\n\
         Hématies\n\
         Hémoglobine\n\
         Hématocrite\n\
         </answer>\n\n\
         Règles :\n\
         - Extrais uniquement ce qui est demandé\n\
         - La réponse doit correspondre au document\n\
         - Ferme toujours les balises",
    answer_start: "<answer>",
    answer_end: "</answer>",
};

const LAB_RESULTS_DE: PromptLocale = PromptLocale {
    lang: "de",
    item_label: "Laborparameter",
    item_label_plural: "Laborparameter",
    vision_enumerate:
        "Welche Laborparameter sind in diesem Dokument aufgelistet?\n\
         Ein Name pro Zeile.\n\
         Falls keine Laborparameter sichtbar sind, antworte: <answer>KEINE</answer>",
    vision_drill_value:
        "Welcher Messwert und welche Einheit hat der Parameter '{item}'?",
    vision_drill_range:
        "Welcher Referenzbereich (Normalwerte) gilt fuer '{item}'?",
    none_keyword: "KEINE",
    system_prompt:
        "Du bist ein Assistent fuer medizinische Datenextraktion.\n\
         Denke in <think>...</think>, dann gib deine Antwort in <answer>...</answer>.\n\n\
         Beispiel:\n\
         Frage: Welche Laborparameter sind auf dieser Seite?\n\
         <think>\n\
         Ich sehe einen Haematologie-Abschnitt mit Haemoglobin, Haematokrit.\n\
         </think>\n\
         <answer>\n\
         Haemoglobin\n\
         Haematokrit\n\
         </answer>\n\n\
         Regeln:\n\
         - Nur extrahieren, was gefragt wird\n\
         - Antwort muss dem Dokument entsprechen\n\
         - Alle Tags schliessen",
    answer_start: "<answer>",
    answer_end: "</answer>",
};

// ── Medications locales ────────────────────────────────────

const MEDICATIONS_EN: PromptLocale = PromptLocale {
    lang: "en",
    item_label: "medication",
    item_label_plural: "prescribed medications",
    vision_enumerate:
        "What are the names of the prescribed medications in this document?\n\
         One name per line.\n\
         If no medications are visible, respond: <answer>NONE</answer>",
    vision_drill_value:
        "What is the prescribed dosage for '{item}' (dose, frequency, route)?",
    vision_drill_range: "",
    none_keyword: "NONE",
    system_prompt:
        "You are a medical data extraction assistant.\n\
         Reason inside <think>...</think>, then give your answer inside <answer>...</answer>.\n\n\
         Example:\n\
         Question: What medications are prescribed?\n\
         <think>\n\
         I see Paracetamol 1000mg and Amoxicillin 500mg on this prescription.\n\
         </think>\n\
         <answer>\n\
         Paracetamol\n\
         Amoxicillin\n\
         </answer>\n\n\
         Rules:\n\
         - Only extract what is asked\n\
         - Answer must match what you see in the document\n\
         - Close all tags",
    answer_start: "<answer>",
    answer_end: "</answer>",
};

const MEDICATIONS_FR: PromptLocale = PromptLocale {
    lang: "fr",
    item_label: "médicament",
    item_label_plural: "médicaments prescrits",
    vision_enumerate:
        "Quels sont les noms des médicaments prescrits dans cette ordonnance ?\n\
         Un nom par ligne.\n\
         Si aucun médicament n'est visible, répondre : <answer>AUCUN</answer>",
    vision_drill_value:
        "Quelle est la posologie prescrite pour « {item} » \
         (dose, fréquence, voie d'administration) ?",
    vision_drill_range: "",
    none_keyword: "AUCUN",
    system_prompt:
        "Tu es un assistant d'extraction médicale.\n\
         Raisonne dans <think>...</think>, puis donne ta réponse dans <answer>...</answer>.\n\n\
         Exemple :\n\
         Question : Quels médicaments sont prescrits ?\n\
         <think>\n\
         Je vois Paracétamol 1000mg et Amoxicilline 500mg sur cette ordonnance.\n\
         </think>\n\
         <answer>\n\
         Paracétamol\n\
         Amoxicilline\n\
         </answer>\n\n\
         Règles :\n\
         - Extrais uniquement ce qui est demandé\n\
         - La réponse doit correspondre au document\n\
         - Ferme toujours les balises",
    answer_start: "<answer>",
    answer_end: "</answer>",
};

const MEDICATIONS_DE: PromptLocale = PromptLocale {
    lang: "de",
    item_label: "Medikament",
    item_label_plural: "verordnete Medikamente",
    vision_enumerate:
        "Welche Medikamente sind in diesem Rezept verordnet?\n\
         Ein Name pro Zeile.\n\
         Falls keine Medikamente sichtbar sind, antworte: <answer>KEINE</answer>",
    vision_drill_value:
        "Welche Dosierung ist fuer '{item}' verordnet \
         (Dosis, Haeufigkeit, Verabreichungsweg)?",
    vision_drill_range: "",
    none_keyword: "KEINE",
    system_prompt:
        "Du bist ein Assistent fuer medizinische Datenextraktion.\n\
         Denke in <think>...</think>, dann gib deine Antwort in <answer>...</answer>.\n\n\
         Beispiel:\n\
         Frage: Welche Medikamente sind verordnet?\n\
         <think>\n\
         Ich sehe Paracetamol 1000mg und Amoxicillin 500mg auf diesem Rezept.\n\
         </think>\n\
         <answer>\n\
         Paracetamol\n\
         Amoxicillin\n\
         </answer>\n\n\
         Regeln:\n\
         - Nur extrahieren, was gefragt wird\n\
         - Antwort muss dem Dokument entsprechen\n\
         - Alle Tags schliessen",
    answer_start: "<answer>",
    answer_end: "</answer>",
};

// ── Diagnoses locales ──────────────────────────────────────

const DIAGNOSES_EN: PromptLocale = PromptLocale {
    lang: "en",
    item_label: "diagnosis",
    item_label_plural: "medical diagnoses",
    vision_enumerate:
        "What medical diagnoses are mentioned in this document?\n\
         One diagnosis per line.\n\
         If no diagnoses are visible, respond: <answer>NONE</answer>",
    vision_drill_value:
        "What is the current status of the diagnosis '{item}' \
         (active, resolved, monitoring)?",
    vision_drill_range: "",
    none_keyword: "NONE",
    system_prompt:
        "You are a medical data extraction assistant.\n\
         Reason inside <think>...</think>, then give your answer inside <answer>...</answer>.\n\n\
         Example:\n\
         Question: What diagnoses are mentioned?\n\
         <think>\n\
         I see Type 2 Diabetes and Hypertension listed as active conditions.\n\
         </think>\n\
         <answer>\n\
         Type 2 Diabetes\n\
         Hypertension\n\
         </answer>\n\n\
         Rules:\n\
         - Only extract what is asked\n\
         - Answer must match what you see in the document\n\
         - Close all tags",
    answer_start: "<answer>",
    answer_end: "</answer>",
};

const DIAGNOSES_FR: PromptLocale = PromptLocale {
    lang: "fr",
    item_label: "diagnostic",
    item_label_plural: "diagnostics médicaux",
    vision_enumerate:
        "Quels diagnostics médicaux sont mentionnés dans ce document ?\n\
         Un diagnostic par ligne.\n\
         Si aucun diagnostic n'est visible, répondre : <answer>AUCUN</answer>",
    vision_drill_value:
        "Quel est le statut actuel du diagnostic « {item} » \
         (actif, résolu, en surveillance) ?",
    vision_drill_range: "",
    none_keyword: "AUCUN",
    system_prompt:
        "Tu es un assistant d'extraction médicale.\n\
         Raisonne dans <think>...</think>, puis donne ta réponse dans <answer>...</answer>.\n\n\
         Exemple :\n\
         Question : Quels diagnostics sont mentionnés ?\n\
         <think>\n\
         Je vois Diabète de type 2 et Hypertension artérielle comme conditions actives.\n\
         </think>\n\
         <answer>\n\
         Diabète de type 2\n\
         Hypertension artérielle\n\
         </answer>\n\n\
         Règles :\n\
         - Extrais uniquement ce qui est demandé\n\
         - La réponse doit correspondre au document\n\
         - Ferme toujours les balises",
    answer_start: "<answer>",
    answer_end: "</answer>",
};

const DIAGNOSES_DE: PromptLocale = PromptLocale {
    lang: "de",
    item_label: "Diagnose",
    item_label_plural: "medizinische Diagnosen",
    vision_enumerate:
        "Welche medizinischen Diagnosen werden in diesem Dokument erwaehnt?\n\
         Eine Diagnose pro Zeile.\n\
         Falls keine Diagnosen sichtbar sind, antworte: <answer>KEINE</answer>",
    vision_drill_value:
        "Welchen aktuellen Status hat die Diagnose '{item}' \
         (aktiv, abgeklungen, unter Beobachtung)?",
    vision_drill_range: "",
    none_keyword: "KEINE",
    system_prompt:
        "Du bist ein Assistent fuer medizinische Datenextraktion.\n\
         Denke in <think>...</think>, dann gib deine Antwort in <answer>...</answer>.\n\n\
         Beispiel:\n\
         Frage: Welche Diagnosen werden erwaehnt?\n\
         <think>\n\
         Ich sehe Diabetes Typ 2 und Bluthochdruck als aktive Diagnosen.\n\
         </think>\n\
         <answer>\n\
         Diabetes Typ 2\n\
         Bluthochdruck\n\
         </answer>\n\n\
         Regeln:\n\
         - Nur extrahieren, was gefragt wird\n\
         - Antwort muss dem Dokument entsprechen\n\
         - Alle Tags schliessen",
    answer_start: "<answer>",
    answer_end: "</answer>",
};

// ── Generic locales (allergies, procedures, referrals, instructions) ──

const GENERIC_EN: PromptLocale = PromptLocale {
    lang: "en",
    item_label: "item",
    item_label_plural: "items",
    vision_enumerate:
        "What items are listed in this document?\n\
         One item per line.\n\
         If none are visible, respond: <answer>NONE</answer>",
    vision_drill_value:
        "What are the details for '{item}'?",
    vision_drill_range: "",
    none_keyword: "NONE",
    system_prompt:
        "You are a medical data extraction assistant.\n\
         Reason inside <think>...</think>, then give your answer inside <answer>...</answer>.\n\n\
         Example:\n\
         Question: What items are listed?\n\
         <think>\n\
         I see an allergy to Penicillin and a referral to cardiology.\n\
         </think>\n\
         <answer>\n\
         Penicillin allergy\n\
         Cardiology referral\n\
         </answer>\n\n\
         Rules:\n\
         - Only extract what is asked\n\
         - Answer must match what you see in the document\n\
         - Close all tags",
    answer_start: "<answer>",
    answer_end: "</answer>",
};

const GENERIC_FR: PromptLocale = PromptLocale {
    lang: "fr",
    item_label: "élément",
    item_label_plural: "éléments",
    vision_enumerate:
        "Quels éléments sont listés dans ce document ?\n\
         Un élément par ligne.\n\
         Si aucun élément n'est visible, répondre : <answer>AUCUN</answer>",
    vision_drill_value:
        "Quels sont les détails pour « {item} » ?",
    vision_drill_range: "",
    none_keyword: "AUCUN",
    system_prompt:
        "Tu es un assistant d'extraction médicale.\n\
         Raisonne dans <think>...</think>, puis donne ta réponse dans <answer>...</answer>.\n\n\
         Exemple :\n\
         Question : Quels éléments sont listés ?\n\
         <think>\n\
         Je vois une allergie à la Pénicilline et une orientation vers la cardiologie.\n\
         </think>\n\
         <answer>\n\
         Allergie Pénicilline\n\
         Orientation cardiologie\n\
         </answer>\n\n\
         Règles :\n\
         - Extrais uniquement ce qui est demandé\n\
         - La réponse doit correspondre au document\n\
         - Ferme toujours les balises",
    answer_start: "<answer>",
    answer_end: "</answer>",
};

const GENERIC_DE: PromptLocale = PromptLocale {
    lang: "de",
    item_label: "Element",
    item_label_plural: "Elemente",
    vision_enumerate:
        "Welche Elemente sind in diesem Dokument aufgelistet?\n\
         Ein Element pro Zeile.\n\
         Falls keine Elemente sichtbar sind, antworte: <answer>KEINE</answer>",
    vision_drill_value:
        "Welche Details gibt es zu '{item}'?",
    vision_drill_range: "",
    none_keyword: "KEINE",
    system_prompt:
        "Du bist ein Assistent fuer medizinische Datenextraktion.\n\
         Denke in <think>...</think>, dann gib deine Antwort in <answer>...</answer>.\n\n\
         Beispiel:\n\
         Frage: Welche Elemente sind aufgelistet?\n\
         <think>\n\
         Ich sehe eine Penicillin-Allergie und eine Ueberweisung zur Kardiologie.\n\
         </think>\n\
         <answer>\n\
         Penicillin-Allergie\n\
         Ueberweisung Kardiologie\n\
         </answer>\n\n\
         Regeln:\n\
         - Nur extrahieren, was gefragt wird\n\
         - Antwort muss dem Dokument entsprechen\n\
         - Alle Tags schliessen",
    answer_start: "<answer>",
    answer_end: "</answer>",
};

/// Resolve the PromptLocale for a domain + language.
///
/// Falls back to English if the language is not supported for this domain.
/// This ensures the app never panics on unsupported languages — it degrades
/// gracefully to English (which BM-06 shows still works, just less accurately).
pub fn locale_for_domain(domain: &str, lang: &str) -> &'static PromptLocale {
    let lang_code = if lang.len() >= 2 { &lang[..2] } else { lang };
    match (domain, lang_code) {
        ("lab_results", "fr") => &LAB_RESULTS_FR,
        ("lab_results", "de") => &LAB_RESULTS_DE,
        ("lab_results", _) => &LAB_RESULTS_EN,

        ("medications", "fr") => &MEDICATIONS_FR,
        ("medications", "de") => &MEDICATIONS_DE,
        ("medications", _) => &MEDICATIONS_EN,

        ("diagnoses", "fr") => &DIAGNOSES_FR,
        ("diagnoses", "de") => &DIAGNOSES_DE,
        ("diagnoses", _) => &DIAGNOSES_EN,

        (_, "fr") => &GENERIC_FR,
        (_, "de") => &GENERIC_DE,
        (_, _) => &GENERIC_EN,
    }
}

// ═══════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── Contract completeness ───────────────────────────

    #[test]
    fn all_seven_document_contracts_declared() {
        assert_eq!(ALL_DOCUMENT_CONTRACTS.len(), 7);
    }

    #[test]
    fn each_contract_has_fields() {
        for contract in ALL_DOCUMENT_CONTRACTS {
            assert!(
                !contract.fields.is_empty(),
                "Contract {} has no fields",
                contract.domain
            );
        }
    }

    #[test]
    fn lab_results_has_six_fields() {
        assert_eq!(LAB_RESULTS.fields.len(), 6);
    }

    #[test]
    fn medications_has_five_fields() {
        assert_eq!(MEDICATIONS.fields.len(), 5);
    }

    #[test]
    fn lab_results_reference_range_split() {
        // Verify the gap fix: reference_range_low and _high are separate fields
        assert!(LAB_RESULTS.field("reference_range_low").is_some());
        assert!(LAB_RESULTS.field("reference_range_high").is_some());
        // No combined "reference_range" field
        assert!(LAB_RESULTS.field("reference_range").is_none());
    }

    #[test]
    fn lab_results_has_collection_date() {
        // Verify the gap fix: collection_date is now extracted
        let field = LAB_RESULTS.field("collection_date").unwrap();
        assert_eq!(field.field_type, FieldType::Date);
        assert_eq!(field.db_column, "collection_date");
    }

    #[test]
    fn medications_has_frequency_type() {
        // Verify the gap fix: frequency_type is now extracted
        let field = MEDICATIONS.field("frequency_type").unwrap();
        assert!(matches!(field.field_type, FieldType::Enum(_)));
    }

    #[test]
    fn abnormal_flag_is_enum() {
        let field = LAB_RESULTS.field("abnormal_flag").unwrap();
        match field.field_type {
            FieldType::Enum(values) => {
                assert_eq!(values.len(), 5);
                assert!(values.contains(&"normal"));
                assert!(values.contains(&"critical_high"));
            }
            _ => panic!("abnormal_flag should be Enum"),
        }
    }

    // ── Contract lookup ─────────────────────────────────

    #[test]
    fn lookup_by_domain_name() {
        let contract = contract_for_domain("lab_results").unwrap();
        assert_eq!(contract.domain, "lab_results");
        assert_eq!(contract.item_label, "test");
    }

    #[test]
    fn lookup_unknown_domain_returns_none() {
        assert!(contract_for_domain("unknown_domain").is_none());
    }

    #[test]
    fn lookup_by_document_domain_enum() {
        use crate::pipeline::prompt_templates::DocumentDomain;
        let contract = contract_for_document_domain(DocumentDomain::LabResults);
        assert_eq!(contract.domain, "lab_results");
    }

    // ── Prompt generation ───────────────────────────────

    #[test]
    fn markdown_list_instruction_generated() {
        let instruction = LAB_RESULTS.markdown_list_instruction();
        assert!(instruction.contains("lab test names"));
        assert!(instruction.contains("result value"));
        assert!(instruction.contains("unit of measurement"));
    }

    #[test]
    fn enumerate_instruction_generated() {
        let instruction = MEDICATIONS.enumerate_instruction();
        assert!(instruction.contains("medication names"));
        assert!(instruction.contains("List only the names"));
    }

    #[test]
    fn drill_instruction_generated() {
        let field = LAB_RESULTS.field("value").unwrap();
        let instruction = LAB_RESULTS.drill_instruction("Hemoglobin", field);
        assert!(instruction.contains("test"));
        assert!(instruction.contains("Hemoglobin"));
        assert!(instruction.contains("result value"));
        assert!(instruction.contains("not specified"));
    }

    #[test]
    fn drill_instruction_enum_field_shows_choices() {
        let field = LAB_RESULTS.field("abnormal_flag").unwrap();
        let instruction = LAB_RESULTS.drill_instruction("Hemoglobin", field);
        assert!(instruction.contains("Choose from:"));
        assert!(instruction.contains("normal"));
        assert!(instruction.contains("critical_high"));
    }

    #[test]
    fn drill_instruction_numeric_field_has_number_hint() {
        let field = LAB_RESULTS.field("value").unwrap();
        let instruction = LAB_RESULTS.drill_instruction("Hemoglobin", field);
        assert!(instruction.contains("Answer with a number"));
    }

    #[test]
    fn drill_instruction_date_field_has_date_hint() {
        let field = LAB_RESULTS.field("collection_date").unwrap();
        let instruction = LAB_RESULTS.drill_instruction("Hemoglobin", field);
        assert!(instruction.contains("Answer with a date"));
    }

    // ── Field validation ────────────────────────────────

    #[test]
    fn numeric_field_accepts_numbers() {
        let field = LAB_RESULTS.field("value").unwrap();
        assert!(field.is_plausible("4.88"));
        assert!(field.is_plausible("13.3"));
        assert!(field.is_plausible("< 0.5"));
        assert!(field.is_plausible("4,28-6,00"));
    }

    #[test]
    fn numeric_field_rejects_pure_text() {
        let field = LAB_RESULTS.field("value").unwrap();
        // Required field with no digits — not plausible
        assert!(!field.is_plausible("normal"));
    }

    #[test]
    fn enum_field_accepts_valid_values() {
        let field = LAB_RESULTS.field("abnormal_flag").unwrap();
        assert!(field.is_plausible("normal"));
        assert!(field.is_plausible("Low")); // Case-insensitive
        assert!(field.is_plausible("critical_high"));
    }

    #[test]
    fn enum_field_rejects_invalid_values() {
        let field = LAB_RESULTS.field("abnormal_flag").unwrap();
        assert!(!field.is_plausible("maybe"));
        assert!(!field.is_plausible("somewhat_high"));
    }

    #[test]
    fn not_specified_accepted_for_optional_fields() {
        let field = LAB_RESULTS.field("collection_date").unwrap();
        assert!(!field.required);
        assert!(field.is_plausible("not specified"));
        assert!(field.is_plausible("N/A"));
    }

    #[test]
    fn not_specified_rejected_for_required_fields() {
        let field = LAB_RESULTS.field("value").unwrap();
        assert!(field.required);
        assert!(!field.is_plausible("not specified"));
    }

    #[test]
    fn boolean_field_accepts_yes_no() {
        let field = PROCEDURES.field("follow_up").unwrap();
        assert!(field.is_plausible("yes"));
        assert!(field.is_plausible("No"));
        assert!(field.is_plausible("true"));
        assert!(field.is_plausible("oui")); // French
    }

    #[test]
    fn date_field_accepts_various_formats() {
        let field = LAB_RESULTS.field("collection_date").unwrap();
        assert!(field.is_plausible("2024-05-16"));
        assert!(field.is_plausible("16/05/2024"));
        assert!(field.is_plausible("May 16, 2024"));
    }

    // ── Field names backward compat ─────────────────────

    #[test]
    fn field_names_matches_drill_fields_for_basic_domains() {
        // Diagnoses, Allergies, Procedures, Referrals, Instructions should
        // produce the same field names as the current drill_fields()
        assert_eq!(DIAGNOSES.field_names(), vec!["date", "status"]);
        assert_eq!(ALLERGIES.field_names(), vec!["reaction", "severity"]);
        assert_eq!(PROCEDURES.field_names(), vec!["date", "outcome", "follow_up"]);
        assert_eq!(REFERRALS.field_names(), vec!["specialty", "reason"]);
        assert_eq!(INSTRUCTIONS.field_names(), vec!["category"]);
    }

    #[test]
    fn lab_results_field_names_extended() {
        // Lab results now has MORE fields than old drill_fields
        let names = LAB_RESULTS.field_names();
        assert!(names.contains(&"value"));
        assert!(names.contains(&"unit"));
        assert!(names.contains(&"reference_range_low"));
        assert!(names.contains(&"reference_range_high"));
        assert!(names.contains(&"abnormal_flag"));
        assert!(names.contains(&"collection_date"));
    }

    #[test]
    fn medications_field_names_extended() {
        let names = MEDICATIONS.field_names();
        assert!(names.contains(&"dose"));
        assert!(names.contains(&"frequency"));
        assert!(names.contains(&"route"));
        assert!(names.contains(&"instructions"));
        assert!(names.contains(&"frequency_type"));
    }

    // ── Display formatting ──────────────────────────────

    #[test]
    fn field_type_display() {
        assert_eq!(format!("{}", FieldType::Text), "text");
        assert_eq!(format!("{}", FieldType::Numeric), "numeric");
        assert_eq!(
            format!("{}", FieldType::Enum(&["a", "b"])),
            "enum(a|b)"
        );
    }

    // ── Required field count ────────────────────────────

    #[test]
    fn lab_results_required_count() {
        // value, unit, abnormal_flag are required
        assert_eq!(LAB_RESULTS.required_field_count(), 3);
    }

    #[test]
    fn diagnoses_no_required_fields() {
        // Both date and status are optional
        assert_eq!(DIAGNOSES.required_field_count(), 0);
    }

    // ── Full prompt builders ────────────────────────────

    #[test]
    fn markdown_list_prompt_wraps_document() {
        let prompt = LAB_RESULTS.build_markdown_list_prompt("Hemoglobin: 13.3 g/dl", 0.95);
        assert!(prompt.contains("<document>"));
        assert!(prompt.contains("</document>"));
        assert!(prompt.contains("Hemoglobin: 13.3 g/dl"));
        assert!(prompt.contains("lab test names"));
    }

    #[test]
    fn markdown_list_prompt_escapes_xml() {
        let prompt = MEDICATIONS.build_markdown_list_prompt("<script>alert('xss')</script>", 0.90);
        assert!(!prompt.contains("<script>"));
        assert!(prompt.contains("&lt;script&gt;"));
    }

    #[test]
    fn markdown_list_prompt_low_confidence_warning() {
        let prompt = LAB_RESULTS.build_markdown_list_prompt("noisy OCR text", 0.50);
        assert!(prompt.contains("LOW confidence"));
        assert!(prompt.contains("uncertain"));
    }

    #[test]
    fn markdown_list_prompt_no_warning_at_high_confidence() {
        let prompt = LAB_RESULTS.build_markdown_list_prompt("clean text", 0.95);
        assert!(!prompt.contains("LOW confidence"));
    }

    #[test]
    fn enumerate_prompt_wraps_document() {
        let prompt = MEDICATIONS.build_enumerate_prompt("Metformin 500mg twice daily");
        assert!(prompt.contains("<document>"));
        assert!(prompt.contains("</document>"));
        assert!(prompt.contains("medication names"));
        assert!(prompt.contains("List only the names"));
    }

    #[test]
    fn enumerate_prompt_escapes_xml() {
        let prompt = ALLERGIES.build_enumerate_prompt("Patient <allergic> to penicillin");
        assert!(!prompt.contains("<allergic>"));
        assert!(prompt.contains("&lt;allergic&gt;"));
    }

    // ── C4: Vision-aware prompt tests ───────────────────

    #[test]
    fn input_mode_variants() {
        assert_ne!(InputMode::Text, InputMode::Vision);
        assert_eq!(InputMode::Text, InputMode::Text);
        assert_eq!(InputMode::Vision, InputMode::Vision);
    }

    #[test]
    fn enumerate_prompt_text_mode_matches_existing() {
        let text_mode = LAB_RESULTS.enumerate_prompt_for(InputMode::Text, "Hemoglobin: 13.3", "");
        let existing = LAB_RESULTS.build_enumerate_prompt("Hemoglobin: 13.3");
        assert_eq!(text_mode, existing);
    }

    #[test]
    fn enumerate_prompt_vision_mode_lab_results() {
        let prompt = LAB_RESULTS.enumerate_prompt_for(InputMode::Vision, "", "");
        assert!(prompt.contains("tests"));
        assert!(prompt.contains("document image"));
        assert!(prompt.contains("one per line"));
        assert!(prompt.contains("NONE")); // 09-CAE: always present
        // Should NOT contain document wrapping
        assert!(!prompt.contains("<document>"));
    }

    #[test]
    fn enumerate_prompt_vision_mode_medications() {
        let prompt = MEDICATIONS.enumerate_prompt_for(InputMode::Vision, "", "");
        assert!(prompt.contains("medications"));
        assert!(prompt.contains("document image"));
        assert!(prompt.contains("NONE")); // 09-CAE: always present
    }

    // ── 09-CAE: Category-aware prompt tests ─────────────

    #[test]
    fn enumerate_prompt_vision_with_category_context() {
        let ctx = category_context(UserDocumentType::LabReport);
        let prompt = LAB_RESULTS.enumerate_prompt_for(InputMode::Vision, "", ctx);
        assert!(prompt.contains("laboratory analysis report"));
        assert!(prompt.contains("tests"));
        assert!(prompt.contains("NONE"));
    }

    #[test]
    fn enumerate_prompt_vision_prescription_context() {
        let ctx = category_context(UserDocumentType::Prescription);
        let prompt = MEDICATIONS.enumerate_prompt_for(InputMode::Vision, "", ctx);
        assert!(prompt.contains("medical prescription"));
        assert!(prompt.contains("medications"));
        assert!(prompt.contains("NONE"));
    }

    #[test]
    fn enumerate_prompt_vision_no_context_still_has_none() {
        let prompt = LAB_RESULTS.enumerate_prompt_for(InputMode::Vision, "", "");
        assert!(prompt.contains("NONE"));
        // No category line when context is empty
        assert!(!prompt.contains("laboratory"));
        assert!(!prompt.contains("prescription"));
    }

    #[test]
    fn drill_prompt_text_mode_wraps_document() {
        let field = LAB_RESULTS.field("value").unwrap();
        let prompt = LAB_RESULTS.drill_prompt_for(InputMode::Text, "Hemoglobin", field, "test doc");
        assert!(prompt.contains("<document>"));
        assert!(prompt.contains("test doc"));
        assert!(prompt.contains("Hemoglobin"));
        assert!(prompt.contains("result value"));
    }

    #[test]
    fn drill_prompt_vision_mode_value_field() {
        let field = LAB_RESULTS.field("value").unwrap();
        let prompt = LAB_RESULTS.drill_prompt_for(InputMode::Vision, "Hémoglobine", field, "");
        assert!(prompt.contains("document image"));
        assert!(prompt.contains("Hémoglobine"));
        assert!(prompt.contains("result value"));
        assert!(prompt.contains("not specified"));
        assert!(prompt.contains("Answer with a number"));
        // Should NOT contain document wrapping
        assert!(!prompt.contains("<document>"));
    }

    #[test]
    fn drill_prompt_vision_mode_date_field() {
        let field = LAB_RESULTS.field("collection_date").unwrap();
        let prompt = LAB_RESULTS.drill_prompt_for(InputMode::Vision, "Hématies", field, "");
        assert!(prompt.contains("Answer with a date"));
    }

    #[test]
    fn drill_prompt_vision_mode_enum_field() {
        let field = LAB_RESULTS.field("abnormal_flag").unwrap();
        let prompt = LAB_RESULTS.drill_prompt_for(InputMode::Vision, "Hémoglobine", field, "");
        assert!(prompt.contains("Choose from:"));
        assert!(prompt.contains("normal"));
        assert!(prompt.contains("critical_high"));
    }

    // ── 09-CAE: Domain filtering tests ──────────────────

    #[test]
    fn domains_for_lab_report() {
        let domains = domains_for_document_type(UserDocumentType::LabReport);
        assert_eq!(domains.len(), 2);
        assert_eq!(domains[0], DocumentDomain::LabResults);
        assert_eq!(domains[1], DocumentDomain::Diagnoses);
    }

    #[test]
    fn domains_for_prescription() {
        let domains = domains_for_document_type(UserDocumentType::Prescription);
        assert_eq!(domains.len(), 3);
        assert_eq!(domains[0], DocumentDomain::Medications);
        assert_eq!(domains[1], DocumentDomain::Instructions);
        assert_eq!(domains[2], DocumentDomain::Diagnoses);
    }

    #[test]
    fn domains_for_medical_image() {
        let domains = domains_for_document_type(UserDocumentType::MedicalImage);
        assert!(domains.is_empty());
    }

    // ── 09-CAE: Category context tests ──────────────────

    #[test]
    fn category_context_lab_report() {
        let ctx = category_context(UserDocumentType::LabReport);
        assert_eq!(ctx, "This is a laboratory analysis report.");
    }

    #[test]
    fn category_context_prescription() {
        let ctx = category_context(UserDocumentType::Prescription);
        assert_eq!(ctx, "This is a medical prescription.");
    }

    #[test]
    fn category_context_medical_image_empty() {
        let ctx = category_context(UserDocumentType::MedicalImage);
        assert!(ctx.is_empty());
    }

    // ── 10-LDC: PromptLocale lookup tests ───────────────────

    #[test]
    fn locale_lab_results_en() {
        let locale = locale_for_domain("lab_results", "en");
        assert_eq!(locale.lang, "en");
        assert_eq!(locale.item_label, "analyte");
        assert!(locale.vision_enumerate.contains("analytes"));
    }

    #[test]
    fn locale_lab_results_fr() {
        let locale = locale_for_domain("lab_results", "fr");
        assert_eq!(locale.lang, "fr");
        assert_eq!(locale.item_label, "analyse");
        assert!(locale.vision_enumerate.contains("analyses biologiques"));
    }

    #[test]
    fn locale_lab_results_de() {
        let locale = locale_for_domain("lab_results", "de");
        assert_eq!(locale.lang, "de");
        assert_eq!(locale.item_label, "Laborparameter");
        assert!(locale.vision_enumerate.contains("Laborparameter"));
    }

    #[test]
    fn locale_medications_en() {
        let locale = locale_for_domain("medications", "en");
        assert_eq!(locale.lang, "en");
        assert!(locale.vision_enumerate.contains("prescribed medications"));
        assert!(locale.vision_drill_range.is_empty());
    }

    #[test]
    fn locale_medications_fr() {
        let locale = locale_for_domain("medications", "fr");
        assert_eq!(locale.lang, "fr");
        assert!(locale.vision_enumerate.contains("médicaments prescrits"));
        assert!(locale.vision_drill_value.contains("posologie"));
    }

    #[test]
    fn locale_medications_de() {
        let locale = locale_for_domain("medications", "de");
        assert_eq!(locale.lang, "de");
        assert!(locale.vision_enumerate.contains("Medikamente"));
    }

    #[test]
    fn locale_unknown_lang_falls_back_to_en() {
        let locale = locale_for_domain("lab_results", "ja");
        assert_eq!(locale.lang, "en");
        assert_eq!(locale.none_keyword, "NONE");
    }

    #[test]
    fn locale_unknown_domain_uses_generic() {
        let locale = locale_for_domain("allergies", "fr");
        assert_eq!(locale.lang, "fr");
        assert_eq!(locale.none_keyword, "AUCUN");
    }

    #[test]
    fn locale_enumerate_contains_none_keyword_en() {
        let locale = locale_for_domain("lab_results", "en");
        assert!(locale.vision_enumerate.contains(locale.none_keyword));
    }

    #[test]
    fn locale_enumerate_contains_none_keyword_fr() {
        let locale = locale_for_domain("lab_results", "fr");
        assert!(locale.vision_enumerate.contains(locale.none_keyword));
    }

    #[test]
    fn locale_enumerate_contains_none_keyword_de() {
        let locale = locale_for_domain("lab_results", "de");
        assert!(locale.vision_enumerate.contains(locale.none_keyword));
    }

    #[test]
    fn locale_system_prompt_no_monolithic_en() {
        let locale = locale_for_domain("lab_results", "en");
        assert!(!locale.system_prompt.contains("Extract ALL"));
        assert!(!locale.system_prompt.contains("Markdown"));
        // 11-SRP: structured prompts use think/answer tokens
        assert!(locale.system_prompt.contains("<think>"));
        assert!(locale.system_prompt.contains("<answer>"));
    }

    #[test]
    fn locale_system_prompt_no_monolithic_fr() {
        let locale = locale_for_domain("lab_results", "fr");
        assert!(!locale.system_prompt.contains("Extract ALL"));
        assert!(!locale.system_prompt.contains("Markdown"));
        // 11-SRP: structured prompts use think/answer tokens
        assert!(locale.system_prompt.contains("<think>"));
        assert!(locale.system_prompt.contains("<answer>"));
    }

    #[test]
    fn locale_system_prompt_no_monolithic_de() {
        let locale = locale_for_domain("lab_results", "de");
        assert!(!locale.system_prompt.contains("Extract ALL"));
        assert!(!locale.system_prompt.contains("Markdown"));
        // 11-SRP: structured prompts use think/answer tokens
        assert!(locale.system_prompt.contains("<think>"));
        assert!(locale.system_prompt.contains("<answer>"));
    }

    #[test]
    fn locale_diagnoses_fr() {
        let locale = locale_for_domain("diagnoses", "fr");
        assert_eq!(locale.lang, "fr");
        assert!(locale.vision_enumerate.contains("diagnostics médicaux"));
    }

    #[test]
    fn locale_drill_value_has_item_placeholder() {
        for domain in &["lab_results", "medications", "diagnoses"] {
            for lang in &["en", "fr", "de"] {
                let locale = locale_for_domain(domain, lang);
                assert!(
                    locale.vision_drill_value.contains("{item}"),
                    "{domain}/{lang} drill_value missing {{item}} placeholder"
                );
            }
        }
    }

    // ── 11-SRP Brick 2: Answer token tests ──────────────

    #[test]
    fn locale_has_answer_tokens() {
        for domain in &["lab_results", "medications", "diagnoses", "allergies"] {
            for lang in &["en", "fr", "de"] {
                let locale = locale_for_domain(domain, lang);
                assert!(
                    !locale.answer_start.is_empty(),
                    "{domain}/{lang} missing answer_start"
                );
                assert!(
                    !locale.answer_end.is_empty(),
                    "{domain}/{lang} missing answer_end"
                );
            }
        }
    }

    #[test]
    fn answer_tokens_structural_check() {
        let locale = locale_for_domain("lab_results", "en");
        assert!(locale.answer_start.contains('<'), "answer_start must be an XML-like tag");
        assert!(locale.answer_end.contains("</"), "answer_end must be a closing XML-like tag");
    }

    #[test]
    fn answer_tokens_consistent_across_locales() {
        // All locales must use the same answer tokens (language-independent delimiters)
        let reference = locale_for_domain("lab_results", "en");
        for domain in &["lab_results", "medications", "diagnoses", "allergies"] {
            for lang in &["en", "fr", "de"] {
                let locale = locale_for_domain(domain, lang);
                assert_eq!(
                    locale.answer_start, reference.answer_start,
                    "{domain}/{lang} answer_start differs from reference"
                );
                assert_eq!(
                    locale.answer_end, reference.answer_end,
                    "{domain}/{lang} answer_end differs from reference"
                );
            }
        }
    }

    // ── 11-SRP Brick 3: Structured system prompt tests ────

    #[test]
    fn system_prompt_contains_think_answer_all_locales() {
        for domain in &["lab_results", "medications", "diagnoses", "allergies"] {
            for lang in &["en", "fr", "de"] {
                let locale = locale_for_domain(domain, lang);
                assert!(
                    locale.system_prompt.contains("<think>"),
                    "{domain}/{lang} system_prompt missing <think> token"
                );
                assert!(
                    locale.system_prompt.contains("<answer>"),
                    "{domain}/{lang} system_prompt missing <answer> token"
                );
                assert!(
                    locale.system_prompt.contains("</think>"),
                    "{domain}/{lang} system_prompt missing </think> closing"
                );
                assert!(
                    locale.system_prompt.contains("</answer>"),
                    "{domain}/{lang} system_prompt missing </answer> closing"
                );
            }
        }
    }

    #[test]
    fn system_prompt_contains_example_all_locales() {
        // Each locale must have a domain-appropriate example
        let en_example = ["Example", "Beispiel", "Exemple"];
        for domain in &["lab_results", "medications", "diagnoses", "allergies"] {
            for lang in &["en", "fr", "de"] {
                let locale = locale_for_domain(domain, lang);
                let has_example = en_example.iter().any(|kw| locale.system_prompt.contains(kw));
                assert!(
                    has_example,
                    "{domain}/{lang} system_prompt missing example section"
                );
            }
        }
    }

    #[test]
    fn enumerate_prompt_none_has_answer_token_all_locales() {
        // NONE instruction in enumerate prompts must be wrapped in answer tokens
        for domain in &["lab_results", "medications", "diagnoses", "allergies"] {
            for lang in &["en", "fr", "de"] {
                let locale = locale_for_domain(domain, lang);
                let expected = format!(
                    "<answer>{}</answer>",
                    locale.none_keyword
                );
                assert!(
                    locale.vision_enumerate.contains(&expected),
                    "{domain}/{lang} vision_enumerate NONE not wrapped in answer tokens: expected '{expected}'"
                );
            }
        }
    }

    #[test]
    fn system_prompt_under_400_tokens_estimate() {
        // Proxy: system prompt should be under 2000 chars (~400 tokens for 4B model)
        for domain in &["lab_results", "medications", "diagnoses", "allergies"] {
            for lang in &["en", "fr", "de"] {
                let locale = locale_for_domain(domain, lang);
                assert!(
                    locale.system_prompt.len() < 2000,
                    "{domain}/{lang} system_prompt too long: {} chars",
                    locale.system_prompt.len()
                );
            }
        }
    }

    #[test]
    fn system_prompt_no_numbered_steps() {
        // 11-SRP: No numbered reasoning steps — examples teach, not rules
        for domain in &["lab_results", "medications", "diagnoses", "allergies"] {
            for lang in &["en", "fr", "de"] {
                let locale = locale_for_domain(domain, lang);
                assert!(
                    !locale.system_prompt.contains("1."),
                    "{domain}/{lang} system_prompt contains numbered steps"
                );
                assert!(
                    !locale.system_prompt.contains("Step 1"),
                    "{domain}/{lang} system_prompt contains 'Step 1'"
                );
            }
        }
    }

    #[test]
    fn system_prompt_has_three_rules() {
        // Each locale should have exactly 3 rules (minimal, not overwhelming for 4B model)
        for domain in &["lab_results", "medications", "diagnoses", "allergies"] {
            for lang in &["en", "fr", "de"] {
                let locale = locale_for_domain(domain, lang);
                let rule_count = locale.system_prompt.matches("\n- ").count();
                assert_eq!(
                    rule_count, 3,
                    "{domain}/{lang} system_prompt has {rule_count} rules, expected 3"
                );
            }
        }
    }
}

//! BTL-10 C2: Entity connection and processing log types.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::DatabaseError;

// ---------------------------------------------------------------------------
// Enums (using inline str_enum pattern from models/enums.rs)
// ---------------------------------------------------------------------------

macro_rules! str_enum {
    ($name:ident { $($variant:ident => $s:literal),+ $(,)? }) => {
        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
        pub enum $name {
            $($variant),+
        }

        impl $name {
            pub fn as_str(&self) -> &'static str {
                match self {
                    $(Self::$variant => $s),+
                }
            }
        }

        impl std::str::FromStr for $name {
            type Err = DatabaseError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                match s {
                    $($s => Ok(Self::$variant)),+,
                    _ => Err(DatabaseError::InvalidEnum {
                        field: stringify!($name).into(),
                        value: s.into(),
                    }),
                }
            }
        }
    };
}

str_enum!(EntityType {
    Medication => "Medication",
    LabResult => "LabResult",
    Diagnosis => "Diagnosis",
    Allergy => "Allergy",
    Procedure => "Procedure",
    Referral => "Referral",
});

str_enum!(RelationshipType {
    PrescribedFor => "PrescribedFor",
    EvidencesFor => "EvidencesFor",
    MonitorsFor => "MonitorsFor",
    ContraindicatedBy => "ContraindicatedBy",
    FollowUpTo => "FollowUpTo",
    ReplacedBy => "ReplacedBy",
});

str_enum!(ProcessingStage {
    Extraction => "extraction",
    Structuring => "structuring",
});

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A semantic connection between two extracted entities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityConnection {
    pub id: Uuid,
    pub source_type: EntityType,
    pub source_id: Uuid,
    pub target_type: EntityType,
    pub target_id: Uuid,
    pub relationship_type: RelationshipType,
    pub confidence: f64,
    pub document_id: Uuid,
    pub created_at: String,
}

/// A processing log entry tracking which model processed a document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingLogEntry {
    pub id: Uuid,
    pub document_id: Uuid,
    pub model_name: String,
    pub model_variant: Option<String>,
    pub processing_stage: ProcessingStage,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub success: bool,
    pub error_message: Option<String>,
}

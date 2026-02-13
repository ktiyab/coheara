//! M0-01: Medication endpoints.
//!
//! Two endpoints:
//! - `GET /api/medications` — list with filters
//! - `GET /api/medications/:id` — full detail

use axum::extract::{Path, Query, State};
use axum::Extension;
use axum::Json;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::api::error::ApiError;
use crate::api::types::{ApiContext, DeviceContext};
use crate::medications;

#[derive(Deserialize)]
pub struct MedListQuery {
    pub status: Option<String>,
    pub prescriber_id: Option<String>,
    pub search: Option<String>,
    pub otc_only: Option<bool>,
}

#[derive(Serialize)]
pub struct MedicationsResponse {
    pub profile_name: String,
    pub medications: Vec<medications::MedicationCard>,
    pub total_active: u32,
    pub total_paused: u32,
    pub total_stopped: u32,
    pub prescribers: Vec<medications::PrescriberOption>,
    pub last_updated: String,
}

/// `GET /api/medications` — medication list for mobile.
pub async fn list(
    State(ctx): State<ApiContext>,
    Extension(_device): Extension<DeviceContext>,
    Query(query): Query<MedListQuery>,
) -> Result<Json<MedicationsResponse>, ApiError> {
    let profile_name = {
        let guard = ctx.core.read_session()?;
        let session = guard.as_ref().ok_or(ApiError::NoActiveProfile)?;
        session.profile_name.clone()
    };

    let conn = ctx.core.open_db()?;

    let filter = medications::MedicationListFilter {
        status: query.status,
        prescriber_id: query.prescriber_id,
        search_query: query.search,
        include_otc: query.otc_only.unwrap_or(false),
    };

    let cards = medications::fetch_medications_filtered(&conn, &filter).map_err(ApiError::from)?;
    let meds = medications::enrich_medication_cards(&conn, cards);
    let (total_active, total_paused, total_stopped) =
        medications::fetch_medication_status_counts(&conn).map_err(ApiError::from)?;
    let prescribers =
        medications::fetch_prescriber_options(&conn).map_err(ApiError::from)?;

    ctx.core.update_activity();

    Ok(Json(MedicationsResponse {
        profile_name,
        medications: meds,
        total_active,
        total_paused,
        total_stopped,
        prescribers,
        last_updated: chrono::Utc::now().to_rfc3339(),
    }))
}

#[derive(Serialize)]
pub struct MedicationDetailResponse {
    pub profile_name: String,
    pub detail: medications::MedicationDetail,
}

/// `GET /api/medications/:id` — full medication detail.
pub async fn detail(
    State(ctx): State<ApiContext>,
    Extension(_device): Extension<DeviceContext>,
    Path(medication_id): Path<String>,
) -> Result<Json<MedicationDetailResponse>, ApiError> {
    let profile_name = {
        let guard = ctx.core.read_session()?;
        let session = guard.as_ref().ok_or(ApiError::NoActiveProfile)?;
        session.profile_name.clone()
    };

    let conn = ctx.core.open_db()?;
    let med_uuid = Uuid::parse_str(&medication_id)
        .map_err(|e| ApiError::BadRequest(format!("Invalid medication ID: {e}")))?;

    let card = medications::fetch_single_medication_card(&conn, &med_uuid)
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::NotFound("Medication not found".into()))?;

    let instructions =
        medications::fetch_medication_instructions(&conn, &med_uuid).map_err(ApiError::from)?;
    let compound_ingredients = if card.is_compound {
        medications::fetch_compound_ingredients(&conn, &med_uuid).map_err(ApiError::from)?
    } else {
        Vec::new()
    };
    let tapering_steps = if card.has_tapering {
        medications::fetch_tapering_steps(&conn, &med_uuid).map_err(ApiError::from)?
    } else {
        Vec::new()
    };
    let aliases =
        medications::fetch_medication_aliases(&conn, &card.generic_name)
            .map_err(ApiError::from)?;
    let dose_changes =
        medications::fetch_dose_history(&conn, &med_uuid).map_err(ApiError::from)?;
    let (document_title, document_date) =
        medications::fetch_source_document(&conn, &med_uuid);

    ctx.core.update_activity();

    Ok(Json(MedicationDetailResponse {
        profile_name,
        detail: medications::MedicationDetail {
            medication: card,
            instructions,
            compound_ingredients,
            tapering_steps,
            aliases,
            dose_changes,
            document_title,
            document_date,
        },
    }))
}

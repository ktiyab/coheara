//! M0-01: Journal endpoints.
//!
//! Two endpoints:
//! - `POST /api/journal/record` — record a symptom entry
//! - `GET /api/journal/history` — recent symptom history

use axum::extract::{Query, State};
use axum::Extension;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::api::error::ApiError;
use crate::api::types::{ApiContext, DeviceContext};
use crate::journal;

/// `POST /api/journal/record` — record a new symptom.
pub async fn record(
    State(ctx): State<ApiContext>,
    Extension(device): Extension<DeviceContext>,
    Json(entry): Json<journal::SymptomEntry>,
) -> Result<Json<journal::RecordResult>, ApiError> {
    // MP-01: Write guard — read-only devices cannot record symptoms
    if !device.can_write() {
        return Err(ApiError::Forbidden);
    }

    // Validate
    if entry.severity < 1 || entry.severity > 5 {
        return Err(ApiError::BadRequest("Severity must be between 1 and 5".into()));
    }
    if entry.category.trim().is_empty() {
        return Err(ApiError::BadRequest("Category is required".into()));
    }
    if entry.specific.trim().is_empty() {
        return Err(ApiError::BadRequest("Specific symptom is required".into()));
    }
    if chrono::NaiveDate::parse_from_str(&entry.onset_date, "%Y-%m-%d").is_err() {
        return Err(ApiError::BadRequest("Invalid onset date format (expected YYYY-MM-DD)".into()));
    }

    let conn = ctx.resolve_db(&device)?;

    let symptom_id = journal::record_symptom(&conn, &entry).map_err(ApiError::from)?;
    let correlations =
        journal::detect_temporal_correlation(&conn, &entry.onset_date).map_err(ApiError::from)?;

    ctx.core.update_activity();

    Ok(Json(journal::RecordResult {
        symptom_id: symptom_id.to_string(),
        correlations,
    }))
}

#[derive(Deserialize)]
pub struct JournalHistoryQuery {
    pub days: Option<u32>,
    pub category: Option<String>,
    pub severity_min: Option<u8>,
}

#[derive(Serialize)]
pub struct JournalHistoryResponse {
    pub profile_name: String,
    pub symptoms: Vec<journal::StoredSymptom>,
}

/// `GET /api/journal/history` — recent symptom history.
pub async fn history(
    State(ctx): State<ApiContext>,
    Extension(device): Extension<DeviceContext>,
    Query(query): Query<JournalHistoryQuery>,
) -> Result<Json<JournalHistoryResponse>, ApiError> {
    let profile_name = {
        let guard = ctx.core.read_session()?;
        let session = guard.as_ref().ok_or(ApiError::NoActiveProfile)?;
        session.profile_name.clone()
    };
    let conn = ctx.resolve_db(&device)?;

    let filter = if query.days.is_some() || query.category.is_some() || query.severity_min.is_some()
    {
        // Build date range from days
        let date_from = query.days.map(|days| {
            (chrono::Local::now() - chrono::Duration::days(i64::from(days)))
                .format("%Y-%m-%d")
                .to_string()
        });

        Some(journal::SymptomFilter {
            category: query.category,
            severity_min: query.severity_min,
            severity_max: None,
            date_from,
            date_to: None,
            still_active: None,
        })
    } else {
        None
    };

    let symptoms =
        journal::fetch_symptoms_filtered(&conn, &filter).map_err(ApiError::from)?;

    ctx.core.update_activity();

    Ok(Json(JournalHistoryResponse { profile_name, symptoms }))
}

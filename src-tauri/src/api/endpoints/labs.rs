//! M0-01: Lab results endpoint.
//!
//! `GET /api/labs/recent` — recent lab results with reference ranges.
//!
//! Note: There is no dedicated labs module in the desktop app.
//! Lab results are queried directly from the `lab_results` table.

use axum::extract::{Query, State};
use axum::Extension;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::api::error::ApiError;
use crate::api::types::{ApiContext, DeviceContext};

#[derive(Deserialize)]
pub struct LabsQuery {
    pub limit: Option<u32>,
}

#[derive(Serialize)]
pub struct LabsResponse {
    pub profile_name: String,
    pub results: Vec<LabResultView>,
    pub last_updated: String,
}

#[derive(Serialize)]
pub struct LabResultView {
    pub id: String,
    pub test_name: String,
    pub value: Option<String>,
    pub unit: Option<String>,
    pub reference_range: Option<String>,
    pub abnormal_flag: String,
    pub collection_date: String,
    pub is_outside_range: bool,
}

/// `GET /api/labs/recent` — recent lab results for mobile.
pub async fn recent(
    State(ctx): State<ApiContext>,
    Extension(_device): Extension<DeviceContext>,
    Query(query): Query<LabsQuery>,
) -> Result<Json<LabsResponse>, ApiError> {
    let profile_name = {
        let guard = ctx.core.read_session()?;
        let session = guard.as_ref().ok_or(ApiError::NoActiveProfile)?;
        session.profile_name.clone()
    };

    let conn = ctx.core.open_db()?;
    let limit = query.limit.unwrap_or(20).min(100);

    let mut stmt = conn
        .prepare(
            "SELECT id, test_name, value, value_text, unit,
                    reference_range_low, reference_range_high,
                    abnormal_flag, collection_date
             FROM lab_results
             ORDER BY collection_date DESC
             LIMIT ?1",
        )
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let results = stmt
        .query_map(rusqlite::params![limit], |row| {
            let value: Option<f64> = row.get(2)?;
            let value_text: Option<String> = row.get(3)?;
            let unit: Option<String> = row.get(4)?;
            let range_low: Option<f64> = row.get(5)?;
            let range_high: Option<f64> = row.get(6)?;
            let abnormal_flag: String = row.get(7)?;

            let display_value = value
                .map(|v| v.to_string())
                .or(value_text);

            let reference_range = match (range_low, range_high) {
                (Some(low), Some(high)) => Some(format!("{low} - {high}")),
                (Some(low), None) => Some(format!(">= {low}")),
                (None, Some(high)) => Some(format!("<= {high}")),
                _ => None,
            };

            let is_outside_range = abnormal_flag != "normal";

            Ok(LabResultView {
                id: row.get(0)?,
                test_name: row.get(1)?,
                value: display_value,
                unit,
                reference_range,
                abnormal_flag,
                collection_date: row.get(8)?,
                is_outside_range,
            })
        })
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .filter_map(|r| r.ok())
        .collect::<Vec<_>>();

    ctx.core.update_activity();

    Ok(Json(LabsResponse {
        profile_name,
        results,
        last_updated: chrono::Utc::now().to_rfc3339(),
    }))
}

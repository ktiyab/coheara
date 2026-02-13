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
    pub trend_direction: Option<String>,
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
            "SELECT lr.id, lr.test_name, lr.value, lr.value_text, lr.unit,
                    lr.reference_range_low, lr.reference_range_high,
                    lr.abnormal_flag, lr.collection_date,
                    (SELECT prev.value FROM lab_results prev
                     WHERE prev.test_name = lr.test_name
                       AND prev.collection_date < lr.collection_date
                       AND prev.value IS NOT NULL
                     ORDER BY prev.collection_date DESC
                     LIMIT 1) AS prev_value
             FROM lab_results lr
             ORDER BY lr.collection_date DESC
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
            let prev_value: Option<f64> = row.get(9)?;

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

            // Compute trend direction with 1% tolerance (RS-M0-01-004)
            let trend_direction = match (value, prev_value) {
                (Some(curr), Some(prev)) => {
                    let threshold = prev.abs() * 0.01;
                    if (curr - prev).abs() <= threshold {
                        Some("stable".to_string())
                    } else if curr > prev {
                        Some("up".to_string())
                    } else {
                        Some("down".to_string())
                    }
                }
                _ => None,
            };

            Ok(LabResultView {
                id: row.get(0)?,
                test_name: row.get(1)?,
                value: display_value,
                unit,
                reference_range,
                abnormal_flag,
                collection_date: row.get(8)?,
                is_outside_range,
                trend_direction,
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

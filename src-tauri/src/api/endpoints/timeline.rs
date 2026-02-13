//! M0-01: Timeline endpoint.
//!
//! `GET /api/timeline/recent` — simplified timeline for mobile.

use axum::extract::{Query, State};
use axum::Extension;
use axum::Json;
use serde::Deserialize;

use crate::api::error::ApiError;
use crate::api::types::{ApiContext, DeviceContext};
use crate::timeline;

#[derive(Deserialize)]
pub struct TimelineQuery {
    pub limit: Option<u32>,
}

/// `GET /api/timeline/recent` — timeline events for mobile.
pub async fn recent(
    State(ctx): State<ApiContext>,
    Extension(_device): Extension<DeviceContext>,
    Query(query): Query<TimelineQuery>,
) -> Result<Json<timeline::TimelineData>, ApiError> {
    let conn = ctx.core.open_db()?;

    let filter = timeline::TimelineFilter {
        date_from: None,
        date_to: None,
        event_types: None,
        professional_id: None,
        since_appointment_id: None,
    };

    let mut data = timeline::get_timeline_data(&conn, &filter).map_err(ApiError::from)?;

    // Trim events to limit
    if let Some(limit) = query.limit {
        data.events.truncate(limit as usize);
    }

    ctx.core.update_activity();

    Ok(Json(data))
}

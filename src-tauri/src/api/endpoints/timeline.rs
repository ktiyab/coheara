//! M0-01: Timeline endpoint.
//!
//! `GET /api/timeline/recent` — simplified timeline for mobile.

use axum::extract::{Query, State};
use axum::Extension;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::api::error::ApiError;
use crate::api::types::{ApiContext, DeviceContext};
use crate::timeline;

#[derive(Deserialize)]
pub struct TimelineQuery {
    pub limit: Option<u32>,
}

#[derive(Serialize)]
pub struct TimelineResponse {
    pub profile_name: String,
    #[serde(flatten)]
    pub data: timeline::TimelineData,
}

/// `GET /api/timeline/recent` — timeline events for mobile.
pub async fn recent(
    State(ctx): State<ApiContext>,
    Extension(_device): Extension<DeviceContext>,
    Query(query): Query<TimelineQuery>,
) -> Result<Json<TimelineResponse>, ApiError> {
    let profile_name = {
        let guard = ctx.core.read_session()?;
        let session = guard.as_ref().ok_or(ApiError::NoActiveProfile)?;
        session.profile_name.clone()
    };
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

    Ok(Json(TimelineResponse { profile_name, data }))
}

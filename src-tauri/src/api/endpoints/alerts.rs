//! M0-01: Critical alerts endpoint.
//!
//! `GET /api/alerts/critical` — active critical alerts for mobile.

use axum::extract::State;
use axum::Extension;
use axum::Json;
use serde::Serialize;

use crate::api::error::ApiError;
use crate::api::types::{ApiContext, DeviceContext};
use crate::trust;

#[derive(Serialize)]
pub struct AlertsResponse {
    pub profile_name: String,
    pub alerts: Vec<trust::CriticalLabAlert>,
}

/// `GET /api/alerts/critical` — critical alerts requiring patient awareness.
pub async fn critical(
    State(ctx): State<ApiContext>,
    Extension(device): Extension<DeviceContext>,
) -> Result<Json<AlertsResponse>, ApiError> {
    let profile_name = {
        let guard = ctx.core.read_session()?;
        let session = guard.as_ref().ok_or(ApiError::NoActiveProfile)?;
        session.profile_name.clone()
    };
    let conn = ctx.resolve_db(&device)?;

    let alerts = trust::fetch_critical_alerts(&conn)
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    ctx.core.update_activity();

    Ok(Json(AlertsResponse { profile_name, alerts }))
}

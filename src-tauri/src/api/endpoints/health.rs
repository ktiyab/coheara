//! M0-01: Health check endpoint.

use axum::extract::State;
use axum::Json;
use serde::Serialize;

use crate::api::error::ApiError;
use crate::api::types::ApiContext;

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub profile_active: bool,
    pub version: &'static str,
}

/// `GET /api/health` — connection check for mobile client.
pub async fn check(
    State(ctx): State<ApiContext>,
) -> Result<Json<HealthResponse>, ApiError> {
    let profile_active = !ctx.core.is_locked();

    Ok(Json(HealthResponse {
        status: "ok",
        profile_active,
        version: crate::config::APP_VERSION,
    }))
}

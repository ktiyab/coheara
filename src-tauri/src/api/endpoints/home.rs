//! M0-01: Home dashboard endpoint.
//!
//! Assembles data from multiple sources into a single mobile-optimized
//! response: profile stats, critical alerts, next appointment, recent journal.

use axum::extract::State;
use axum::Extension;
use axum::Json;
use serde::Serialize;

use crate::api::error::ApiError;
use crate::api::types::{ApiContext, DeviceContext};
use crate::home;
use crate::trust;

#[derive(Serialize)]
pub struct HomeResponse {
    pub profile_name: String,
    pub stats: home::ProfileStats,
    pub recent_documents: Vec<home::DocumentCard>,
    pub onboarding: home::OnboardingProgress,
    pub critical_alerts: Vec<AlertSummary>,
    pub last_sync: String,
}

#[derive(Serialize)]
pub struct AlertSummary {
    pub id: String,
    pub test_name: String,
    pub value: String,
    pub severity: String,
}

/// `GET /api/home` — dashboard data for mobile home screen.
pub async fn dashboard(
    State(ctx): State<ApiContext>,
    Extension(device): Extension<DeviceContext>,
) -> Result<Json<HomeResponse>, ApiError> {
    let profile_name = {
        let guard = ctx.core.read_session()?;
        let session = guard.as_ref().ok_or(ApiError::NoActiveProfile)?;
        session.profile_name.clone()
    };

    let conn = ctx.resolve_db(&device)?;

    let stats = home::fetch_profile_stats(&conn).map_err(ApiError::from)?;
    let recent_documents = home::fetch_recent_documents(&conn, 20, 0).map_err(ApiError::from)?;
    let onboarding = home::compute_onboarding(&conn).map_err(ApiError::from)?;

    // Critical alerts from trust module
    let critical_alerts = trust::fetch_critical_alerts(&conn)
        .unwrap_or_default()
        .into_iter()
        .map(|a| AlertSummary {
            id: a.id.clone(),
            test_name: a.test_name,
            value: format!("{} {}", a.value, a.unit),
            severity: "critical".to_string(),
        })
        .collect();

    ctx.core.update_activity();

    Ok(Json(HomeResponse {
        profile_name,
        stats,
        recent_documents,
        onboarding,
        critical_alerts,
        last_sync: chrono::Utc::now().to_rfc3339(),
    }))
}

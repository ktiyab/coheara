//! M0-04: Delta sync endpoint.
//!
//! `POST /api/sync` — version-based delta synchronization between desktop and phone.
//!
//! Phone sends its known version counters (+ optional journal entries).
//! Desktop compares versions, returns only changed entity types.
//! Returns 204 No Content if nothing changed and no journal entries.

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Extension;
use axum::Json;

use crate::api::error::ApiError;
use crate::api::types::{ApiContext, DeviceContext};
use crate::sync;

/// `POST /api/sync` — delta sync between desktop and phone.
///
/// Request body: `SyncRequest` with version counters and optional journal entries.
/// Response: `SyncResponse` with changed entities, or 204 if nothing changed.
pub async fn delta(
    State(ctx): State<ApiContext>,
    Extension(device): Extension<DeviceContext>,
    Json(request): Json<sync::SyncRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let profile_name = {
        let guard = ctx.core.read_session()?;
        let session = guard.as_ref().ok_or(ApiError::NoActiveProfile)?;
        session.profile_name.clone()
    };

    let conn = ctx.resolve_db(&device)?;

    // Log sync access
    ctx.core.log_access(
        crate::core_state::AccessSource::MobileDevice {
            device_id: device.device_id.clone(),
            profile_id: Some(device.target_profile_id.to_string()),
        },
        "sync_request",
        &format!(
            "versions:meds={},labs={},timeline={},alerts={},appts={},profile={} journal_entries:{}",
            request.versions.medications,
            request.versions.labs,
            request.versions.timeline,
            request.versions.alerts,
            request.versions.appointments,
            request.versions.profile,
            request.journal_entries.len(),
        ),
    );

    let response = sync::build_sync_response(&conn, &request, &profile_name)
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    ctx.core.update_activity();

    match response {
        None => Ok(StatusCode::NO_CONTENT.into_response()),
        Some(resp) => {
            // Log what was sent
            let mut sent_types = Vec::new();
            if resp.medications.is_some() {
                sent_types.push("medications");
            }
            if resp.labs.is_some() {
                sent_types.push("labs");
            }
            if resp.timeline.is_some() {
                sent_types.push("timeline");
            }
            if resp.alerts.is_some() {
                sent_types.push("alerts");
            }
            if resp.appointment.is_some() {
                sent_types.push("appointments");
            }
            if resp.profile.is_some() {
                sent_types.push("profile");
            }

            ctx.core.log_access(
                crate::core_state::AccessSource::MobileDevice {
                    device_id: device.device_id,
                    profile_id: Some(device.target_profile_id.to_string()),
                },
                "sync_respond",
                &format!("entities_sent:[{}]", sent_types.join(",")),
            );

            Ok(Json(resp).into_response())
        }
    }
}

//! M0-01: Audit logging middleware.
//!
//! Logs every API request with device_id, method, path, and
//! response status. Runs innermost (after auth has injected DeviceContext).

use axum::http::Request;
use axum::middleware::Next;
use axum::response::Response;

use crate::api::types::{ApiContext, DeviceContext};
use crate::core_state::AccessSource;

/// Log API access for audit trail.
/// Accesses `ApiContext` from request extensions.
pub async fn log_access(
    req: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let method = req.method().to_string();
    let path = req.uri().path().to_string();

    let ctx = req.extensions().get::<ApiContext>().cloned();

    // Extract device context if available (set by auth middleware)
    // E8: Include target_profile_id for audit trail enrichment
    let source = req
        .extensions()
        .get::<DeviceContext>()
        .map(|d| AccessSource::MobileDevice {
            device_id: d.device_id.clone(),
            profile_id: Some(d.target_profile_id.to_string()),
        })
        .unwrap_or(AccessSource::DesktopUi);

    let response = next.run(req).await;

    if let Some(ctx) = ctx {
        let status = response.status().as_u16();
        ctx.core
            .log_access(source, &format!("{method} {path}"), &format!("status:{status}"));
    }

    response
}

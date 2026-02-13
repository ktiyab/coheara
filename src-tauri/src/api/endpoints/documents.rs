//! M0-01: Document upload endpoint (stub).
//!
//! `POST /api/documents/upload` — upload a document from mobile.
//!
//! Full implementation deferred to M0-03 (WebSocket sync).
//! For now, returns a "not yet available" response.

use axum::extract::State;
use axum::Extension;
use axum::Json;
use serde::Serialize;

use crate::api::error::ApiError;
use crate::api::types::{ApiContext, DeviceContext};

#[derive(Serialize)]
pub struct UploadResponse {
    pub status: &'static str,
    pub message: &'static str,
}

/// `POST /api/documents/upload` — placeholder for mobile document upload.
pub async fn upload(
    State(_ctx): State<ApiContext>,
    Extension(_device): Extension<DeviceContext>,
) -> Result<Json<UploadResponse>, ApiError> {
    // Document upload via API is deferred to M0-03 WebSocket sync.
    // For now, use the WiFi Transfer feature on the desktop.
    Err(ApiError::BadRequest(
        "Document upload via API not yet available. Use WiFi Transfer on desktop.".into(),
    ))
}

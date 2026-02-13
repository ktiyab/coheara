//! M0-01: Appointment endpoints.
//!
//! Two endpoints:
//! - `GET /api/appointments` — list upcoming appointments
//! - `GET /api/appointments/:id/prep` — appointment prep document

use axum::extract::{Path, State};
use axum::Extension;
use axum::Json;
use serde::Serialize;

use crate::api::error::ApiError;
use crate::api::types::{ApiContext, DeviceContext};
use crate::appointment;

#[derive(Serialize)]
pub struct AppointmentsResponse {
    pub appointments: Vec<appointment::StoredAppointment>,
}

/// `GET /api/appointments` — list appointments.
pub async fn list(
    State(ctx): State<ApiContext>,
    Extension(_device): Extension<DeviceContext>,
) -> Result<Json<AppointmentsResponse>, ApiError> {
    let conn = ctx.core.open_db()?;
    let appointments = appointment::list_appointments(&conn).map_err(ApiError::from)?;

    ctx.core.update_activity();

    Ok(Json(AppointmentsResponse { appointments }))
}

#[derive(Serialize)]
pub struct PrepResponse {
    pub prep: appointment::AppointmentPrep,
}

/// `GET /api/appointments/:id/prep` — get appointment prep.
pub async fn prep(
    State(ctx): State<ApiContext>,
    Extension(_device): Extension<DeviceContext>,
    Path(appointment_id): Path<String>,
) -> Result<Json<PrepResponse>, ApiError> {
    let conn = ctx.core.open_db()?;

    // Look up the appointment to get professional_id and date
    let (professional_id, date_str): (String, String) = conn
        .query_row(
            "SELECT professional_id, date FROM appointments WHERE id = ?1",
            rusqlite::params![appointment_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|_| ApiError::NotFound("Appointment not found".into()))?;

    let date = chrono::NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
        .map_err(|_| ApiError::Internal("Invalid appointment date in database".into()))?;

    let prep = appointment::prepare_appointment_prep(
        &conn,
        &professional_id,
        date,
        &appointment_id,
    )
    .map_err(ApiError::from)?;

    ctx.core.update_activity();

    Ok(Json(PrepResponse { prep }))
}

use rusqlite::{params, Connection};

use crate::db::DatabaseError;
use crate::models::*;

pub fn insert_referral(conn: &Connection, referral: &Referral) -> Result<(), DatabaseError> {
    conn.execute(
        "INSERT INTO referrals (id, referring_professional_id, referred_to_professional_id,
         reason, date, status, document_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            referral.id.to_string(),
            referral.referring_professional_id.to_string(),
            referral.referred_to_professional_id.to_string(),
            referral.reason,
            referral.date.to_string(),
            referral.status.as_str(),
            referral.document_id.map(|id| id.to_string()),
        ],
    )?;
    Ok(())
}

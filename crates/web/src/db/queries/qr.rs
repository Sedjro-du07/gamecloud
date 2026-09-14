//! QR attendance tokens.
//!
//! Two audit findings are fixed here.
//!
//! **The token is no longer stored in plaintext.** `qr_tokens.token`
//! used to hold the signed JWT verbatim, so anybody who could read the
//! table held working bearer tokens. We store a SHA-256 hash and look
//! up by hash, exactly as `refresh_tokens` already did.
//!
//! **Attendance is claimed per member, not per token.** The old
//! `is_used` flag was set by the first scan, which meant that at a
//! Monday session with twenty members present, one person got the XP
//! and the other nineteen got `410 invalid_qr_token`. A token is now
//! valid until it expires (or hits an optional capacity), and a unique
//! index on `(user_id, qr_token_id)` in `attendance` stops one member
//! claiming it twice.

use chrono::{DateTime, Utc};
use gamecloud_shared::{xp::XpSource, DomainError};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    db::queries::xp::{self, XpGrant},
    error::{WebError, WebResult},
    services::tokens,
};

/// Persist a freshly issued QR token.
///
/// Takes the plaintext only to hash it; the plaintext is never written.
///
/// # Errors
/// Propagates database errors.
pub async fn insert_qr_token(
    pool: &PgPool,
    token: &str,
    event_name: &str,
    event_type: &str,
    xp_value: i32,
    created_by: Uuid,
    expires_at: DateTime<Utc>,
    max_scans: Option<i32>,
) -> WebResult<Uuid> {
    let id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO qr_tokens (token_hash, event_name, event_type, xp_value,
                               created_by, expires_at, max_scans)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        RETURNING id
        "#,
    )
    .bind(tokens::hash(token))
    .bind(event_name)
    .bind(event_type)
    .bind(xp_value)
    .bind(created_by)
    .bind(expires_at)
    .bind(max_scans)
    .fetch_one(pool)
    .await?;
    Ok(id)
}

/// Information returned to a successful scan.
#[derive(Debug, Clone)]
pub struct ScannedToken {
    /// The matched token row's ID.
    pub id: Uuid,
    /// Event display name.
    pub event_name: String,
    /// Event type discriminator.
    pub event_type: String,
    /// XP actually credited, after multipliers.
    pub xp_awarded: i32,
    /// How many members have now claimed this token.
    pub scan_count: i32,
}

/// Claim a QR token for one member.
///
/// Locks the token row, checks expiry and capacity, records the
/// attendance, and awards the XP through the shared engine — so a scan
/// now moves the member's rank, streak, quests and badges exactly like
/// any other XP event. The pre-audit version inlined its own ledger
/// insert and never recomputed the rank, which is why attendance XP
/// silently failed to promote anyone.
///
/// # Errors
/// `InvalidQrToken` when unknown or expired, `QrAlreadyClaimed` on a
/// repeat scan, `QrCapacityReached` when the token is full.
pub async fn claim_qr_token(
    pool: &PgPool,
    announce_channel: Option<u64>,
    token: &str,
    user_id: Uuid,
) -> WebResult<ScannedToken> {
    let token_hash = tokens::hash(token);
    let mut tx = pool.begin().await?;

    let row: Option<(Uuid, String, String, i32, DateTime<Utc>, Option<i32>, i32)> =
        sqlx::query_as(
            r#"
            SELECT id, event_name, event_type, xp_value, expires_at, max_scans, scan_count
              FROM qr_tokens
             WHERE token_hash = $1
             FOR UPDATE
            "#,
        )
        .bind(&token_hash)
        .fetch_optional(&mut *tx)
        .await?;

    let Some((id, event_name, event_type, xp_value, expires_at, max_scans, scan_count)) = row
    else {
        return Err(WebError::Domain(DomainError::InvalidQrToken));
    };

    if expires_at <= Utc::now() {
        return Err(WebError::Domain(DomainError::InvalidQrToken));
    }
    if let Some(max) = max_scans {
        if scan_count >= max {
            return Err(WebError::Domain(DomainError::QrCapacityReached));
        }
    }

    // The unique index on (user_id, qr_token_id) is the real replay
    // defence; checking first turns the constraint violation into a
    // precise 409 instead of an opaque 500.
    let already: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM attendance WHERE user_id = $1 AND qr_token_id = $2)",
    )
    .bind(user_id)
    .bind(id)
    .fetch_one(&mut *tx)
    .await?;

    if already {
        return Err(WebError::Domain(DomainError::QrAlreadyClaimed));
    }

    sqlx::query(
        r#"
        INSERT INTO attendance (user_id, event_name, event_type, xp_rewarded, qr_token_id)
        VALUES ($1, $2, $3, $4, $5)
        "#,
    )
    .bind(user_id)
    .bind(&event_name)
    .bind(&event_type)
    .bind(xp_value)
    .bind(id)
    .execute(&mut *tx)
    .await?;

    let scan_count: i32 = sqlx::query_scalar(
        "UPDATE qr_tokens SET scan_count = scan_count + 1 WHERE id = $1 RETURNING scan_count",
    )
    .bind(id)
    .fetch_one(&mut *tx)
    .await?;

    let outcome = xp::grant_in_tx(
        &mut tx,
        announce_channel,
        &XpGrant::new(user_id, xp_value, XpSource::Qr)
            .describe(&format!("Présence : {event_name}")),
    )
    .await?;

    tx.commit().await?;

    Ok(ScannedToken {
        id,
        event_name,
        event_type,
        xp_awarded: outcome.awarded,
        scan_count,
    })
}

/// An event's attendance sheet.
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct AttendanceRow {
    /// Member id.
    pub user_id: Uuid,
    /// Display name.
    pub display_name: String,
    /// When they scanned.
    pub scanned_at: DateTime<Utc>,
    /// XP they were credited.
    pub xp_rewarded: i32,
}

/// Who claimed a given token.
///
/// # Errors
/// Propagates database errors.
pub async fn attendance_for_token(
    pool: &PgPool,
    token_id: Uuid,
) -> WebResult<Vec<AttendanceRow>> {
    let rows = sqlx::query_as::<_, AttendanceRow>(
        r#"
        SELECT a.user_id,
               COALESCE(u.current_title, u.discord_id) AS display_name,
               a.scanned_at,
               a.xp_rewarded
          FROM attendance a
          JOIN users u ON u.id = a.user_id
         WHERE a.qr_token_id = $1
         ORDER BY a.scanned_at ASC
        "#,
    )
    .bind(token_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// A member's own attendance history.
///
/// # Errors
/// Propagates database errors.
pub async fn history_for_user(pool: &PgPool, user_id: Uuid) -> WebResult<Vec<AttendanceRow>> {
    let rows = sqlx::query_as::<_, AttendanceRow>(
        r#"
        SELECT a.user_id,
               a.event_name AS display_name,
               a.scanned_at,
               a.xp_rewarded
          FROM attendance a
         WHERE a.user_id = $1
         ORDER BY a.scanned_at DESC
         LIMIT 50
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

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
    config::DiscordChannels,
    db::queries::xp::{self, XpGrant},
    error::{WebError, WebResult},
    services::{
        notifications::{self, Announcement},
        tokens,
    },
};

/// A QR token about to be persisted.
///
/// Grouped into a struct rather than eight positional arguments, so a
/// caller cannot silently transpose `event_name` and `event_type`.
#[derive(Debug, Clone)]
pub struct NewQrToken<'a> {
    /// The signed JWT. Hashed on the way in; never stored in clear.
    pub token: &'a str,
    /// Event display name.
    pub event_name: &'a str,
    /// `Session`, `OfficeHours`, `StandUp`, `GameJam` or `Special`.
    pub event_type: &'a str,
    /// XP each attendee earns.
    pub xp_value: i32,
    /// Member who minted it.
    pub created_by: Uuid,
    /// When it stops being claimable.
    pub expires_at: DateTime<Utc>,
    /// Optional ceiling on how many members may claim it.
    pub max_scans: Option<i32>,
    /// Calendar event this code admits to, when there is one.
    ///
    /// Optional because a code can still be minted for something that
    /// was never scheduled — an impromptu stand-up — but when it is set
    /// the attendance it produces becomes answerable to the calendar
    /// instead of being a loose string.
    pub event_id: Option<Uuid>,
}

/// Persist a freshly issued QR token.
///
/// Takes the plaintext only to hash it; the plaintext is never written.
///
/// # Errors
/// Propagates database errors.
pub async fn insert_qr_token(pool: &PgPool, new: &NewQrToken<'_>) -> WebResult<Uuid> {
    let id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO qr_tokens (token_hash, event_name, event_type, xp_value,
                               created_by, expires_at, max_scans, event_id)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        RETURNING id
        "#,
    )
    .bind(tokens::hash(new.token))
    .bind(new.event_name)
    .bind(new.event_type)
    .bind(new.xp_value)
    .bind(new.created_by)
    .bind(new.expires_at)
    .bind(new.max_scans)
    .bind(new.event_id)
    .fetch_one(pool)
    .await?;
    Ok(id)
}

/// The locked `qr_tokens` row a scan operates on.
#[derive(Debug, sqlx::FromRow)]
struct TokenRow {
    id: Uuid,
    event_id: Option<Uuid>,
    event_name: String,
    event_type: String,
    xp_value: i32,
    expires_at: DateTime<Utc>,
    max_scans: Option<i32>,
    scan_count: i32,
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

/// Post a recorded attendance in the attendance channel.
///
/// As it happens, so organisers can watch the room fill up without
/// refreshing the sheet. In the scan's transaction: a scan that rolls back
/// is never announced.
async fn announce_attendance(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    channels: DiscordChannels,
    user_id: Uuid,
    event_name: &str,
    xp: i32,
) -> WebResult<()> {
    let display: String = sqlx::query_scalar(
        "SELECT member_display_name(current_title, discord_global_name, \
                                    discord_username, discord_id) \
           FROM users WHERE id = $1",
    )
    .bind(user_id)
    .fetch_one(&mut **tx)
    .await?;
    notifications::enqueue(
        tx,
        channels,
        &Announcement::attendance(user_id, &display, event_name, xp),
    )
    .await
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
    channels: DiscordChannels,
    token: &str,
    user_id: Uuid,
) -> WebResult<ScannedToken> {
    let token_hash = tokens::hash(token);
    let mut tx = pool.begin().await?;

    let row: Option<TokenRow> = sqlx::query_as(
        r#"
        SELECT id, event_id, event_name, event_type, xp_value, expires_at,
               max_scans, scan_count
          FROM qr_tokens
         WHERE token_hash = $1
         FOR UPDATE
        "#,
    )
    .bind(&token_hash)
    .fetch_optional(&mut *tx)
    .await?;

    let Some(TokenRow {
        id,
        event_id,
        event_name,
        event_type,
        xp_value,
        expires_at,
        max_scans,
        scan_count,
    }) = row
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

    // Two unique indexes are the real replay defence — one on
    // (user_id, qr_token_id), one on (user_id, event_id). Checking
    // first turns a constraint violation into a precise 409 instead of
    // an opaque 500. The event check matters on its own: an organiser
    // who reprints the code mints a *second* token for the same event,
    // and without it the same member could be paid twice for one
    // session.
    let already: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM attendance
             WHERE user_id = $1
               AND (qr_token_id = $2
                 OR ($3::uuid IS NOT NULL AND event_id = $3))
        )
        "#,
    )
    .bind(user_id)
    .bind(id)
    .bind(event_id)
    .fetch_one(&mut *tx)
    .await?;

    if already {
        return Err(WebError::Domain(DomainError::QrAlreadyClaimed));
    }

    sqlx::query(
        r#"
        INSERT INTO attendance (user_id, event_name, event_type, xp_rewarded,
                                qr_token_id, event_id)
        VALUES ($1, $2, $3, $4, $5, $6)
        "#,
    )
    .bind(user_id)
    .bind(&event_name)
    .bind(&event_type)
    .bind(xp_value)
    .bind(id)
    .bind(event_id)
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
        channels,
        &XpGrant::new(user_id, xp_value, XpSource::Qr)
            .describe(&format!("Présence : {event_name}")),
    )
    .await?;

    announce_attendance(&mut tx, channels, user_id, &event_name, outcome.awarded).await?;

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
               member_display_name(u.current_title, u.discord_global_name, u.discord_username, u.discord_id) AS display_name,
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

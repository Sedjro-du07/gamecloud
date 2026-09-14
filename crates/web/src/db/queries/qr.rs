//! QR token queries.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{WebError, WebResult};

/// Persist a freshly issued QR token.
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
) -> WebResult<Uuid> {
    let id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO qr_tokens (token, event_name, event_type, xp_value, created_by, expires_at)
        VALUES ($1, $2, $3, $4, $5, $6)
        RETURNING id
        "#,
    )
    .bind(token)
    .bind(event_name)
    .bind(event_type)
    .bind(xp_value)
    .bind(created_by)
    .bind(expires_at)
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
    /// XP awarded by this scan.
    pub xp_value: i32,
}

/// Atomically claim a QR token: lock the row, verify it is still
/// valid, mark it `is_used = true`, record an `attendance` row, and
/// grant the XP.
///
/// Returns `WebError::Domain(InvalidQrToken)` if the token is unknown,
/// expired, or already consumed.
///
/// The XP grant step calls into `db::queries::xp::grant_xp` *inside*
/// the same transaction by performing the equivalent SQL inline; we do
/// this rather than nesting calls so the whole scan is one atomic
/// commit.
///
/// # Errors
/// `WebError::Domain(InvalidQrToken)` for token issues; otherwise
/// propagates database errors.
pub async fn claim_qr_token(
    pool: &PgPool,
    token: &str,
    user_id: Uuid,
) -> WebResult<ScannedToken> {
    let mut tx = pool.begin().await?;

    let row: Option<(Uuid, String, String, i32, DateTime<Utc>, bool)> = sqlx::query_as(
        r#"
        SELECT id, event_name, event_type, xp_value, expires_at, is_used
        FROM qr_tokens
        WHERE token = $1
        FOR UPDATE
        "#,
    )
    .bind(token)
    .fetch_optional(&mut *tx)
    .await?;

    let Some((id, event_name, event_type, xp_value, expires_at, is_used)) = row else {
        return Err(WebError::Domain(
            gamecloud_shared::DomainError::InvalidQrToken,
        ));
    };

    if is_used || expires_at <= Utc::now() {
        return Err(WebError::Domain(
            gamecloud_shared::DomainError::InvalidQrToken,
        ));
    }

    sqlx::query("UPDATE qr_tokens SET is_used = TRUE WHERE id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;

    sqlx::query(
        r#"
        INSERT INTO attendance (user_id, event_name, event_type, xp_rewarded)
        VALUES ($1, $2, $3, $4)
        "#,
    )
    .bind(user_id)
    .bind(&event_name)
    .bind(&event_type)
    .bind(xp_value)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO xp_logs (user_id, amount, source, description)
        VALUES ($1, $2, 'QR', $3)
        "#,
    )
    .bind(user_id)
    .bind(xp_value)
    .bind(format!("Scanned {event_name}"))
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        r#"
        UPDATE users
            SET xp_total = xp_total + $2::BIGINT,
                last_activity_at = NOW()
            WHERE id = $1
        "#,
    )
    .bind(user_id)
    .bind(i64::from(xp_value))
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(ScannedToken {
        id,
        event_name,
        event_type,
        xp_value,
    })
}

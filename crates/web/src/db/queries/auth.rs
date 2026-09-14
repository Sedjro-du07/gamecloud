//! Authentication-related queries: email OTPs and refresh tokens.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{WebError, WebResult};

// ===========================================================================
// Email OTPs
// ===========================================================================

/// Persist a new OTP for the given user, replacing any existing one.
///
/// # Errors
/// Propagates database errors.
pub async fn upsert_otp(
    pool: &PgPool,
    user_id: Uuid,
    email: &str,
    code_hash: &str,
    expires_at: DateTime<Utc>,
) -> WebResult<()> {
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM email_otps WHERE user_id = $1")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        r#"
        INSERT INTO email_otps (user_id, email, code_hash, expires_at)
        VALUES ($1, $2, $3, $4)
        "#,
    )
    .bind(user_id)
    .bind(email)
    .bind(code_hash)
    .bind(expires_at)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

/// Outstanding OTP for a user, if any.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct OtpRow {
    /// Stored Argon2 hash.
    pub code_hash: String,
    /// Expiration time.
    pub expires_at: DateTime<Utc>,
    /// Number of failed attempts so far.
    pub attempts: i32,
    /// Email being verified.
    pub email: String,
}

/// Fetch the outstanding OTP for a user.
///
/// # Errors
/// Propagates database errors.
pub async fn fetch_otp(pool: &PgPool, user_id: Uuid) -> WebResult<Option<OtpRow>> {
    let row = sqlx::query_as::<_, OtpRow>(
        "SELECT code_hash, expires_at, attempts, email FROM email_otps WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Increment the attempts counter for the user's outstanding OTP.
///
/// # Errors
/// Propagates database errors.
pub async fn increment_otp_attempts(pool: &PgPool, user_id: Uuid) -> WebResult<()> {
    sqlx::query("UPDATE email_otps SET attempts = attempts + 1 WHERE user_id = $1")
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Promote a user from `Pending` (or `EmailSubmitted`) to `Visitor`,
/// in a single transaction with the OTP deletion.
///
/// # Errors
/// Propagates database errors.
pub async fn finalize_email_verification(
    pool: &PgPool,
    user_id: Uuid,
    email: &str,
) -> WebResult<()> {
    let mut tx = pool.begin().await?;
    sqlx::query(
        r#"
        UPDATE users
            SET email = $2,
                email_verified = TRUE,
                global_rank = CASE
                    WHEN global_rank = 'Pending' THEN 'Visitor'
                    ELSE global_rank
                END
            WHERE id = $1
        "#,
    )
    .bind(user_id)
    .bind(email)
    .execute(&mut *tx)
    .await?;

    sqlx::query("DELETE FROM email_otps WHERE user_id = $1")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(())
}

// ===========================================================================
// Refresh tokens
// ===========================================================================

/// Insert a new refresh token row.
///
/// # Errors
/// Propagates database errors.
pub async fn insert_refresh_token(
    pool: &PgPool,
    user_id: Uuid,
    token_hash: &str,
    expires_at: DateTime<Utc>,
    user_agent: Option<&str>,
    ip_address: Option<&str>,
) -> WebResult<()> {
    sqlx::query(
        r#"
        INSERT INTO refresh_tokens (user_id, token_hash, expires_at, user_agent, ip_address)
        VALUES ($1, $2, $3, $4, $5::INET)
        "#,
    )
    .bind(user_id)
    .bind(token_hash)
    .bind(expires_at)
    .bind(user_agent)
    .bind(ip_address)
    .execute(pool)
    .await?;
    Ok(())
}

/// Lookup result for a refresh token.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct RefreshTokenLookup {
    /// Owning user.
    pub user_id: Uuid,
    /// Whether the row has been revoked already.
    pub revoked: bool,
    /// Expiration time.
    pub expires_at: DateTime<Utc>,
}

/// Lookup a refresh token by its hash.
///
/// # Errors
/// Returns `Unauthorized` if the token is unknown.
pub async fn lookup_refresh_token(
    pool: &PgPool,
    token_hash: &str,
) -> WebResult<RefreshTokenLookup> {
    sqlx::query_as::<_, RefreshTokenLookup>(
        "SELECT user_id, revoked, expires_at FROM refresh_tokens WHERE token_hash = $1",
    )
    .bind(token_hash)
    .fetch_optional(pool)
    .await?
    .ok_or(WebError::Unauthorized)
}

/// Mark a refresh token as revoked.
///
/// # Errors
/// Propagates database errors.
pub async fn revoke_refresh_token(pool: &PgPool, token_hash: &str) -> WebResult<()> {
    sqlx::query("UPDATE refresh_tokens SET revoked = TRUE WHERE token_hash = $1")
        .bind(token_hash)
        .execute(pool)
        .await?;
    Ok(())
}

/// Revoke every refresh token for a user. Used as the response to a
/// detected token reuse (security event).
///
/// # Errors
/// Propagates database errors.
pub async fn revoke_all_for_user(pool: &PgPool, user_id: Uuid) -> WebResult<()> {
    sqlx::query("UPDATE refresh_tokens SET revoked = TRUE WHERE user_id = $1 AND revoked = FALSE")
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}

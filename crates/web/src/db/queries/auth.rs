//! Authentication-related queries: refresh tokens and session revocation.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{WebError, WebResult};

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

/// Invalidate every access token already issued to a member.
///
/// Access tokens are stateless, so the only way to retire one before its
/// `exp` is to record when revocation happened and refuse anything older.
/// [`crate::middleware::auth::CurrentUser`] enforces it on every request
/// using the member row it already loads.
///
/// # Errors
/// Propagates database errors.
pub async fn revoke_sessions(pool: &PgPool, user_id: Uuid) -> WebResult<()> {
    sqlx::query("UPDATE users SET sessions_valid_from = NOW() WHERE id = $1")
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}

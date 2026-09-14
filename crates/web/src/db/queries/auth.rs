//! Authentication-related queries: email OTPs and refresh tokens.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{WebError, WebResult};

// ===========================================================================
// Email OTPs
// ===========================================================================

/// Persist a new OTP for the given user.
///
/// **The failed-attempt counter survives the replacement.** The
/// pre-audit version deleted the row and inserted a fresh one with
/// `attempts = 0`, so the five-attempt lockout could be reset at will
/// by re-requesting a code — making a six-digit secret guessable in
/// batches of five. `cumulative_attempts` is what the handler actually
/// enforces against, and it only ever grows.
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
    sqlx::query(
        r#"
        INSERT INTO email_otps (user_id, email, code_hash, expires_at,
                                attempts, cumulative_attempts, resend_count, last_sent_at)
        VALUES ($1, $2, $3, $4, 0, 0, 0, NOW())
        ON CONFLICT (user_id) DO UPDATE
            SET email               = EXCLUDED.email,
                code_hash           = EXCLUDED.code_hash,
                expires_at          = EXCLUDED.expires_at,
                attempts            = 0,
                cumulative_attempts = email_otps.cumulative_attempts,
                resend_count        = email_otps.resend_count + 1,
                last_sent_at        = NOW()
        "#,
    )
    .bind(user_id)
    .bind(email)
    .bind(code_hash)
    .bind(expires_at)
    .execute(pool)
    .await?;
    Ok(())
}

/// Outstanding OTP for a user, if any.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct OtpRow {
    /// Stored Argon2 hash.
    pub code_hash: String,
    /// Expiration time.
    pub expires_at: DateTime<Utc>,
    /// Failed attempts against the *current* code.
    pub attempts: i32,
    /// Failed attempts across every code ever issued to this user.
    /// This is the number the lockout is enforced against.
    pub cumulative_attempts: i32,
    /// How many times a new code has been requested.
    pub resend_count: i32,
    /// When the current code was sent.
    pub last_sent_at: DateTime<Utc>,
    /// Email being verified.
    pub email: String,
}

/// Fetch the outstanding OTP for a user.
///
/// # Errors
/// Propagates database errors.
pub async fn fetch_otp(pool: &PgPool, user_id: Uuid) -> WebResult<Option<OtpRow>> {
    let row = sqlx::query_as::<_, OtpRow>(
        r#"
        SELECT code_hash, expires_at, attempts, cumulative_attempts,
               resend_count, last_sent_at, email
          FROM email_otps
         WHERE user_id = $1
        "#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Increment both attempt counters for the user's outstanding OTP.
///
/// # Errors
/// Propagates database errors.
pub async fn increment_otp_attempts(pool: &PgPool, user_id: Uuid) -> WebResult<()> {
    sqlx::query(
        r#"
        UPDATE email_otps
           SET attempts            = attempts + 1,
               cumulative_attempts = cumulative_attempts + 1
         WHERE user_id = $1
        "#,
    )
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Promote a user from `Pending` (or `EmailSubmitted`) to `Visitor`,
/// in a single transaction with the OTP deletion.
///
/// `users.email` is `CITEXT UNIQUE`, so an address already verified by
/// somebody else would surface as an opaque 500. We check first and
/// return a precise `EmailAlreadyTaken` conflict instead.
///
/// # Errors
/// `EmailAlreadyTaken` when another account holds the address;
/// otherwise propagates database errors.
pub async fn finalize_email_verification(
    pool: &PgPool,
    user_id: Uuid,
    email: &str,
) -> WebResult<()> {
    let mut tx = pool.begin().await?;

    let taken: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM users WHERE email = $1 AND id <> $2)",
    )
    .bind(email)
    .bind(user_id)
    .fetch_one(&mut *tx)
    .await?;

    if taken {
        return Err(WebError::Domain(
            gamecloud_shared::DomainError::EmailAlreadyTaken,
        ));
    }

    // A member whose tracks were already set — the Bureau is enrolled in
    // every track ahead of time — has nothing left to onboard, and joining
    // a track is otherwise the only step onto the XP ladder. They go
    // straight to the title their XP is worth instead of stalling at
    // Visitor.
    let xp_total: i64 = sqlx::query_scalar("SELECT xp_total FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_one(&mut *tx)
        .await?;
    let reached = gamecloud_shared::roles::GlobalRank::from_xp(xp_total);

    sqlx::query(
        r#"
        UPDATE users
            SET email = $2,
                email_verified = TRUE,
                global_rank = CASE
                    WHEN global_rank = 'Pending'
                     AND EXISTS (SELECT 1 FROM track_memberships m
                                  WHERE m.user_id = users.id AND m.left_at IS NULL)
                        THEN $3
                    WHEN global_rank = 'Pending' THEN 'Visitor'
                    ELSE global_rank
                END
            WHERE id = $1
        "#,
    )
    .bind(user_id)
    .bind(email)
    .bind(reached.as_str())
    .execute(&mut *tx)
    .await?;
    crate::services::notifications::request_role_sync(&mut *tx).await?;

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

/// Whether another account already holds this verified address.
///
/// Checked before an OTP is sent, so the endpoint cannot be used to
/// mail a code to an address the caller has no claim on.
///
/// # Errors
/// Propagates database errors.
pub async fn email_is_taken(pool: &PgPool, email: &str, excluding: Uuid) -> WebResult<bool> {
    let taken: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM users WHERE email = $1 AND id <> $2)",
    )
    .bind(email)
    .bind(excluding)
    .fetch_one(pool)
    .await?;
    Ok(taken)
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

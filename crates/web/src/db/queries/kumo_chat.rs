//! Conversations with Kumo held on the platform.
//!
//! The platform only stores what is written; the bot relays it to Kumo
//! and files the answer back (see the bot's `events::kumo`). No account
//! is needed: a conversation is found again by the hash of a cookie
//! token, or through the member when they are signed in.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::WebResult;

/// Longest message somebody may write.
pub const MAX_BODY_CHARS: usize = 1000;
/// Messages one conversation may send in ten minutes.
pub const MAX_RECENT_MESSAGES: i64 = 8;
/// Messages waiting to be relayed, across every conversation, beyond
/// which new ones are refused. Anybody can write without an account, so
/// this is what stops a flood from reaching Kumo's channel.
pub const MAX_PENDING_TOTAL: i64 = 100;

/// Check a message before storing it. Returns it trimmed.
///
/// # Errors
/// The reason, in French, to show as is.
pub fn validate_body(body: &str) -> Result<String, String> {
    let body = body.trim();
    if body.is_empty() {
        return Err("écris un message".into());
    }
    if body.chars().count() > MAX_BODY_CHARS {
        return Err(format!("message trop long ({MAX_BODY_CHARS} caractères maximum)"));
    }
    Ok(body.to_string())
}

/// The conversation a cookie token belongs to.
///
/// # Errors
/// Propagates database errors.
pub async fn find_by_token(pool: &PgPool, token_hash: &str) -> WebResult<Option<Uuid>> {
    Ok(sqlx::query_scalar("SELECT id FROM kumo_conversations WHERE token_hash = $1")
        .bind(token_hash)
        .fetch_optional(pool)
        .await?)
}

/// A member's latest conversation, for when they come back on another
/// browser.
///
/// # Errors
/// Propagates database errors.
pub async fn latest_for_user(pool: &PgPool, user_id: Uuid) -> WebResult<Option<Uuid>> {
    Ok(sqlx::query_scalar(
        "SELECT id FROM kumo_conversations WHERE user_id = $1 ORDER BY last_message_at DESC LIMIT 1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?)
}

/// Start a conversation.
///
/// # Errors
/// Propagates database errors.
pub async fn create(pool: &PgPool, token_hash: &str, user_id: Option<Uuid>) -> WebResult<Uuid> {
    Ok(sqlx::query_scalar(
        "INSERT INTO kumo_conversations (token_hash, user_id) VALUES ($1, $2) RETURNING id",
    )
    .bind(token_hash)
    .bind(user_id)
    .fetch_one(pool)
    .await?)
}

/// Tie a conversation to the member who is now signed in, if it was
/// anonymous.
///
/// # Errors
/// Propagates database errors.
pub async fn attach_user(pool: &PgPool, id: Uuid, user_id: Uuid) -> WebResult<()> {
    sqlx::query("UPDATE kumo_conversations SET user_id = $2 WHERE id = $1 AND user_id IS NULL")
        .bind(id)
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Messages a conversation sent in the last ten minutes.
///
/// # Errors
/// Propagates database errors.
pub async fn recent_from_visitor(pool: &PgPool, id: Uuid) -> WebResult<i64> {
    Ok(sqlx::query_scalar(
        "SELECT COUNT(*) FROM kumo_messages \
          WHERE conversation_id = $1 AND NOT from_kumo AND created_at > NOW() - INTERVAL '10 minutes'",
    )
    .bind(id)
    .fetch_one(pool)
    .await?)
}

/// Messages waiting to be relayed, everywhere.
///
/// # Errors
/// Propagates database errors.
pub async fn pending_total(pool: &PgPool) -> WebResult<i64> {
    Ok(sqlx::query_scalar(
        "SELECT COUNT(*) FROM kumo_messages WHERE NOT from_kumo AND relayed_at IS NULL AND NOT relay_failed",
    )
    .fetch_one(pool)
    .await?)
}

/// Store a message for the bot to relay.
///
/// # Errors
/// Propagates database errors.
pub async fn post(pool: &PgPool, id: Uuid, body: &str) -> WebResult<()> {
    let mut tx = pool.begin().await?;
    sqlx::query("INSERT INTO kumo_messages (conversation_id, from_kumo, body) VALUES ($1, FALSE, $2)")
        .bind(id)
        .bind(body)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE kumo_conversations SET last_message_at = NOW() WHERE id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

/// One message of a conversation.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ChatRow {
    /// Kumo's answer, rather than the visitor's message.
    pub from_kumo: bool,
    /// Text.
    pub body: String,
    /// When it was written.
    pub created_at: DateTime<Utc>,
    /// When the bot posted it for Kumo.
    pub relayed_at: Option<DateTime<Utc>>,
    /// Given up after repeated failures.
    pub relay_failed: bool,
    /// For an answer: `None` when Kumo wrote it, otherwise the name of
    /// the Bureau member who answered in Kumo's place.
    pub answered_by: Option<String>,
}

/// The last two hundred messages of a conversation, oldest first.
///
/// # Errors
/// Propagates database errors.
pub async fn messages(pool: &PgPool, id: Uuid) -> WebResult<Vec<ChatRow>> {
    Ok(sqlx::query_as::<_, ChatRow>(
        r#"
        SELECT from_kumo, body, created_at, relayed_at, relay_failed, answered_by FROM (
            SELECT from_kumo, body, created_at, relayed_at, relay_failed, answered_by
              FROM kumo_messages
             WHERE conversation_id = $1
             ORDER BY created_at DESC
             LIMIT 200
        ) latest
        ORDER BY created_at
        "#,
    )
    .bind(id)
    .fetch_all(pool)
    .await?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_message_is_trimmed_and_must_say_something() {
        assert_eq!(validate_body("  Bonjour Kumo  ").as_deref(), Ok("Bonjour Kumo"));
        assert!(validate_body("   ").is_err());
    }

    #[test]
    fn an_overlong_message_is_refused() {
        assert!(validate_body(&"a".repeat(MAX_BODY_CHARS)).is_ok());
        assert!(validate_body(&"a".repeat(MAX_BODY_CHARS + 1)).is_err());
    }
}

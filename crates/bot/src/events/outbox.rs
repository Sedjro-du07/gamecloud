//! `notifications_outbox` poller.
//!
//! The web binary writes rows here when it wants the bot to push a
//! Discord embed. The bot polls every few seconds, claims pending
//! rows, dispatches them, and marks them `Sent` (or increments
//! `attempts` on failure, up to 5).
//!
//! Concurrent bot instances are not expected; if they ever are, the
//! claim query uses `FOR UPDATE SKIP LOCKED` so only one consumer
//! sees each row.

use std::time::Duration;

use serenity::all::{CreateEmbed, CreateMessage, Http};
use sqlx::PgPool;
use uuid::Uuid;

use crate::state::BotState;

/// Spawn the background polling task. Runs forever.
pub fn spawn(state: BotState, http: std::sync::Arc<Http>) {
    let interval = Duration::from_secs(state.config().outbox_poll_seconds.max(1));
    tokio::spawn(async move {
        loop {
            if let Err(e) = process_batch(&state, &http).await {
                tracing::error!(error = ?e, "outbox: batch failed");
            }
            tokio::time::sleep(interval).await;
        }
    });
}

#[derive(sqlx::FromRow)]
struct OutboxRow {
    id: Uuid,
    // Reserved for future kinds that DM the user directly rather than
    // posting in a channel; the column is read by such handlers.
    #[allow(dead_code)]
    user_id: Option<Uuid>,
    kind: String,
    payload: serde_json::Value,
    attempts: i32,
}

async fn process_batch(state: &BotState, http: &Http) -> anyhow::Result<()> {
    // Claim up to 16 rows with SKIP LOCKED. We move them to
    // `attempts++` immediately so a second pass doesn't re-claim them
    // even if the dispatch is slow.
    let rows = claim(state.pool()).await?;
    for row in rows {
        match dispatch(state, http, &row).await {
            Ok(()) => mark_sent(state.pool(), row.id).await?,
            Err(e) => {
                let final_attempt = row.attempts + 1 >= 5;
                mark_failure(state.pool(), row.id, &e.to_string(), final_attempt).await?;
                tracing::warn!(
                    error = %e,
                    id = %row.id,
                    attempts = row.attempts + 1,
                    "outbox: dispatch failed",
                );
            }
        }
    }
    Ok(())
}

async fn claim(pool: &PgPool) -> sqlx::Result<Vec<OutboxRow>> {
    sqlx::query_as::<_, OutboxRow>(
        r#"
        UPDATE notifications_outbox
        SET attempts = attempts + 1
        WHERE id IN (
            SELECT id FROM notifications_outbox
            WHERE status = 'Pending'
            ORDER BY created_at
            FOR UPDATE SKIP LOCKED
            LIMIT 16
        )
        RETURNING id, user_id, kind, payload, attempts
        "#,
    )
    .fetch_all(pool)
    .await
}

async fn mark_sent(pool: &PgPool, id: Uuid) -> sqlx::Result<()> {
    sqlx::query(
        "UPDATE notifications_outbox SET status='Sent', sent_at=NOW() WHERE id=$1",
    )
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

async fn mark_failure(
    pool: &PgPool,
    id: Uuid,
    error: &str,
    is_final: bool,
) -> sqlx::Result<()> {
    let status = if is_final { "Failed" } else { "Pending" };
    sqlx::query(
        r#"
        UPDATE notifications_outbox
        SET status = $2, last_error = $3
        WHERE id = $1
        "#,
    )
    .bind(id)
    .bind(status)
    .bind(error)
    .execute(pool)
    .await?;
    Ok(())
}

/// Convert an outbox row into a Discord message and send it.
async fn dispatch(_state: &BotState, http: &Http, row: &OutboxRow) -> anyhow::Result<()> {
    // Every payload at minimum contains `channel_id` (the destination)
    // and `title` + `description` for the embed. Specific kinds may
    // add fields; we currently only render the generic shape.
    let channel_id: u64 = row
        .payload
        .get("channel_id")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| anyhow::anyhow!("payload missing channel_id"))?;
    let title = row
        .payload
        .get("title")
        .and_then(serde_json::Value::as_str)
        .unwrap_or(&row.kind);
    let description = row
        .payload
        .get("description")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    let color = row
        .payload
        .get("color")
        .and_then(serde_json::Value::as_u64)
        .map_or(0x9c4dff_u32, |c| u32::try_from(c).unwrap_or(0x9c4dff));

    let embed = CreateEmbed::new()
        .title(title)
        .description(description)
        .color(color);
    let msg = CreateMessage::new().embed(embed);
    serenity::all::ChannelId::new(channel_id)
        .send_message(http, msg)
        .await?;
    Ok(())
}

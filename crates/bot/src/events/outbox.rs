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

use gamecloud_shared::roles::Track;
use serde_json::Value;
use serenity::all::{
    CreateAllowedMentions, CreateEmbed, CreateMessage, GuildId, Http, UserId,
};
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
    let mut roles_synced = false;
    for row in rows {
        // The platform changed an office, a track or a rank and wants
        // Discord to follow now rather than at the next periodic pass.
        // One full pass covers every such request in the batch.
        if row.kind == "RoleSync" {
            if !roles_synced {
                super::roles::sync_all(state, http).await;
                roles_synced = true;
            }
            mark_sent(state.pool(), row.id).await?;
            continue;
        }
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
async fn dispatch(state: &BotState, http: &Http, row: &OutboxRow) -> anyhow::Result<()> {
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

    // Anything that concerns one member and nobody else goes to them
    // directly. Telling somebody in a public channel that their project
    // was refused is a different act from telling them.
    if row.payload.get("dm").and_then(Value::as_bool) == Some(true) {
        if deliver_dm(state, http, row, embed).await {
            return Ok(());
        }
        // The DM was refused — closed DMs, or the member left. Falling
        // through to the channel would publish the private body, which
        // is the one outcome this branch exists to prevent. Post a
        // notice carrying no content instead, so they still learn
        // something is waiting for them.
        return notify_without_content(state, http, row, channel_id).await;
    }

    // The web process asks for an *audience*; resolving it to a role id
    // is the bot's job, because only the bot reads the guild. A role
    // that has been renamed or recreated therefore fixes itself, and a
    // role that does not exist costs the ping, not the message.
    let mut msg = CreateMessage::new().embed(embed);
    if let Some(mention) = resolve_mention(state, http, row).await {
        msg = msg
            .content(mention)
            .allowed_mentions(
                CreateAllowedMentions::new()
                    .everyone(true)
                    .all_roles(true)
                    .all_users(true),
            );
    }

    serenity::all::ChannelId::new(channel_id)
        .send_message(http, msg)
        .await?;
    Ok(())
}

/// Turn the payload's `mention` intent into text Discord will ping.
///
/// Returns `None` when nothing should be pinged — which is the case for
/// almost every announcement. Pinging is reserved for things that are
/// actionable and time-bound: a session being scheduled or called off.
/// A rank-up is pleasant news, and a channel that pings for pleasant
/// news is a channel people mute.
async fn resolve_mention(state: &BotState, http: &Http, row: &OutboxRow) -> Option<String> {
    let intent = row.payload.get("mention").and_then(Value::as_str)?;
    match intent {
        // Only worth writing if it will actually ping. Discord renders a
        // forbidden `@everyone` as dead literal text, which reads like a
        // bug to everyone in the channel — better to announce quietly
        // than to announce badly.
        "everyone" => {
            let guild = GuildId::new(state.config().guild_id?);
            let me = http.get_current_user().await.ok()?;
            let member = guild.member(http, me.id).await.ok()?;
            let roles = guild.roles(http).await.ok()?;
            let allowed = member.roles.iter().filter_map(|r| roles.get(r)).any(|r| {
                r.permissions.mention_everyone() || r.permissions.administrator()
            });
            if allowed {
                Some("@everyone".to_string())
            } else {
                tracing::info!(
                    "outbox: bot lacks MENTION_EVERYONE; announcing without the ping"
                );
                None
            }
        }
        // The Bureau role is found by name, the same way track roles
        // are, so a rename on the server fixes itself without a deploy.
        "bureau" => role_mention(state, http, |name| name.contains("Bureau")).await,
        "member" => discord_id_of(state, row).await.map(|id| format!("<@{id}>")),
        "track" => {
            let track = row.payload.get("track").and_then(Value::as_str)?;
            let track = Track::parse(track)?;
            let wanted = super::roles::track_role_name(track);
            role_mention(state, http, |name| name == wanted).await
        }
        _ => None,
    }
}

/// The Discord id of the member an announcement is about.
///
/// The web process stores platform UUIDs; only the bot addresses people
/// on Discord, so the translation happens here rather than being baked
/// into the payload — a member who relinks their Discord account is then
/// reachable again without rewriting queued rows.
async fn discord_id_of(state: &BotState, row: &OutboxRow) -> Option<String> {
    let user_id = row.user_id?;
    sqlx::query_scalar::<_, String>("SELECT discord_id FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(state.pool())
        .await
        .ok()
        .flatten()
}

/// Send an announcement to the member privately.
///
/// Returns whether it was delivered. A closed DM is not a failure worth
/// retrying — the member has chosen not to receive them — so the caller
/// falls back to the channel rather than leaving the row stuck.
async fn deliver_dm(
    state: &BotState,
    http: &Http,
    row: &OutboxRow,
    embed: CreateEmbed,
) -> bool {
    let Some(discord_id) = discord_id_of(state, row).await else {
        return false;
    };
    let Ok(id) = discord_id.parse::<u64>() else {
        return false;
    };

    match UserId::new(id).create_dm_channel(http).await {
        Ok(dm) => match dm.id.send_message(http, CreateMessage::new().embed(embed)).await {
            Ok(_) => true,
            Err(e) => {
                tracing::info!(error = %e, user = %discord_id, "outbox: DM refused");
                false
            }
        },
        Err(e) => {
            tracing::info!(error = %e, user = %discord_id, "outbox: cannot open a DM");
            false
        }
    }
}

/// Tell a member something is waiting, without saying what.
///
/// Used when a private announcement could not be delivered privately.
/// It mentions them in the channel and stops there: the body stays
/// unpublished.
async fn notify_without_content(
    state: &BotState,
    http: &Http,
    row: &OutboxRow,
    channel_id: u64,
) -> anyhow::Result<()> {
    let Some(discord_id) = discord_id_of(state, row).await else {
        // Nobody to tell and nothing safe to say: drop it rather than
        // publish. The row is still marked sent, because retrying will
        // not conjure a Discord account.
        tracing::warn!(kind = %row.kind, "outbox: private announcement had no recipient");
        return Ok(());
    };

    let embed = CreateEmbed::new()
        .title("📬 Une notification vous attend")
        .description(
            "Vos messages privés sont fermés, donc le détail reste sur la plateforme. \
             Ouvrez GameCloud OS pour le lire.",
        )
        .color(0x9a_a0b3);

    serenity::all::ChannelId::new(channel_id)
        .send_message(
            http,
            CreateMessage::new()
                .content(format!("<@{discord_id}>"))
                .embed(embed)
                .allowed_mentions(CreateAllowedMentions::new().all_users(true)),
        )
        .await?;
    Ok(())
}

/// Find a guild role by predicate and render it as a mention.
///
/// Matching by name rather than by a configured id means a role that is
/// renamed or recreated keeps working, and a role that has vanished
/// costs the ping rather than the whole announcement.
async fn role_mention(
    state: &BotState,
    http: &Http,
    matches: impl Fn(&str) -> bool,
) -> Option<String> {
    let guild = GuildId::new(state.config().guild_id?);
    let roles = guild.roles(http).await.ok()?;
    roles
        .values()
        .find(|r| matches(&r.name))
        .map(|r| format!("<@&{}>", r.id))
}

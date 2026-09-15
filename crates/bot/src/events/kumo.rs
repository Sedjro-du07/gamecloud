//! Relaying between people and Kumo.
//!
//! Anybody may reach Kumo, member or not, two ways:
//!
//! - on the platform's chat page, with no account at all: the platform
//!   stores the message and this bot picks it up;
//! - on Discord, by writing to this bot in private.
//!
//! Either way the bot posts the message in the relay channel, where Kumo
//! reads, and remembers who asked. When Kumo answers there, the answer
//! goes back where the question came from: into the platform
//! conversation, or in private on Discord.
//!
//! A Bureau member may answer in Kumo's place: in the relay channel, a
//! reply to a relayed message is delivered the same way, signed with
//! their name. Only a reply to a relayed message counts, so the Bureau can
//! still talk among themselves in the channel without anything leaking.
//!
//! Discord does not let one bot write to another in private, which is
//! why a channel sits in the middle; and somebody who shares no server
//! with this bot cannot write to it at all, which is why the platform
//! chat exists. Both rely on Kumo being willing to answer a bot.

use std::time::Duration;

use serenity::all::{
    ChannelId, Context, CreateAllowedMentions, CreateEmbed, CreateMessage, Embed, Http, Message,
    MessageId, UserId,
};
use sqlx::PgPool;
use uuid::Uuid;

use crate::state::BotState;

/// Longest message relayed, leaving room for the mention in Discord's
/// 2 000-character limit.
const MAX_RELAYED_CHARS: usize = 1800;

/// Longest answer filed into a platform conversation.
const MAX_STORED_ANSWER_CHARS: usize = 4000;

/// How recent a relayed message must be for a reply of Kumo's that does
/// not quote it to still be routed to its author.
const UNQUOTED_REPLY_MINUTES: i32 = 10;

/// How often messages written on the platform are picked up.
const PLATFORM_POLL: Duration = Duration::from_secs(3);

/// Attempts before a platform message is given up on.
const MAX_RELAY_ATTEMPTS: i32 = 5;

/// Who is answering.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Responder {
    /// Kumo itself.
    Kumo,
    /// A Bureau member answering in Kumo's place, by name.
    Bureau(String),
}

impl Responder {
    /// How the answer is signed.
    fn label(&self) -> String {
        match self {
            Self::Kumo => "Kumo".to_string(),
            Self::Bureau(name) => format!("{name} (Bureau)"),
        }
    }
}

/// Where a relayed message came from, and so where the answer goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Requester {
    /// Somebody who wrote to the bot in private.
    Discord(u64),
    /// A conversation on the platform.
    Platform(Uuid),
}

/// Handle a message: relay a private one to Kumo, or an answer back.
pub async fn on_message(ctx: &Context, msg: &Message, state: &BotState) {
    let config = state.config();
    let (Some(kumo), Some(relay)) = (config.kumo_bot_id, config.kumo_relay_channel_id) else {
        return;
    };

    if msg.guild_id.is_none() && !msg.author.bot {
        from_discord(ctx, msg, state, kumo, ChannelId::new(relay)).await;
    } else if msg.channel_id.get() == relay {
        if msg.author.id.get() == kumo {
            back_to_requester(ctx, msg, state, &Responder::Kumo).await;
        } else if !msg.author.bot {
            // Anybody else must hold a Bureau office — on the platform,
            // which decides offices — and must be replying to a relayed
            // message.
            let Some(name) = bureau_member(state.pool(), msg.author.id.get()).await else {
                return;
            };
            if back_to_requester(ctx, msg, state, &Responder::Bureau(name)).await {
                if let Err(e) = msg.react(&ctx.http, '✅').await {
                    tracing::debug!(error = %e, "kumo: could not confirm a Bureau answer");
                }
            }
        }
    }
}

/// The name of a Discord user who holds a Bureau office on the platform.
async fn bureau_member(pool: &PgPool, discord_id: u64) -> Option<String> {
    let (office, name): (Option<String>, String) = sqlx::query_as(
        "SELECT bureau_role, member_display_name(current_title, discord_global_name, \
                                                 discord_username, discord_id) \
           FROM users WHERE discord_id = $1",
    )
    .bind(discord_id.to_string())
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()?;
    office
        .as_deref()
        .and_then(gamecloud_shared::roles::BureauRole::parse)
        .is_some_and(gamecloud_shared::roles::BureauRole::holds_office)
        .then_some(name)
}

/// Start relaying messages written on the platform.
///
/// Does nothing unless the relay is configured.
pub fn spawn(state: BotState, http: std::sync::Arc<Http>) {
    let config = state.config();
    let (Some(kumo), Some(relay)) = (config.kumo_bot_id, config.kumo_relay_channel_id) else {
        tracing::info!("KUMO_BOT_ID or KUMO_RELAY_CHANNEL_ID unset; Kumo relay disabled");
        return;
    };
    tokio::spawn(async move {
        loop {
            if let Err(e) = relay_platform_batch(&state, &http, kumo, ChannelId::new(relay)).await {
                tracing::error!(error = ?e, "kumo: platform relay batch failed");
            }
            tokio::time::sleep(PLATFORM_POLL).await;
        }
    });
}

/// Post a message in the relay channel for Kumo.
async fn post_for_kumo(http: &Http, relay: ChannelId, kumo: u64, text: &str) -> serenity::Result<MessageId> {
    let text: String = text.chars().take(MAX_RELAYED_CHARS).collect();
    relay
        .send_message(
            http,
            CreateMessage::new()
                .content(format!("<@{kumo}> {text}"))
                // Kumo is pinged so it notices; nobody the message names is.
                .allowed_mentions(CreateAllowedMentions::new().users([UserId::new(kumo)])),
        )
        .await
        .map(|posted| posted.id)
}

/// Relay the messages waiting on the platform, oldest first.
///
/// Rows are locked while they are posted, so a second bot instance never
/// relays the same message twice.
async fn relay_platform_batch(state: &BotState, http: &Http, kumo: u64, relay: ChannelId) -> anyhow::Result<()> {
    let mut tx = state.pool().begin().await?;
    let pending: Vec<(Uuid, Uuid, String)> = sqlx::query_as(
        "SELECT id, conversation_id, body FROM kumo_messages \
          WHERE NOT from_kumo AND relayed_at IS NULL AND NOT relay_failed \
          ORDER BY created_at LIMIT 10 FOR UPDATE SKIP LOCKED",
    )
    .fetch_all(&mut *tx)
    .await?;

    for (id, conversation, body) in pending {
        match post_for_kumo(http, relay, kumo, &body).await {
            Ok(posted) => {
                sqlx::query(
                    "UPDATE kumo_messages SET relayed_at = NOW(), relay_attempts = relay_attempts + 1 \
                      WHERE id = $1",
                )
                .bind(id)
                .execute(&mut *tx)
                .await?;
                sqlx::query("INSERT INTO kumo_relays (relay_message_id, conversation_id) VALUES ($1, $2)")
                    .bind(posted.to_string())
                    .bind(conversation)
                    .execute(&mut *tx)
                    .await?;
            }
            Err(e) => {
                tracing::warn!(error = %e, "kumo: platform message not relayed");
                sqlx::query(
                    "UPDATE kumo_messages \
                        SET relay_attempts = relay_attempts + 1, \
                            relay_failed = relay_attempts + 1 >= $2 \
                      WHERE id = $1",
                )
                .bind(id)
                .bind(MAX_RELAY_ATTEMPTS)
                .execute(&mut *tx)
                .await?;
            }
        }
    }

    tx.commit().await?;
    Ok(())
}

/// Relay somebody's private message on Discord.
async fn from_discord(ctx: &Context, msg: &Message, state: &BotState, kumo: u64, relay: ChannelId) {
    let attachments: Vec<&str> = msg.attachments.iter().map(|a| a.url.as_str()).collect();
    let text = with_attachments(msg.content.trim(), &attachments);
    if text.is_empty() {
        return;
    }

    match post_for_kumo(&ctx.http, relay, kumo, &text).await {
        Ok(posted) => {
            let stored = sqlx::query(
                "INSERT INTO kumo_relays (relay_message_id, requester_discord_id) VALUES ($1, $2)",
            )
            .bind(posted.to_string())
            .bind(msg.author.id.to_string())
            .execute(state.pool())
            .await;
            if let Err(e) = stored {
                tracing::warn!(error = %e, "kumo: relay not recorded; the answer cannot come back");
            }
            // A reaction rather than a reply: it confirms without adding a
            // message to every exchange.
            if let Err(e) = msg.react(&ctx.http, '📨').await {
                tracing::debug!(error = %e, "kumo: could not acknowledge");
            }
        }
        Err(e) => {
            tracing::warn!(error = %e, "kumo: could not post in the relay channel");
            let _ = msg
                .channel_id
                .say(&ctx.http, "Je n'arrive pas à joindre Kumo pour l'instant. Réessaie plus tard.")
                .await;
        }
    }
}

/// Who an answer is for.
///
/// Matched by the relayed message it replies to. Kumo's answers that
/// reply to nothing go to the latest question relayed in the last few
/// minutes, since Kumo may not use Discord's replies; a Bureau member's
/// must reply to one, or they are just talking in the channel.
async fn requester_for(pool: &PgPool, msg: &Message, unquoted_ok: bool) -> Option<Requester> {
    let quoted = msg
        .message_reference
        .as_ref()
        .and_then(|r| r.message_id)
        .map(|id| id.to_string());

    let by_quote: Option<(Option<String>, Option<Uuid>)> = match quoted {
        Some(id) => sqlx::query_as(
            "SELECT requester_discord_id, conversation_id FROM kumo_relays WHERE relay_message_id = $1",
        )
        .bind(id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten(),
        None => None,
    };
    let row = match by_quote {
        Some(row) => Some(row),
        None if unquoted_ok => sqlx::query_as(
            "SELECT requester_discord_id, conversation_id FROM kumo_relays \
              WHERE created_at > NOW() - make_interval(mins => $1) \
              ORDER BY created_at DESC LIMIT 1",
        )
        .bind(UNQUOTED_REPLY_MINUTES)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten(),
        None => None,
    };

    match row? {
        (_, Some(conversation)) => Some(Requester::Platform(conversation)),
        (Some(discord), None) => discord.parse().ok().map(Requester::Discord),
        (None, None) => None,
    }
}

/// Send an answer back where the question came from. Returns whether it
/// was delivered.
async fn back_to_requester(ctx: &Context, msg: &Message, state: &BotState, responder: &Responder) -> bool {
    let unquoted_ok = *responder == Responder::Kumo;
    let Some(requester) = requester_for(state.pool(), msg, unquoted_ok).await else {
        return false;
    };
    let me = ctx.cache.current_user().id.get();
    let attachments: Vec<&str> = msg.attachments.iter().map(|a| a.url.as_str()).collect();
    let text = with_attachments(&without_mention(&msg.content, me), &attachments);

    match requester {
        Requester::Platform(conversation) => {
            // The platform shows text only, so embeds are written out.
            let mut body = text;
            for embed in &msg.embeds {
                let written = embed_text(embed);
                if !written.is_empty() {
                    if !body.is_empty() {
                        body.push_str("\n\n");
                    }
                    body.push_str(&written);
                }
            }
            // The platform never shows a Discord id: mentions become pseudos.
            let pseudos: std::collections::HashMap<u64, String> =
                msg.mentions.iter().map(|u| (u.id.get(), u.name.clone())).collect();
            let mut body = gamecloud_shared::mentions::humanize(&body, |id| pseudos.get(&id).cloned());
            if body.trim().is_empty() {
                body = "(réponse sans texte)".to_string();
            }
            let body: String = body.chars().take(MAX_STORED_ANSWER_CHARS).collect();
            let answered_by = match responder {
                Responder::Kumo => None,
                Responder::Bureau(name) => Some(name.as_str()),
            };
            match file_answer(state.pool(), conversation, &body, answered_by).await {
                Ok(()) => true,
                Err(e) => {
                    tracing::warn!(error = %e, "kumo: answer not filed into the conversation");
                    false
                }
            }
        }
        Requester::Discord(user) => {
            let label = responder.label();
            let header = if text.is_empty() {
                format!("💬 **Réponse de {label}**")
            } else {
                format!("💬 **{label}** : {text}")
            };
            let answer = CreateMessage::new()
                .content(header.chars().take(2000).collect::<String>())
                .embeds(msg.embeds.iter().cloned().map(CreateEmbed::from).collect())
                .allowed_mentions(CreateAllowedMentions::new());
            match UserId::new(user).create_dm_channel(&ctx.http).await {
                Ok(dm) => match dm.id.send_message(&ctx.http, answer).await {
                    Ok(_) => true,
                    Err(e) => {
                        tracing::info!(error = %e, user, "kumo: answer refused (closed DMs?)");
                        false
                    }
                },
                Err(e) => {
                    tracing::info!(error = %e, user, "kumo: cannot open a DM");
                    false
                }
            }
        }
    }
}

/// File an answer into a platform conversation.
async fn file_answer(pool: &PgPool, conversation: Uuid, body: &str, answered_by: Option<&str>) -> sqlx::Result<()> {
    let mut tx = pool.begin().await?;
    sqlx::query(
        "INSERT INTO kumo_messages (conversation_id, from_kumo, body, relayed_at, answered_by) \
         VALUES ($1, TRUE, $2, NOW(), $3)",
    )
    .bind(conversation)
    .bind(body)
    .bind(answered_by)
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE kumo_conversations SET last_message_at = NOW() WHERE id = $1")
        .bind(conversation)
        .execute(&mut *tx)
        .await?;
    tx.commit().await
}

/// A message with this bot's mention taken out: it means nothing to the
/// person reading the answer.
fn without_mention(content: &str, bot_id: u64) -> String {
    content
        .replace(&format!("<@{bot_id}>"), "")
        .replace(&format!("<@!{bot_id}>"), "")
        .trim()
        .to_string()
}

/// Text followed by one attachment link per line.
fn with_attachments(text: &str, attachments: &[&str]) -> String {
    let mut out = text.trim().to_string();
    for url in attachments {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(url);
    }
    out
}

/// An embed written out as plain text.
fn embed_text(embed: &Embed) -> String {
    let mut lines = Vec::new();
    if let Some(title) = embed.title.as_deref().filter(|t| !t.trim().is_empty()) {
        lines.push(title.trim().to_string());
    }
    if let Some(description) = embed.description.as_deref().filter(|d| !d.trim().is_empty()) {
        lines.push(description.trim().to_string());
    }
    for field in &embed.fields {
        lines.push(format!("{} : {}", field.name.trim(), field.value.trim()));
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bots_own_mention_is_taken_out_of_an_answer() {
        assert_eq!(without_mention("<@42> Salut !", 42), "Salut !");
        assert_eq!(without_mention("Salut <@!42>", 42), "Salut");
        // Somebody else's mention stays.
        assert_eq!(without_mention("<@7> Salut", 42), "<@7> Salut");
    }

    #[test]
    fn an_answer_is_signed_by_whoever_wrote_it() {
        assert_eq!(Responder::Kumo.label(), "Kumo");
        assert_eq!(Responder::Bureau("Ada".into()).label(), "Ada (Bureau)");
    }

    #[test]
    fn attachments_follow_the_text_one_per_line() {
        assert_eq!(with_attachments("Voici", &["https://a", "https://b"]), "Voici\nhttps://a\nhttps://b");
        assert_eq!(with_attachments("", &["https://a"]), "https://a");
        assert_eq!(with_attachments("  seul  ", &[]), "seul");
    }
}

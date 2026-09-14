//! DraftBot interception.
//!
//! DraftBot announces level-ups in a specific channel. The embed it
//! posts has, in its `description` (or sometimes `title`), text like:
//!
//! > 🎉 Bravo **Joachim** ! Tu viens de passer **niveau 14** !
//!
//! We:
//!
//! 1. Filter on `channel_id == DRAFTBOT_CHANNEL_ID` and
//!    `author.id == DRAFTBOT_USER_ID`.
//! 2. Pull the level number out of the embed.
//! 3. Pull the recipient's Discord user ID out of the message
//!    *mentions* (DraftBot pings the user) — never out of the
//!    nickname, which can be edited.
//! 4. POST to the web binary's `/api/sync/draftbot` endpoint.

use std::sync::LazyLock;

use regex::Regex;
use serenity::all::{Context, Message};

use crate::state::BotState;

/// Match `niveau 14`, `level 14`, `level **14**`, etc. We allow
/// optional Markdown bold around the number.
static LEVEL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:niveau|level|lvl)\s*\*{0,2}(\d{1,4})\*{0,2}").expect("valid regex")
});

/// Inspect a `MessageCreate` and, if it is a DraftBot level-up posted
/// in the configured channel, forward the parsed event to the web
/// binary. Any failure is logged and swallowed — we never want a
/// parse error to crash the bot.
pub async fn on_message_create(ctx: &Context, msg: &Message, state: &BotState) {
    let cfg = state.config();
    if msg.channel_id.get() != cfg.draftbot_channel_id {
        return;
    }
    if msg.author.id.get() != cfg.draftbot_user_id {
        return;
    }

    // The level can appear either in the embed description or in the
    // raw message content; we check both.
    let mut hay = String::new();
    hay.push_str(&msg.content);
    for embed in &msg.embeds {
        if let Some(d) = &embed.description {
            hay.push('\n');
            hay.push_str(d);
        }
        if let Some(t) = &embed.title {
            hay.push('\n');
            hay.push_str(t);
        }
    }

    let Some(level) = LEVEL_RE
        .captures(&hay)
        .and_then(|c| c.get(1))
        .and_then(|m| m.as_str().parse::<i32>().ok())
    else {
        tracing::debug!("draftbot: no level number found in message");
        return;
    };

    // The recipient is identified by the *mention* — the message
    // pings them. We take the first user mention.
    let Some(target) = msg.mentions.first() else {
        tracing::warn!("draftbot: level-up message had no mentions");
        return;
    };

    if let Err(e) = forward(state, target.id.to_string(), level).await {
        tracing::error!(error = ?e, "draftbot: forward failed");
    }

    // Acknowledge with a reaction so it's visible the bot saw it.
    let _ = msg.react(&ctx.http, '✅').await;
}

#[derive(serde::Serialize)]
struct ForwardBody {
    discord_id: String,
    new_level: i32,
}

async fn forward(state: &BotState, discord_id: String, new_level: i32) -> anyhow::Result<()> {
    let cfg = state.config();
    let url = format!("{}/api/sync/draftbot", cfg.gamecloud_sync_url.trim_end_matches('/'));
    state
        .http()
        .post(url)
        .header("X-API-Key", &cfg.draftbot_api_key)
        .json(&ForwardBody {
            discord_id,
            new_level,
        })
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::LEVEL_RE;

    #[test]
    fn level_regex_matches_french() {
        let m = LEVEL_RE.captures("Bravo, tu passes niveau 14 !").unwrap();
        assert_eq!(m.get(1).unwrap().as_str(), "14");
    }

    #[test]
    fn level_regex_matches_bold() {
        let m = LEVEL_RE.captures("Tu passes **niveau 7** !").unwrap();
        assert_eq!(m.get(1).unwrap().as_str(), "7");
    }

    #[test]
    fn level_regex_matches_english() {
        let m = LEVEL_RE.captures("you reached level 21").unwrap();
        assert_eq!(m.get(1).unwrap().as_str(), "21");
    }

    #[test]
    fn level_regex_no_match() {
        assert!(LEVEL_RE.captures("hello world").is_none());
    }
}

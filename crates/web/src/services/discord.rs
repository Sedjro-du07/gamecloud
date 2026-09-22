//! The few calls the web process makes to Discord with the bot's token.
//!
//! Everything else on Discord is the bot's job. These cannot wait for
//! it: whether somebody signing in is on the server decides where the
//! login takes them, an admitted candidate's invitation is shown to them
//! on the page the Bureau just graded them from, and the Bureau's
//! appointment screen has to offer everybody on the server — not only
//! the minority who have already signed in to the platform.

use std::time::Duration;

use crate::config::Config;

const API: &str = "https://discord.com/api/v10";

/// How long a candidate's invitation stays valid: a week.
const INVITE_MAX_AGE_SECONDS: u64 = 7 * 24 * 3600;

/// Whether a Discord user is on the association's server.
///
/// `None` when it cannot be told — no bot token or guild configured, or
/// Discord did not answer. Callers keep what they already knew in that
/// case rather than guessing either way.
pub async fn is_guild_member(config: &Config, discord_id: &str) -> Option<bool> {
    let token = config.discord_bot_token.as_deref()?;
    let guild = config.discord_guild_id?;
    if !discord_id.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }

    let response = reqwest::Client::new()
        .get(format!("{API}/guilds/{guild}/members/{discord_id}"))
        .header("Authorization", format!("Bot {token}"))
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| tracing::warn!(error = %e, "discord: membership check failed"))
        .ok()?;

    match response.status().as_u16() {
        200 => Some(true),
        404 => Some(false),
        status => {
            tracing::warn!(status, "discord: unexpected answer to a membership check");
            None
        }
    }
}

/// One human on the association's Discord server.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct GuildMember {
    /// Snowflake.
    pub id: String,
    /// Globally unique handle.
    pub username: String,
    /// Display name, when the member set one.
    pub global_name: Option<String>,
}

/// Everybody on the server, bots excluded.
///
/// `None` when it cannot be told — no bot token or guild, or Discord did
/// not answer — so the caller can fall back to the accounts it knows
/// rather than presenting an empty list as if the server were empty.
/// One page of a thousand covers the association many times over; the
/// list is not paginated further.
pub async fn list_guild_members(config: &Config) -> Option<Vec<GuildMember>> {
    #[derive(serde::Deserialize)]
    struct Raw {
        user: RawUser,
    }
    #[derive(serde::Deserialize)]
    struct RawUser {
        id: String,
        username: String,
        global_name: Option<String>,
        #[serde(default)]
        bot: bool,
    }

    let token = config.discord_bot_token.as_deref()?;
    let guild = config.discord_guild_id?;

    let response = reqwest::Client::new()
        .get(format!("{API}/guilds/{guild}/members?limit=1000"))
        .header("Authorization", format!("Bot {token}"))
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| tracing::warn!(error = %e, "discord: member list failed"))
        .ok()?;
    if !response.status().is_success() {
        tracing::warn!(status = response.status().as_u16(), "discord: member list refused");
        return None;
    }
    let raw: Vec<Raw> = response
        .json()
        .await
        .map_err(|e| tracing::warn!(error = %e, "discord: member list unreadable"))
        .ok()?;

    Some(
        raw.into_iter()
            .filter(|m| !m.user.bot)
            .map(|m| GuildMember {
                id: m.user.id,
                username: m.user.username,
                global_name: m.user.global_name,
            })
            .collect(),
    )
}

/// Create a single-use invitation to the server, valid for a week.
///
/// `None` when no bot token or invitation channel is configured, or when
/// Discord refused; the caller tells the Bureau to send one by hand.
pub async fn create_invite(config: &Config) -> Option<String> {
    #[derive(serde::Deserialize)]
    struct Invite {
        code: String,
    }

    let token = config.discord_bot_token.as_deref()?;
    let channel = config.discord_invite_channel?;

    let response = reqwest::Client::new()
        .post(format!("{API}/channels/{channel}/invites"))
        .header("Authorization", format!("Bot {token}"))
        .header("X-Audit-Log-Reason", "GameCloud OS: candidat admis")
        .json(&serde_json::json!({
            "max_age": INVITE_MAX_AGE_SECONDS,
            "max_uses": 1,
            "unique": true,
        }))
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| tracing::warn!(error = %e, "discord: invite creation failed"))
        .ok()?;

    if !response.status().is_success() {
        tracing::warn!(status = response.status().as_u16(), "discord: invite refused");
        return None;
    }
    let invite: Invite = response
        .json()
        .await
        .map_err(|e| tracing::warn!(error = %e, "discord: invite answer unreadable"))
        .ok()?;
    Some(format!("https://discord.gg/{}", invite.code))
}

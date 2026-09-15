//! Bot configuration.

use std::env::VarError;

use thiserror::Error;

/// Configuration error.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// A required variable is missing.
    #[error("missing required environment variable: {0}")]
    MissingVar(&'static str),

    /// A variable was set but could not be parsed.
    #[error("invalid value for {var}: {message}")]
    InvalidVar {
        /// Variable name.
        var: &'static str,
        /// Reason.
        message: String,
    },
}

/// Top-level bot configuration.
#[derive(Debug, Clone)]
pub struct Config {
    /// Discord bot token.
    pub discord_token: String,
    /// PostgreSQL DSN (shared with the web binary).
    pub database_url: String,
    /// Discord channel ID where DraftBot posts level-up announcements.
    pub draftbot_channel_id: u64,
    /// Discord user ID of DraftBot (so we only parse messages from
    /// that bot, not from any user).
    pub draftbot_user_id: u64,
    /// Internal HTTP endpoint of the web binary, used to forward the
    /// parsed level-up event.
    pub gamecloud_sync_url: String,
    /// Shared API key sent in `X-API-Key` to that endpoint.
    pub draftbot_api_key: String,
    /// Whether DraftBot level-ups are turned into platform XP. Off while
    /// DraftBot is suspended: the platform is the only XP system.
    pub draftbot_enabled: bool,
    /// Poll interval for the `notifications_outbox`, in seconds.
    pub outbox_poll_seconds: u64,
    /// Guild whose roles mirror platform ranks. When absent, rank-role
    /// synchronisation is disabled and the bot touches nobody's roles.
    pub guild_id: Option<u64>,
    /// How often to reconcile every member's rank role, in seconds.
    pub role_sync_seconds: u64,
    /// Channel holding the live leaderboard the bot keeps up to date.
    pub leaderboard_channel_id: Option<u64>,
    /// Channel holding the guide the bot keeps up to date.
    pub guide_channel_id: Option<u64>,
    /// Channels the guide points members to. Read from the same
    /// variables the web process routes announcements with.
    pub announce_channel_id: Option<u64>,
    /// Newly opened quests.
    pub quests_channel_id: Option<u64>,
    /// Released projects.
    pub hall_channel_id: Option<u64>,
    /// Submissions waiting for validation.
    pub reviews_channel_id: Option<u64>,
    /// Attendance recorded at sessions.
    pub presences_channel_id: Option<u64>,
    /// Files and links members share.
    pub shares_channel_id: Option<u64>,
    /// Validated resources and useful links.
    pub resources_channel_id: Option<u64>,
}

impl Config {
    /// Load configuration from the process environment.
    ///
    /// # Errors
    /// Returns the first variable that is missing or invalid.
    pub fn from_env() -> Result<Self, ConfigError> {
        let _ = dotenvy::dotenv();

        Ok(Self {
            discord_token: required("DISCORD_TOKEN")?,
            database_url: required("DATABASE_URL")?,
            draftbot_channel_id: parse("DRAFTBOT_CHANNEL_ID")?,
            draftbot_user_id: parse("DRAFTBOT_USER_ID")?,
            gamecloud_sync_url: required("GAMECLOUD_SYNC_URL")?,
            draftbot_api_key: required("DRAFTBOT_API_KEY")?,
            draftbot_enabled: parse_or("DRAFTBOT_ENABLED", true)?,
            outbox_poll_seconds: parse_or("OUTBOX_POLL_SECONDS", 5)?,
            guild_id: parse_optional("DISCORD_GUILD_ID")?,
            role_sync_seconds: parse_or("ROLE_SYNC_SECONDS", 900)?,
            leaderboard_channel_id: parse_optional("DISCORD_LEADERBOARD_CHANNEL_ID")?,
            guide_channel_id: parse_optional("DISCORD_GUIDE_CHANNEL_ID")?,
            announce_channel_id: parse_optional("DISCORD_ANNOUNCE_CHANNEL_ID")?,
            quests_channel_id: parse_optional("DISCORD_QUESTS_CHANNEL_ID")?,
            hall_channel_id: parse_optional("DISCORD_HALL_CHANNEL_ID")?,
            reviews_channel_id: parse_optional("DISCORD_REVIEWS_CHANNEL_ID")?,
            presences_channel_id: parse_optional("DISCORD_PRESENCES_CHANNEL_ID")?,
            shares_channel_id: parse_optional("DISCORD_SHARES_CHANNEL_ID")?,
            resources_channel_id: parse_optional("DISCORD_RESOURCES_CHANNEL_ID")?,
        })
    }
}

fn get(key: &'static str) -> Result<String, ConfigError> {
    match std::env::var(key) {
        Ok(v) => Ok(v),
        Err(VarError::NotPresent) => Err(ConfigError::MissingVar(key)),
        Err(VarError::NotUnicode(_)) => Err(ConfigError::InvalidVar {
            var: key,
            message: "value is not valid UTF-8".into(),
        }),
    }
}

fn required(key: &'static str) -> Result<String, ConfigError> {
    let v = get(key)?;
    if v.trim().is_empty() {
        return Err(ConfigError::MissingVar(key));
    }
    Ok(v)
}

fn parse<T>(key: &'static str) -> Result<T, ConfigError>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    required(key)?
        .parse::<T>()
        .map_err(|e| ConfigError::InvalidVar {
            var: key,
            message: e.to_string(),
        })
}

fn parse_or<T>(key: &'static str, default: T) -> Result<T, ConfigError>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    match std::env::var(key) {
        Ok(v) => v.parse().map_err(|e: T::Err| ConfigError::InvalidVar {
            var: key,
            message: e.to_string(),
        }),
        Err(VarError::NotPresent) => Ok(default),
        Err(VarError::NotUnicode(_)) => Err(ConfigError::InvalidVar {
            var: key,
            message: "value is not valid UTF-8".into(),
        }),
    }
}

/// Parse an optional numeric variable. Absent or empty yields `None`;
/// present but malformed is an error rather than a silent default.
fn parse_optional<T>(key: &'static str) -> Result<Option<T>, ConfigError>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    match std::env::var(key) {
        Ok(v) if v.trim().is_empty() => Ok(None),
        Ok(v) => v
            .parse::<T>()
            .map(Some)
            .map_err(|e| ConfigError::InvalidVar {
                var: key,
                message: e.to_string(),
            }),
        Err(VarError::NotPresent) => Ok(None),
        Err(VarError::NotUnicode(_)) => Err(ConfigError::InvalidVar {
            var: key,
            message: "value is not valid UTF-8".into(),
        }),
    }
}

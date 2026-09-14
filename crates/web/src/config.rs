//! Application configuration.
//!
//! All runtime configuration is loaded from environment variables
//! exactly once at startup, validated, and made available through a
//! `Config` value held in the `AppState`. Failing to load any required
//! variable aborts startup — never panic in a handler.

use std::env::VarError;

use gamecloud_shared::roles::Track;
use thiserror::Error;

/// Configuration error.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// A required environment variable is missing.
    #[error("missing required environment variable: {0}")]
    MissingVar(&'static str),

    /// An environment variable was set but could not be parsed.
    #[error("invalid value for {var}: {message}")]
    InvalidVar {
        /// The variable name.
        var: &'static str,
        /// Why parsing failed.
        message: String,
    },
}

/// Top-level configuration.
#[derive(Debug, Clone)]
pub struct Config {
    /// HTTP listen address, e.g. `0.0.0.0:3000`.
    pub bind_addr: String,
    /// Public origin used in OAuth redirects, cookies, and emails.
    pub public_origin: String,
    /// Whether the runtime should treat itself as production
    /// (Secure cookies, no debug pages).
    pub is_production: bool,

    /// PostgreSQL DSN.
    pub database_url: String,

    /// JWT signing secret. Must be ≥ 32 bytes.
    pub jwt_secret: Vec<u8>,
    /// Access token TTL, in seconds.
    pub jwt_access_ttl_seconds: u64,
    /// Refresh token TTL, in seconds.
    pub jwt_refresh_ttl_seconds: u64,
    /// QR token TTL, in seconds (default: same as access).
    pub jwt_qr_ttl_seconds: u64,

    /// Discord OAuth.
    pub discord_client_id: String,
    /// Discord OAuth.
    pub discord_client_secret: String,
    /// Discord OAuth redirect URI registered in the Discord application.
    pub discord_redirect_uri: String,

    /// HMAC secret used to verify GitHub webhook signatures. Optional
    /// in dev: when absent, the `/api/webhooks/github` endpoint
    /// returns 503 instead of crashing at startup.
    pub github_webhook_secret: Option<Vec<u8>>,

    /// Shared secret for the DraftBot → Axum sync endpoint.
    pub draftbot_api_key: String,

    /// SMTP host.
    pub smtp_host: String,
    /// SMTP port (typically 587 with STARTTLS).
    pub smtp_port: u16,
    /// SMTP login.
    pub smtp_username: String,
    /// SMTP password (should come from a secret manager).
    pub smtp_password: String,
    /// `From:` header used by outgoing mail.
    pub smtp_from: String,

    /// Supabase project URL (used to build presigned upload URLs).
    /// Optional in dev: file uploads are disabled when absent.
    pub supabase_url: Option<String>,
    /// Supabase service-role key. Server-side only; never sent to the
    /// client. Optional in dev (see above).
    pub supabase_service_key: Option<String>,

    /// Where each kind of announcement is posted.
    pub channels: DiscordChannels,

    /// GitHub organisation that hosts project repositories. Optional:
    /// without it (or without a token) projects are created without a
    /// repository and the field can be filled in by hand.
    pub github_org: Option<String>,
    /// Fine-grained PAT with `Administration: read & write` on that
    /// organisation. Server-side only.
    pub github_token: Option<String>,

    /// Requests permitted per IP per minute on authentication and other
    /// sensitive endpoints.
    pub rate_limit_sensitive_per_min: u32,
    /// Requests permitted per IP per minute on everything else.
    pub rate_limit_default_per_min: u32,
    /// Whether `X-Forwarded-For` may be trusted to identify the client.
    /// Only enable when a reverse proxy you control overwrites the
    /// header; otherwise callers can spoof it to dodge the limiter.
    pub trust_forwarded_for: bool,
}

impl Config {
    /// Load configuration from the process environment.
    ///
    /// Reads `.env` files via `dotenvy` first when present.
    ///
    /// # Errors
    ///
    /// Returns the first variable that is missing or invalid.
    pub fn from_env() -> Result<Self, ConfigError> {
        // Best-effort: missing .env is fine in production.
        let _ = dotenvy::dotenv();

        Ok(Self {
            bind_addr: get("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:3000".to_string()),
            public_origin: required("PUBLIC_ORIGIN")?,
            is_production: get("APP_ENV").is_ok_and(|v| v == "production"),

            database_url: required("DATABASE_URL")?,

            jwt_secret: required("JWT_SECRET").and_then(|s| secret("JWT_SECRET", s))?,
            jwt_access_ttl_seconds: parse_or("JWT_ACCESS_TTL", 3600)?,
            jwt_refresh_ttl_seconds: parse_or("JWT_REFRESH_TTL", 604_800)?,
            jwt_qr_ttl_seconds: parse_or("JWT_QR_TTL", 7200)?,

            discord_client_id: required("DISCORD_CLIENT_ID")?,
            discord_client_secret: required("DISCORD_CLIENT_SECRET")?,
            discord_redirect_uri: required("DISCORD_REDIRECT_URI")?,

            github_webhook_secret: optional("GITHUB_WEBHOOK_SECRET")
                .map(|s| secret("GITHUB_WEBHOOK_SECRET", s))
                .transpose()?,

            draftbot_api_key: required("DRAFTBOT_API_KEY")
                .and_then(|s| secret("DRAFTBOT_API_KEY", s))
                .map(|b| String::from_utf8(b).unwrap_or_default())?,

            smtp_host: required("SMTP_HOST")?,
            smtp_port: parse_or("SMTP_PORT", 587_u16)?,
            smtp_username: required("SMTP_USERNAME")?,
            smtp_password: required("SMTP_PASSWORD")?,
            smtp_from: required("SMTP_FROM")?,

            supabase_url: optional("SUPABASE_URL"),
            supabase_service_key: optional("SUPABASE_SERVICE_KEY"),

            channels: DiscordChannels {
                announce: channel("DISCORD_ANNOUNCE_CHANNEL_ID")?,
                quests: channel("DISCORD_QUESTS_CHANNEL_ID")?,
                leaderboard: channel("DISCORD_LEADERBOARD_CHANNEL_ID")?,
                hall_of_fame: channel("DISCORD_HALL_CHANNEL_ID")?,
                review_queue: channel("DISCORD_REVIEWS_CHANNEL_ID")?,
                journal: channel("DISCORD_JOURNAL_CHANNEL_ID")?,
                bureau: channel("DISCORD_BUREAU_CHANNEL_ID")?,
                presences: channel("DISCORD_PRESENCES_CHANNEL_ID")?,
                tracks: track_channels(),
            },

            github_org: optional("GITHUB_ORG"),
            github_token: optional("GITHUB_TOKEN"),

            rate_limit_sensitive_per_min: parse_or("RATE_LIMIT_SENSITIVE_PER_MIN", 10_u32)?,
            rate_limit_default_per_min: parse_or("RATE_LIMIT_DEFAULT_PER_MIN", 120_u32)?,
            trust_forwarded_for: get("TRUST_FORWARDED_FOR").is_ok_and(|v| v == "true"),
        })
    }
}

/// Discord channels the platform posts into.
///
/// Every field is optional: a deployment with none of them configured
/// runs normally and simply stays silent. `announce` is the fallback for
/// anything without a dedicated home — except the audit journal, which
/// must never spill into a public channel.
#[derive(Debug, Clone, Copy, Default)]
pub struct DiscordChannels {
    /// Rank-ups, badges, track joins — the celebratory feed.
    pub announce: Option<u64>,
    /// Newly opened quests.
    pub quests: Option<u64>,
    /// Season standings.
    pub leaderboard: Option<u64>,
    /// Released projects.
    pub hall_of_fame: Option<u64>,
    /// Work waiting for a validator: projects in review, submitted
    /// resources.
    pub review_queue: Option<u64>,
    /// Audit trail. Bureau-only; falls back to `bureau`, never to a
    /// public channel.
    pub journal: Option<u64>,
    /// The Bureau's own channel. Meetings called for the Bureau are
    /// announced here and nowhere else.
    pub bureau: Option<u64>,
    /// Attendance recorded by QR scan. No fallback: a line per scan would
    /// drown the general feed.
    pub presences: Option<u64>,
    /// One channel per track, in [`Track::ALL`] order. A review request
    /// lands in the channel of the discipline being asked, which is the
    /// difference between a queue nobody reads and a question addressed
    /// to the people who can answer it.
    pub tracks: [Option<u64>; 8],
}

impl DiscordChannels {
    /// Where an announcement of this kind belongs.
    ///
    /// Unrecognised kinds land in `announce`. The journal is the one
    /// exception to the fallback: an audit entry names who did what to
    /// whom, so posting it to a public channel because the dedicated one
    /// is unset would be worse than dropping it.
    #[must_use]
    pub fn for_kind(&self, kind: &str) -> Option<u64> {
        self.route(kind, None)
    }

    /// Where an announcement belongs, given its kind and the track it
    /// concerns.
    ///
    /// A review request goes to that track's own channel when one is
    /// configured, falling back to the shared validation queue and then
    /// to the general feed.
    #[must_use]
    pub fn route(&self, kind: &str, track: Option<Track>) -> Option<u64> {
        match kind {
            "ProjectReleased" => self.hall_of_fame.or(self.announce),
            "QuestOpened" => self.quests.or(self.announce),
            "SeasonStandings" => self.leaderboard.or(self.announce),
            // The Bureau's own channel is private too, so the journal may
            // share it: one Bureau channel on the server, not two.
            "AuditEntry" => self.journal.or(self.bureau),
            // A Bureau meeting goes to the Bureau's own channel and
            // nowhere else. No fallback on purpose: announcing a private
            // meeting in the public channel because one id happens to be
            // unset would be worse than not announcing it at all.
            "BureauMeeting" | "BureauMeetingCancelled" => self.bureau,
            "AttendanceRecorded" => self.presences,
            // An event concerning one track belongs in that track's
            // channel, where the people it concerns already are.
            "EventScheduled" | "EventCancelled" => track
                .and_then(|t| self.for_track(t))
                .or(self.announce),
            "ProjectSubmitted" | "ResourceSubmitted" => track
                .and_then(|t| self.for_track(t))
                .or(self.review_queue)
                .or(self.announce),
            _ => self.announce,
        }
    }

    /// The channel belonging to a track, if one is configured.
    #[must_use]
    pub fn for_track(&self, track: Track) -> Option<u64> {
        Track::ALL
            .iter()
            .position(|t| *t == track)
            .and_then(|i| self.tracks[i])
    }
}

/// Parse `DISCORD_TRACK_CHANNELS`, a comma-separated `Track=id` list.
///
/// One variable rather than eight keeps the environment readable, and an
/// unparseable entry is skipped with a warning instead of stopping the
/// boot — a mistyped channel id should cost one silent track, not the
/// whole deployment.
fn track_channels() -> [Option<u64>; 8] {
    let mut out = [None; 8];
    let Some(raw) = optional("DISCORD_TRACK_CHANNELS") else {
        return out;
    };
    for entry in raw.split(',').map(str::trim).filter(|e| !e.is_empty()) {
        let Some((name, id)) = entry.split_once('=') else {
            tracing::warn!(entry, "DISCORD_TRACK_CHANNELS: expected Track=id");
            continue;
        };
        let (Some(track), Ok(id)) = (Track::parse(name.trim()), id.trim().parse::<u64>()) else {
            tracing::warn!(entry, "DISCORD_TRACK_CHANNELS: unknown track or bad id");
            continue;
        };
        if let Some(i) = Track::ALL.iter().position(|t| *t == track) {
            out[i] = Some(id);
        }
    }
    out
}

/// Parse an optional Discord snowflake from the environment.
fn channel(key: &'static str) -> Result<Option<u64>, ConfigError> {
    optional(key)
        .map(|v| {
            v.parse::<u64>().map_err(|e| ConfigError::InvalidVar {
                var: key,
                message: e.to_string(),
            })
        })
        .transpose()
}

/// Placeholder prefixes shipped in `.env.example`. A secret that still
/// starts with one of these has not been generated, and would leave the
/// deployment trivially forgeable — so we refuse to boot rather than
/// run with a guessable JWT signing key.
const PLACEHOLDER_PREFIXES: [&str; 6] = [
    "change",
    "CHANGE",
    "your-",
    "YOUR_",
    "replace",
    "xxxxx",
];

/// Validate a secret: long enough to resist offline attack, and not one
/// of the documented placeholders.
///
/// # Errors
/// `InvalidVar` when the value is too short or still a placeholder.
fn secret(var: &'static str, value: String) -> Result<Vec<u8>, ConfigError> {
    if value.len() < 32 {
        return Err(ConfigError::InvalidVar {
            var,
            message: "must be at least 32 bytes — generate one with `openssl rand -base64 48`"
                .into(),
        });
    }
    if PLACEHOLDER_PREFIXES
        .iter()
        .any(|p| value.starts_with(p))
    {
        return Err(ConfigError::InvalidVar {
            var,
            message:
                "still set to the placeholder from .env.example — generate a real value with `openssl rand -base64 48`"
                    .into(),
        });
    }
    Ok(value.into_bytes())
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

/// Like `required`, but returns `None` instead of an error when the
/// variable is absent or empty. Used for optional features (Supabase
/// Storage, GitHub webhook) that we want to be able to disable in dev.
fn optional(key: &'static str) -> Option<String> {
    match std::env::var(key) {
        Ok(v) if !v.trim().is_empty() => Some(v),
        _ => None,
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = "8qP1x4tZ0nR7vK2mB5cY9wS3jL6hD0fA1gE4uI7oQ2rT5yV8";

    #[test]
    fn accepts_a_generated_secret() {
        assert!(secret("JWT_SECRET", GOOD.to_string()).is_ok());
    }

    #[test]
    fn rejects_short_secrets() {
        let err = secret("JWT_SECRET", "too-short".to_string()).unwrap_err();
        assert!(err.to_string().contains("at least 32 bytes"));
    }

    #[test]
    fn rejects_the_env_example_placeholder() {
        // Long enough to pass the length gate — this is exactly the trap
        // the audit found in the checked-in .env.
        let placeholder = "change-me-to-a-long-random-string-please-1234567890";
        assert!(placeholder.len() >= 32);
        let err = secret("JWT_SECRET", placeholder.to_string()).unwrap_err();
        assert!(err.to_string().contains("placeholder"));
    }

    #[test]
    fn rejects_every_documented_placeholder_prefix() {
        for prefix in PLACEHOLDER_PREFIXES {
            let value = format!("{prefix}{}", "0".repeat(64));
            assert!(
                secret("DRAFTBOT_API_KEY", value).is_err(),
                "prefix {prefix} slipped through"
            );
        }
    }
}

//! Application configuration.
//!
//! All runtime configuration is loaded from environment variables
//! exactly once at startup, validated, and made available through a
//! `Config` value held in the `AppState`. Failing to load any required
//! variable aborts startup — never panic in a handler.

use std::env::VarError;

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

            jwt_secret: required("JWT_SECRET").and_then(|s| {
                if s.len() < 32 {
                    Err(ConfigError::InvalidVar {
                        var: "JWT_SECRET",
                        message: "must be at least 32 bytes".into(),
                    })
                } else {
                    Ok(s.into_bytes())
                }
            })?,
            jwt_access_ttl_seconds: parse_or("JWT_ACCESS_TTL", 3600)?,
            jwt_refresh_ttl_seconds: parse_or("JWT_REFRESH_TTL", 604_800)?,
            jwt_qr_ttl_seconds: parse_or("JWT_QR_TTL", 7200)?,

            discord_client_id: required("DISCORD_CLIENT_ID")?,
            discord_client_secret: required("DISCORD_CLIENT_SECRET")?,
            discord_redirect_uri: required("DISCORD_REDIRECT_URI")?,

            github_webhook_secret: optional("GITHUB_WEBHOOK_SECRET").map(String::into_bytes),

            draftbot_api_key: required("DRAFTBOT_API_KEY")?,

            smtp_host: required("SMTP_HOST")?,
            smtp_port: parse_or("SMTP_PORT", 587_u16)?,
            smtp_username: required("SMTP_USERNAME")?,
            smtp_password: required("SMTP_PASSWORD")?,
            smtp_from: required("SMTP_FROM")?,

            supabase_url: optional("SUPABASE_URL"),
            supabase_service_key: optional("SUPABASE_SERVICE_KEY"),
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

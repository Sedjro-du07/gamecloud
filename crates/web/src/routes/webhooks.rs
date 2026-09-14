//! External-service ingress webhooks.
//!
//! - `POST /api/webhooks/github`   — HMAC-verified GitHub events
//!   (push, PR, review, issues).
//! - `POST /api/sync/draftbot`     — `X-API-Key`-protected DraftBot
//!   level-up forwarder posted by the Discord bot.
//!
//! Both hand off to [`db::queries::xp::grant`], which owns caps,
//! multipliers, streaks, quests, badges and announcements. Handlers
//! here are responsible only for authenticating the sender, mapping the
//! payload onto a member, and choosing the base award.

use axum::{
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::post,
    Router,
};
use gamecloud_shared::xp::{
    XpSource, XP_GITHUB_COMMIT, XP_GITHUB_COMMIT_DAILY_CAP, XP_GITHUB_ISSUE_RESOLVED,
    XP_GITHUB_PR_MERGED, XP_GITHUB_REVIEW,
};
use serde::Deserialize;
use serde_json::Value;

use crate::{
    db::queries::{
        audit,
        users,
        xp::{self, XpGrant},
    },
    error::{WebError, WebResult},
    middleware::github_signature,
    state::AppState,
};

/// Build the webhook router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/webhooks/github", post(github))
        .route("/sync/draftbot", post(draftbot))
}

// ---------------------------------------------------------------------------
// GitHub
// ---------------------------------------------------------------------------

const HEADER_GITHUB_EVENT: &str = "X-GitHub-Event";
const HEADER_GITHUB_SIGNATURE: &str = "X-Hub-Signature-256";

/// Ledger prefix for commit awards.
///
/// The daily commit cap is scoped to rows carrying this prefix, so a
/// merged PR or a review no longer eats the commit budget — and, more
/// importantly, PR/review/issue XP is no longer silently uncapped
/// because it shared a bucket it never checked.
const COMMIT_DESCRIPTION_PREFIX: &str = "Push";

async fn github(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    let event = headers
        .get(HEADER_GITHUB_EVENT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let sig_header = headers
        .get(HEADER_GITHUB_SIGNATURE)
        .and_then(|v| v.to_str().ok());

    // GitHub webhook is optional: when no secret is configured we
    // return 503 instead of accepting unsigned webhooks. This lets
    // the rest of the platform run in dev without a GitHub setup.
    let Some(secret) = state.config().github_webhook_secret.as_deref() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            "github webhook not configured",
        )
            .into_response();
    };

    if let Err(e) = github_signature::verify(secret, &body, sig_header) {
        return e.into_response();
    }

    if event == "ping" {
        return (StatusCode::OK, "pong").into_response();
    }

    let payload: Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(_) => return WebError::Validation("invalid JSON".into()).into_response(),
    };

    match handle_github(&state, &event, &payload).await {
        Ok(()) => (StatusCode::OK, "ok").into_response(),
        Err(e) => e.into_response(),
    }
}

async fn handle_github(state: &AppState, event: &str, payload: &Value) -> WebResult<()> {
    match event {
        "push" => handle_push(state, payload).await,
        "pull_request" => handle_pull_request(state, payload).await,
        "pull_request_review" => handle_pull_request_review(state, payload).await,
        "issues" => handle_issues(state, payload).await,
        _ => Ok(()),
    }
}

/// Resolve a platform user from a GitHub login. Returns `None` if no
/// platform user has linked that login.
async fn resolve_user(state: &AppState, login: &str) -> WebResult<Option<uuid::Uuid>> {
    let row: Option<(uuid::Uuid,)> =
        sqlx::query_as("SELECT id FROM users WHERE github_username = $1")
            .bind(login)
            .fetch_optional(state.pool())
            .await?;
    Ok(row.map(|(id,)| id))
}

async fn handle_push(state: &AppState, payload: &Value) -> WebResult<()> {
    let Some(login) = payload
        .pointer("/pusher/name")
        .and_then(Value::as_str)
        .or_else(|| payload.pointer("/sender/login").and_then(Value::as_str))
    else {
        return Ok(());
    };
    let Some(user_id) = resolve_user(state, login).await? else {
        return Ok(());
    };
    let n_commits = payload
        .pointer("/commits")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    if n_commits == 0 {
        return Ok(());
    }

    let base = i32::try_from(n_commits)
        .unwrap_or(i32::MAX)
        .saturating_mul(XP_GITHUB_COMMIT);
    let description = format!("{COMMIT_DESCRIPTION_PREFIX} x{n_commits}");

    let outcome = xp::grant(
        state.pool(),
        state.channels(),
        &XpGrant::new(user_id, base, XpSource::Github)
            .describe(&description)
            .capped(
                XP_GITHUB_COMMIT_DAILY_CAP,
                Some(COMMIT_DESCRIPTION_PREFIX),
            ),
    )
    .await?;

    if outcome.capped {
        tracing::info!(%user_id, "commit XP trimmed by the daily cap");
    }
    Ok(())
}

async fn handle_pull_request(state: &AppState, payload: &Value) -> WebResult<()> {
    if payload
        .pointer("/pull_request/merged")
        .and_then(Value::as_bool)
        != Some(true)
    {
        return Ok(());
    }
    let Some(login) = payload.pointer("/sender/login").and_then(Value::as_str) else {
        return Ok(());
    };
    let Some(user_id) = resolve_user(state, login).await? else {
        return Ok(());
    };
    xp::grant(
        state.pool(),
        state.channels(),
        &XpGrant::new(user_id, XP_GITHUB_PR_MERGED, XpSource::Github).describe("PR merged"),
    )
    .await?;
    Ok(())
}

async fn handle_pull_request_review(state: &AppState, payload: &Value) -> WebResult<()> {
    if payload.pointer("/action").and_then(Value::as_str) != Some("submitted") {
        return Ok(());
    }
    let Some(login) = payload.pointer("/review/user/login").and_then(Value::as_str) else {
        return Ok(());
    };
    let Some(user_id) = resolve_user(state, login).await? else {
        return Ok(());
    };
    xp::grant(
        state.pool(),
        state.channels(),
        &XpGrant::new(user_id, XP_GITHUB_REVIEW, XpSource::Github).describe("Code review"),
    )
    .await?;
    Ok(())
}

async fn handle_issues(state: &AppState, payload: &Value) -> WebResult<()> {
    if payload.pointer("/action").and_then(Value::as_str) != Some("closed") {
        return Ok(());
    }
    let Some(login) = payload.pointer("/sender/login").and_then(Value::as_str) else {
        return Ok(());
    };
    let Some(user_id) = resolve_user(state, login).await? else {
        return Ok(());
    };
    xp::grant(
        state.pool(),
        state.channels(),
        &XpGrant::new(user_id, XP_GITHUB_ISSUE_RESOLVED, XpSource::Github)
            .describe("Issue resolved"),
    )
    .await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// DraftBot
// ---------------------------------------------------------------------------

const HEADER_DRAFTBOT_KEY: &str = "X-API-Key";

/// Highest DraftBot level we will honour.
///
/// The bot parses the level out of a chat message with a regex that
/// accepts up to four digits, so an edited or spoofed announcement
/// could claim level 9999 and mint 49 995 XP. Clamping here keeps the
/// blast radius of a bad parse to something a Bureau member would
/// notice rather than something that rewrites the leaderboard.
const DRAFTBOT_MAX_LEVEL: i32 = 200;

/// XP granted per DraftBot level.
const DRAFTBOT_XP_PER_LEVEL: i32 = 5;

#[derive(Deserialize)]
struct DraftBotEvent {
    discord_id: String,
    new_level: i32,
}

/// DraftBot level-up forwarder.
///
/// The body is read as raw bytes and only parsed *after* the API key
/// checks out — the previous version used the `Json` extractor, which
/// runs before the handler body and therefore parsed attacker-supplied
/// JSON on unauthenticated requests.
async fn draftbot(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> WebResult<&'static str> {
    let key = headers
        .get(HEADER_DRAFTBOT_KEY)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if !constant_time_eq(key.as_bytes(), state.config().draftbot_api_key.as_bytes()) {
        return Err(WebError::Unauthorized);
    }

    let event: DraftBotEvent = serde_json::from_slice(&body)
        .map_err(|e| WebError::Validation(format!("invalid json: {e}")))?;

    let level = event.new_level.clamp(0, DRAFTBOT_MAX_LEVEL);
    if level != event.new_level {
        tracing::warn!(
            claimed = event.new_level,
            clamped = level,
            "draftbot level out of range"
        );
    }
    if level <= 0 {
        return Ok("ignored");
    }

    let Some(user) = users::find_by_discord_id(state.pool(), &event.discord_id).await? else {
        // Not on the platform yet. The level is kept rather than dropped:
        // the activity is real, and it is paid the day they sign in.
        sqlx::query(
            "INSERT INTO draftbot_pending_levels (discord_id, level) VALUES ($1, $2) \
             ON CONFLICT DO NOTHING",
        )
        .bind(&event.discord_id)
        .bind(level)
        .execute(state.pool())
        .await?;
        return Ok("queued");
    };

    credit_draftbot_level(&state, user.id, level).await
}

/// Pay the DraftBot levels a member earned before signing in.
///
/// Called at login. Each level goes through [`credit_draftbot_level`],
/// so a level already paid is skipped and a crash between paying and
/// clearing the queue cannot pay twice.
///
/// # Errors
/// Propagates database errors.
pub(super) async fn credit_draftbot_backlog(
    state: &AppState,
    user_id: uuid::Uuid,
    discord_id: &str,
) -> WebResult<()> {
    let levels: Vec<i32> = sqlx::query_scalar(
        "SELECT level FROM draftbot_pending_levels WHERE discord_id = $1 ORDER BY level",
    )
    .bind(discord_id)
    .fetch_all(state.pool())
    .await?;

    for level in &levels {
        credit_draftbot_level(state, user_id, *level).await?;
    }
    if !levels.is_empty() {
        sqlx::query("DELETE FROM draftbot_pending_levels WHERE discord_id = $1")
            .bind(discord_id)
            .execute(state.pool())
            .await?;
    }
    Ok(())
}

/// Credit one DraftBot level to a member, at most once.
async fn credit_draftbot_level(
    state: &AppState,
    user_id: uuid::Uuid,
    level: i32,
) -> WebResult<&'static str> {
    // Idempotency: the same level for the same member is only ever paid
    // once. A re-posted or edited DraftBot announcement, or a gateway
    // replay after a reconnect, therefore cannot farm XP.
    let description = format!("DraftBot niveau {level}");
    let already: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM xp_logs
             WHERE user_id = $1 AND source = 'Discord' AND description = $2
        )
        "#,
    )
    .bind(user_id)
    .bind(&description)
    .fetch_one(state.pool())
    .await?;

    if already {
        tracing::debug!(user = %user_id, level, "draftbot level already credited");
        return Ok("duplicate");
    }

    let bonus = level.saturating_mul(DRAFTBOT_XP_PER_LEVEL);
    xp::grant(
        state.pool(),
        state.channels(),
        &XpGrant::new(user_id, bonus, XpSource::Discord).describe(&description),
    )
    .await?;

    audit::record(
        state.pool(),
        None,
        "draftbot.level_up",
        Some("user"),
        Some(user_id),
        serde_json::json!({ "level": level, "xp": bonus }),
    )
    .await?;

    Ok("ok")
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_time_eq_matches_equality() {
        assert!(constant_time_eq(b"secret", b"secret"));
        assert!(!constant_time_eq(b"secret", b"secrEt"));
        assert!(!constant_time_eq(b"secret", b"secret-longer"));
        assert!(constant_time_eq(b"", b""));
    }

    #[test]
    fn draftbot_levels_are_clamped() {
        assert_eq!(9999_i32.clamp(0, DRAFTBOT_MAX_LEVEL), DRAFTBOT_MAX_LEVEL);
        assert_eq!((-5_i32).clamp(0, DRAFTBOT_MAX_LEVEL), 0);
        assert_eq!(14_i32.clamp(0, DRAFTBOT_MAX_LEVEL), 14);
    }

    #[test]
    fn clamped_bonus_stays_reasonable() {
        let worst = DRAFTBOT_MAX_LEVEL.saturating_mul(DRAFTBOT_XP_PER_LEVEL);
        assert_eq!(worst, 1_000);
        // Well under the 2 500 XP `Expert` threshold, so a bad parse can
        // never single-handedly promote somebody several ranks.
        assert!(worst < 2_500);
    }
}

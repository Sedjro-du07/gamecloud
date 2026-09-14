//! External-service ingress webhooks.
//!
//! - `POST /api/webhooks/github`   — HMAC-verified GitHub events
//!   (push, PR, review, issues).
//! - `POST /api/sync/draftbot`     — `X-API-Key`-protected DraftBot
//!   level-up forwarder posted by the Discord bot.
//!
//! Handlers here apply the daily-cap rule, streak/multi-track
//! multipliers, and write `xp_logs` + `audit_logs` transactionally.

use axum::{
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::post,
    Json, Router,
};
use gamecloud_shared::{
    xp::{
        compute_final_xp, multi_track_bonus, streak_multiplier, XpSource, XP_GITHUB_COMMIT,
        XP_GITHUB_COMMIT_DAILY_CAP, XP_GITHUB_ISSUE_RESOLVED, XP_GITHUB_PR_MERGED,
        XP_GITHUB_REVIEW,
    },
};
use serde::Deserialize;
use serde_json::Value;

use crate::{
    db::queries::{users, xp},
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
        return (StatusCode::SERVICE_UNAVAILABLE, "github webhook not configured")
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

    // Daily cap on commit XP.
    let already = xp::xp_today_for_source(state.pool(), user_id, XpSource::Github).await?;
    let remaining = i64::from(XP_GITHUB_COMMIT_DAILY_CAP).saturating_sub(already);
    if remaining <= 0 {
        return Ok(());
    }

    let raw =
        i64::from(XP_GITHUB_COMMIT).saturating_mul(i64::try_from(n_commits).unwrap_or(0));
    let capped = raw.min(remaining);
    let final_xp = apply_multipliers(state, user_id, capped.try_into().unwrap_or(0)).await?;
    xp::grant_xp(
        state.pool(),
        user_id,
        final_xp,
        XpSource::Github,
        None,
        Some(&format!("Push x{n_commits}")),
    )
    .await
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
    let final_xp = apply_multipliers(state, user_id, XP_GITHUB_PR_MERGED).await?;
    xp::grant_xp(
        state.pool(),
        user_id,
        final_xp,
        XpSource::Github,
        None,
        Some("PR merged"),
    )
    .await
}

async fn handle_pull_request_review(state: &AppState, payload: &Value) -> WebResult<()> {
    if payload.pointer("/action").and_then(Value::as_str) != Some("submitted") {
        return Ok(());
    }
    let Some(login) = payload
        .pointer("/review/user/login")
        .and_then(Value::as_str)
    else {
        return Ok(());
    };
    let Some(user_id) = resolve_user(state, login).await? else {
        return Ok(());
    };
    let final_xp = apply_multipliers(state, user_id, XP_GITHUB_REVIEW).await?;
    xp::grant_xp(
        state.pool(),
        user_id,
        final_xp,
        XpSource::Github,
        None,
        Some("Code review"),
    )
    .await
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
    let final_xp = apply_multipliers(state, user_id, XP_GITHUB_ISSUE_RESOLVED).await?;
    xp::grant_xp(
        state.pool(),
        user_id,
        final_xp,
        XpSource::Github,
        None,
        Some("Issue resolved"),
    )
    .await
}

/// Look up the user's streak and active-track count, then apply the
/// multipliers from `gamecloud_shared::xp`.
async fn apply_multipliers(state: &AppState, user_id: uuid::Uuid, base: i32) -> WebResult<i32> {
    let row: (i32, i64) = sqlx::query_as(
        r#"
        SELECT
            u.streak_days,
            COALESCE((SELECT COUNT(*) FROM track_memberships m
                      WHERE m.user_id = u.id
                        AND m.last_active_at > NOW() - INTERVAL '30 days'), 0) AS active
        FROM users u WHERE u.id = $1
        "#,
    )
    .bind(user_id)
    .fetch_one(state.pool())
    .await?;
    let streak = row.0;
    let active = usize::try_from(row.1).unwrap_or(0);
    let _ = (streak_multiplier, multi_track_bonus); // anchor imports; used inside compute_final_xp
    Ok(compute_final_xp(base, streak, active))
}

// ---------------------------------------------------------------------------
// DraftBot
// ---------------------------------------------------------------------------

const HEADER_DRAFTBOT_KEY: &str = "X-API-Key";

#[derive(Deserialize)]
struct DraftBotEvent {
    discord_id: String,
    new_level: i32,
}

async fn draftbot(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(event): Json<DraftBotEvent>,
) -> WebResult<&'static str> {
    let key = headers
        .get(HEADER_DRAFTBOT_KEY)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if !constant_time_eq(key.as_bytes(), state.config().draftbot_api_key.as_bytes()) {
        return Err(WebError::Unauthorized);
    }

    let Some(user) = users::find_by_discord_id(state.pool(), &event.discord_id).await? else {
        return Ok("ignored");
    };

    let bonus = (event.new_level * 5).max(0);
    if bonus > 0 {
        let bonus = apply_multipliers(&state, user.id, bonus).await?;
        xp::grant_xp(
            state.pool(),
            user.id,
            bonus,
            XpSource::Discord,
            None,
            Some(&format!("DraftBot level {}", event.new_level)),
        )
        .await?;
    }
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

//! Kumo ⇄ platform XP sync endpoints.
//!
//! - `POST /api/sync/kumo/activity`   — Kumo reports what its members
//!   earned by chatting; the platform pays it at the configured rate.
//! - `GET  /api/sync/kumo/platform-xp` — the platform publishes what it
//!   awarded, for Kumo to mirror.
//!
//! Both are `X-API-Key` protected with `KUMO_SYNC_API_KEY`. Kumo calls
//! the platform (never the reverse), so Kumo needs no open port.

use axum::{
    body::Bytes,
    extract::State,
    http::HeaderMap,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::{
    db::queries::kumo_sync::{self, KumoReport},
    error::{WebError, WebResult},
    state::AppState,
};

/// Most members accepted in one report. The association is a few dozen
/// people; this only bounds a bad request.
const MAX_REPORT_MEMBERS: usize = 1000;

/// Build the router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/sync/kumo/activity", post(activity))
        .route("/sync/kumo/platform-xp", get(platform_xp))
}

/// Check the API key. A missing key in the config closes the endpoints.
fn authorize(state: &AppState, headers: &HeaderMap) -> WebResult<()> {
    let Some(expected) = state.config().kumo_sync_api_key.as_deref() else {
        return Err(WebError::Upstream("kumo sync is not configured".into()));
    };
    let key = headers
        .get("X-API-Key")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if super::webhooks::constant_time_eq(key.as_bytes(), expected.as_bytes()) {
        Ok(())
    } else {
        Err(WebError::Unauthorized)
    }
}

#[derive(Deserialize)]
struct ActivityBody {
    members: Vec<ActivityMember>,
}

#[derive(Deserialize)]
struct ActivityMember {
    discord_id: String,
    organic_total: i64,
    organic_base: i64,
}

#[derive(Serialize)]
struct ActivityReply {
    members: usize,
    platform_xp_paid: i64,
}

/// Kumo → platform. The body is parsed only once the key checks out.
async fn activity(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> WebResult<Json<ActivityReply>> {
    authorize(&state, &headers)?;
    let body: ActivityBody = serde_json::from_slice(&body)
        .map_err(|e| WebError::Validation(format!("invalid json: {e}")))?;
    if body.members.len() > MAX_REPORT_MEMBERS {
        return Err(WebError::Validation("too many members".into()));
    }

    let ratio = state.config().kumo_xp_per_platform_xp;
    let mut paid = 0;
    for m in &body.members {
        if m.discord_id.is_empty() || m.discord_id.len() > 32 || m.organic_total < 0 {
            continue;
        }
        paid += kumo_sync::report(
            state.pool(),
            state.channels(),
            ratio,
            KumoReport {
                discord_id: &m.discord_id,
                organic_total: m.organic_total,
                organic_base: m.organic_base,
            },
        )
        .await?;
    }
    Ok(Json(ActivityReply {
        members: body.members.len(),
        platform_xp_paid: paid,
    }))
}

#[derive(Serialize)]
struct PlatformXpReply {
    /// Kumo XP per platform XP; Kumo multiplies by this.
    ratio: i64,
    members: Vec<PlatformXpMember>,
}

#[derive(Serialize)]
struct PlatformXpMember {
    discord_id: String,
    /// Cumulative XP the platform itself awarded.
    platform_xp: i64,
}

/// Platform → Kumo.
async fn platform_xp(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> WebResult<Json<PlatformXpReply>> {
    authorize(&state, &headers)?;
    let members = kumo_sync::platform_origin_xp(state.pool())
        .await?
        .into_iter()
        .map(|(discord_id, platform_xp)| PlatformXpMember {
            discord_id,
            platform_xp,
        })
        .collect();
    Ok(Json(PlatformXpReply {
        ratio: state.config().kumo_xp_per_platform_xp,
        members,
    }))
}

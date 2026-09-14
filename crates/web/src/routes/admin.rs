//! Bureau-only routes.
//!
//! - `GET  /api/admin/audit`          — the audit trail.
//! - `POST /api/admin/xp`             — grant or revoke XP by hand.
//! - `POST /api/admin/bureau-role`    — assign a bureau role.
//! - `POST /api/admin/track-role`     — appoint a track Lead / CoLead.
//! - `POST /api/admin/badge`          — confer a manual badge.
//! - `GET  /api/admin/seasons`        — list seasons.
//! - `POST /api/admin/seasons`        — open a season.
//!
//! Every handler here writes an audit entry. That is the point of the
//! module: these are the actions that change somebody else's standing,
//! so they must leave a trace the club can inspect.

use axum::{
    extract::{Query, State},
    routing::{get, post},
    Json, Router,
};
use gamecloud_shared::{
    roles::{Action, Track, TrackRole},
    xp::XpSource,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    db::queries::{
        audit, badges, seasons, tracks, users,
        xp::{self, XpGrant},
    },
    error::{WebError, WebResult},
    middleware::auth::CurrentUser,
    state::AppState,
};

/// Build the admin router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/audit", get(audit_log))
        .route("/xp", post(grant_xp))
        .route("/bureau-role", post(set_bureau_role))
        .route("/track-role", post(set_track_role))
        .route("/badge", post(award_badge))
        .route("/seasons", get(list_seasons).post(create_season))
}

/// Load the caller's authority and require an action, in one step.
async fn require(state: &AppState, user: &CurrentUser, action: Action) -> WebResult<()> {
    let authority = users::load_authority(state.pool(), user.id).await?;
    if authority.can(action) {
        Ok(())
    } else {
        Err(WebError::Forbidden)
    }
}

// ---------------------------------------------------------------------------
// Audit
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct AuditQuery {
    limit: Option<i64>,
}

async fn audit_log(
    State(state): State<AppState>,
    user: CurrentUser,
    Query(q): Query<AuditQuery>,
) -> WebResult<Json<Vec<audit::AuditEntry>>> {
    require(&state, &user, Action::ViewAuditLogs).await?;
    Ok(Json(
        audit::recent(state.pool(), q.limit.unwrap_or(100)).await?,
    ))
}

// ---------------------------------------------------------------------------
// Manual XP
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct GrantXpBody {
    user_id: Uuid,
    /// May be negative to revoke. The ledger keeps both directions.
    amount: i32,
    reason: String,
    track: Option<String>,
}

#[derive(Serialize)]
struct GrantXpResponse {
    awarded: i32,
    new_badges: Vec<String>,
}

/// Grant (or revoke) XP by hand.
///
/// Multipliers are deliberately *not* applied: when a Bureau member
/// types 50, the recipient gets 50. A manual grant is a judgement, not
/// an activity, so inflating it by somebody's login streak would make
/// the number meaningless.
async fn grant_xp(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(body): Json<GrantXpBody>,
) -> WebResult<Json<GrantXpResponse>> {
    require(&state, &user, Action::GrantManualXp).await?;

    if body.reason.trim().is_empty() {
        return Err(WebError::Validation(
            "a manual grant must carry a reason".into(),
        ));
    }
    if let Some(track) = &body.track {
        if Track::parse(track).is_none() {
            return Err(WebError::Domain(
                gamecloud_shared::DomainError::UnknownTrack(track.clone()),
            ));
        }
    }

    let mut grant = XpGrant::new(body.user_id, body.amount, XpSource::Manual)
        .describe(&body.reason)
        .exact();
    if let Some(track) = &body.track {
        grant = grant.in_track(track);
    }

    let outcome = xp::grant(state.pool(), state.announce_channel(), &grant).await?;

    audit::record(
        state.pool(),
        Some(user.id),
        "admin.grant_xp",
        Some("user"),
        Some(body.user_id),
        serde_json::json!({
            "amount": body.amount,
            "awarded": outcome.awarded,
            "reason": body.reason,
            "track": body.track,
        }),
    )
    .await?;

    Ok(Json(GrantXpResponse {
        awarded: outcome.awarded,
        new_badges: outcome
            .new_badges
            .iter()
            .map(|b| b.as_str().to_string())
            .collect(),
    }))
}

// ---------------------------------------------------------------------------
// Roles
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct BureauRoleBody {
    user_id: Uuid,
    /// `None` clears the role.
    role: Option<String>,
}

async fn set_bureau_role(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(body): Json<BureauRoleBody>,
) -> WebResult<Json<serde_json::Value>> {
    require(&state, &user, Action::AssignBureauRole).await?;
    users::set_bureau_role(state.pool(), body.user_id, body.role.as_deref()).await?;

    audit::record(
        state.pool(),
        Some(user.id),
        "admin.set_bureau_role",
        Some("user"),
        Some(body.user_id),
        serde_json::json!({ "role": body.role }),
    )
    .await?;

    Ok(Json(serde_json::json!({ "ok": true })))
}

#[derive(Deserialize)]
struct TrackRoleBody {
    user_id: Uuid,
    track: String,
    /// `Lead` or `CoLead` — the two roles XP never confers.
    role: String,
}

/// Appoint a track Lead or CoLead.
///
/// A Lead appointment is Bureau business; a CoLead may be named by the
/// track's own Lead. The permission table already encodes that
/// distinction, so we just pick the right action.
async fn set_track_role(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(body): Json<TrackRoleBody>,
) -> WebResult<Json<serde_json::Value>> {
    let Some(track) = Track::parse(&body.track) else {
        return Err(WebError::Domain(
            gamecloud_shared::DomainError::UnknownTrack(body.track.clone()),
        ));
    };

    let role = match body.role.as_str() {
        "Lead" => TrackRole::Lead,
        "CoLead" => TrackRole::CoLead,
        "Mentor" => TrackRole::Mentor,
        "Reviewer" => TrackRole::Reviewer,
        "Contributor" => TrackRole::Contributor,
        "Observer" => TrackRole::Observer,
        other => {
            return Err(WebError::Validation(format!("unknown track role '{other}'")));
        }
    };

    let action = if role == TrackRole::Lead {
        Action::AppointTrackLead(track)
    } else {
        Action::AppointTrackCoLead(track)
    };
    require(&state, &user, action).await?;

    tracks::set_role(state.pool(), body.user_id, track, role).await?;

    audit::record(
        state.pool(),
        Some(user.id),
        "admin.set_track_role",
        Some("user"),
        Some(body.user_id),
        serde_json::json!({ "track": track.as_str(), "role": role.as_str() }),
    )
    .await?;

    Ok(Json(serde_json::json!({ "ok": true })))
}

#[derive(Deserialize)]
struct BadgeBody {
    user_id: Uuid,
    badge_type: String,
}

async fn award_badge(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(body): Json<BadgeBody>,
) -> WebResult<Json<serde_json::Value>> {
    require(&state, &user, Action::AssignBureauRole).await?;
    badges::award_manual(state.pool(), body.user_id, &body.badge_type, user.id).await?;

    audit::record(
        state.pool(),
        Some(user.id),
        "admin.award_badge",
        Some("user"),
        Some(body.user_id),
        serde_json::json!({ "badge": body.badge_type }),
    )
    .await?;

    Ok(Json(serde_json::json!({ "ok": true })))
}

// ---------------------------------------------------------------------------
// Seasons
// ---------------------------------------------------------------------------

async fn list_seasons(
    State(state): State<AppState>,
    user: CurrentUser,
) -> WebResult<Json<Vec<seasons::Season>>> {
    require(&state, &user, Action::AccessAdminPanel).await?;
    Ok(Json(seasons::list(state.pool()).await?))
}

#[derive(Deserialize)]
struct NewSeasonBody {
    name: String,
    slug: String,
    description: Option<String>,
    starts_at: chrono::DateTime<chrono::Utc>,
    ends_at: chrono::DateTime<chrono::Utc>,
}

async fn create_season(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(body): Json<NewSeasonBody>,
) -> WebResult<Json<seasons::Season>> {
    require(&state, &user, Action::AssignBureauRole).await?;

    let season = seasons::create(
        state.pool(),
        &body.name,
        &body.slug,
        body.description.as_deref(),
        body.starts_at,
        body.ends_at,
    )
    .await?;

    audit::record(
        state.pool(),
        Some(user.id),
        "admin.create_season",
        Some("season"),
        Some(season.id),
        serde_json::json!({ "name": season.name, "slug": season.slug }),
    )
    .await?;

    Ok(Json(season))
}

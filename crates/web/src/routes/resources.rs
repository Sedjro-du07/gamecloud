//! Resource library routes.
//!
//! - `GET    /api/resources`             — browse.
//! - `POST   /api/resources`             — submit.
//! - `POST   /api/resources/:id/validate` — sign off (Archiviste / track Lead).
//! - `POST   /api/resources/:id/vote`    — upvote.
//! - `DELETE /api/resources/:id/vote`    — withdraw the upvote.

use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Json, Router,
};
use gamecloud_shared::roles::Action;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    db::queries::{resources, users},
    error::{WebError, WebResult},
    middleware::auth::CurrentUser,
    state::AppState,
};

/// Build the resources router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list).post(submit))
        .route("/{id}/validate", post(validate))
        .route("/{id}/vote", post(vote).delete(unvote))
}

#[derive(Deserialize)]
struct ListQuery {
    track: Option<String>,
    /// Include submissions awaiting validation. Honoured only for
    /// members who can validate them.
    pending: Option<bool>,
}

async fn list(
    State(state): State<AppState>,
    user: CurrentUser,
    Query(q): Query<ListQuery>,
) -> WebResult<Json<Vec<resources::ResourceView>>> {
    if !users::is_member(state.pool(), user.id).await? {
        return Err(WebError::Forbidden);
    }
    let include_unvalidated = if q.pending.unwrap_or(false) {
        users::load_authority(state.pool(), user.id)
            .await?
            .can(Action::ValidateResource)
    } else {
        false
    };

    Ok(Json(
        resources::list(
            state.pool(),
            user.id,
            q.track.as_deref(),
            include_unvalidated,
        )
        .await?,
    ))
}

#[derive(Serialize)]
struct CreatedResponse {
    id: Uuid,
}

async fn submit(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(body): Json<resources::NewResource>,
) -> WebResult<Json<CreatedResponse>> {
    if !user.record.email_verified {
        return Err(WebError::Domain(
            gamecloud_shared::DomainError::EmailNotVerified,
        ));
    }
    let id = resources::submit(state.pool(), state.channels(), user.id, &body).await?;
    Ok(Json(CreatedResponse { id }))
}

#[derive(Serialize)]
struct ValidateResponse {
    /// False when the entry was already validated — the call is a
    /// no-op rather than a double payout.
    newly_validated: bool,
}

async fn validate(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
) -> WebResult<Json<ValidateResponse>> {
    let authority = users::load_authority(state.pool(), user.id).await?;
    if !authority.can(Action::ValidateResource) {
        return Err(WebError::Forbidden);
    }
    let newly_validated =
        resources::validate_entry(state.pool(), state.channels(), user.id, id).await?;
    Ok(Json(ValidateResponse { newly_validated }))
}

#[derive(Serialize)]
struct VoteResponse {
    votes: i32,
}

async fn vote(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
) -> WebResult<Json<VoteResponse>> {
    let votes = resources::vote(state.pool(), user.id, id).await?;
    Ok(Json(VoteResponse { votes }))
}

async fn unvote(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
) -> WebResult<Json<VoteResponse>> {
    let votes = resources::unvote(state.pool(), user.id, id).await?;
    Ok(Json(VoteResponse { votes }))
}

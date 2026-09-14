//! Project routes — the review pipeline.
//!
//! - `GET    /api/projects`               — listing (public by default).
//! - `POST   /api/projects`               — create a Draft.
//! - `GET    /api/projects/:id`           — detail with verdicts.
//! - `POST   /api/projects/:id/contributors` — credit somebody.
//! - `POST   /api/projects/:id/submit`    — open a review round.
//! - `POST   /api/projects/:id/review`    — record one track's verdict.
//! - `POST   /api/projects/:id/release`   — publish and pay out.
//!
//! Every permission check goes through `Authority::can`, which before
//! this change was consulted in exactly one place in the whole codebase.

use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Json, Router,
};
use gamecloud_shared::{
    projects::Verdict,
    roles::{Action, Track},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    db::queries::{projects, users},
    error::{WebError, WebResult},
    middleware::auth::CurrentUser,
    state::AppState,
};

/// Build the projects router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list).post(create))
        .route("/{id}", get(detail))
        .route("/{id}/contributors", post(add_contributor))
        .route("/{id}/submit", post(submit))
        .route("/{id}/review", post(review))
        .route("/{id}/release", post(release))
}

#[derive(Deserialize)]
struct ListQuery {
    track: Option<String>,
    /// Ask for pre-release work. Honoured only for members who hold a
    /// role in the track concerned.
    internal: Option<bool>,
}

async fn list(
    State(state): State<AppState>,
    user: Option<CurrentUser>,
    Query(q): Query<ListQuery>,
) -> WebResult<Json<Vec<projects::ProjectSummary>>> {
    let mut include_internal = false;

    if q.internal.unwrap_or(false) {
        if let Some(user) = &user {
            let authority = users::load_authority(state.pool(), user.id).await?;
            include_internal = match q.track.as_deref().and_then(Track::parse) {
                Some(track) => authority.can(Action::ViewTrackInternalProjects(track)),
                // Without a track filter, only somebody who can see the
                // admin panel gets the unfiltered internal view.
                None => authority.can(Action::AccessAdminPanel),
            };
        }
    }

    Ok(Json(
        projects::list(state.pool(), q.track.as_deref(), include_internal).await?,
    ))
}

async fn detail(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> WebResult<Json<projects::ProjectDetail>> {
    Ok(Json(projects::detail(state.pool(), id).await?))
}

#[derive(Serialize)]
struct CreatedResponse {
    id: Uuid,
}

async fn create(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(body): Json<projects::NewProject>,
) -> WebResult<Json<CreatedResponse>> {
    let authority = users::load_authority(state.pool(), user.id).await?;
    if !authority.can(Action::CreateProject) {
        return Err(WebError::Forbidden);
    }
    let id = projects::create(state.pool(), user.id, &body).await?;
    Ok(Json(CreatedResponse { id }))
}

#[derive(Deserialize)]
struct ContributorBody {
    user_id: Uuid,
    track: String,
    role_in_project: Option<String>,
}

async fn add_contributor(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
    Json(body): Json<ContributorBody>,
) -> WebResult<Json<serde_json::Value>> {
    let authority = users::load_authority(state.pool(), user.id).await?;
    if !authority.can(Action::CreateProject) {
        return Err(WebError::Forbidden);
    }
    projects::add_contributor(
        state.pool(),
        id,
        body.user_id,
        &body.track,
        body.role_in_project.as_deref(),
    )
    .await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

#[derive(Serialize)]
struct SubmitResponse {
    /// Tracks that now owe a verdict.
    tracks: Vec<String>,
}

async fn submit(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
) -> WebResult<Json<SubmitResponse>> {
    let authority = users::load_authority(state.pool(), user.id).await?;
    if !authority.can(Action::SubmitProjectForReview) {
        return Err(WebError::Forbidden);
    }
    let tracks = projects::submit_for_review(
        state.pool(),
        state.announce_channel(),
        user.id,
        id,
    )
    .await?;
    Ok(Json(SubmitResponse { tracks }))
}

#[derive(Deserialize)]
struct ReviewBody {
    track: String,
    /// `Approved`, `Rejected` or `NotApplicable`.
    verdict: String,
    feedback: Option<String>,
}

#[derive(Serialize)]
struct ReviewResponse {
    /// Project status after folding in this verdict.
    status: String,
}

/// Record one track's verdict.
///
/// The reviewer must hold `Reviewer` or above **in the track they are
/// judging** — an Audio mentor has no standing to approve the
/// Engineering side of a build.
async fn review(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
    Json(body): Json<ReviewBody>,
) -> WebResult<Json<ReviewResponse>> {
    let Some(track) = Track::parse(&body.track) else {
        return Err(WebError::Domain(
            gamecloud_shared::DomainError::UnknownTrack(body.track.clone()),
        ));
    };
    let Some(verdict) = Verdict::parse(&body.verdict) else {
        return Err(WebError::Validation(format!(
            "unknown verdict '{}' (expected Approved, Rejected or NotApplicable)",
            body.verdict
        )));
    };

    let authority = users::load_authority(state.pool(), user.id).await?;
    if !authority.can(Action::ReviewProjectForTrack(track)) {
        return Err(WebError::Forbidden);
    }

    let status = projects::record_verdict(
        state.pool(),
        state.announce_channel(),
        user.id,
        id,
        track.as_str(),
        verdict,
        body.feedback.as_deref(),
    )
    .await?;

    Ok(Json(ReviewResponse {
        status: status.as_str().to_string(),
    }))
}

/// Publish an approved project.
///
/// Requires `Lead` of the project's **primary** track — publishing is
/// the one action the track owner alone can take.
async fn release(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
) -> WebResult<Json<projects::ReleaseReport>> {
    let detail = projects::detail(state.pool(), id).await?;
    let Some(primary) = Track::parse(&detail.summary.primary_track) else {
        return Err(WebError::Internal(anyhow::anyhow!(
            "project {id} has an unknown primary track"
        )));
    };

    let authority = users::load_authority(state.pool(), user.id).await?;
    if !authority.can(Action::PublishProjectAsReleased(primary)) {
        return Err(WebError::Forbidden);
    }

    let report = projects::release(
        state.pool(),
        state.announce_channel(),
        user.id,
        id,
    )
    .await?;
    Ok(Json(report))
}

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
    /// Repository created for the project, when the GitHub integration
    /// is configured and GitHub cooperated.
    github_repo_url: Option<String>,
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

    // The repository is a convenience, not a precondition. GitHub being
    // down must not stop a member starting a project, so a failure here
    // is logged and the project stands without a repository.
    let github_repo_url = provision_repo(&state, &user, id, &body).await;

    Ok(Json(CreatedResponse {
        id,
        github_repo_url,
    }))
}

/// Create the project's repository, add the author, install the webhook.
///
/// Returns the repository URL when everything that matters succeeded.
/// Each step is independent: a repository with no webhook is still more
/// useful than no repository, so a webhook failure does not discard it.
async fn provision_repo(
    state: &AppState,
    user: &CurrentUser,
    project_id: Uuid,
    body: &projects::NewProject,
) -> Option<String> {
    let github = state.github()?;

    let repo = match github
        .create_repo(&body.name, body.short_description.as_deref())
        .await
    {
        Ok(repo) => repo,
        Err(e) => {
            tracing::warn!(error = %e, project = %project_id, "github: repository not created");
            return None;
        }
    };

    // Push access for the author, when they have linked a GitHub login.
    // Without one there is nobody to add — and linking it is also what
    // makes their commits pay XP, so the profile page nags for it.
    if let Some(login) = user.record.github_username.as_deref() {
        if let Err(e) = github.add_collaborator(&repo.name, login).await {
            tracing::warn!(error = %e, login, "github: collaborator not added");
        }
    } else {
        tracing::info!(
            project = %project_id,
            "github: author has no linked GitHub login; repository left without a collaborator"
        );
    }

    // The webhook is what makes commits in this repository pay XP. A
    // localhost origin can never receive one, so do not install a hook
    // that would only ever fail.
    let cfg = state.config();
    let origin = &cfg.public_origin;
    match (&cfg.github_webhook_secret, origin.contains("localhost")) {
        (Some(secret), false) => {
            let callback = format!("{}/api/webhooks/github", origin.trim_end_matches('/'));
            if let Err(e) = github
                .add_webhook(&repo.name, &callback, &String::from_utf8_lossy(secret))
                .await
            {
                tracing::warn!(error = %e, repo = %repo.name, "github: webhook not installed");
            }
        }
        (_, true) => tracing::info!(
            "github: PUBLIC_ORIGIN is localhost, skipping webhook (it could never be delivered)"
        ),
        (None, _) => tracing::info!("github: GITHUB_WEBHOOK_SECRET unset, skipping webhook"),
    }

    if let Err(e) = projects::set_repo_url(state.pool(), project_id, &repo.html_url).await {
        tracing::warn!(error = %e, "github: repository created but URL not recorded");
    }
    Some(repo.html_url)
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
        state.channels(),
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
        state.channels(),
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
        state.channels(),
        user.id,
        id,
    )
    .await?;

    // A released project is the association's shop window, so its
    // repository stops being private. Creating private and opening at
    // release is the safe order: a repository can always be opened
    // later, but code that leaked cannot be un-leaked.
    if let (Some(github), Some(repo)) = (
        state.github(),
        detail
            .github_repo_url
            .as_deref()
            .and_then(projects::repo_name_from_url),
    ) {
        if let Err(e) = github.make_public(repo).await {
            tracing::warn!(error = %e, repo, "github: repository not opened on release");
        } else {
            tracing::info!(repo, "github: repository opened on release");
        }
    }

    Ok(Json(report))
}

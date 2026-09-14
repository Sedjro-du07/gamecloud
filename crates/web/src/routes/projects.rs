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
        .route(
            "/{id}/files",
            get(list_files).post(upload_file)
                // Builds are large by nature. The handler streams to a
                // temporary file and rejects anything over 500 MB itself,
                // so the framework limit would only get in the way.
                .layer(axum::extract::DefaultBodyLimit::disable()),
        )
        .route("/files/{file_id}/download", get(download_file))
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
    // `?internal=true` asks to see work in progress. What comes back is
    // still decided per track by the permission matrix — asking for it
    // grants nothing on its own.
    let mut tracks = Vec::new();
    let mut viewer = None;

    if q.internal.unwrap_or(false) {
        if let Some(user) = &user {
            let authority = users::load_authority(state.pool(), user.id).await?;
            tracks = users::visible_tracks(&authority);
            viewer = Some(user.id);
        }
    }

    Ok(Json(
        projects::list(
            state.pool(),
            q.track.as_deref(),
            &projects::Visibility {
                tracks: &tracks,
                viewer,
            },
        )
        .await?,
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
    let github_repo_url = crate::services::github::provision_project_repo(
        &state,
        &crate::services::github::NewRepo {
            project_id: id,
            name: &body.name,
            description: body.short_description.as_deref(),
            author_login: user.record.github_username.as_deref(),
        },
    )
    .await;

    Ok(Json(CreatedResponse {
        id,
        github_repo_url,
    }))
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
        state.channels(),
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
    /// Mark out of 100 for this track's share of the work. Optional: a
    /// reviewer may gate without grading.
    score: Option<i32>,
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
        &projects::Judgement {
            project_id: id,
            track: track.as_str(),
            verdict,
            feedback: body.feedback.as_deref(),
            score: body.score,
        },
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

// ---------------------------------------------------------------------------
// Builds
// ---------------------------------------------------------------------------

/// What a build upload carried.
struct Upload {
    version: String,
    changelog: Option<String>,
    /// Filename, temp path, size and checksum of the received file.
    file: Option<(String, std::path::PathBuf, u64, String)>,
}

/// Drain a multipart body into a temporary file.
///
/// Streaming to disk rather than buffering is the whole point: a 500 MB
/// build held in memory would be a denial of service against our own
/// server. The SHA-256 is computed on the way past, which costs nothing
/// and lets a download be checked against the record later.
async fn consume_upload(mut multipart: axum::extract::Multipart) -> WebResult<Upload> {
    use sha2::{Digest, Sha256};
    use tokio::io::AsyncWriteExt;

    let mut out = Upload {
        version: "v1.0".to_string(),
        changelog: None,
        file: None,
    };

    while let Some(mut field) = multipart
        .next_field()
        .await
        .map_err(|e| WebError::Validation(format!("formulaire illisible : {e}")))?
    {
        match field.name().unwrap_or_default() {
            "version" => out.version = field.text().await.unwrap_or(out.version),
            "changelog" => {
                out.changelog = field.text().await.ok().filter(|t| !t.trim().is_empty());
            }
            "file" => {
                let filename = field
                    .file_name()
                    .map(ToString::to_string)
                    .ok_or_else(|| WebError::Validation("fichier sans nom".into()))?;

                let path = std::env::temp_dir().join(format!("gc-upload-{}", Uuid::new_v4()));
                let mut sink = tokio::fs::File::create(&path)
                    .await
                    .map_err(|e| WebError::Internal(anyhow::anyhow!("temp file: {e}")))?;

                let mut hasher = Sha256::new();
                let mut size: u64 = 0;
                while let Some(chunk) = field
                    .chunk()
                    .await
                    .map_err(|e| WebError::Validation(format!("lecture interrompue : {e}")))?
                {
                    size += chunk.len() as u64;
                    if size > crate::db::queries::files::MAX_FILE_BYTES {
                        let _ = tokio::fs::remove_file(&path).await;
                        return Err(WebError::Validation(
                            "fichier trop volumineux (500 Mo maximum)".into(),
                        ));
                    }
                    hasher.update(&chunk);
                    sink.write_all(&chunk)
                        .await
                        .map_err(|e| WebError::Internal(anyhow::anyhow!("temp write: {e}")))?;
                }
                sink.flush()
                    .await
                    .map_err(|e| WebError::Internal(anyhow::anyhow!("temp flush: {e}")))?;

                let digest = hasher.finalize();
                let mut checksum = String::with_capacity(digest.len() * 2);
                for byte in digest {
                    use std::fmt::Write;
                    let _ = write!(&mut checksum, "{byte:02x}");
                }
                out.file = Some((filename, path, size, checksum));
            }
            _ => {}
        }
    }

    Ok(out)
}

/// Upload a build for a project.
///
/// The bytes go to a GitHub release beside the code, streamed through a
/// temporary file so a 500 MB build never sits in this process's memory.
/// The SHA-256 is computed on the way past, which costs nothing extra
/// and lets a download be checked against the record afterwards.
async fn upload_file(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
    multipart: axum::extract::Multipart,
) -> WebResult<Json<serde_json::Value>> {
    let detail = projects::detail(state.pool(), id).await?;
    let authority = users::load_authority(state.pool(), user.id).await?;

    // Publishing a build speaks for the whole team, so it is not open to
    // any member who happens to have the rank to create projects. Either
    // you are credited on this project, or you lead the discipline that
    // owns it.
    let on_team = detail.contributors.iter().any(|c| c.user_id == user.id);
    let leads_it = Track::parse(&detail.summary.primary_track).is_some_and(|t| {
        authority.has_track_role(t, gamecloud_shared::roles::TrackRole::CoLead)
    });
    if !on_team && !leads_it {
        return Err(WebError::Forbidden);
    }

    let repo = detail
        .github_repo_url
        .as_deref()
        .and_then(projects::repo_name_from_url)
        .ok_or_else(|| {
            WebError::Validation("ce projet n'a pas de dépôt GitHub associé".into())
        })?;
    let github = state
        .github()
        .ok_or_else(|| WebError::Validation("l'intégration GitHub n'est pas configurée".into()))?;

    let Upload {
        version,
        changelog,
        file,
    } = consume_upload(multipart).await?;

    let (filename, path, size, checksum) =
        file.ok_or_else(|| WebError::Validation("aucun fichier reçu".into()))?;

    let handle = tokio::fs::File::open(&path)
        .await
        .map_err(|e| WebError::Internal(anyhow::anyhow!("temp reopen: {e}")))?;

    let result = github
        .upload_asset(repo, &version, &filename, handle, size)
        .await;
    // The temp file has done its job either way.
    let _ = tokio::fs::remove_file(&path).await;
    let asset = result?;

    let file_id = crate::db::queries::files::record(
        state.pool(),
        &crate::db::queries::files::NewFile {
            project_id: id,
            uploaded_by: user.id,
            filename: &filename,
            size_bytes: i64::try_from(size).unwrap_or(i64::MAX),
            checksum: &checksum,
            asset_id: asset.id,
            version: &version,
            changelog: changelog.as_deref(),
        },
    )
    .await?;

    Ok(Json(serde_json::json!({
        "id": file_id,
        "filename": asset.name,
        "size_bytes": asset.size,
        "version": version,
        "checksum_sha256": checksum,
    })))
}

/// List a project's builds.
async fn list_files(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> WebResult<Json<Vec<crate::db::queries::files::ProjectFile>>> {
    Ok(Json(crate::db::queries::files::list(state.pool(), id).await?))
}

/// Stream a build back to the member.
///
/// The platform proxies rather than redirecting. A private repository's
/// release asset is not fetchable by link, so a redirect would simply
/// fail — and proxying keeps the decision about who may download inside
/// the permission model rather than delegating it to GitHub's.
async fn download_file(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(file_id): Path<Uuid>,
) -> WebResult<axum::response::Response> {
    use axum::response::IntoResponse;

    let file = crate::db::queries::files::find(state.pool(), file_id).await?;
    let detail = projects::detail(state.pool(), file.project_id).await?;

    // Published work is open to every member. Work still in progress is
    // for the people concerned: those credited on it, and the tracks
    // actually being asked for a verdict — not merely the primary one,
    // since an Audio reviewer needs the build to judge the audio.
    let status = gamecloud_shared::projects::ProjectStatus::parse(&detail.summary.status);
    let is_public = status.is_some_and(gamecloud_shared::projects::ProjectStatus::is_public);
    if !is_public {
        let authority = users::load_authority(state.pool(), user.id).await?;
        let on_team = detail.contributors.iter().any(|c| c.user_id == user.id);

        let concerned = detail
            .validations
            .iter()
            .map(|v| v.track.as_str())
            .chain(std::iter::once(detail.summary.primary_track.as_str()))
            .filter_map(Track::parse)
            .any(|t| authority.can(Action::ViewTrackInternalProjects(t)));

        if !on_team && !concerned {
            return Err(WebError::Forbidden);
        }
    }

    let repo = detail
        .github_repo_url
        .as_deref()
        .and_then(projects::repo_name_from_url)
        .ok_or(WebError::NotFound)?;
    let asset_id: u64 = file
        .storage_path
        .parse()
        .map_err(|_| WebError::Internal(anyhow::anyhow!("bad asset id")))?;

    let github = state.github().ok_or(WebError::NotFound)?;
    let upstream = github.download_asset(repo, asset_id).await?;

    let stream = upstream.bytes_stream();
    let body = axum::body::Body::from_stream(stream);

    Ok((
        [
            (
                axum::http::header::CONTENT_TYPE,
                "application/octet-stream".to_string(),
            ),
            (
                axum::http::header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{}\"", file.filename),
            ),
        ],
        body,
    )
        .into_response())
}

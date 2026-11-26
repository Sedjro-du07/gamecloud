//! Game jam routes that carry whole files.
//!
//! - `POST /api/jams/:id/entry`            — hand in the team's game (multipart:
//!   `title`, `description`, `url`, `file`); back to the jam page.
//! - `GET  /api/jams/entries/:team/file`   — download a game, for members.
//!
//! Everything else (teams, votes, results) is a server function.

use axum::{
    extract::{DefaultBodyLimit, Multipart, Path, State},
    http::header,
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
    Router,
};
use uuid::Uuid;

use crate::{
    db::queries::{jams, users},
    error::{WebError, WebResult},
    middleware::auth::CurrentUser,
    services::uploads,
    state::AppState,
};

/// The jam routes.
pub fn router() -> Router<AppState> {
    Router::new()
        // The upload streams to disk and enforces its own 500 MB cap.
        .route("/{id}/entry", post(submit).layer(DefaultBodyLimit::disable()))
        .route("/entries/{team}/file", get(download))
}

fn dir(state: &AppState) -> std::path::PathBuf {
    state.config().shares_dir.join("jams")
}

fn back(jam: Uuid, result: WebResult<()>) -> Response {
    match result {
        Ok(()) => Redirect::to(&format!("/jams/{jam}?ok=rendu")).into_response(),
        Err(e) => {
            let message = match e {
                WebError::Validation(m) => m,
                WebError::Unauthorized => "connecte-toi d'abord".into(),
                WebError::NotFound => "jam introuvable".into(),
                other => {
                    tracing::error!(error = ?other, "jam entry failed");
                    "l'envoi a échoué, réessaie dans un instant".into()
                }
            };
            let query = serde_urlencoded::to_string([("erreur", message)]).unwrap_or_default();
            Redirect::to(&format!("/jams/{jam}?{query}")).into_response()
        }
    }
}

// Taken by value because `map_err` hands the error over by value.
#[allow(clippy::needless_pass_by_value)]
fn unreadable(e: axum::extract::multipart::MultipartError) -> WebError {
    WebError::Validation(format!("formulaire illisible : {e}"))
}

async fn submit(
    State(state): State<AppState>,
    user: Option<CurrentUser>,
    Path(id): Path<Uuid>,
    multipart: Multipart,
) -> Response {
    let result = async {
        let user = user.ok_or(WebError::Unauthorized)?;
        receive(&state, id, user.id, multipart).await
    }
    .await;
    back(id, result)
}

async fn receive(state: &AppState, jam_id: Uuid, user_id: Uuid, mut multipart: Multipart) -> WebResult<()> {
    let dir = dir(state);
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| WebError::Internal(anyhow::anyhow!("jam dir: {e}")))?;

    let mut entry = jams::NewEntry::default();
    let read: WebResult<()> = async {
        while let Some(mut field) = multipart.next_field().await.map_err(unreadable)? {
            match field.name().unwrap_or_default() {
                "title" => entry.title = field.text().await.map_err(unreadable)?,
                "description" => entry.description = Some(field.text().await.map_err(unreadable)?),
                "url" => entry.url = Some(field.text().await.map_err(unreadable)?),
                "file" => {
                    let Some(name) = field.file_name().filter(|n| !n.is_empty()).map(uploads::clean_filename) else {
                        continue;
                    };
                    let stored = Uuid::new_v4().to_string();
                    let written = uploads::stream_to_file(&mut field, &dir.join(&stored)).await?;
                    if written.size == 0 {
                        let _ = tokio::fs::remove_file(dir.join(&stored)).await;
                        continue;
                    }
                    entry.file = Some((name, i64::try_from(written.size).unwrap_or(i64::MAX), written.checksum, stored));
                }
                _ => {}
            }
        }
        Ok(())
    }
    .await;

    let recorded = match read {
        Ok(()) => jams::submit(state.pool(), jam_id, user_id, &entry).await,
        Err(e) => Err(e),
    };
    match recorded {
        Ok(replaced) => {
            if let Some(old) = replaced {
                let _ = tokio::fs::remove_file(dir.join(old)).await;
            }
            Ok(())
        }
        Err(e) => {
            if let Some((_, _, _, stored)) = &entry.file {
                let _ = tokio::fs::remove_file(dir.join(stored)).await;
            }
            Err(e)
        }
    }
}

async fn download(
    State(state): State<AppState>,
    user: Option<CurrentUser>,
    Path(team): Path<Uuid>,
) -> WebResult<Response> {
    let Some(user) = user else {
        return Ok(Redirect::to("/api/auth/login").into_response());
    };
    if !users::is_member(state.pool(), user.id).await? {
        return Err(WebError::Forbidden);
    }
    let (stored, filename, size) = jams::entry_file(state.pool(), team).await?;
    let stored = Uuid::parse_str(&stored).map_err(|_| WebError::NotFound)?;
    let handle = tokio::fs::File::open(dir(&state).join(stored.to_string()))
        .await
        .map_err(|_| WebError::NotFound)?;
    let body = axum::body::Body::from_stream(tokio_util::io::ReaderStream::new(handle));
    Ok((
        [
            (header::CONTENT_TYPE, "application/octet-stream".to_string()),
            (header::CONTENT_DISPOSITION, uploads::attachment_header(&filename)),
            (header::CONTENT_LENGTH, size.to_string()),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff".to_string()),
        ],
        body,
    )
        .into_response())
}

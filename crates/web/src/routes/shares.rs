//! Member share routes.
//!
//! - `POST /api/shares`               — post a file or a link (multipart).
//! - `GET  /api/shares/:id/download`  — download, counted.
//!
//! Listing and removal are server functions; these two stay plain HTTP
//! because one takes a 500 MB body and the other streams one back, and
//! neither belongs on the WASM boundary.

use axum::{
    extract::{DefaultBodyLimit, Multipart, Path, State},
    http::{header, HeaderMap},
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
    Json, Router,
};
use gamecloud_shared::DomainError;
use uuid::Uuid;

use crate::{
    db::queries::shares::{self, Content, NewShare},
    error::{WebError, WebResult},
    middleware::auth::CurrentUser,
    services::uploads::{self, Written},
    state::AppState,
};

/// Build the shares router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/",
            // The handler streams to disk and refuses anything over
            // 500 MB itself; the global 16 MB limit would only get in
            // the way.
            post(upload).layer(DefaultBodyLimit::disable()),
        )
        .route("/{id}/download", get(download))
}

/// The form as received.
#[derive(Default)]
struct Form {
    title: String,
    description: Option<String>,
    kind: String,
    url: Option<String>,
    /// Original name, stored name, and what was written.
    file: Option<(String, String, Written)>,
}

/// Post a share.
///
/// Answers a browser form with a redirect back to the page, carrying
/// either `ok` or the reason it failed, so the member never lands on a
/// raw JSON body. Anything else gets JSON.
async fn upload(
    State(state): State<AppState>,
    user: Option<CurrentUser>,
    headers: HeaderMap,
    multipart: Multipart,
) -> Response {
    let from_browser = headers
        .get(header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|accept| accept.contains("text/html"));

    let result = match user {
        Some(user) => receive(&state, &user, multipart).await,
        None => Err(WebError::Unauthorized),
    };

    match (result, from_browser) {
        (Ok(_), true) => Redirect::to("/shares?ok=1").into_response(),
        (Ok(id), false) => Json(serde_json::json!({ "id": id })).into_response(),
        (Err(e), true) => {
            let message = form_message(&e);
            let query = serde_urlencoded::to_string([("erreur", message)]).unwrap_or_default();
            Redirect::to(&format!("/shares?{query}")).into_response()
        }
        (Err(e), false) => e.into_response(),
    }
}

/// What the page shows when a posted form fails.
fn form_message(e: &WebError) -> String {
    match e {
        WebError::Unauthorized => "connecte-toi pour partager".into(),
        WebError::Domain(DomainError::EmailNotVerified) => {
            "vérifie ton adresse Epitech avant de partager".into()
        }
        WebError::Validation(message) => message.clone(),
        other => {
            tracing::error!(error = ?other, "share upload failed");
            "l'envoi a échoué, réessaie dans un instant".into()
        }
    }
}

async fn receive(state: &AppState, user: &CurrentUser, mut multipart: Multipart) -> WebResult<Uuid> {
    if !user.record.email_verified {
        return Err(WebError::Domain(DomainError::EmailNotVerified));
    }

    let dir = state.config().shares_dir.clone();
    tokio::fs::create_dir_all(&dir).await.map_err(|e| {
        WebError::Internal(anyhow::anyhow!("shares dir {}: {e}", dir.display()))
    })?;

    let mut form = Form::default();
    let read = fill(&mut form, &mut multipart, &dir).await;
    let created = match read {
        Ok(()) => record(state, user, &form).await,
        Err(e) => Err(e),
    };

    // Whatever went wrong after the bytes landed, they must not stay on
    // disk with no row pointing at them.
    if created.is_err() {
        if let Some((_, stored, _)) = &form.file {
            let _ = tokio::fs::remove_file(dir.join(stored)).await;
        }
    }
    created
}

async fn fill(form: &mut Form, multipart: &mut Multipart, dir: &std::path::Path) -> WebResult<()> {
    let unreadable = |e: axum::extract::multipart::MultipartError| {
        WebError::Validation(format!("formulaire illisible : {e}"))
    };

    while let Some(mut field) = multipart.next_field().await.map_err(unreadable)? {
        match field.name().unwrap_or_default() {
            "title" => form.title = field.text().await.map_err(unreadable)?,
            "kind" => form.kind = field.text().await.map_err(unreadable)?,
            "description" => form.description = Some(field.text().await.map_err(unreadable)?),
            "url" => {
                let url = field.text().await.map_err(unreadable)?;
                form.url = Some(url.trim().to_string()).filter(|u| !u.is_empty());
            }
            "file" => {
                // A browser sends an empty file part when nothing was
                // picked; that is "no file", not a file called "".
                let Some(name) = field
                    .file_name()
                    .filter(|n| !n.is_empty())
                    .map(uploads::clean_filename)
                else {
                    continue;
                };
                if form.file.is_some() {
                    return Err(WebError::Validation("un seul fichier par partage".into()));
                }
                let stored = Uuid::new_v4().to_string();
                let written = uploads::stream_to_file(&mut field, &dir.join(&stored)).await?;
                let empty = written.size == 0;
                form.file = Some((name, stored, written));
                if empty {
                    return Err(WebError::Validation("le fichier est vide".into()));
                }
            }
            _ => {}
        }
    }
    Ok(())
}

async fn record(state: &AppState, user: &CurrentUser, form: &Form) -> WebResult<Uuid> {
    let content = match (&form.file, form.url.as_deref()) {
        (Some(_), Some(_)) => {
            return Err(WebError::Validation(
                "choisis un fichier ou un lien, pas les deux".into(),
            ))
        }
        (None, None) => return Err(WebError::Validation("ajoute un fichier ou un lien".into())),
        (Some((filename, stored, written)), None) => Content::File {
            filename,
            size_bytes: i64::try_from(written.size).unwrap_or(i64::MAX),
            checksum: &written.checksum,
            storage_name: stored,
        },
        (None, Some(url)) => Content::Link(url),
    };

    let page_url = format!("{}/shares", state.config().public_origin.trim_end_matches('/'));
    shares::create(
        state.pool(),
        state.channels(),
        &page_url,
        &NewShare {
            title: &form.title,
            description: form.description.as_deref(),
            kind: &form.kind,
            content,
            uploaded_by: user.id,
        },
    )
    .await
}

/// Download a share, counting it.
///
/// Members only. A link is counted and then redirected to, so both kinds
/// of share report the same number. A visitor who is not signed in is
/// sent to sign in rather than shown an error.
async fn download(
    State(state): State<AppState>,
    user: Option<CurrentUser>,
    Path(id): Path<Uuid>,
) -> WebResult<Response> {
    let Some(user) = user else {
        return Ok(Redirect::to("/api/auth/login").into_response());
    };
    // Signed in but not yet a member: send them to finish signing up
    // rather than showing a JSON refusal in place of their download.
    if !user.record.email_verified {
        return Ok(Redirect::to("/onboarding/email").into_response());
    }

    let share = shares::find(state.pool(), id).await?;

    if let Some(url) = &share.url {
        // Re-checked even though the insert checked it: this value is
        // about to become a Location header.
        shares::validate_link(url)?;
        shares::count_download(state.pool(), id).await?;
        return Ok(Redirect::temporary(url).into_response());
    }

    // Stored names are UUIDs this server generated. Parsing one back is
    // what guarantees the path cannot leave the shares directory.
    let stored = share
        .storage_name
        .as_deref()
        .and_then(|s| Uuid::parse_str(s).ok())
        .ok_or(WebError::NotFound)?;
    let path = state.config().shares_dir.join(stored.to_string());
    let handle = tokio::fs::File::open(&path).await.map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            tracing::warn!(share = %id, path = %path.display(), "share file missing on disk");
            WebError::NotFound
        } else {
            WebError::Internal(anyhow::anyhow!("share open: {e}"))
        }
    })?;

    shares::count_download(state.pool(), id).await?;

    let filename = share.filename.as_deref().unwrap_or("fichier");
    let body = axum::body::Body::from_stream(tokio_util::io::ReaderStream::new(handle));
    Ok((
        [
            (header::CONTENT_TYPE, "application/octet-stream".to_string()),
            (header::CONTENT_DISPOSITION, uploads::attachment_header(filename)),
            (
                header::CONTENT_LENGTH,
                share.size_bytes.unwrap_or_default().to_string(),
            ),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff".to_string()),
        ],
        body,
    )
        .into_response())
}

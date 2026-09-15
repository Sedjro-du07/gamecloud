//! Entrance test routes.
//!
//! - `POST /api/tests`                          — open a session (Bureau, multipart).
//! - `GET  /api/tests/:id/subject`              — the PDF subject.
//! - `POST /api/tests/:id/submit`               — hand in work (candidate, multipart).
//! - `GET  /api/tests/submissions/:id/download` — a candidate's work (Bureau).
//!
//! Listing, grading, closing and deleting are server functions. These four
//! stay plain HTTP because they carry or return whole files.
//!
//! Who may do what: the Bureau (any office holder) runs sessions and reads
//! the work; an account whose Epitech address is not verified — somebody
//! who is not a member yet — takes tests. Members never see any of it.

use std::path::{Path as FsPath, PathBuf};

use axum::{
    extract::{DefaultBodyLimit, Multipart, Path, State},
    http::{header, HeaderMap},
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
    Json, Router,
};
use chrono::Utc;
use gamecloud_shared::roles::Action;
use uuid::Uuid;

use crate::{
    db::queries::{entrance, users},
    error::{WebError, WebResult},
    middleware::auth::CurrentUser,
    services::uploads::{self, Written},
    state::AppState,
};

/// Longest session the Bureau may open: thirty days.
const MAX_HOURS: i64 = 24 * 30;

/// Build the entrance tests router.
pub fn router() -> Router<AppState> {
    Router::new()
        // Both uploads stream to disk and enforce their own 500 MB cap.
        .route("/", post(open_session).layer(DefaultBodyLimit::disable()))
        .route("/{id}/subject", get(subject))
        .route("/{id}/submit", post(submit).layer(DefaultBodyLimit::disable()))
        .route("/submissions/{id}/download", get(download_submission))
}

/// Where subjects are kept.
#[must_use]
pub fn subjects_dir(state: &AppState) -> PathBuf {
    state.config().tests_dir.join("subjects")
}

/// Where candidates' work is kept.
#[must_use]
pub fn submissions_dir(state: &AppState) -> PathBuf {
    state.config().tests_dir.join("submissions")
}

async fn is_bureau(state: &AppState, user: &CurrentUser) -> WebResult<bool> {
    Ok(users::load_authority(state.pool(), user.id)
        .await?
        .can(Action::AccessAdminPanel))
}

/// Answer a posted form: back to the page with `ok` or the reason it
/// failed, so nobody lands on a raw JSON body. Anything else gets JSON.
fn answer(result: WebResult<serde_json::Value>, headers: &HeaderMap, ok: &str) -> Response {
    let from_browser = headers
        .get(header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|accept| accept.contains("text/html"));

    match (result, from_browser) {
        (Ok(_), true) => Redirect::to(&format!("/tests?ok={ok}")).into_response(),
        (Ok(body), false) => Json(body).into_response(),
        (Err(e), true) => {
            let query = serde_urlencoded::to_string([("erreur", message(&e))]).unwrap_or_default();
            Redirect::to(&format!("/tests?{query}")).into_response()
        }
        (Err(e), false) => e.into_response(),
    }
}

/// What the page shows when a posted form fails.
fn message(e: &WebError) -> String {
    match e {
        WebError::Unauthorized => "connecte-toi avec Discord d'abord".into(),
        WebError::Forbidden => "réservé au Bureau et aux candidats".into(),
        WebError::NotFound => "test introuvable".into(),
        WebError::Validation(message) => message.clone(),
        other => {
            tracing::error!(error = ?other, "entrance test form failed");
            "l'envoi a échoué, réessaie dans un instant".into()
        }
    }
}

// Taken by value because `map_err` hands the error over by value.
#[allow(clippy::needless_pass_by_value)]
fn unreadable(e: axum::extract::multipart::MultipartError) -> WebError {
    WebError::Validation(format!("formulaire illisible : {e}"))
}

async fn ensure_dir(dir: &FsPath) -> WebResult<()> {
    tokio::fs::create_dir_all(dir)
        .await
        .map_err(|e| WebError::Internal(anyhow::anyhow!("tests dir {}: {e}", dir.display())))
}

/// Receive one file field into `dir` under a fresh UUID.
async fn receive_file(
    field: &mut axum::extract::multipart::Field<'_>,
    dir: &FsPath,
) -> WebResult<Option<(String, String, Written)>> {
    // A browser sends an empty part when no file was picked.
    let Some(name) = field
        .file_name()
        .filter(|n| !n.is_empty())
        .map(uploads::clean_filename)
    else {
        return Ok(None);
    };
    let stored = Uuid::new_v4().to_string();
    let written = uploads::stream_to_file(field, &dir.join(&stored)).await?;
    if written.size == 0 {
        let _ = tokio::fs::remove_file(dir.join(&stored)).await;
        return Err(WebError::Validation("le fichier est vide".into()));
    }
    Ok(Some((name, stored, written)))
}

// ---------------------------------------------------------------------------
// Opening a session
// ---------------------------------------------------------------------------

async fn open_session(
    State(state): State<AppState>,
    user: Option<CurrentUser>,
    headers: HeaderMap,
    multipart: Multipart,
) -> Response {
    let result: WebResult<serde_json::Value> = async {
        let user = user.ok_or(WebError::Unauthorized)?;
        if !is_bureau(&state, &user).await? {
            return Err(WebError::Forbidden);
        }
        let id = receive_session(&state, &user, multipart).await?;
        Ok(serde_json::json!({ "id": id }))
    }
    .await;
    answer(result, &headers, "session")
}

/// The session form as received.
#[derive(Default)]
struct SessionForm {
    title: String,
    description: Option<String>,
    hours: String,
    subject: Option<(String, String, Written)>,
}

async fn receive_session(state: &AppState, user: &CurrentUser, mut multipart: Multipart) -> WebResult<Uuid> {
    let dir = subjects_dir(state);
    ensure_dir(&dir).await?;

    let mut form = SessionForm::default();
    let read = read_session(&mut form, &mut multipart, &dir).await;
    let created = match read {
        Ok(()) => create_session(state, user, &form, &dir).await,
        Err(e) => Err(e),
    };
    // A refused session must not leave its subject on disk.
    if created.is_err() {
        if let Some((_, stored, _)) = &form.subject {
            let _ = tokio::fs::remove_file(dir.join(stored)).await;
        }
    }
    created
}

async fn read_session(form: &mut SessionForm, multipart: &mut Multipart, dir: &FsPath) -> WebResult<()> {
    while let Some(mut field) = multipart.next_field().await.map_err(unreadable)? {
        match field.name().unwrap_or_default() {
            "title" => form.title = field.text().await.map_err(unreadable)?,
            "description" => form.description = Some(field.text().await.map_err(unreadable)?),
            "hours" => form.hours = field.text().await.map_err(unreadable)?,
            "subject" => {
                if form.subject.is_some() {
                    return Err(WebError::Validation("un seul sujet par test".into()));
                }
                form.subject = receive_file(&mut field, dir).await?;
            }
            _ => {}
        }
    }
    Ok(())
}

async fn create_session(state: &AppState, user: &CurrentUser, form: &SessionForm, dir: &FsPath) -> WebResult<Uuid> {
    let Some((filename, stored, written)) = &form.subject else {
        return Err(WebError::Validation("ajoute le sujet, en PDF".into()));
    };
    if !is_pdf(&dir.join(stored)).await {
        return Err(WebError::Validation("le sujet doit être un fichier PDF".into()));
    }
    let hours: i64 = form
        .hours
        .trim()
        .parse()
        .map_err(|_| WebError::Validation("indique la durée du test en heures".into()))?;
    if !(1..=MAX_HOURS).contains(&hours) {
        return Err(WebError::Validation(format!(
            "la durée doit être comprise entre 1 et {MAX_HOURS} heures"
        )));
    }

    entrance::create(
        state.pool(),
        &entrance::NewTest {
            title: &form.title,
            description: form.description.as_deref(),
            closes_at: Utc::now() + chrono::Duration::hours(hours),
            subject_filename: filename,
            subject_size: i64::try_from(written.size).unwrap_or(i64::MAX),
            subject_checksum: &written.checksum,
            subject_storage: stored,
            created_by: user.id,
        },
    )
    .await
}

/// Whether a stored file really is a PDF, judged by its first bytes
/// rather than by the name somebody gave it.
async fn is_pdf(path: &FsPath) -> bool {
    use tokio::io::AsyncReadExt;

    let Ok(mut file) = tokio::fs::File::open(path).await else {
        return false;
    };
    let mut head = [0_u8; 5];
    file.read_exact(&mut head).await.is_ok() && &head == b"%PDF-"
}

// ---------------------------------------------------------------------------
// Subject
// ---------------------------------------------------------------------------

/// The subject. The Bureau always; a candidate while the session is open.
async fn subject(
    State(state): State<AppState>,
    user: Option<CurrentUser>,
    Path(id): Path<Uuid>,
) -> WebResult<Response> {
    let Some(user) = user else {
        return Ok(Redirect::to("/api/auth/login").into_response());
    };
    let test = entrance::find(state.pool(), id).await?;
    let candidate_may = !user.record.email_verified && test.closes_at > Utc::now();
    if !candidate_may && !is_bureau(&state, &user).await? {
        return Err(WebError::Forbidden);
    }
    stream_file(
        &subjects_dir(&state),
        &test.subject_storage,
        &test.subject_filename,
        test.subject_size,
        "application/pdf",
    )
    .await
}

// ---------------------------------------------------------------------------
// Handing in
// ---------------------------------------------------------------------------

async fn submit(
    State(state): State<AppState>,
    user: Option<CurrentUser>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    multipart: Multipart,
) -> Response {
    let result: WebResult<serde_json::Value> = async {
        let user = user.ok_or(WebError::Unauthorized)?;
        if user.record.email_verified {
            return Err(WebError::Validation(
                "les tests d'entrée sont pour les candidats : tu es déjà membre".into(),
            ));
        }
        receive_submission(&state, &user, id, multipart).await?;
        Ok(serde_json::json!({ "ok": true }))
    }
    .await;
    answer(result, &headers, "rendu")
}

async fn receive_submission(state: &AppState, user: &CurrentUser, test_id: Uuid, mut multipart: Multipart) -> WebResult<()> {
    // Refuse before taking the bytes: no point storing half a gigabyte
    // for a session that closed yesterday. `entrance::submit` checks
    // again under a lock.
    let test = entrance::find(state.pool(), test_id).await?;
    if test.closes_at <= Utc::now() {
        return Err(WebError::Validation("ce test est terminé : les rendus sont fermés".into()));
    }

    let dir = submissions_dir(state);
    ensure_dir(&dir).await?;

    let mut file: Option<(String, String, Written)> = None;
    let mut comment: Option<String> = None;
    let read: WebResult<()> = async {
        while let Some(mut field) = multipart.next_field().await.map_err(unreadable)? {
            match field.name().unwrap_or_default() {
                "comment" => comment = Some(field.text().await.map_err(unreadable)?),
                "file" => {
                    if file.is_some() {
                        return Err(WebError::Validation("un seul fichier par rendu".into()));
                    }
                    file = receive_file(&mut field, &dir).await?;
                }
                _ => {}
            }
        }
        Ok(())
    }
    .await;

    let recorded = match (read, &file) {
        (Err(e), _) => Err(e),
        (Ok(()), None) => Err(WebError::Validation("ajoute ton travail (un fichier)".into())),
        (Ok(()), Some((filename, stored, written))) => {
            entrance::submit(
                state.pool(),
                &entrance::NewSubmission {
                    test_id,
                    user_id: user.id,
                    filename,
                    size_bytes: i64::try_from(written.size).unwrap_or(i64::MAX),
                    checksum: &written.checksum,
                    storage_name: stored,
                    comment: comment.as_deref(),
                },
            )
            .await
        }
    };

    match recorded {
        Ok(replaced) => {
            if let Some(old) = replaced {
                let _ = tokio::fs::remove_file(dir.join(old)).await;
            }
            Ok(())
        }
        Err(e) => {
            if let Some((_, stored, _)) = &file {
                let _ = tokio::fs::remove_file(dir.join(stored)).await;
            }
            Err(e)
        }
    }
}

// ---------------------------------------------------------------------------
// Reading the work
// ---------------------------------------------------------------------------

async fn download_submission(
    State(state): State<AppState>,
    user: Option<CurrentUser>,
    Path(id): Path<Uuid>,
) -> WebResult<Response> {
    let Some(user) = user else {
        return Ok(Redirect::to("/api/auth/login").into_response());
    };
    if !is_bureau(&state, &user).await? {
        return Err(WebError::Forbidden);
    }
    let work = entrance::find_submission(state.pool(), id).await?;
    stream_file(
        &submissions_dir(&state),
        &work.storage_name,
        &work.filename,
        work.size_bytes,
        "application/octet-stream",
    )
    .await
}

/// Stream a stored file as a download.
///
/// Stored names are UUIDs this server generated; parsing one back is what
/// guarantees the path cannot leave `dir`.
async fn stream_file(dir: &FsPath, stored: &str, filename: &str, size: i64, content_type: &str) -> WebResult<Response> {
    let stored = Uuid::parse_str(stored).map_err(|_| WebError::NotFound)?;
    let path = dir.join(stored.to_string());
    let handle = tokio::fs::File::open(&path).await.map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            tracing::warn!(path = %path.display(), "entrance test file missing on disk");
            WebError::NotFound
        } else {
            WebError::Internal(anyhow::anyhow!("entrance test file open: {e}"))
        }
    })?;
    let body = axum::body::Body::from_stream(tokio_util::io::ReaderStream::new(handle));
    Ok((
        [
            (header::CONTENT_TYPE, content_type.to_string()),
            (header::CONTENT_DISPOSITION, uploads::attachment_header(filename)),
            (header::CONTENT_LENGTH, size.to_string()),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff".to_string()),
        ],
        body,
    )
        .into_response())
}

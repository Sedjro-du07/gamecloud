//! Entrance tests and admission.
//!
//! The Bureau opens a session with a PDF subject and a closing time;
//! candidates hand in one piece of work per session, replaceable until it
//! closes; the Bureau admits or turns each one down. Admission is what
//! lets somebody who is not on the Discord server sign up.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    db::queries::audit,
    error::{WebError, WebResult},
};

/// Longest session title.
const MAX_TITLE_CHARS: usize = 120;
/// Longest session description.
const MAX_DESCRIPTION_CHARS: usize = 2000;
/// Longest note a candidate may leave with their work.
pub const MAX_COMMENT_CHARS: usize = 1000;

/// A test session.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct EntranceTest {
    /// Row id.
    pub id: Uuid,
    /// Title.
    pub title: String,
    /// Instructions beyond the subject.
    pub description: Option<String>,
    /// Original name of the PDF subject.
    pub subject_filename: String,
    /// Its size in bytes.
    pub subject_size: i64,
    /// Name it is stored under in the subjects directory.
    pub subject_storage: String,
    /// When hand-ins stop being accepted.
    pub closes_at: DateTime<Utc>,
    /// When the session was opened.
    pub created_at: DateTime<Utc>,
    /// Work handed in so far.
    pub submissions: i64,
}

const SELECT_TEST: &str = r#"
    SELECT t.id, t.title, t.description, t.subject_filename, t.subject_size,
           t.subject_storage, t.closes_at, t.created_at,
           (SELECT COUNT(*) FROM entrance_submissions s WHERE s.test_id = t.id) AS submissions
      FROM entrance_tests t
"#;

/// Every session, latest closing first.
///
/// # Errors
/// Propagates database errors.
pub async fn list(pool: &PgPool) -> WebResult<Vec<EntranceTest>> {
    Ok(
        sqlx::query_as::<_, EntranceTest>(&format!("{SELECT_TEST} ORDER BY t.closes_at DESC LIMIT 200"))
            .fetch_all(pool)
            .await?,
    )
}

/// One session.
///
/// # Errors
/// `NotFound` when it does not exist.
pub async fn find(pool: &PgPool, id: Uuid) -> WebResult<EntranceTest> {
    sqlx::query_as::<_, EntranceTest>(&format!("{SELECT_TEST} WHERE t.id = $1"))
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or(WebError::NotFound)
}

/// A session about to be opened.
#[derive(Debug, Clone)]
pub struct NewTest<'a> {
    /// Title.
    pub title: &'a str,
    /// Instructions beyond the subject.
    pub description: Option<&'a str>,
    /// When hand-ins stop.
    pub closes_at: DateTime<Utc>,
    /// Original name of the subject.
    pub subject_filename: &'a str,
    /// Its size.
    pub subject_size: i64,
    /// Its SHA-256.
    pub subject_checksum: &'a str,
    /// Name it is stored under.
    pub subject_storage: &'a str,
    /// Bureau member opening it.
    pub created_by: Uuid,
}

/// Check what the Bureau typed.
///
/// # Errors
/// `Validation` describing the first problem, in French: it is shown as is.
pub fn validate_session(title: &str, description: Option<&str>) -> WebResult<()> {
    let title = title.trim();
    if title.is_empty() {
        return Err(WebError::Validation("donne un titre au test".into()));
    }
    if title.chars().count() > MAX_TITLE_CHARS {
        return Err(WebError::Validation(format!(
            "titre trop long ({MAX_TITLE_CHARS} caractères maximum)"
        )));
    }
    if description.is_some_and(|d| d.chars().count() > MAX_DESCRIPTION_CHARS) {
        return Err(WebError::Validation(format!(
            "consignes trop longues ({MAX_DESCRIPTION_CHARS} caractères maximum)"
        )));
    }
    Ok(())
}

/// Open a session.
///
/// # Errors
/// Propagates validation and database errors.
pub async fn create(pool: &PgPool, new: &NewTest<'_>) -> WebResult<Uuid> {
    validate_session(new.title, new.description)?;
    if new.closes_at <= Utc::now() {
        return Err(WebError::Validation("la fin du test doit être dans le futur".into()));
    }
    let id = sqlx::query_scalar(
        r#"
        INSERT INTO entrance_tests (title, description, subject_filename, subject_size,
                                    subject_checksum, subject_storage, closes_at, created_by)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        RETURNING id
        "#,
    )
    .bind(new.title.trim())
    .bind(new.description.map(str::trim).filter(|d| !d.is_empty()))
    .bind(new.subject_filename)
    .bind(new.subject_size)
    .bind(new.subject_checksum)
    .bind(new.subject_storage)
    .bind(new.closes_at)
    .bind(new.created_by)
    .fetch_one(pool)
    .await?;
    Ok(id)
}

/// Close a session now. A session already closed stays as it was.
///
/// # Errors
/// `NotFound` when it does not exist.
pub async fn close_now(pool: &PgPool, id: Uuid) -> WebResult<()> {
    let closed = sqlx::query(
        "UPDATE entrance_tests SET closes_at = NOW() WHERE id = $1 AND closes_at > NOW()",
    )
    .bind(id)
    .execute(pool)
    .await?;
    if closed.rows_affected() == 0 {
        find(pool, id).await?;
    }
    Ok(())
}

/// Delete a session and its hand-ins, returning the stored names of the
/// subject and of every piece of work, for the caller to remove.
///
/// # Errors
/// `NotFound` when it does not exist.
pub async fn delete(pool: &PgPool, id: Uuid) -> WebResult<(String, Vec<String>)> {
    let mut tx = pool.begin().await?;
    let work: Vec<String> =
        sqlx::query_scalar("SELECT storage_name FROM entrance_submissions WHERE test_id = $1")
            .bind(id)
            .fetch_all(&mut *tx)
            .await?;
    let subject: String =
        sqlx::query_scalar("DELETE FROM entrance_tests WHERE id = $1 RETURNING subject_storage")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(WebError::NotFound)?;
    tx.commit().await?;
    Ok((subject, work))
}

/// A piece of work handed in.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Submission {
    /// Row id.
    pub id: Uuid,
    /// Session.
    pub test_id: Uuid,
    /// Candidate.
    pub user_id: Uuid,
    /// Their display name.
    pub candidate_name: String,
    /// Their Discord handle.
    pub discord_username: Option<String>,
    /// Original filename.
    pub filename: String,
    /// Size in bytes.
    pub size_bytes: i64,
    /// Name it is stored under.
    pub storage_name: String,
    /// The candidate's note.
    pub comment: Option<String>,
    /// When it was (last) handed in.
    pub submitted_at: DateTime<Utc>,
    /// `Admitted`, `Rejected`, or `None` while ungraded.
    pub verdict: Option<String>,
}

const SELECT_SUBMISSION: &str = r#"
    SELECT s.id, s.test_id, s.user_id,
           member_display_name(u.current_title, u.discord_global_name,
                               u.discord_username, u.discord_id) AS candidate_name,
           u.discord_username, s.filename, s.size_bytes, s.storage_name, s.comment,
           s.submitted_at, s.verdict
      FROM entrance_submissions s
      JOIN users u ON u.id = s.user_id
"#;

/// Work handed in for a session, first handed in first.
///
/// # Errors
/// Propagates database errors.
pub async fn submissions(pool: &PgPool, test_id: Uuid) -> WebResult<Vec<Submission>> {
    Ok(sqlx::query_as::<_, Submission>(&format!(
        "{SELECT_SUBMISSION} WHERE s.test_id = $1 ORDER BY s.submitted_at"
    ))
    .bind(test_id)
    .fetch_all(pool)
    .await?)
}

/// Everything one candidate handed in.
///
/// # Errors
/// Propagates database errors.
pub async fn my_submissions(pool: &PgPool, user_id: Uuid) -> WebResult<Vec<Submission>> {
    Ok(
        sqlx::query_as::<_, Submission>(&format!("{SELECT_SUBMISSION} WHERE s.user_id = $1"))
            .bind(user_id)
            .fetch_all(pool)
            .await?,
    )
}

/// One piece of work.
///
/// # Errors
/// `NotFound` when it does not exist.
pub async fn find_submission(pool: &PgPool, id: Uuid) -> WebResult<Submission> {
    sqlx::query_as::<_, Submission>(&format!("{SELECT_SUBMISSION} WHERE s.id = $1"))
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or(WebError::NotFound)
}

/// Work about to be recorded.
#[derive(Debug, Clone)]
pub struct NewSubmission<'a> {
    /// Session.
    pub test_id: Uuid,
    /// Candidate.
    pub user_id: Uuid,
    /// Original filename.
    pub filename: &'a str,
    /// Size in bytes.
    pub size_bytes: i64,
    /// SHA-256.
    pub checksum: &'a str,
    /// Name it is stored under.
    pub storage_name: &'a str,
    /// The candidate's note.
    pub comment: Option<&'a str>,
}

/// Record a hand-in, replacing the candidate's previous one for the same
/// session. Returns the stored name of the work it replaced, if any.
///
/// The closing time is checked under a lock on the session, so a hand-in
/// racing the deadline is either in or out, never half-recorded.
///
/// # Errors
/// `Validation` when the session is closed or the work already graded.
pub async fn submit(pool: &PgPool, new: &NewSubmission<'_>) -> WebResult<Option<String>> {
    if new.comment.is_some_and(|c| c.chars().count() > MAX_COMMENT_CHARS) {
        return Err(WebError::Validation(format!(
            "message trop long ({MAX_COMMENT_CHARS} caractères maximum)"
        )));
    }
    let comment = new.comment.map(str::trim).filter(|c| !c.is_empty());

    let mut tx = pool.begin().await?;
    let closes_at: DateTime<Utc> =
        sqlx::query_scalar("SELECT closes_at FROM entrance_tests WHERE id = $1 FOR SHARE")
            .bind(new.test_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(WebError::NotFound)?;
    if closes_at <= Utc::now() {
        return Err(WebError::Validation("ce test est terminé : les rendus sont fermés".into()));
    }

    let previous: Option<(String, Option<String>)> = sqlx::query_as(
        "SELECT storage_name, verdict FROM entrance_submissions \
          WHERE test_id = $1 AND user_id = $2 FOR UPDATE",
    )
    .bind(new.test_id)
    .bind(new.user_id)
    .fetch_optional(&mut *tx)
    .await?;

    let replaced = match previous {
        Some((_, Some(_))) => {
            return Err(WebError::Validation(
                "ton rendu a déjà été corrigé : il ne peut plus être remplacé".into(),
            ))
        }
        Some((old, None)) => {
            sqlx::query(
                r#"
                UPDATE entrance_submissions
                   SET filename = $3, size_bytes = $4, checksum = $5, storage_name = $6,
                       comment = $7, submitted_at = NOW()
                 WHERE test_id = $1 AND user_id = $2
                "#,
            )
            .bind(new.test_id)
            .bind(new.user_id)
            .bind(new.filename)
            .bind(new.size_bytes)
            .bind(new.checksum)
            .bind(new.storage_name)
            .bind(comment)
            .execute(&mut *tx)
            .await?;
            Some(old)
        }
        None => {
            sqlx::query(
                r#"
                INSERT INTO entrance_submissions (test_id, user_id, filename, size_bytes,
                                                  checksum, storage_name, comment)
                VALUES ($1, $2, $3, $4, $5, $6, $7)
                "#,
            )
            .bind(new.test_id)
            .bind(new.user_id)
            .bind(new.filename)
            .bind(new.size_bytes)
            .bind(new.checksum)
            .bind(new.storage_name)
            .bind(comment)
            .execute(&mut *tx)
            .await?;
            None
        }
    };

    tx.commit().await?;
    Ok(replaced)
}

/// Grade a piece of work: `Admitted` or `Rejected`. Returns the candidate.
///
/// Admitting clears the candidate flag and records the admission, which
/// is what opens the sign-up. Graded work stays graded: a verdict is not
/// changed by clicking again.
///
/// # Errors
/// `Validation` for an unknown verdict or work already graded; `NotFound`
/// when the work does not exist.
pub async fn judge(pool: &PgPool, submission_id: Uuid, reviewer: Uuid, verdict: &str) -> WebResult<Uuid> {
    if !matches!(verdict, "Admitted" | "Rejected") {
        return Err(WebError::Validation(format!("verdict inconnu « {verdict} »")));
    }

    let mut tx = pool.begin().await?;
    let (candidate, current): (Uuid, Option<String>) = sqlx::query_as(
        "SELECT user_id, verdict FROM entrance_submissions WHERE id = $1 FOR UPDATE",
    )
    .bind(submission_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(WebError::NotFound)?;
    if current.is_some() {
        return Err(WebError::Validation("ce rendu est déjà corrigé".into()));
    }

    sqlx::query(
        "UPDATE entrance_submissions SET verdict = $2, reviewed_by = $3, reviewed_at = NOW() \
          WHERE id = $1",
    )
    .bind(submission_id)
    .bind(verdict)
    .bind(reviewer)
    .execute(&mut *tx)
    .await?;

    if verdict == "Admitted" {
        sqlx::query(
            "UPDATE users SET candidate = FALSE, admitted_at = COALESCE(admitted_at, NOW()) \
              WHERE id = $1",
        )
        .bind(candidate)
        .execute(&mut *tx)
        .await?;
    }

    audit::record_in_tx(
        &mut tx,
        Some(reviewer),
        if verdict == "Admitted" { "entrance.admitted" } else { "entrance.rejected" },
        Some("entrance_submission"),
        Some(submission_id),
        serde_json::json!({ "candidate": candidate }),
    )
    .await?;

    tx.commit().await?;
    Ok(candidate)
}

/// Where an account stands with admission.
#[derive(Debug, Clone, Default, sqlx::FromRow)]
pub struct Admission {
    /// Not on the server and not admitted: may take tests, not sign up.
    pub candidate: bool,
    /// When the Bureau admitted them.
    pub admitted_at: Option<DateTime<Utc>>,
    /// Their invitation to the server.
    pub discord_invite_url: Option<String>,
}

/// An account's admission. An unknown account reads as nothing at all.
///
/// # Errors
/// Propagates database errors.
pub async fn admission(pool: &PgPool, user_id: Uuid) -> WebResult<Admission> {
    Ok(sqlx::query_as::<_, Admission>(
        "SELECT candidate, admitted_at, discord_invite_url FROM users WHERE id = $1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .unwrap_or_default())
}

/// Mark an account as a candidate, or not.
///
/// # Errors
/// Propagates database errors.
pub async fn set_candidate(pool: &PgPool, user_id: Uuid, candidate: bool) -> WebResult<()> {
    sqlx::query("UPDATE users SET candidate = $2 WHERE id = $1 AND candidate IS DISTINCT FROM $2")
        .bind(user_id)
        .bind(candidate)
        .execute(pool)
        .await?;
    Ok(())
}

/// Record an admitted candidate's invitation.
///
/// # Errors
/// Propagates database errors.
pub async fn set_invite(pool: &PgPool, user_id: Uuid, url: &str) -> WebResult<()> {
    sqlx::query("UPDATE users SET discord_invite_url = $2 WHERE id = $1")
        .bind(user_id)
        .bind(url)
        .execute(pool)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_session_needs_a_title() {
        assert!(validate_session("Test d'entrée — septembre", None).is_ok());
        assert!(validate_session("   ", None).is_err());
    }

    #[test]
    fn overlong_titles_and_instructions_are_refused() {
        assert!(validate_session(&"a".repeat(121), None).is_err());
        assert!(validate_session("ok", Some(&"a".repeat(2001))).is_err());
        assert!(validate_session("ok", Some(&"a".repeat(2000))).is_ok());
    }
}

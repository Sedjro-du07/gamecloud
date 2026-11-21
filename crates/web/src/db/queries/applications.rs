//! Pre-registration for entrance tests.
//!
//! Somebody who is not on the Discord server pre-registers for an open
//! session with their name, e-mail, GitHub and Discord username, no
//! account needed. They follow their application through a private link
//! whose token only exists hashed here. The Bureau grants access to the
//! subject, the applicant hands in their work, the Bureau admits or turns
//! them down; admission comes with an invitation to the server.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    config::DiscordChannels,
    db::queries::{audit, entrance::MAX_COMMENT_CHARS},
    error::{WebError, WebResult},
    services::{
        notifications::{self, Announcement},
        tokens,
    },
};

/// Longest full name.
const MAX_NAME_CHARS: usize = 100;
/// Longest e-mail address (RFC 5321).
const MAX_EMAIL_CHARS: usize = 254;
/// Longest motivation text.
const MAX_MOTIVATION_CHARS: usize = 1000;

/// What the applicant typed, once checked and normalised.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Applicant {
    /// Full name as typed, trimmed.
    pub full_name: String,
    /// E-mail address, lowercased.
    pub email: String,
    /// GitHub username, without the URL around it.
    pub github: String,
    /// Discord username, lowercased, without `@`.
    pub discord: String,
    /// Why they want to join.
    pub motivation: Option<String>,
}

/// Check and normalise a pre-registration form.
///
/// # Errors
/// `Validation` describing the first problem, in French: it is shown as is.
pub fn validate(
    full_name: &str,
    email: &str,
    github: &str,
    discord: &str,
    motivation: Option<&str>,
) -> WebResult<Applicant> {
    let invalid = |m: &str| Err(WebError::Validation(m.to_string()));

    let full_name = full_name.trim();
    if full_name.is_empty() {
        return invalid("indique ton nom et ton prénom");
    }
    if full_name.chars().count() > MAX_NAME_CHARS {
        return invalid("nom trop long");
    }

    let email = email.trim().to_lowercase();
    let well_formed = email.split_once('@').is_some_and(|(local, domain)| {
        !local.is_empty()
            && domain.contains('.')
            && !domain.starts_with('.')
            && !domain.ends_with('.')
            && !domain.contains('@')
    });
    if !well_formed || email.chars().count() > MAX_EMAIL_CHARS || email.contains(char::is_whitespace) {
        return invalid("adresse e-mail invalide");
    }

    let github = normalise_github(github).ok_or_else(|| {
        WebError::Validation(
            "nom d'utilisateur GitHub invalide (lettres, chiffres et tirets, 39 caractères maximum)".into(),
        )
    })?;

    let discord = normalise_discord(discord).ok_or_else(|| {
        WebError::Validation(
            "nom d'utilisateur Discord invalide : c'est ton identifiant unique (2 à 32 caractères : \
             lettres minuscules, chiffres, « _ » et « . »), pas ton nom d'affichage"
                .into(),
        )
    })?;

    let motivation = motivation.map(str::trim).filter(|m| !m.is_empty());
    if motivation.is_some_and(|m| m.chars().count() > MAX_MOTIVATION_CHARS) {
        return Err(WebError::Validation(format!(
            "motivation trop longue ({MAX_MOTIVATION_CHARS} caractères maximum)"
        )));
    }

    Ok(Applicant {
        full_name: full_name.to_string(),
        email,
        github,
        discord,
        motivation: motivation.map(str::to_string),
    })
}

/// A GitHub username, from the bare name, `@name` or a profile URL.
fn normalise_github(raw: &str) -> Option<String> {
    let raw = raw.trim().trim_end_matches('/');
    let name = raw
        .rsplit_once("github.com/")
        .map_or(raw, |(_, rest)| rest)
        .trim_start_matches('@');
    let valid = (1..=39).contains(&name.len())
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        && !name.starts_with('-')
        && !name.ends_with('-');
    valid.then(|| name.to_string())
}

/// A Discord username: the unique handle, not the display name.
fn normalise_discord(raw: &str) -> Option<String> {
    let name = raw.trim().trim_start_matches('@').to_lowercase();
    let valid = (2..=32).contains(&name.len())
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '.')
        && !name.contains("..");
    valid.then_some(name)
}

/// An application, with its session.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Application {
    /// Row id.
    pub id: Uuid,
    /// Session.
    pub test_id: Uuid,
    /// Session title.
    pub test_title: String,
    /// Session instructions.
    pub test_description: Option<String>,
    /// When the session closes.
    pub closes_at: DateTime<Utc>,
    /// Subject size.
    pub subject_size: i64,
    /// Full name.
    pub full_name: String,
    /// E-mail.
    pub email: String,
    /// GitHub username.
    pub github_username: String,
    /// Discord username.
    pub discord_username: String,
    /// Motivation.
    pub motivation: Option<String>,
    /// `Requested`, `Granted`, `Refused`, `HandedIn`, `Admitted`, `Rejected`.
    pub status: String,
    /// Work: original filename.
    pub filename: Option<String>,
    /// Work: size.
    pub size_bytes: Option<i64>,
    /// Work: stored name.
    pub storage_name: Option<String>,
    /// Applicant's note with the work.
    pub comment: Option<String>,
    /// When the work was (last) handed in.
    pub submitted_at: Option<DateTime<Utc>>,
    /// Invitation to the server, once admitted.
    pub discord_invite_url: Option<String>,
    /// When they pre-registered.
    pub created_at: DateTime<Utc>,
}

const SELECT: &str = r#"
    SELECT a.id, a.test_id, t.title AS test_title, t.description AS test_description,
           t.closes_at, t.subject_size, a.full_name, a.email, a.github_username,
           a.discord_username, a.motivation, a.status, a.filename, a.size_bytes,
           a.storage_name, a.comment, a.submitted_at, a.discord_invite_url, a.created_at
      FROM entrance_applications a
      JOIN entrance_tests t ON t.id = a.test_id
"#;

/// The application a private link points to.
///
/// # Errors
/// `NotFound` for an unknown or replaced link.
pub async fn find_by_token(pool: &PgPool, token: &str) -> WebResult<Application> {
    sqlx::query_as::<_, Application>(&format!("{SELECT} WHERE a.token_hash = $1"))
        .bind(tokens::hash(token))
        .fetch_optional(pool)
        .await?
        .ok_or(WebError::NotFound)
}

/// One application, for the Bureau.
///
/// # Errors
/// `NotFound` when it does not exist.
pub async fn find(pool: &PgPool, id: Uuid) -> WebResult<Application> {
    sqlx::query_as::<_, Application>(&format!("{SELECT} WHERE a.id = $1"))
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or(WebError::NotFound)
}

/// Every application to a session, first registered first.
///
/// # Errors
/// Propagates database errors.
pub async fn for_test(pool: &PgPool, test_id: Uuid) -> WebResult<Vec<Application>> {
    Ok(sqlx::query_as::<_, Application>(&format!(
        "{SELECT} WHERE a.test_id = $1 ORDER BY a.created_at"
    ))
    .bind(test_id)
    .fetch_all(pool)
    .await?)
}

/// Tell the Bureau, in its own channel, that an application needs it.
async fn tell_bureau(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    channels: DiscordChannels,
    title: &str,
    description: String,
) -> WebResult<()> {
    notifications::enqueue(
        tx,
        channels,
        &Announcement::entrance_application(title, description),
    )
    .await
}

/// Pre-register for an open session. Returns the plaintext token of the
/// applicant's private link: shown once, never stored.
///
/// # Errors
/// `Validation` when the session is closed or this Discord username has
/// already pre-registered for it; `NotFound` for an unknown session.
pub async fn create(
    pool: &PgPool,
    channels: DiscordChannels,
    test_id: Uuid,
    applicant: &Applicant,
) -> WebResult<String> {
    let mut tx = pool.begin().await?;
    let (title, closes_at): (String, DateTime<Utc>) =
        sqlx::query_as("SELECT title, closes_at FROM entrance_tests WHERE id = $1 FOR SHARE")
            .bind(test_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(WebError::NotFound)?;
    if closes_at <= Utc::now() {
        return Err(WebError::Validation("ce test est terminé : la préinscription est fermée".into()));
    }

    let token = tokens::generate();
    let inserted = sqlx::query(
        r#"
        INSERT INTO entrance_applications (test_id, token_hash, full_name, email,
                                           github_username, discord_username, motivation)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        ON CONFLICT DO NOTHING
        "#,
    )
    .bind(test_id)
    .bind(tokens::hash(&token))
    .bind(&applicant.full_name)
    .bind(&applicant.email)
    .bind(&applicant.github)
    .bind(&applicant.discord)
    .bind(&applicant.motivation)
    .execute(&mut *tx)
    .await?;
    if inserted.rows_affected() == 0 {
        return Err(WebError::Validation(
            "ce nom d'utilisateur Discord est déjà préinscrit à ce test : reprends ton lien de suivi, \
             ou demande au Bureau de te le renvoyer"
                .into(),
        ));
    }

    tell_bureau(
        &mut tx,
        channels,
        "📝 Nouvelle préinscription",
        format!(
            "**{}** (@{}) demande l'accès au sujet de **{title}**.\nÀ autoriser ou refuser sur la page Tests d'entrée.",
            applicant.full_name, applicant.discord
        ),
    )
    .await?;

    tx.commit().await?;
    Ok(token)
}

/// Grant or refuse access to the subject. Only a pending request can be
/// decided; a decision is not changed by clicking again.
///
/// # Errors
/// `Validation` when the request was already decided; `NotFound` when it
/// does not exist.
pub async fn decide_access(pool: &PgPool, id: Uuid, reviewer: Uuid, grant: bool) -> WebResult<()> {
    let status = if grant { "Granted" } else { "Refused" };
    let mut tx = pool.begin().await?;
    let updated = sqlx::query(
        "UPDATE entrance_applications \
            SET status = $2, access_decided_by = $3, access_decided_at = NOW() \
          WHERE id = $1 AND status = 'Requested'",
    )
    .bind(id)
    .bind(status)
    .bind(reviewer)
    .execute(&mut *tx)
    .await?;
    if updated.rows_affected() == 0 {
        find(pool, id).await?;
        return Err(WebError::Validation("cette demande est déjà traitée".into()));
    }
    audit::record_in_tx(
        &mut tx,
        Some(reviewer),
        if grant { "entrance.access_granted" } else { "entrance.access_refused" },
        Some("entrance_application"),
        Some(id),
        serde_json::json!({}),
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

/// Work about to be recorded for an application.
#[derive(Debug, Clone)]
pub struct HandIn<'a> {
    /// Original filename.
    pub filename: &'a str,
    /// Size in bytes.
    pub size_bytes: i64,
    /// SHA-256.
    pub checksum: &'a str,
    /// Name it is stored under.
    pub storage_name: &'a str,
    /// The applicant's note.
    pub comment: Option<&'a str>,
}

/// Record the applicant's work, replacing what they handed in before.
/// Returns the stored name of the work it replaced, if any.
///
/// The session's closing time is checked under a lock, so a hand-in
/// racing the deadline is either in or out.
///
/// # Errors
/// `Validation` when access was not granted, the session is closed or the
/// work already graded.
pub async fn hand_in(
    pool: &PgPool,
    channels: DiscordChannels,
    token: &str,
    work: &HandIn<'_>,
) -> WebResult<Option<String>> {
    if work.comment.is_some_and(|c| c.chars().count() > MAX_COMMENT_CHARS) {
        return Err(WebError::Validation(format!(
            "message trop long ({MAX_COMMENT_CHARS} caractères maximum)"
        )));
    }
    let comment = work.comment.map(str::trim).filter(|c| !c.is_empty());

    let mut tx = pool.begin().await?;
    let (id, status, previous, closes_at, title, name, discord): (
        Uuid,
        String,
        Option<String>,
        DateTime<Utc>,
        String,
        String,
        String,
    ) = sqlx::query_as(
        r#"
        SELECT a.id, a.status, a.storage_name, t.closes_at, t.title, a.full_name, a.discord_username
          FROM entrance_applications a
          JOIN entrance_tests t ON t.id = a.test_id
         WHERE a.token_hash = $1
           FOR UPDATE OF a
        "#,
    )
    .bind(tokens::hash(token))
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(WebError::NotFound)?;

    match status.as_str() {
        "Granted" | "HandedIn" => {}
        "Requested" => {
            return Err(WebError::Validation(
                "le Bureau n'a pas encore autorisé ton accès au sujet".into(),
            ))
        }
        "Admitted" | "Rejected" => {
            return Err(WebError::Validation(
                "ton rendu a déjà été corrigé : il ne peut plus être remplacé".into(),
            ))
        }
        _ => return Err(WebError::Validation("ta préinscription n'a pas été retenue".into())),
    }
    if closes_at <= Utc::now() {
        return Err(WebError::Validation("ce test est terminé : les rendus sont fermés".into()));
    }

    sqlx::query(
        r#"
        UPDATE entrance_applications
           SET status = 'HandedIn', filename = $2, size_bytes = $3, checksum = $4,
               storage_name = $5, comment = $6, submitted_at = NOW()
         WHERE id = $1
        "#,
    )
    .bind(id)
    .bind(work.filename)
    .bind(work.size_bytes)
    .bind(work.checksum)
    .bind(work.storage_name)
    .bind(comment)
    .execute(&mut *tx)
    .await?;

    // The first hand-in is a new request for the Bureau; a replacement is not.
    if status == "Granted" {
        tell_bureau(
            &mut tx,
            channels,
            "📥 Rendu de test reçu",
            format!(
                "**{name}** (@{discord}) a rendu son travail pour **{title}**.\nÀ corriger sur la page Tests d'entrée."
            ),
        )
        .await?;
    }

    tx.commit().await?;
    Ok(previous)
}

/// Admit or turn down an applicant on their work.
///
/// # Errors
/// `Validation` for an unknown verdict or work not handed in / already
/// graded; `NotFound` when the application does not exist.
pub async fn judge(pool: &PgPool, id: Uuid, reviewer: Uuid, verdict: &str) -> WebResult<()> {
    if !matches!(verdict, "Admitted" | "Rejected") {
        return Err(WebError::Validation(format!("verdict inconnu « {verdict} »")));
    }
    let mut tx = pool.begin().await?;
    let updated = sqlx::query(
        "UPDATE entrance_applications SET status = $2, reviewed_by = $3, reviewed_at = NOW() \
          WHERE id = $1 AND status = 'HandedIn'",
    )
    .bind(id)
    .bind(verdict)
    .bind(reviewer)
    .execute(&mut *tx)
    .await?;
    if updated.rows_affected() == 0 {
        find(pool, id).await?;
        return Err(WebError::Validation(
            "seul un rendu en attente de correction peut être jugé".into(),
        ));
    }
    audit::record_in_tx(
        &mut tx,
        Some(reviewer),
        if verdict == "Admitted" { "entrance.admitted" } else { "entrance.rejected" },
        Some("entrance_application"),
        Some(id),
        serde_json::json!({}),
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

/// Record an admitted applicant's invitation.
///
/// # Errors
/// Propagates database errors.
pub async fn set_invite(pool: &PgPool, id: Uuid, url: &str) -> WebResult<()> {
    sqlx::query("UPDATE entrance_applications SET discord_invite_url = $2 WHERE id = $1")
        .bind(id)
        .bind(url)
        .execute(pool)
        .await?;
    Ok(())
}

/// Replace an application's private link, for somebody who lost theirs.
/// Returns the new plaintext token; the old link stops working.
///
/// # Errors
/// `NotFound` when the application does not exist.
pub async fn reissue_token(pool: &PgPool, id: Uuid, reviewer: Uuid) -> WebResult<String> {
    let token = tokens::generate();
    let mut tx = pool.begin().await?;
    let updated = sqlx::query("UPDATE entrance_applications SET token_hash = $2 WHERE id = $1")
        .bind(id)
        .bind(tokens::hash(&token))
        .execute(&mut *tx)
        .await?;
    if updated.rows_affected() == 0 {
        return Err(WebError::NotFound);
    }
    audit::record_in_tx(
        &mut tx,
        Some(reviewer),
        "entrance.link_reissued",
        Some("entrance_application"),
        Some(id),
        serde_json::json!({}),
    )
    .await?;
    tx.commit().await?;
    Ok(token)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(name: &str, email: &str, github: &str, discord: &str) -> WebResult<Applicant> {
        validate(name, email, github, discord, None)
    }

    #[test]
    fn a_complete_form_is_normalised() {
        let a = validate(
            "  Yoann Mensah ",
            " Yoann.Mensah@epitech.eu ",
            "https://github.com/yoann-m/",
            "@Yoann_42",
            Some("  "),
        )
        .unwrap();
        assert_eq!(a.full_name, "Yoann Mensah");
        assert_eq!(a.email, "yoann.mensah@epitech.eu");
        assert_eq!(a.github, "yoann-m");
        assert_eq!(a.discord, "yoann_42");
        assert_eq!(a.motivation, None);
    }

    #[test]
    fn every_field_is_required() {
        assert!(ok("", "a@b.co", "gh", "dc").is_err());
        assert!(ok("A", "", "gh", "dc").is_err());
        assert!(ok("A", "a@b.co", "", "dc").is_err());
        assert!(ok("A", "a@b.co", "gh", "").is_err());
    }

    #[test]
    fn malformed_addresses_are_refused() {
        for bad in ["plain", "@b.co", "a@b", "a@.co", "a@b.", "a b@c.co", "a@b@c.co"] {
            assert!(ok("A", bad, "gh", "dc").is_err(), "{bad}");
        }
    }

    #[test]
    fn display_names_are_not_discord_usernames() {
        // A one-word display name such as "Sena" reads as the valid
        // username "sena": only what cannot be a username is refused.
        for bad in ["James Degla", "-", "a", "été", "x..y", &"a".repeat(33)] {
            assert!(ok("A", "a@b.co", "gh", bad).is_err(), "{bad}");
        }
        for good in ["hvk_clever", "lorxntz_", "yukimo204", "drlerm", "a.b"] {
            assert!(ok("A", "a@b.co", "gh", good).is_ok(), "{good}");
        }
    }

    #[test]
    fn github_names_follow_github_rules() {
        assert!(ok("A", "a@b.co", "-lead", "dc").is_err());
        assert!(ok("A", "a@b.co", "under_score", "dc").is_err());
        assert!(ok("A", "a@b.co", &"a".repeat(40), "dc").is_err());
        assert!(ok("A", "a@b.co", "@octo-cat", "dc").is_ok());
    }

    #[test]
    fn motivation_has_a_ceiling() {
        assert!(validate("A", "a@b.co", "gh", "dc", Some(&"m".repeat(1001))).is_err());
        assert!(validate("A", "a@b.co", "gh", "dc", Some(&"m".repeat(1000))).is_ok());
    }
}

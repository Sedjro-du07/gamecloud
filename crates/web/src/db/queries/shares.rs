//! Member shares.
//!
//! Anyone signed in can post a script, some lore, a game or a pack of
//! assets — as a file the platform keeps, or as a link to wherever it
//! already lives. Every member can download, and each download is
//! counted, links included: they go through the platform too, so the
//! number means the same thing whatever the share is.
//!
//! No review and no XP. Shares are an open shelf, not the project
//! pipeline; paying XP for uploads would reward volume, not quality.

use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    config::DiscordChannels,
    error::{WebError, WebResult},
    services::notifications::{self, Announcement},
};

/// Kinds a share can be, in the order the page offers them.
pub const KINDS: [&str; 5] = ["Script", "Lore", "Game", "Asset", "Other"];

/// Longest title accepted.
const MAX_TITLE_CHARS: usize = 120;
/// Longest description accepted.
const MAX_DESCRIPTION_CHARS: usize = 2000;

/// A share as listed.
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct Share {
    /// Row id.
    pub id: Uuid,
    /// Title.
    pub title: String,
    /// What it is about.
    pub description: Option<String>,
    /// One of [`KINDS`].
    pub kind: String,
    /// Original filename, for a file share.
    pub filename: Option<String>,
    /// Size in bytes, for a file share.
    pub size_bytes: Option<i64>,
    /// SHA-256, for a file share.
    pub checksum_sha256: Option<String>,
    /// Name the bytes are stored under, for a file share.
    pub storage_name: Option<String>,
    /// Target, for a link share.
    pub url: Option<String>,
    /// Who posted it.
    pub uploaded_by: Uuid,
    /// Their display name.
    pub uploaded_by_name: String,
    /// Downloads so far.
    pub download_count: i64,
    /// When it was posted.
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// What a share points at.
#[derive(Debug, Clone)]
pub enum Content<'a> {
    /// A file already written to the shares directory.
    File {
        /// Original filename.
        filename: &'a str,
        /// Size in bytes.
        size_bytes: i64,
        /// SHA-256.
        checksum: &'a str,
        /// Name in the shares directory.
        storage_name: &'a str,
    },
    /// A link.
    Link(&'a str),
}

/// A share about to be recorded.
#[derive(Debug, Clone)]
pub struct NewShare<'a> {
    /// Title.
    pub title: &'a str,
    /// Description, if any.
    pub description: Option<&'a str>,
    /// One of [`KINDS`].
    pub kind: &'a str,
    /// File or link.
    pub content: Content<'a>,
    /// Who posted it.
    pub uploaded_by: Uuid,
}

/// Check the parts a member typed.
///
/// # Errors
/// `Validation` describing the first problem, in French because it is
/// shown on the page as is.
pub fn validate(title: &str, description: Option<&str>, kind: &str) -> WebResult<()> {
    let title = title.trim();
    if title.is_empty() {
        return Err(WebError::Validation("donne un titre à ton partage".into()));
    }
    if title.chars().count() > MAX_TITLE_CHARS {
        return Err(WebError::Validation(format!(
            "titre trop long ({MAX_TITLE_CHARS} caractères maximum)"
        )));
    }
    if description.is_some_and(|d| d.chars().count() > MAX_DESCRIPTION_CHARS) {
        return Err(WebError::Validation(format!(
            "description trop longue ({MAX_DESCRIPTION_CHARS} caractères maximum)"
        )));
    }
    if !KINDS.contains(&kind) {
        return Err(WebError::Validation(format!("type de partage inconnu « {kind} »")));
    }
    Ok(())
}

/// Check a link share's target.
///
/// # Errors
/// `Validation` unless it is an http(s) URL. Anything else — a
/// `javascript:` URL above all — must never reach a redirect.
pub fn validate_link(url: &str) -> WebResult<()> {
    let ok = url::Url::parse(url)
        .is_ok_and(|u| matches!(u.scheme(), "http" | "https") && u.host_str().is_some());
    if ok {
        Ok(())
    } else {
        Err(WebError::Validation(
            "le lien doit commencer par https:// (ou http://)".into(),
        ))
    }
}

/// Record a share and announce it.
///
/// The announcement commits with the row, so a share that failed to
/// save is never announced.
///
/// # Errors
/// Propagates validation and database errors.
pub async fn create(
    pool: &PgPool,
    channels: DiscordChannels,
    page_url: &str,
    new: &NewShare<'_>,
) -> WebResult<Uuid> {
    validate(new.title, new.description, new.kind)?;
    let detail = match &new.content {
        Content::File {
            filename,
            size_bytes,
            ..
        } => format!("📁 {filename} · {}", crate::api::format_bytes(*size_bytes)),
        Content::Link(url) => url::Url::parse(url)
            .ok()
            .and_then(|u| u.host_str().map(|h| format!("🔗 {}", h.trim_start_matches("www."))))
            .unwrap_or_else(|| "🔗 lien".to_string()),
    };
    let (filename, size, checksum, storage, url) = match &new.content {
        Content::File {
            filename,
            size_bytes,
            checksum,
            storage_name,
        } => (
            Some(*filename),
            Some(*size_bytes),
            Some(*checksum),
            Some(*storage_name),
            None,
        ),
        Content::Link(url) => {
            validate_link(url)?;
            (None, None, None, None, Some(*url))
        }
    };

    let mut tx = pool.begin().await?;
    let id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO shares (title, description, kind, filename, size_bytes,
                            checksum_sha256, storage_name, url, uploaded_by)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        RETURNING id
        "#,
    )
    .bind(new.title.trim())
    .bind(new.description.map(str::trim).filter(|d| !d.is_empty()))
    .bind(new.kind)
    .bind(filename)
    .bind(size)
    .bind(checksum)
    .bind(storage)
    .bind(url)
    .bind(new.uploaded_by)
    .fetch_one(&mut *tx)
    .await?;

    let author: String = sqlx::query_scalar(
        "SELECT member_display_name(current_title, discord_global_name, discord_username, discord_id) \
           FROM users WHERE id = $1",
    )
    .bind(new.uploaded_by)
    .fetch_one(&mut *tx)
    .await?;
    let kind = kind_label(new.kind);
    notifications::enqueue(
        &mut tx,
        channels,
        &Announcement::share_posted(
            new.title.trim(),
            kind,
            &author,
            new.description,
            &detail,
            page_url,
        ),
    )
    .await?;

    tx.commit().await?;
    Ok(id)
}

const SELECT: &str = r#"
    SELECT s.id, s.title, s.description, s.kind, s.filename, s.size_bytes,
           s.checksum_sha256, s.storage_name, s.url, s.uploaded_by,
           member_display_name(u.current_title, u.discord_global_name,
                               u.discord_username, u.discord_id) AS uploaded_by_name,
           s.download_count, s.created_at
      FROM shares s
      JOIN users u ON u.id = s.uploaded_by
"#;

/// Every share, newest first.
///
/// # Errors
/// Propagates database errors.
pub async fn list(pool: &PgPool) -> WebResult<Vec<Share>> {
    let rows = sqlx::query_as::<_, Share>(&format!(
        "{SELECT} ORDER BY s.created_at DESC LIMIT 500"
    ))
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// One share.
///
/// # Errors
/// `NotFound` when the id does not exist.
pub async fn find(pool: &PgPool, id: Uuid) -> WebResult<Share> {
    sqlx::query_as::<_, Share>(&format!("{SELECT} WHERE s.id = $1"))
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or(WebError::NotFound)
}

/// Count one download.
///
/// A single `UPDATE … + 1`, so two members downloading at the same
/// instant both count.
///
/// # Errors
/// Propagates database errors.
pub async fn count_download(pool: &PgPool, id: Uuid) -> WebResult<()> {
    sqlx::query("UPDATE shares SET download_count = download_count + 1 WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Delete a share's row, returning the stored file name to remove.
///
/// # Errors
/// `NotFound` when the id does not exist.
pub async fn delete(pool: &PgPool, id: Uuid) -> WebResult<Option<String>> {
    let row: Option<(Option<String>,)> =
        sqlx::query_as("DELETE FROM shares WHERE id = $1 RETURNING storage_name")
            .bind(id)
            .fetch_optional(pool)
            .await?;
    row.map(|(name,)| name).ok_or(WebError::NotFound)
}

/// French label for a kind.
#[must_use]
pub fn kind_label(kind: &str) -> &'static str {
    match kind {
        "Script" => "Script",
        "Lore" => "Lore",
        "Game" => "Jeu",
        "Asset" => "Assets",
        _ => "Autre",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_every_offered_kind() {
        for kind in KINDS {
            assert!(validate("Un titre", None, kind).is_ok(), "{kind}");
        }
    }

    #[test]
    fn refuses_an_unknown_kind_or_a_blank_title() {
        assert!(validate("Un titre", None, "Meme").is_err());
        assert!(validate("   ", None, "Lore").is_err());
    }

    #[test]
    fn refuses_an_overlong_title_or_description() {
        assert!(validate(&"a".repeat(121), None, "Lore").is_err());
        assert!(validate("ok", Some(&"a".repeat(2001)), "Lore").is_err());
    }

    #[test]
    fn a_link_must_be_http() {
        assert!(validate_link("https://itch.io/jam").is_ok());
        assert!(validate_link("http://example.com/a.zip").is_ok());
        assert!(validate_link("javascript:alert(1)").is_err());
        assert!(validate_link("ftp://files.example.com").is_err());
        assert!(validate_link("https://").is_err());
        assert!(validate_link("itch.io").is_err());
    }
}

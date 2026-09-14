//! The community resource library.
//!
//! Members submit tutorials, tools, assets and docs; the Archiviste (or
//! a track Lead) validates them, which is what pays the submitter their
//! XP and marks the entry trustworthy. Votes surface what the club
//! actually found useful.
//!
//! Votes live in their own table with a composite primary key, so one
//! member counts once — the pre-existing `resources.votes` column was a
//! bare counter with nothing stopping a thousand clicks. It is now a
//! cached aggregate kept in step by a trigger (migration 0011).

use gamecloud_shared::{
    roles::Track,
    xp::{XpSource, XP_DISCORD_RESOURCE_VALIDATED},
    DomainError,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    db::queries::{
        audit,
        xp::{self, XpGrant},
    },
    error::{WebError, WebResult},
};

/// A library entry as shown in listings.
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct ResourceView {
    /// Entry id.
    pub id: Uuid,
    /// Title.
    pub title: String,
    /// Link.
    pub url: String,
    /// `Tutorial`, `Tool`, `Asset`, `Doc` or `Video`.
    pub resource_type: Option<String>,
    /// Tracks this is relevant to.
    pub tracks: Vec<String>,
    /// Specializations this is relevant to.
    pub specializations: Vec<String>,
    /// Difficulty band.
    pub level: Option<String>,
    /// Submitter's display name.
    pub submitted_by_name: String,
    /// Whether a validator has signed off.
    pub is_validated: bool,
    /// Whether the Bureau marked it official.
    pub is_official: bool,
    /// Vote tally.
    pub votes: i32,
    /// Whether the calling member has voted.
    pub has_voted: bool,
    /// Submission instant.
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// List entries, newest and best-voted first.
///
/// Unvalidated submissions are visible only to members who can validate
/// them, so the library a member browses is one the club vouched for.
///
/// # Errors
/// Propagates database errors.
pub async fn list(
    pool: &PgPool,
    viewer: Uuid,
    track: Option<&str>,
    include_unvalidated: bool,
) -> WebResult<Vec<ResourceView>> {
    let rows = sqlx::query_as::<_, ResourceView>(
        r#"
        SELECT r.id,
               r.title,
               r.url,
               r.resource_type,
               r.tracks,
               r.specializations,
               r.level,
               COALESCE(u.current_title, u.discord_id) AS submitted_by_name,
               (r.validated_by IS NOT NULL) AS is_validated,
               r.is_official,
               r.votes,
               EXISTS(SELECT 1 FROM resource_votes v
                       WHERE v.resource_id = r.id AND v.user_id = $1) AS has_voted,
               r.created_at
          FROM resources r
          JOIN users u ON u.id = r.submitted_by
         WHERE ($2::TEXT IS NULL OR $2 = ANY(r.tracks))
           AND ($3::BOOLEAN OR r.validated_by IS NOT NULL)
         ORDER BY r.is_official DESC, r.votes DESC, r.created_at DESC
         LIMIT 200
        "#,
    )
    .bind(viewer)
    .bind(track)
    .bind(include_unvalidated)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Fields accepted when submitting a resource.
#[derive(Debug, Clone, Deserialize)]
pub struct NewResource {
    /// Title.
    pub title: String,
    /// Link. Must be http(s).
    pub url: String,
    /// Kind of resource.
    pub resource_type: Option<String>,
    /// Relevant tracks.
    #[serde(default)]
    pub tracks: Vec<String>,
    /// Relevant specializations.
    #[serde(default)]
    pub specializations: Vec<String>,
    /// Difficulty band.
    pub level: Option<String>,
}

/// Validate a submission.
///
/// # Errors
/// `Validation` or `UnknownTrack` describing the first problem.
pub fn validate(resource: &NewResource) -> WebResult<()> {
    if resource.title.trim().is_empty() {
        return Err(WebError::Validation("title must not be empty".into()));
    }
    if !resource.url.starts_with("https://") && !resource.url.starts_with("http://") {
        return Err(WebError::Validation("url must be an http(s) link".into()));
    }
    if let Some(kind) = &resource.resource_type {
        if !matches!(
            kind.as_str(),
            "Tutorial" | "Tool" | "Asset" | "Doc" | "Video"
        ) {
            return Err(WebError::Validation(format!(
                "unknown resource_type '{kind}'"
            )));
        }
    }
    if let Some(level) = &resource.level {
        if !matches!(level.as_str(), "Initiate" | "Junior" | "Senior" | "Expert") {
            return Err(WebError::Validation(format!("unknown level '{level}'")));
        }
    }
    for track in &resource.tracks {
        if Track::parse(track).is_none() {
            return Err(WebError::Domain(DomainError::UnknownTrack(track.clone())));
        }
    }
    Ok(())
}

/// Submit a resource. It stays unvalidated until a validator signs off.
///
/// # Errors
/// Propagates validation and database errors.
pub async fn submit(pool: &PgPool, author: Uuid, resource: &NewResource) -> WebResult<Uuid> {
    validate(resource)?;
    let id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO resources (title, url, resource_type, tracks, specializations, level, submitted_by)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        RETURNING id
        "#,
    )
    .bind(&resource.title)
    .bind(&resource.url)
    .bind(&resource.resource_type)
    .bind(&resource.tracks)
    .bind(&resource.specializations)
    .bind(&resource.level)
    .bind(author)
    .fetch_one(pool)
    .await?;
    Ok(id)
}

/// Validate a resource, paying the submitter.
///
/// Idempotent: a second validation of the same entry pays nothing,
/// because the XP is tied to the transition, not to the act of clicking.
///
/// # Errors
/// `NotFound` when the entry does not exist; otherwise database errors.
pub async fn validate_entry(
    pool: &PgPool,
    announce_channel: Option<u64>,
    validator: Uuid,
    resource_id: Uuid,
) -> WebResult<bool> {
    let mut tx = pool.begin().await?;

    let row: Option<(Uuid, String, Option<Uuid>)> = sqlx::query_as(
        "SELECT submitted_by, title, validated_by FROM resources WHERE id = $1 FOR UPDATE",
    )
    .bind(resource_id)
    .fetch_optional(&mut *tx)
    .await?;

    let Some((submitter, title, already)) = row else {
        return Err(WebError::NotFound);
    };
    if already.is_some() {
        return Ok(false);
    }

    sqlx::query("UPDATE resources SET validated_by = $2 WHERE id = $1")
        .bind(resource_id)
        .bind(validator)
        .execute(&mut *tx)
        .await?;

    xp::grant_in_tx(
        &mut tx,
        announce_channel,
        &XpGrant::new(
            submitter,
            XP_DISCORD_RESOURCE_VALIDATED,
            XpSource::Discord,
        )
        .describe(&format!("Ressource validée : {title}")),
    )
    .await?;

    audit::record_in_tx(
        &mut tx,
        Some(validator),
        "resource.validated",
        Some("resource"),
        Some(resource_id),
        serde_json::json!({ "title": title }),
    )
    .await?;

    tx.commit().await?;
    Ok(true)
}

/// Cast a vote. One per member per resource.
///
/// # Errors
/// `AlreadyVoted` on a repeat; otherwise database errors.
pub async fn vote(pool: &PgPool, voter: Uuid, resource_id: Uuid) -> WebResult<i32> {
    let inserted = sqlx::query(
        r#"
        INSERT INTO resource_votes (resource_id, user_id)
        VALUES ($1, $2)
        ON CONFLICT (resource_id, user_id) DO NOTHING
        "#,
    )
    .bind(resource_id)
    .bind(voter)
    .execute(pool)
    .await?;

    if inserted.rows_affected() == 0 {
        return Err(WebError::Domain(DomainError::AlreadyVoted));
    }

    let votes: i32 = sqlx::query_scalar("SELECT votes FROM resources WHERE id = $1")
        .bind(resource_id)
        .fetch_optional(pool)
        .await?
        .ok_or(WebError::NotFound)?;
    Ok(votes)
}

/// Withdraw a vote.
///
/// # Errors
/// Propagates database errors.
pub async fn unvote(pool: &PgPool, voter: Uuid, resource_id: Uuid) -> WebResult<i32> {
    sqlx::query("DELETE FROM resource_votes WHERE resource_id = $1 AND user_id = $2")
        .bind(resource_id)
        .bind(voter)
        .execute(pool)
        .await?;

    let votes: i32 = sqlx::query_scalar("SELECT votes FROM resources WHERE id = $1")
        .bind(resource_id)
        .fetch_optional(pool)
        .await?
        .ok_or(WebError::NotFound)?;
    Ok(votes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> NewResource {
        NewResource {
            title: "Godot 4 — shaders de base".into(),
            url: "https://docs.godotengine.org".into(),
            resource_type: Some("Tutorial".into()),
            tracks: vec!["VisualArt".into()],
            specializations: vec!["VFX".into()],
            level: Some("Junior".into()),
        }
    }

    #[test]
    fn accepts_a_well_formed_resource() {
        assert!(validate(&sample()).is_ok());
    }

    #[test]
    fn rejects_non_http_urls() {
        let mut r = sample();
        r.url = "ftp://files.example.com".into();
        assert!(validate(&r).is_err());
        // The interesting case: a javascript: URL must never reach a
        // rendered <a href>.
        r.url = "javascript:alert(1)".into();
        assert!(validate(&r).is_err());
    }

    #[test]
    fn rejects_an_empty_title() {
        let mut r = sample();
        r.title = " ".into();
        assert!(validate(&r).is_err());
    }

    #[test]
    fn rejects_unknown_types_and_levels() {
        let mut r = sample();
        r.resource_type = Some("Meme".into());
        assert!(validate(&r).is_err());

        let mut r = sample();
        r.level = Some("Godlike".into());
        assert!(validate(&r).is_err());
    }

    #[test]
    fn rejects_unknown_tracks() {
        let mut r = sample();
        r.tracks = vec!["VisualArt".into(), "Gardening".into()];
        assert!(validate(&r).is_err());
    }

    #[test]
    fn allows_a_resource_with_no_optional_metadata() {
        let r = NewResource {
            title: "Un lien".into(),
            url: "http://example.com".into(),
            resource_type: None,
            tracks: vec![],
            specializations: vec![],
            level: None,
        };
        assert!(validate(&r).is_ok());
    }
}

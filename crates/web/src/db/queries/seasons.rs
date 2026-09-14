//! Seasons.
//!
//! A season is a time window that scopes the leaderboard. Ranks, levels
//! and badges stay cumulative across seasons — the season only changes
//! *what the board shows*, so nobody loses anything they earned when a
//! new one opens.
//!
//! The schema enforces that at most one season is open at any instant
//! (an exclusion constraint over the time range), so "the current
//! season" is always unambiguous.

use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{WebError, WebResult};

/// A season as exposed to the API.
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct Season {
    /// Season id.
    pub id: Uuid,
    /// Display name, e.g. "Saison 1 — La Forge".
    pub name: String,
    /// URL-safe identifier.
    pub slug: String,
    /// What the season is about.
    pub description: Option<String>,
    /// Opening instant, inclusive.
    pub starts_at: chrono::DateTime<chrono::Utc>,
    /// Closing instant, exclusive.
    pub ends_at: chrono::DateTime<chrono::Utc>,
}

/// The season open right now.
///
/// # Errors
/// Propagates database errors.
pub async fn current(pool: &PgPool) -> WebResult<Option<Season>> {
    let row = sqlx::query_as::<_, Season>(
        r#"
        SELECT id, name, slug, description, starts_at, ends_at
          FROM seasons
         WHERE starts_at <= NOW() AND ends_at > NOW()
         LIMIT 1
        "#,
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Every season, newest first.
///
/// # Errors
/// Propagates database errors.
pub async fn list(pool: &PgPool) -> WebResult<Vec<Season>> {
    let rows = sqlx::query_as::<_, Season>(
        r#"
        SELECT id, name, slug, description, starts_at, ends_at
          FROM seasons
         ORDER BY starts_at DESC
        "#,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Look a season up by its slug.
///
/// # Errors
/// `NotFound` when no season carries that slug.
pub async fn by_slug(pool: &PgPool, slug: &str) -> WebResult<Season> {
    sqlx::query_as::<_, Season>(
        r#"
        SELECT id, name, slug, description, starts_at, ends_at
          FROM seasons
         WHERE slug = $1
        "#,
    )
    .bind(slug)
    .fetch_optional(pool)
    .await?
    .ok_or(WebError::NotFound)
}

/// Open a new season.
///
/// The exclusion constraint rejects a window overlapping an existing
/// season, which surfaces as a `Conflict` rather than a 500.
///
/// # Errors
/// `Conflict` on overlap; otherwise propagates database errors.
pub async fn create(
    pool: &PgPool,
    name: &str,
    slug: &str,
    description: Option<&str>,
    starts_at: chrono::DateTime<chrono::Utc>,
    ends_at: chrono::DateTime<chrono::Utc>,
) -> WebResult<Season> {
    if ends_at <= starts_at {
        return Err(WebError::Validation(
            "a season must end after it starts".into(),
        ));
    }

    let result = sqlx::query_as::<_, Season>(
        r#"
        INSERT INTO seasons (name, slug, description, starts_at, ends_at)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING id, name, slug, description, starts_at, ends_at
        "#,
    )
    .bind(name)
    .bind(slug)
    .bind(description)
    .bind(starts_at)
    .bind(ends_at)
    .fetch_one(pool)
    .await;

    match result {
        Ok(season) => Ok(season),
        Err(sqlx::Error::Database(e)) if e.is_unique_violation() => {
            Err(WebError::Conflict("a season with that slug already exists"))
        }
        // 23P01 = exclusion_violation: the window overlaps another season.
        Err(sqlx::Error::Database(e)) if e.code().as_deref() == Some("23P01") => Err(
            WebError::Conflict("that window overlaps an existing season"),
        ),
        Err(e) => Err(WebError::Database(e)),
    }
}

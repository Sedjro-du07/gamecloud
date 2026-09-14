//! Badge reads and manual awards.
//!
//! Automatic badges are granted by the XP engine
//! ([`crate::db::queries::xp::refresh_badges`]). This module serves the
//! read model and the Bureau-only path for the four badges the engine
//! deliberately does not own — `FoundingMember`, `Alumni`,
//! `ExternalMentor` and `BugHunter`, which depend on facts the platform
//! cannot observe.

use gamecloud_shared::roles::SpecialBadge;
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{WebError, WebResult};

/// A badge held by a member, with its display metadata resolved.
#[derive(Debug, Clone, Serialize)]
pub struct BadgeView {
    /// Canonical identifier.
    pub badge_type: String,
    /// Gamified label.
    pub title: String,
    /// How it is earned.
    pub description: String,
    /// When it was awarded.
    pub awarded_at: chrono::DateTime<chrono::Utc>,
}

/// Badges held by a member, newest first.
///
/// Rows whose `badge_type` no longer maps to a known badge are skipped
/// rather than failing the request — a renamed badge should not break a
/// member's profile page.
///
/// # Errors
/// Propagates database errors.
pub async fn list_for_user(pool: &PgPool, user_id: Uuid) -> WebResult<Vec<BadgeView>> {
    let rows: Vec<(String, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
        r#"
        SELECT badge_type, awarded_at
          FROM special_badges
         WHERE user_id = $1
         ORDER BY awarded_at DESC
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .filter_map(|(badge_type, awarded_at)| {
            let badge = SpecialBadge::parse(&badge_type)?;
            Some(BadgeView {
                badge_type,
                title: badge.title().to_string(),
                description: badge.description().to_string(),
                awarded_at,
            })
        })
        .collect())
}

/// The full catalogue, for the "badges you could earn" view.
#[must_use]
pub fn catalogue() -> Vec<BadgeView> {
    SpecialBadge::ALL
        .iter()
        .map(|badge| BadgeView {
            badge_type: badge.as_str().to_string(),
            title: badge.title().to_string(),
            description: badge.description().to_string(),
            awarded_at: chrono::DateTime::UNIX_EPOCH,
        })
        .collect()
}

/// Award a manual badge.
///
/// Refuses badges the engine owns: hand-awarding `Streaker` would be
/// overwritten (or contradicted) by the next evaluation, so the API
/// says no rather than pretending.
///
/// # Errors
/// `Validation` for an unknown or automatic badge; otherwise propagates
/// database errors.
pub async fn award_manual(
    pool: &PgPool,
    user_id: Uuid,
    badge_type: &str,
    awarded_by: Uuid,
) -> WebResult<()> {
    let Some(badge) = SpecialBadge::parse(badge_type) else {
        return Err(WebError::Validation(format!(
            "unknown badge '{badge_type}'"
        )));
    };
    if badge.is_automatic() {
        return Err(WebError::Validation(format!(
            "'{badge_type}' is awarded automatically and cannot be granted by hand"
        )));
    }

    sqlx::query(
        r#"
        INSERT INTO special_badges (user_id, badge_type, awarded_by)
        VALUES ($1, $2, $3)
        ON CONFLICT (user_id, badge_type) DO NOTHING
        "#,
    )
    .bind(user_id)
    .bind(badge.as_str())
    .bind(awarded_by)
    .execute(pool)
    .await?;
    Ok(())
}

/// Revoke a badge.
///
/// # Errors
/// Propagates database errors.
pub async fn revoke(pool: &PgPool, user_id: Uuid, badge_type: &str) -> WebResult<()> {
    sqlx::query("DELETE FROM special_badges WHERE user_id = $1 AND badge_type = $2")
        .bind(user_id)
        .bind(badge_type)
        .execute(pool)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalogue_lists_every_badge() {
        assert_eq!(catalogue().len(), SpecialBadge::ALL.len());
    }

    #[test]
    fn catalogue_entries_carry_display_metadata() {
        for entry in catalogue() {
            assert!(!entry.title.is_empty());
            assert!(!entry.description.is_empty());
            assert!(SpecialBadge::parse(&entry.badge_type).is_some());
        }
    }
}

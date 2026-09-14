//! Track membership.
//!
//! This module closes the largest functional gap the audit found: no
//! code anywhere inserted into `track_memberships`. Members could never
//! join a track, so track XP never accrued, track roles never
//! progressed, the `/track` command and the per-track leaderboard were
//! permanently empty, and `multi_track_bonus` always saw zero active
//! tracks and returned 1.0 — leaving one of the three axes of the whole
//! design inert.
//!
//! Joining a track is also what moves a member from `Visitor` to
//! `Initiate`, which is the step the rank ladder was always specified
//! to have and never had.

use gamecloud_shared::{
    roles::{specializations_for, GlobalRank, Track, TrackRole},
    DomainError,
};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    config::DiscordChannels,
    error::{WebError, WebResult},
    services::notifications::{self, Announcement},
};

/// A member's standing inside one track.
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct MembershipView {
    /// Canonical track identifier.
    pub track: String,
    /// Chosen specialization, if any.
    pub specialization: Option<String>,
    /// Role held inside the track.
    pub track_role: String,
    /// XP accumulated in this track.
    pub track_xp: i64,
    /// When the member joined.
    pub joined_at: chrono::DateTime<chrono::Utc>,
    /// Last activity credited to this track.
    pub last_active_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Validate that `specialization` is one of the canonical options for
/// `track`.
///
/// # Errors
/// `UnknownSpecialization` when it is not.
pub fn validate_specialization(track: Track, specialization: &str) -> WebResult<()> {
    if specializations_for(track)
        .iter()
        .any(|s| s.eq_ignore_ascii_case(specialization))
    {
        Ok(())
    } else {
        Err(WebError::Domain(DomainError::UnknownSpecialization {
            track: track.as_str().to_string(),
            spec: specialization.to_string(),
        }))
    }
}

/// Every track the member belongs to.
///
/// # Errors
/// Propagates database errors.
pub async fn list_for_user(pool: &PgPool, user_id: Uuid) -> WebResult<Vec<MembershipView>> {
    let rows = sqlx::query_as::<_, MembershipView>(
        r#"
        SELECT track, specialization, track_role, track_xp, joined_at, last_active_at
          FROM track_memberships
         WHERE user_id = $1 AND left_at IS NULL
         ORDER BY track_xp DESC, track ASC
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Join a track.
///
/// Idempotent per (member, track) by way of the unique index: a second
/// attempt reports `AlreadyInTrack` rather than creating a duplicate
/// pool. Completing the first join promotes a `Visitor` to `Initiate`
/// in the same transaction, and announces the move.
///
/// # Errors
/// `UnknownTrack`, `UnknownSpecialization`, `AlreadyInTrack`, or a
/// database error.
pub async fn join(
    pool: &PgPool,
    channels: DiscordChannels,
    user_id: Uuid,
    track_name: &str,
    specialization: Option<&str>,
) -> WebResult<MembershipView> {
    let Some(track) = Track::parse(track_name) else {
        return Err(WebError::Domain(DomainError::UnknownTrack(
            track_name.to_string(),
        )));
    };
    if let Some(spec) = specialization {
        validate_specialization(track, spec)?;
    }

    let mut tx = pool.begin().await?;

    let existing: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM track_memberships
             WHERE user_id = $1 AND track = $2 AND left_at IS NULL
        )
        "#,
    )
    .bind(user_id)
    .bind(track.as_str())
    .fetch_one(&mut *tx)
    .await?;

    if existing {
        return Err(WebError::Domain(DomainError::AlreadyInTrack));
    }

    sqlx::query(
        r#"
        INSERT INTO track_memberships (user_id, track, specialization, track_role, track_xp, last_active_at)
        VALUES ($1, $2, $3, $4, 0, NOW())
        ON CONFLICT (user_id, track) DO UPDATE
            SET left_at        = NULL,
                specialization = COALESCE(EXCLUDED.specialization, track_memberships.specialization),
                last_active_at = NOW()
        "#,
    )
    .bind(user_id)
    .bind(track.as_str())
    .bind(specialization)
    .bind(TrackRole::Observer.as_str())
    .execute(&mut *tx)
    .await?;

    // Visitor -> Initiate. This is the only path onto the XP ladder:
    // `next_rank` refuses to promote a Visitor no matter how much XP
    // they hold, precisely so that onboarding cannot be skipped.
    let promoted: Option<(String, bool)> = sqlx::query_as(
        r#"
        UPDATE users
           SET global_rank = 'Initiate'
         WHERE id = $1
           AND global_rank = 'Visitor'
           AND email_verified = TRUE
        RETURNING COALESCE(current_title, discord_id), TRUE
        "#,
    )
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await?;

    if let Some((display, _)) = promoted {
        notifications::enqueue(
            &mut tx,
            channels,
            &Announcement::rank_up(
                user_id,
                &display,
                GlobalRank::Initiate.title(),
                GlobalRank::Initiate.ring_color(),
            ),
        )
        .await?;
    }

    let display: String = sqlx::query_scalar(
        "SELECT COALESCE(current_title, discord_id) FROM users WHERE id = $1",
    )
    .bind(user_id)
    .fetch_one(&mut *tx)
    .await?;

    notifications::enqueue(
        &mut tx,
        channels,
        &Announcement::track_joined(user_id, &display, track.as_str()),
    )
    .await?;

    let view = sqlx::query_as::<_, MembershipView>(
        r#"
        SELECT track, specialization, track_role, track_xp, joined_at, last_active_at
          FROM track_memberships
         WHERE user_id = $1 AND track = $2
        "#,
    )
    .bind(user_id)
    .bind(track.as_str())
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(view)
}

/// Leave a track. The accumulated track XP is kept so rejoining does
/// not reset progress; only the membership row's activity is cleared.
///
/// # Errors
/// Propagates database errors.
pub async fn leave(pool: &PgPool, user_id: Uuid, track_name: &str) -> WebResult<()> {
    let Some(track) = Track::parse(track_name) else {
        return Err(WebError::Domain(DomainError::UnknownTrack(
            track_name.to_string(),
        )));
    };
    // Soft leave: the row carries `track_xp`, so deleting it would
    // destroy the progress earned in this discipline. Rejoining clears
    // `left_at` and the standing comes back.
    sqlx::query(
        "UPDATE track_memberships SET left_at = NOW() WHERE user_id = $1 AND track = $2",
    )
        .bind(user_id)
        .bind(track.as_str())
        .execute(pool)
        .await?;
    Ok(())
}

/// Appoint a member to a track role (`CoLead` or `Lead`). These are the
/// two roles [`TrackRole::from_track_xp`] never assigns automatically.
///
/// # Errors
/// Propagates database errors.
pub async fn set_role(
    pool: &PgPool,
    user_id: Uuid,
    track: Track,
    role: TrackRole,
) -> WebResult<()> {
    sqlx::query(
        "UPDATE track_memberships SET track_role = $3 WHERE user_id = $1 AND track = $2 AND left_at IS NULL",
    )
    .bind(user_id)
    .bind(track.as_str())
    .bind(role.as_str())
    .execute(pool)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_a_canonical_specialization() {
        assert!(validate_specialization(Track::Engineering, "Gameplay").is_ok());
        // Case-insensitive: the UI sends whatever the member typed.
        assert!(validate_specialization(Track::Engineering, "gameplay").is_ok());
    }

    #[test]
    fn rejects_a_specialization_from_another_track() {
        // "Composer" is an Audio specialization, not an Engineering one.
        assert!(validate_specialization(Track::Engineering, "Composer").is_err());
    }

    #[test]
    fn rejects_invented_specializations() {
        assert!(validate_specialization(Track::Narrative, "Vibes Engineer").is_err());
    }

    #[test]
    fn every_track_accepts_its_own_first_specialization() {
        for track in Track::ALL {
            let first = specializations_for(track)[0];
            assert!(
                validate_specialization(track, first).is_ok(),
                "track {track:?} rejected its own specialization {first}"
            );
        }
    }
}

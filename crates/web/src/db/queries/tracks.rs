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
         -- Most important title first, then the most XP within a title.
         ORDER BY array_position(
                      ARRAY['Lead', 'CoLead', 'Mentor', 'Reviewer', 'Contributor', 'Observer'],
                      track_role),
                  track_xp DESC, track ASC
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

    // Visitor -> onto the XP ladder. This is the only path onto it:
    // `next_rank` refuses to promote a Visitor no matter how much XP
    // they hold, precisely so that onboarding cannot be skipped. The
    // title is computed from the XP already held rather than fixed at
    // Initiate — XP carried over from before the platform counts the
    // moment onboarding is done, not at the member's next award.
    let xp_total: i64 = sqlx::query_scalar("SELECT xp_total FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_one(&mut *tx)
        .await?;
    let reached = GlobalRank::from_xp(xp_total);
    let promoted: Option<(String, bool)> = sqlx::query_as(
        r#"
        UPDATE users
           SET global_rank = $2
         WHERE id = $1
           AND global_rank IN ('Pending', 'Visitor')
        RETURNING member_display_name(current_title, discord_global_name, discord_username, discord_id), TRUE
        "#,
    )
    .bind(user_id)
    .bind(reached.as_str())
    .fetch_optional(&mut *tx)
    .await?;

    if let Some((display, _)) = promoted {
        notifications::enqueue(
            &mut tx,
            channels,
            &Announcement::rank_up(user_id, &display, reached.title(), reached.ring_color()),
        )
        .await?;
    }

    let display: String = sqlx::query_scalar(
        "SELECT member_display_name(current_title, discord_global_name, discord_username, discord_id) FROM users WHERE id = $1",
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
    notifications::request_role_sync(&mut *tx).await?;

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
    notifications::request_role_sync(pool).await?;
    Ok(())
}

/// Appoint a member to a track role (`CoLead` or `Lead`). These are the
/// two roles [`TrackRole::from_track_xp`] never assigns automatically.
///
/// # Errors
/// Propagates database errors.
pub async fn set_role(
    pool: &PgPool,
    channels: crate::config::DiscordChannels,
    user_id: Uuid,
    track: Track,
    role: TrackRole,
) -> WebResult<()> {
    let mut tx = pool.begin().await?;

    // Appointing somebody *joins* them to the track when they are not
    // already in it. The previous version was a bare UPDATE, so naming
    // a Lead for a track they had never joined matched zero rows,
    // returned `Ok`, and the interface cheerfully reported "Nomination
    // enregistrée" while nothing whatsoever had happened — which is
    // exactly how several real appointments went missing.
    //
    // A member who left the track is re-admitted rather than left
    // behind a `left_at`, since being appointed is a deliberate act by
    // somebody with the authority to do it.
    let changed = sqlx::query(
        r"
        INSERT INTO track_memberships (user_id, track, track_role)
        VALUES ($1, $2, $3)
        ON CONFLICT (user_id, track) DO UPDATE
            SET track_role = EXCLUDED.track_role,
                left_at    = NULL
         WHERE track_memberships.track_role IS DISTINCT FROM EXCLUDED.track_role
            OR track_memberships.left_at IS NOT NULL
        ",
    )
    .bind(user_id)
    .bind(track.as_str())
    .bind(role.as_str())
    .execute(&mut *tx)
    .await?
    .rows_affected();

    // Being appointed is something you are told, not something you
    // discover by noticing a new button. Only on a real change: a no-op
    // re-appointment should not ping anybody.
    if changed > 0 {
        crate::services::notifications::enqueue(
            &mut tx,
            channels,
            &crate::services::notifications::Announcement::track_appointment(
                user_id,
                track,
                role.as_str(),
            ),
        )
        .await?;
    }

    tx.commit().await?;
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

// ---------------------------------------------------------------------------
// Track board
// ---------------------------------------------------------------------------

/// One member of a track.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct BoardMember {
    /// Name to show.
    pub display_name: String,
    /// `Lead`, `CoLead`, `Mentor`, `Reviewer`, `Contributor`, `Observer`.
    pub track_role: String,
    /// XP earned inside this track.
    pub track_xp: i64,
    /// Chosen specialization, when set.
    pub specialization: Option<String>,
}

/// One project as it stands with this track.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct BoardProject {
    /// Project id.
    pub id: Uuid,
    /// Project name.
    pub name: String,
    /// Project status.
    pub status: String,
    /// This track's verdict.
    pub verdict: String,
    /// This track's mark, when one was given.
    pub score: Option<i32>,
    /// Who rendered the verdict.
    pub reviewer_name: Option<String>,
    /// Builds attached to the project.
    pub file_count: i64,
}

/// Everything the track page needs, minus its calendar.
#[derive(Debug, Clone)]
pub struct Board {
    /// The viewer's role in this track, when they belong to it.
    pub my_role: Option<String>,
    /// XP pooled across the track's members.
    pub total_xp: i64,
    /// Mean of the marks this track has given.
    pub average_score: Option<f64>,
    /// Members, strongest role first.
    pub members: Vec<BoardMember>,
    /// Projects this track has a say in.
    pub projects: Vec<BoardProject>,
}

/// Load one track's board.
///
/// Members are ordered by *role* before XP, because the page is a "who
/// do I talk to" list before it is a ranking — the Lead belongs at the
/// top even on the day somebody out-earns them.
///
/// # Errors
/// Propagates database errors.
pub async fn board(pool: &PgPool, track: &str, viewer: Option<Uuid>) -> WebResult<Board> {
    let members = sqlx::query_as::<_, BoardMember>(
        r"
        SELECT member_display_name(u.current_title, u.discord_global_name,
                                   u.discord_username, u.discord_id) AS display_name,
               tm.track_role,
               tm.track_xp,
               tm.specialization
          FROM track_memberships tm
          JOIN users u ON u.id = tm.user_id
         WHERE tm.track = $1 AND tm.left_at IS NULL
         ORDER BY CASE tm.track_role
                      WHEN 'Lead'        THEN 0
                      WHEN 'CoLead'      THEN 1
                      WHEN 'Mentor'      THEN 2
                      WHEN 'Reviewer'    THEN 3
                      WHEN 'Contributor' THEN 4
                      ELSE 5
                  END,
                  tm.track_xp DESC
        ",
    )
    .bind(track)
    .fetch_all(pool)
    .await?;

    let projects = sqlx::query_as::<_, BoardProject>(
        r"
        SELECT p.id,
               p.name,
               p.status,
               tv.status AS verdict,
               tv.score,
               CASE WHEN tv.reviewed_by IS NULL THEN NULL
                    ELSE member_display_name(r.current_title, r.discord_global_name,
                                             r.discord_username, r.discord_id)
               END AS reviewer_name,
               (SELECT COUNT(*) FROM project_files f WHERE f.project_id = p.id) AS file_count
          FROM track_validations tv
          JOIN projects p ON p.id = tv.project_id
          LEFT JOIN users r ON r.id = tv.reviewed_by
         WHERE tv.track = $1
         ORDER BY CASE tv.status WHEN 'Pending' THEN 0 ELSE 1 END,
                  p.created_at DESC
        ",
    )
    .bind(track)
    .fetch_all(pool)
    .await?;

    let total_xp: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(track_xp), 0)::bigint FROM track_memberships \
         WHERE track = $1 AND left_at IS NULL",
    )
    .bind(track)
    .fetch_one(pool)
    .await?;

    let average_score: Option<f64> = sqlx::query_scalar(
        "SELECT AVG(score)::float8 FROM track_validations WHERE track = $1 AND score IS NOT NULL",
    )
    .bind(track)
    .fetch_one(pool)
    .await?;

    let my_role: Option<String> = match viewer {
        Some(id) => {
            sqlx::query_scalar(
                "SELECT track_role FROM track_memberships \
                 WHERE track = $1 AND user_id = $2 AND left_at IS NULL",
            )
            .bind(track)
            .bind(id)
            .fetch_optional(pool)
            .await?
        }
        None => None,
    };

    Ok(Board {
        my_role,
        total_xp,
        average_score,
        members,
        projects,
    })
}

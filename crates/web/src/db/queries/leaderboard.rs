//! Leaderboards.
//!
//! Three scopes, all served from the same `xp_logs` ledger:
//!
//! - **All-time** — `users.xp_total`. The permanent record.
//! - **Season** — XP stamped with the open season. This is the one that
//!   matters day to day: an all-time board freezes, because the members
//!   who founded the club sit on top of it forever and a first-year who
//!   joins in September can see they will never catch up. Seasons give
//!   everyone a board they can actually win without taking anything
//!   away from the people who earned their rank.
//! - **Track** — `track_memberships.track_xp`, for the per-discipline
//!   standings.

use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::WebResult;

/// One row of a leaderboard.
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct LeaderboardRow {
    /// Member id.
    pub user_id: Uuid,
    /// Display name (custom title, else Discord id).
    pub display_name: String,
    /// Avatar URL, if set.
    pub avatar_url: Option<String>,
    /// XP within the requested scope.
    pub xp: i64,
    /// Global rank string.
    pub global_rank: String,
    /// Current level.
    pub level: i32,
    /// Total XP, whatever the scope — what the member title is worth.
    pub total_xp: i64,
    /// Current streak.
    pub streak_days: i32,
}

/// All-time standings by total XP.
///
/// # Errors
/// Propagates database errors.
pub async fn all_time(pool: &PgPool, limit: i64) -> WebResult<Vec<LeaderboardRow>> {
    let rows = sqlx::query_as::<_, LeaderboardRow>(
        r#"
        SELECT u.id AS user_id,
               member_display_name(u.current_title, u.discord_global_name, u.discord_username, u.discord_id) AS display_name,
               COALESCE(u.avatar_custom_url, u.avatar_url) AS avatar_url,
               u.xp_total AS xp,
               u.global_rank,
               u.level,
               u.xp_total AS total_xp,
               u.streak_days
          FROM users u
         WHERE (NOT u.candidate)
         ORDER BY u.xp_total DESC, u.created_at ASC
         LIMIT $1
        "#,
    )
    .bind(limit.clamp(1, 200))
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Standings within a season.
///
/// Members with no XP this season are omitted rather than listed at
/// zero — a season board is about who is playing now.
///
/// # Errors
/// Propagates database errors.
pub async fn by_season(
    pool: &PgPool,
    season_id: Uuid,
    limit: i64,
) -> WebResult<Vec<LeaderboardRow>> {
    let rows = sqlx::query_as::<_, LeaderboardRow>(
        r#"
        SELECT u.id AS user_id,
               member_display_name(u.current_title, u.discord_global_name, u.discord_username, u.discord_id) AS display_name,
               COALESCE(u.avatar_custom_url, u.avatar_url) AS avatar_url,
               COALESCE(SUM(x.amount), 0)::BIGINT AS xp,
               u.global_rank,
               u.level,
               u.xp_total AS total_xp,
               u.streak_days
          FROM users u
          JOIN xp_logs x ON x.user_id = u.id AND x.season_id = $1
         WHERE (NOT u.candidate)
         GROUP BY u.id
        HAVING SUM(x.amount) > 0
         ORDER BY xp DESC, u.created_at ASC
         LIMIT $2
        "#,
    )
    .bind(season_id)
    .bind(limit.clamp(1, 200))
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Standings within one track.
///
/// # Errors
/// Propagates database errors.
pub async fn by_track(pool: &PgPool, track: &str, limit: i64) -> WebResult<Vec<LeaderboardRow>> {
    let rows = sqlx::query_as::<_, LeaderboardRow>(
        r#"
        SELECT u.id AS user_id,
               member_display_name(u.current_title, u.discord_global_name, u.discord_username, u.discord_id) AS display_name,
               COALESCE(u.avatar_custom_url, u.avatar_url) AS avatar_url,
               m.track_xp AS xp,
               m.track_role AS global_rank,
               u.level,
               u.xp_total AS total_xp,
               u.streak_days
          FROM track_memberships m
          JOIN users u ON u.id = m.user_id
         WHERE m.track = $1 AND m.left_at IS NULL AND (NOT u.candidate)
         ORDER BY m.track_xp DESC, m.joined_at ASC
         LIMIT $2
        "#,
    )
    .bind(track)
    .bind(limit.clamp(1, 200))
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Where one member sits on the all-time board. `None` when they have
/// neither a verified email nor any XP, and therefore no ranking — the
/// same rule the boards themselves use.
///
/// # Errors
/// Propagates database errors.
pub async fn position_of(pool: &PgPool, user_id: Uuid) -> WebResult<Option<i64>> {
    let position: Option<i64> = sqlx::query_scalar(
        r#"
        SELECT position FROM (
            SELECT id, ROW_NUMBER() OVER (ORDER BY xp_total DESC, created_at ASC) AS position
              FROM users
             WHERE (NOT candidate)
        ) ranked
        WHERE ranked.id = $1
        "#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .flatten();
    Ok(position)
}

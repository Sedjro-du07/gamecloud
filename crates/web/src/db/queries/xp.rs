//! XP grant queries.

use gamecloud_shared::xp::XpSource;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::WebResult;

/// Sum of XP from the given source for the given user, today (UTC).
/// Used to enforce daily caps before granting XP.
///
/// # Errors
/// Propagates database errors.
pub async fn xp_today_for_source(
    pool: &PgPool,
    user_id: Uuid,
    source: XpSource,
) -> WebResult<i64> {
    let total: Option<i64> = sqlx::query_scalar(
        r#"
        SELECT COALESCE(SUM(amount), 0)::BIGINT
        FROM xp_logs
        WHERE user_id = $1
          AND source = $2
          AND created_at >= date_trunc('day', NOW())
        "#,
    )
    .bind(user_id)
    .bind(source.as_str())
    .fetch_one(pool)
    .await?;
    Ok(total.unwrap_or(0))
}

/// Atomically grant XP to a user, recording the change in `xp_logs`,
/// updating `users.xp_total` and `global_rank`, and updating the
/// matching `track_memberships.track_xp` row when `track` is set.
///
/// `amount` is the *final* XP value (multipliers already applied).
///
/// # Errors
/// Propagates database errors.
pub async fn grant_xp(
    pool: &PgPool,
    user_id: Uuid,
    amount: i32,
    source: XpSource,
    track: Option<&str>,
    description: Option<&str>,
) -> WebResult<()> {
    let mut tx = pool.begin().await?;

    sqlx::query(
        r#"
        INSERT INTO xp_logs (user_id, amount, source, track, description)
        VALUES ($1, $2, $3, $4, $5)
        "#,
    )
    .bind(user_id)
    .bind(amount)
    .bind(source.as_str())
    .bind(track)
    .bind(description)
    .execute(&mut *tx)
    .await?;

    // Update the user's global XP. The 70% global ratio applies only
    // when the source is track-scoped; non-track sources contribute
    // 100%. The arithmetic is done in Rust to keep the SQL simple.
    let global_delta = if track.is_some() {
        let scaled = f64::from(amount) * gamecloud_shared::xp::TRACK_TO_GLOBAL_XP_RATIO;
        scaled.round() as i32
    } else {
        amount
    };

    sqlx::query(
        r#"
        UPDATE users
            SET xp_total = GREATEST(xp_total + $2::BIGINT, 0),
                last_activity_at = NOW(),
                global_rank = CASE
                    WHEN email_verified = FALSE THEN global_rank
                    WHEN xp_total + $2::BIGINT >= 25000 THEN 'Myth'
                    WHEN xp_total + $2::BIGINT >= 10000 THEN 'Legend'
                    WHEN xp_total + $2::BIGINT >= 5000  THEN 'Veteran'
                    WHEN xp_total + $2::BIGINT >= 2500  THEN 'Expert'
                    WHEN xp_total + $2::BIGINT >= 1000  THEN 'SeniorDev'
                    WHEN xp_total + $2::BIGINT >= 400   THEN 'JuniorDev'
                    WHEN xp_total + $2::BIGINT >= 150   THEN 'Apprentice'
                    WHEN global_rank IN ('Pending', 'Visitor') THEN global_rank
                    ELSE 'Initiate'
                END
            WHERE id = $1
        "#,
    )
    .bind(user_id)
    .bind(i64::from(global_delta))
    .execute(&mut *tx)
    .await?;

    if let Some(track) = track {
        sqlx::query(
            r#"
            UPDATE track_memberships
                SET track_xp = GREATEST(track_xp + $3::BIGINT, 0),
                    last_active_at = NOW(),
                    track_role = CASE
                        WHEN track_role IN ('CoLead', 'Lead') THEN track_role
                        WHEN track_xp + $3::BIGINT >= 500 THEN 'Mentor'
                        WHEN track_xp + $3::BIGINT >= 300 THEN 'Reviewer'
                        WHEN track_xp + $3::BIGINT >= 100 THEN 'Contributor'
                        ELSE 'Observer'
                    END
                WHERE user_id = $1 AND track = $2
            "#,
        )
        .bind(user_id)
        .bind(track)
        .bind(i64::from(amount))
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(())
}

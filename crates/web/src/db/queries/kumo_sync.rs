//! Two-way XP sync with Kumo — the platform's half.
//!
//! See [`gamecloud_shared::kumo`] for the counters and the rate. This
//! module owns the database side: paying what Kumo reports, and
//! publishing what the platform has awarded.
//!
//! Everything Kumo-origin is tagged `Synchro Kumo…` in the ledger, which
//! is how the involvement score leaves it out (chatting says nothing
//! about this month's work) and how it is told apart from the 1:1
//! carry-over, `Report de l'XP …`.

use gamecloud_shared::{
    kumo::{credit_units, platform_origin},
    xp::XpSource,
};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::{
    config::DiscordChannels,
    db::queries::{
        audit,
        xp::{self, XpGrant},
    },
    error::WebResult,
};

/// Ledger description of a Kumo payment.
const DESCRIPTION: &str = "Synchro Kumo : activité Discord";

/// What Kumo says about one member.
#[derive(Debug, Clone, Copy)]
pub struct KumoReport<'a> {
    /// Discord id of the member.
    pub discord_id: &'a str,
    /// Cumulative XP earned by chatting, on Kumo's side.
    pub organic_total: i64,
    /// Kumo's organic XP on the day the sync was switched on.
    pub organic_base: i64,
}

/// Record a Kumo report and pay what it newly justifies.
///
/// Returns the platform XP paid. The sync row is locked by the upsert
/// for the rest of the transaction, so two concurrent reports for the
/// same member are paid one after the other, never both on the same
/// counter.
///
/// # Errors
/// Propagates database errors.
pub async fn report(
    pool: &PgPool,
    channels: DiscordChannels,
    ratio: i64,
    report: KumoReport<'_>,
) -> WebResult<i64> {
    let mut tx = pool.begin().await?;
    // The baseline is decided once, when the row is created: the
    // snapshot value for members already carried over, Kumo's base for
    // the others. A later report can never move it.
    sqlx::query(
        r#"
        INSERT INTO kumo_sync (discord_id, baseline, organic_total)
        VALUES (
            $1,
            COALESCE((SELECT xp::bigint FROM legacy_xp WHERE discord_id = $1), $3),
            $2
        )
        ON CONFLICT (discord_id) DO UPDATE
           SET organic_total = GREATEST(kumo_sync.organic_total, EXCLUDED.organic_total),
               updated_at    = NOW()
        "#,
    )
    .bind(report.discord_id)
    .bind(report.organic_total.max(0))
    .bind(report.organic_base.max(0))
    .execute(&mut *tx)
    .await?;

    let paid = settle(&mut tx, channels, ratio, report.discord_id).await?;
    tx.commit().await?;
    Ok(paid)
}

/// Pay, at first login, the Kumo activity reported before the member
/// had an account.
///
/// # Errors
/// Propagates database errors.
pub async fn pay_pending(
    pool: &PgPool,
    channels: DiscordChannels,
    ratio: i64,
    user_id: Uuid,
    discord_id: &str,
) -> WebResult<i64> {
    let mut tx = pool.begin().await?;
    let paid = settle(&mut tx, channels, ratio, discord_id).await?;
    tx.commit().await?;
    if paid > 0 {
        tracing::info!(user = %user_id, paid, "kumo activity paid at first login");
    }
    Ok(paid)
}

/// Lock the member's sync row, pay the difference, advance `credited`.
async fn settle(
    tx: &mut Transaction<'_, Postgres>,
    channels: DiscordChannels,
    ratio: i64,
    discord_id: &str,
) -> WebResult<i64> {
    let row: Option<(i64, i64, i64)> = sqlx::query_as(
        "SELECT baseline, organic_total, credited FROM kumo_sync \
          WHERE discord_id = $1 FOR UPDATE",
    )
    .bind(discord_id)
    .fetch_optional(&mut **tx)
    .await?;
    let Some((baseline, total, credited)) = row else {
        return Ok(0);
    };

    let owed = credit_units(total, baseline, ratio) - credited;
    if owed <= 0 {
        return Ok(0);
    }
    // No account yet: leave `credited` alone, it is paid at login.
    let Some(user_id) =
        sqlx::query_scalar::<_, Uuid>("SELECT id FROM users WHERE discord_id = $1")
            .bind(discord_id)
            .fetch_optional(&mut **tx)
            .await?
    else {
        return Ok(0);
    };

    let amount = i32::try_from(owed).unwrap_or(i32::MAX);
    // `exact`: the rate is 5 Kumo XP to 1, whatever the streak.
    let outcome = xp::grant_in_tx(
        tx,
        channels,
        &XpGrant::new(user_id, amount, XpSource::Discord)
            .describe(DESCRIPTION)
            .exact(),
    )
    .await?;
    let paid = i64::from(outcome.awarded);
    if paid == 0 {
        return Ok(0);
    }

    sqlx::query("UPDATE kumo_sync SET credited = credited + $2, updated_at = NOW() WHERE discord_id = $1")
        .bind(discord_id)
        .bind(paid)
        .execute(&mut **tx)
        .await?;
    audit::record_in_tx(
        tx,
        None,
        "kumo.xp_synced",
        Some("user"),
        Some(user_id),
        serde_json::json!({ "platform_xp": paid, "kumo_total": total, "baseline": baseline }),
    )
    .await?;
    Ok(paid)
}

/// One member's platform-origin XP, for Kumo to mirror.
#[derive(Debug, sqlx::FromRow)]
struct OriginRow {
    discord_id: String,
    xp_total: i64,
    carried_over: i64,
    kumo_credited: i64,
}

/// Every member's platform-origin XP, keyed by Discord id.
///
/// `xp_total` minus the 1:1 carry-over minus what Kumo itself paid, so
/// that nothing Kumo-origin is ever sent back to Kumo.
///
/// # Errors
/// Propagates database errors.
pub async fn platform_origin_xp(pool: &PgPool) -> WebResult<Vec<(String, i64)>> {
    let rows: Vec<OriginRow> = sqlx::query_as(
        r#"
        SELECT u.discord_id,
               u.xp_total::bigint AS xp_total,
               COALESCE((SELECT SUM(l.amount) FROM xp_logs l
                          WHERE l.user_id = u.id
                            AND l.description LIKE 'Report de l''XP %'), 0)::bigint AS carried_over,
               COALESCE(k.credited, 0)::bigint AS kumo_credited
          FROM users u
          LEFT JOIN kumo_sync k ON k.discord_id = u.discord_id
         WHERE u.discord_id IS NOT NULL
        "#,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| {
            (
                r.discord_id,
                platform_origin(r.xp_total, r.carried_over, r.kumo_credited),
            )
        })
        .collect())
}

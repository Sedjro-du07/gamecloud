//! The XP engine.
//!
//! Every XP award on the platform funnels through [`grant`]. Before the
//! May 2026 audit there were two divergent paths — the webhook handler
//! called `grant_xp`, while the QR scan inlined its own ledger insert
//! and skipped the rank recomputation entirely, so attendance XP never
//! promoted anybody. There is now one path, and it is responsible for
//! the whole cascade an XP event triggers:
//!
//! 1. resolve the member's multiplier inputs (streak, active tracks);
//! 2. apply multipliers, *then* the daily cap (order matters — see
//!    [`gamecloud_shared::xp::apply_daily_cap`]);
//! 3. append to the `xp_logs` ledger, stamped with the open season;
//! 4. update the member's total, rank, level and streak;
//! 5. update the track pool when the award is track-scoped;
//! 6. advance any quest the event satisfies, completing and paying out
//!    those that reach their target;
//! 7. re-evaluate badges;
//! 8. enqueue Discord announcements for anything worth celebrating.
//!
//! The whole cascade is one transaction. A member never sees a rank-up
//! announced for XP that rolled back, and never keeps XP whose badge
//! insert failed.

use chrono::{Datelike, NaiveDate, Timelike, Utc};
use gamecloud_shared::{
    badges::{self, BadgeSnapshot},
    roles::{GlobalRank, SpecialBadge},
    xp::{
        apply_daily_cap, compute_final_xp, evaluate_streak, level_for_xp, QuestCondition, XpSource,
        TRACK_TO_GLOBAL_XP_RATIO,
    },
};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::{
    config::DiscordChannels,
    error::WebResult,
    services::notifications::{self, Announcement},
};

// ---------------------------------------------------------------------------
// Inputs
// ---------------------------------------------------------------------------

/// A daily ceiling applied to an award.
#[derive(Debug, Clone, Copy)]
pub struct DailyCap<'a> {
    /// Maximum XP from this bucket in a single UTC day.
    pub limit: i32,
    /// When set, only ledger rows whose `description` starts with this
    /// prefix count toward the ceiling.
    ///
    /// This exists because the commit cap must measure *commits*. The
    /// pre-audit code summed every `GitHub` row, so a merged PR ate the
    /// commit budget and PR XP itself went uncapped.
    pub description_prefix: Option<&'a str>,
}

/// One XP award, before multipliers and caps.
#[derive(Debug, Clone)]
pub struct XpGrant<'a> {
    /// Recipient.
    pub user_id: Uuid,
    /// Award before multipliers.
    pub base: i32,
    /// Where the XP came from.
    pub source: XpSource,
    /// Track to credit, when the award is track-scoped.
    pub track: Option<&'a str>,
    /// Human-readable ledger line.
    pub description: Option<&'a str>,
    /// Optional daily ceiling.
    pub cap: Option<DailyCap<'a>>,
    /// Whether streak and multi-track multipliers apply. Manual grants
    /// and quest payouts set this to `false` so the Bureau gets the
    /// number it typed.
    pub apply_multipliers: bool,
}

impl<'a> XpGrant<'a> {
    /// A plain award with multipliers on and no ceiling.
    #[must_use]
    pub fn new(user_id: Uuid, base: i32, source: XpSource) -> Self {
        Self {
            user_id,
            base,
            source,
            track: None,
            description: None,
            cap: None,
            apply_multipliers: true,
        }
    }

    /// Attach a ledger description.
    #[must_use]
    pub fn describe(mut self, description: &'a str) -> Self {
        self.description = Some(description);
        self
    }

    /// Credit a track pool as well as the global one.
    #[must_use]
    pub fn in_track(mut self, track: &'a str) -> Self {
        self.track = Some(track);
        self
    }

    /// Apply a daily ceiling.
    #[must_use]
    pub fn capped(mut self, limit: i32, description_prefix: Option<&'a str>) -> Self {
        self.cap = Some(DailyCap {
            limit,
            description_prefix,
        });
        self
    }

    /// Award the literal amount, ignoring streak and track multipliers.
    #[must_use]
    pub fn exact(mut self) -> Self {
        self.apply_multipliers = false;
        self
    }
}

// ---------------------------------------------------------------------------
// Outputs
// ---------------------------------------------------------------------------

/// A quest finished by this award.
#[derive(Debug, Clone)]
pub struct CompletedQuest {
    /// Quest identifier.
    pub id: Uuid,
    /// Quest title, for the announcement.
    pub title: String,
    /// XP paid out for finishing it.
    pub xp_reward: i32,
}

/// What an award actually did.
#[derive(Debug, Clone, Default)]
pub struct GrantOutcome {
    /// XP actually credited after multipliers and caps. Zero means the
    /// daily ceiling swallowed the award.
    pub awarded: i32,
    /// True when the award was trimmed or zeroed by its ceiling.
    pub capped: bool,
    /// Rank before and after, when it changed.
    pub rank_change: Option<(GlobalRank, GlobalRank)>,
    /// Level before and after, when it changed.
    pub level_change: Option<(i32, i32)>,
    /// The member's streak after this award.
    pub streak_days: i32,
    /// Badges unlocked by this award.
    pub new_badges: Vec<SpecialBadge>,
    /// Quests finished by this award.
    pub completed_quests: Vec<CompletedQuest>,
}

// ---------------------------------------------------------------------------
// Entry points
// ---------------------------------------------------------------------------

/// Award XP in its own transaction.
///
/// # Errors
/// Propagates database errors.
pub async fn grant(
    pool: &PgPool,
    channels: DiscordChannels,
    request: &XpGrant<'_>,
) -> WebResult<GrantOutcome> {
    let mut tx = pool.begin().await?;
    let outcome = grant_in_tx(&mut tx, channels, request).await?;
    tx.commit().await?;
    Ok(outcome)
}

/// Award XP inside a transaction the caller owns.
///
/// Used by flows that must be atomic with something else — a QR scan
/// also writes an `attendance` row, a project release awards every
/// contributor at once.
///
/// # Errors
/// Propagates database errors.
pub async fn grant_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    channels: DiscordChannels,
    request: &XpGrant<'_>,
) -> WebResult<GrantOutcome> {
    let member = load_member(tx, request.user_id).await?;
    let Some(member) = member else {
        // Nothing to award to. Not an error: webhooks routinely fire for
        // GitHub logins nobody has linked.
        return Ok(GrantOutcome::default());
    };

    // --- 1 & 2: multipliers, then the ceiling ------------------------
    let multiplied = if request.apply_multipliers {
        compute_final_xp(request.base, member.streak_days, member.active_tracks)
    } else {
        request.base
    };

    let (awarded, capped) = match request.cap {
        Some(cap) => {
            let already =
                xp_today(tx, request.user_id, request.source, cap.description_prefix).await?;
            let trimmed = apply_daily_cap(multiplied, already, cap.limit);
            (trimmed, trimmed < multiplied)
        }
        None => (multiplied, false),
    };

    if awarded == 0 {
        return Ok(GrantOutcome {
            capped,
            streak_days: member.streak_days,
            ..Default::default()
        });
    }

    // --- 3: ledger ---------------------------------------------------
    let season_id = current_season_id(tx).await?;
    append_ledger(tx, request, awarded, season_id).await?;

    // --- 4: member totals, rank, level, streak -----------------------
    let global_delta = if request.track.is_some() {
        let scaled = f64::from(awarded) * TRACK_TO_GLOBAL_XP_RATIO;
        scaled.round() as i32
    } else {
        awarded
    };

    let xp_after = (member.xp_total + i64::from(global_delta)).max(0);
    let rank_before = GlobalRank::parse(&member.global_rank);
    let rank_after = next_rank(rank_before, xp_after, member.email_verified);
    let level_before = member.level;
    let level_after = level_for_xp(xp_after);

    let streak_after = apply_member_update(
        tx,
        request.user_id,
        &member,
        xp_after,
        rank_after,
        level_after,
    )
    .await?;

    // --- 5: track pool -----------------------------------------------
    if let Some(track) = request.track {
        credit_track_pool(tx, request.user_id, track, awarded).await?;
    }

    // --- 6: quests ---------------------------------------------------
    let (completed_quests, rank_final, level_final) = settle_quests(
        tx,
        request,
        season_id,
        member.email_verified,
        xp_after,
        rank_after,
        level_after,
    )
    .await?;

    // --- 7: badges ---------------------------------------------------
    let new_badges = refresh_badges(tx, request.user_id, streak_after).await?;

    // A hand-made adjustment is the one XP movement with no visible
    // cause: nothing the member did produced it. Telling them, with the
    // reason the Bureau gave, is what keeps the ledger from feeling
    // arbitrary. Everything else is announced by `announce` below.
    if request.source == XpSource::Manual {
        notifications::enqueue(
            tx,
            channels,
            &Announcement::manual_xp(
                request.user_id,
                awarded,
                request.description.unwrap_or("Ajustement du Bureau"),
            ),
        )
        .await?;
    }

    // A new title changes the member's rank role on Discord.
    if rank_final != rank_before {
        notifications::request_role_sync(&mut **tx).await?;
    }

    // --- 8: announcements --------------------------------------------
    announce(
        tx,
        channels,
        request.user_id,
        &member.display_name(),
        (rank_final > rank_before).then_some(rank_final),
        &new_badges,
        &completed_quests,
    )
    .await?;

    Ok(GrantOutcome {
        awarded,
        capped,
        rank_change: (rank_final != rank_before).then_some((rank_before, rank_final)),
        level_change: (level_final != level_before).then_some((level_before, level_final)),
        streak_days: streak_after,
        new_badges,
        completed_quests,
    })
}

/// Credit a track's XP pool and re-derive the member's track role.
///
/// `CoLead` and `Lead` are appointed, never earned, so they are left
/// alone — the CASE deliberately short-circuits on them.
async fn credit_track_pool(
    tx: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    track: &str,
    awarded: i32,
) -> WebResult<()> {
    sqlx::query(
        r#"
        UPDATE track_memberships
           SET track_xp       = GREATEST(track_xp + $3::BIGINT, 0),
               last_active_at = NOW(),
               track_role     = CASE
                   WHEN track_role IN ('CoLead', 'Lead') THEN track_role
                   WHEN track_xp + $3::BIGINT >= 500 THEN 'Mentor'
                   WHEN track_xp + $3::BIGINT >= 300 THEN 'Reviewer'
                   WHEN track_xp + $3::BIGINT >= 100 THEN 'Contributor'
                   ELSE 'Observer'
               END
         WHERE user_id = $1 AND track = $2 AND left_at IS NULL
        "#,
    )
    .bind(user_id)
    .bind(track)
    .bind(i64::from(awarded))
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Advance quests, pay out any that finished, and re-derive the rank
/// and level those payouts may have moved.
///
/// Quest rewards are credited *after* the main award has already
/// updated the member's totals, so this is where the two are
/// reconciled. Extracted from [`grant_in_tx`] to keep that function a
/// readable sequence of stages.
async fn settle_quests(
    tx: &mut Transaction<'_, Postgres>,
    request: &XpGrant<'_>,
    season_id: Option<Uuid>,
    email_verified: bool,
    xp_after: i64,
    rank_after: GlobalRank,
    level_after: i32,
) -> WebResult<(Vec<CompletedQuest>, GlobalRank, i32)> {
    let completed = advance_quests(tx, request.user_id, request.source, request.track).await?;

    let mut xp_from_quests = 0_i64;
    for quest in &completed {
        xp_from_quests += i64::from(quest.xp_reward);
        pay_quest_reward(tx, request.user_id, quest, season_id).await?;
    }

    if xp_from_quests == 0 {
        return Ok((completed, rank_after, level_after));
    }

    let xp = xp_after + xp_from_quests;
    let rank = next_rank(rank_after, xp, email_verified);
    let level = level_for_xp(xp);

    sqlx::query("UPDATE users SET xp_total = $2, global_rank = $3, level = $4 WHERE id = $1")
        .bind(request.user_id)
        .bind(xp)
        .bind(rank.as_str())
        .bind(level)
        .execute(&mut **tx)
        .await?;

    Ok((completed, rank, level))
}

/// Write the member's new totals, rank, level and streak.
///
/// Returns the streak the member now holds. Extracted from
/// [`grant_in_tx`] to keep the cascade readable.
async fn apply_member_update(
    tx: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    member: &MemberSnapshot,
    xp_after: i64,
    rank_after: GlobalRank,
    level_after: i32,
) -> WebResult<i32> {
    let today = days_from_epoch(Utc::now().date_naive());
    let streak_outcome = evaluate_streak(member.streak_days, member.last_streak_day, today);
    let streak_after = streak_outcome.resolve(member.streak_days);
    let longest_after = member.longest_streak.max(streak_after);

    sqlx::query(
        r#"
        UPDATE users
           SET xp_total         = $2,
               global_rank      = $3,
               level            = $4,
               streak_days      = $5,
               longest_streak   = $6,
               last_streak_day  = $7,
               last_activity_at = NOW()
         WHERE id = $1
        "#,
    )
    .bind(user_id)
    .bind(xp_after)
    .bind(rank_after.as_str())
    .bind(level_after)
    .bind(streak_after)
    .bind(longest_after)
    .bind(epoch_to_date(today))
    .execute(&mut **tx)
    .await?;

    Ok(streak_after)
}

/// Enqueue every announcement an award earned.
///
/// Split out of [`grant_in_tx`] to keep that function readable: the
/// cascade it drives is already long enough without three loops of
/// message formatting at the end.
async fn announce(
    tx: &mut Transaction<'_, Postgres>,
    channels: DiscordChannels,
    user_id: Uuid,
    display: &str,
    new_rank: Option<GlobalRank>,
    new_badges: &[SpecialBadge],
    completed_quests: &[CompletedQuest],
) -> WebResult<()> {
    if let Some(rank) = new_rank {
        notifications::enqueue(
            tx,
            channels,
            &Announcement::rank_up(user_id, display, rank.title(), rank.ring_color()),
        )
        .await?;
    }
    for badge in new_badges {
        notifications::enqueue(
            tx,
            channels,
            &Announcement::badge(user_id, display, badge.title(), badge.description()),
        )
        .await?;
    }
    for quest in completed_quests {
        notifications::enqueue(
            tx,
            channels,
            &Announcement::quest(user_id, display, &quest.title, quest.xp_reward),
        )
        .await?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Member snapshot
// ---------------------------------------------------------------------------

#[derive(Debug, sqlx::FromRow)]
struct MemberRow {
    display_name: String,
    xp_total: i64,
    level: i32,
    global_rank: String,
    email_verified: bool,
    streak_days: i32,
    longest_streak: i32,
    last_streak_day: Option<NaiveDate>,
    active_tracks: i64,
}

impl MemberRow {
    /// Name for announcements. Resolved in SQL by `member_display_name`
    /// so a rank-up posted to Discord reads the same as the leaderboard
    /// the member checks straight afterwards.
    fn display_name(&self) -> String {
        self.display_name.clone()
    }
}

/// Load the member plus the multiplier inputs in one round trip.
async fn load_member(
    tx: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
) -> WebResult<Option<MemberSnapshot>> {
    let row: Option<MemberRow> = sqlx::query_as(
        r#"
        SELECT member_display_name(u.current_title, u.discord_global_name,
                                   u.discord_username, u.discord_id) AS display_name,
               u.xp_total,
               u.level,
               u.global_rank,
               u.email_verified,
               u.streak_days,
               u.longest_streak,
               u.last_streak_day,
               COALESCE((
                   SELECT COUNT(*) FROM track_memberships m
                    WHERE m.user_id = u.id
                      AND m.left_at IS NULL
                      AND m.last_active_at > NOW() - INTERVAL '30 days'
               ), 0) AS active_tracks
          FROM users u
         WHERE u.id = $1
        "#,
    )
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await?;

    Ok(row.map(|r| MemberSnapshot {
        active_tracks: usize::try_from(r.active_tracks).unwrap_or(0),
        last_streak_day: r.last_streak_day.map(days_from_epoch),
        inner: r,
    }))
}

struct MemberSnapshot {
    inner: MemberRow,
    active_tracks: usize,
    last_streak_day: Option<i64>,
}

impl std::ops::Deref for MemberSnapshot {
    type Target = MemberRow;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

// ---------------------------------------------------------------------------
// Rank gating
// ---------------------------------------------------------------------------

/// Decide the rank a member should hold at `xp_total`.
///
/// The three lowest rungs are *not* XP-derived and must never be
/// awarded by this function:
///
/// - `Pending` — no verified email. XP cannot move the rank at all.
/// - `Visitor` — verified, but onboarding (track selection) not done.
/// - `Initiate` — onboarding complete; this is the floor of the XP
///   ladder.
///
/// The pre-audit SQL `CASE` skipped `Initiate` entirely, so a Visitor
/// crossing 150 XP jumped straight to `Apprentice`. Deferring to
/// [`GlobalRank::from_xp`] restores the one-source-of-truth rule the
/// shared crate documents.
fn next_rank(current: GlobalRank, xp_total: i64, email_verified: bool) -> GlobalRank {
    if !email_verified || current == GlobalRank::Pending || current == GlobalRank::Visitor {
        return current;
    }
    GlobalRank::from_xp(xp_total).max(GlobalRank::Initiate)
}

/// Credit XP carried over from before the platform, at most once.
///
/// It goes to the global total only — it lifts the member title, never a
/// track title, and grants no office — and it bypasses multipliers, caps
/// and quests: it is a balance being brought across, not something the
/// member just did. Setting `credited_at` in the same transaction is what
/// makes a second login pay nothing.
///
/// # Errors
/// Propagates database errors.
pub async fn credit_legacy_xp(pool: &PgPool, user_id: Uuid, discord_id: &str) -> WebResult<()> {
    let mut tx = pool.begin().await?;
    let carried: Option<(i32, String)> = sqlx::query_as(
        "UPDATE legacy_xp SET credited_at = NOW() \
          WHERE discord_id = $1 AND credited_at IS NULL \
          RETURNING xp, source",
    )
    .bind(discord_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((xp, source)) = carried else {
        return Ok(());
    };

    let (total, rank, verified): (i64, String, bool) = sqlx::query_as(
        "SELECT xp_total, global_rank, email_verified FROM users WHERE id = $1 FOR UPDATE",
    )
    .bind(user_id)
    .fetch_one(&mut *tx)
    .await?;
    let total = total + i64::from(xp);
    let rank = next_rank(GlobalRank::parse(&rank), total, verified);

    sqlx::query("UPDATE users SET xp_total = $2, level = $3, global_rank = $4 WHERE id = $1")
        .bind(user_id)
        .bind(total)
        .bind(level_for_xp(total))
        .bind(rank.as_str())
        .execute(&mut *tx)
        .await?;
    // Stamped with the open season, or the season board would leave out
    // the XP that put the member on it.
    let season_id = current_season_id(&mut tx).await?;
    sqlx::query(
        "INSERT INTO xp_logs (user_id, amount, source, description, season_id) \
         VALUES ($1, $2, 'Discord', $3, $4)",
    )
    .bind(user_id)
    .bind(xp)
    .bind(format!("Report de l'XP {source}"))
    .bind(season_id)
    .execute(&mut *tx)
    .await?;
    notifications::request_role_sync(&mut *tx).await?;

    tx.commit().await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Ledger
// ---------------------------------------------------------------------------

async fn append_ledger(
    tx: &mut Transaction<'_, Postgres>,
    request: &XpGrant<'_>,
    amount: i32,
    season_id: Option<Uuid>,
) -> WebResult<()> {
    sqlx::query(
        r#"
        INSERT INTO xp_logs (user_id, amount, source, track, description, season_id)
        VALUES ($1, $2, $3, $4, $5, $6)
        "#,
    )
    .bind(request.user_id)
    .bind(amount)
    .bind(request.source.as_str())
    .bind(request.track)
    .bind(request.description)
    .bind(season_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// XP already earned today from a source, optionally narrowed to one
/// kind of ledger line.
async fn xp_today(
    tx: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    source: XpSource,
    description_prefix: Option<&str>,
) -> WebResult<i64> {
    let total: Option<i64> = sqlx::query_scalar(
        r#"
        SELECT COALESCE(SUM(amount), 0)::BIGINT
          FROM xp_logs
         WHERE user_id = $1
           AND source  = $2
           AND created_at >= date_trunc('day', NOW())
           AND ($3::TEXT IS NULL OR description LIKE $3 || '%')
        "#,
    )
    .bind(user_id)
    .bind(source.as_str())
    .bind(description_prefix)
    .fetch_one(&mut **tx)
    .await?;
    Ok(total.unwrap_or(0))
}

/// Sum of XP from a source today. Public because the webhook handler
/// reports remaining budget.
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
         WHERE user_id = $1 AND source = $2
           AND created_at >= date_trunc('day', NOW())
        "#,
    )
    .bind(user_id)
    .bind(source.as_str())
    .fetch_one(pool)
    .await?;
    Ok(total.unwrap_or(0))
}

// ---------------------------------------------------------------------------
// Seasons
// ---------------------------------------------------------------------------

/// The season open right now, if any.
///
/// # Errors
/// Propagates database errors.
pub async fn current_season_id(tx: &mut Transaction<'_, Postgres>) -> WebResult<Option<Uuid>> {
    let id: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM seasons WHERE starts_at <= NOW() AND ends_at > NOW() LIMIT 1",
    )
    .fetch_optional(&mut **tx)
    .await?
    .flatten();
    Ok(id)
}

// ---------------------------------------------------------------------------
// Quests
// ---------------------------------------------------------------------------

/// Advance every open quest this event satisfies, returning those that
/// reached their target.
async fn advance_quests(
    tx: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    source: XpSource,
    track: Option<&str>,
) -> WebResult<Vec<CompletedQuest>> {
    // Which conditions does this XP source advance?
    let conditions: Vec<&str> = [
        QuestCondition::Push,
        QuestCondition::Attend,
        QuestCondition::Review,
        QuestCondition::Submit,
    ]
    .into_iter()
    .filter(|c| c.triggered_by() == Some(source))
    .map(QuestCondition::as_str)
    .collect();

    if conditions.is_empty() {
        return Ok(Vec::new());
    }

    advance_quests_for_conditions(tx, user_id, &conditions, track).await
}

/// Advance quests matching explicit condition names.
///
/// Exposed so the project handler can drive the `Submit` condition,
/// which has no XP source of its own.
///
/// # Errors
/// Propagates database errors.
pub async fn advance_quests_for_conditions(
    tx: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    conditions: &[&str],
    track: Option<&str>,
) -> WebResult<Vec<CompletedQuest>> {
    // Candidate quests: open right now, matching one of the conditions,
    // either untracked or matching this award's track, and not already
    // completed by this member.
    let rows: Vec<(Uuid, String, i32, i32)> = sqlx::query_as(
        r#"
        SELECT q.id, q.title, q.xp_reward, q.condition_value
          FROM quests q
         WHERE q.starts_at <= NOW()
           AND q.ends_at   >  NOW()
           AND q.condition_type = ANY($2)
           AND (q.track IS NULL OR q.track = $3)
           AND NOT EXISTS (
               SELECT 1 FROM quest_completions c
                WHERE c.quest_id = q.id AND c.user_id = $1
           )
        "#,
    )
    .bind(user_id)
    .bind(conditions)
    .bind(track)
    .fetch_all(&mut **tx)
    .await?;

    let mut completed = Vec::new();

    for (quest_id, title, xp_reward, target) in rows {
        let counter: i32 = sqlx::query_scalar(
            r#"
            INSERT INTO quest_progress (quest_id, user_id, counter, updated_at)
            VALUES ($1, $2, 1, NOW())
            ON CONFLICT (quest_id, user_id) DO UPDATE
                SET counter = quest_progress.counter + 1,
                    updated_at = NOW()
            RETURNING counter
            "#,
        )
        .bind(quest_id)
        .bind(user_id)
        .fetch_one(&mut **tx)
        .await?;

        if counter >= target {
            // ON CONFLICT DO NOTHING makes completion idempotent even if
            // two events race through this block.
            let inserted = sqlx::query(
                r#"
                INSERT INTO quest_completions (quest_id, user_id)
                VALUES ($1, $2)
                ON CONFLICT (quest_id, user_id) DO NOTHING
                "#,
            )
            .bind(quest_id)
            .bind(user_id)
            .execute(&mut **tx)
            .await?;

            if inserted.rows_affected() > 0 {
                completed.push(CompletedQuest {
                    id: quest_id,
                    title,
                    xp_reward,
                });
            }
        }
    }

    Ok(completed)
}

/// Credit a finished quest's reward.
///
/// Deliberately a plain ledger append rather than a recursive [`grant`]:
/// quest XP must not be multiplied, must not be capped, and must not
/// itself advance quests. The caller folds the amount into the totals.
async fn pay_quest_reward(
    tx: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    quest: &CompletedQuest,
    season_id: Option<Uuid>,
) -> WebResult<()> {
    sqlx::query(
        r#"
        INSERT INTO xp_logs (user_id, amount, source, description, season_id)
        VALUES ($1, $2, 'Quest', $3, $4)
        "#,
    )
    .bind(user_id)
    .bind(quest.xp_reward)
    .bind(format!("Quête : {}", quest.title))
    .bind(season_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Badges
// ---------------------------------------------------------------------------

/// Rebuild the member's badge snapshot and award anything newly earned.
///
/// # Errors
/// Propagates database errors.
pub async fn refresh_badges(
    tx: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    streak_days: i32,
) -> WebResult<Vec<SpecialBadge>> {
    let snapshot = build_badge_snapshot(tx, user_id, streak_days).await?;
    let earned = badges::evaluate(&snapshot);
    if earned.is_empty() {
        return Ok(Vec::new());
    }

    let mut awarded = Vec::new();
    for badge in earned {
        let result = sqlx::query(
            r#"
            INSERT INTO special_badges (user_id, badge_type)
            VALUES ($1, $2)
            ON CONFLICT (user_id, badge_type) DO NOTHING
            "#,
        )
        .bind(user_id)
        .bind(badge.as_str())
        .execute(&mut **tx)
        .await?;

        // Only a row that did not already exist counts as "new", which
        // is what keeps the announcement from firing on every event.
        if result.rows_affected() > 0 {
            awarded.push(badge);
        }
    }
    Ok(awarded)
}

#[derive(sqlx::FromRow)]
struct BadgeCounters {
    active_tracks: i64,
    gamejam_attendances: i64,
    gamejam_wins: i64,
    validations_given: i64,
    mentorings: i64,
    has_nocturnal_push: bool,
    is_month_top3: bool,
}

async fn build_badge_snapshot(
    tx: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    streak_days: i32,
) -> WebResult<BadgeSnapshot> {
    let night_from = i32::try_from(badges::NIGHT_OWL_FROM_HOUR).unwrap_or(0);
    let night_until = i32::try_from(badges::NIGHT_OWL_UNTIL_HOUR).unwrap_or(5);

    let counters: BadgeCounters = sqlx::query_as(
        r#"
        SELECT
            COALESCE((SELECT COUNT(*) FROM track_memberships m
                       WHERE m.user_id = $1
                         AND m.left_at IS NULL
                         AND m.last_active_at > NOW() - INTERVAL '30 days'), 0)
                AS active_tracks,
            COALESCE((SELECT COUNT(*) FROM attendance a
                       WHERE a.user_id = $1 AND a.event_type = 'GameJam'), 0)
                AS gamejam_attendances,
            COALESCE((SELECT COUNT(*) FROM xp_logs x
                       WHERE x.user_id = $1 AND x.source = 'QR'
                         AND x.description ILIKE '%victoire%'), 0)
                AS gamejam_wins,
            COALESCE((SELECT COUNT(*) FROM track_validations v
                       WHERE v.reviewed_by = $1 AND v.status <> 'Pending'), 0)
                AS validations_given,
            COALESCE((SELECT COUNT(*) FROM xp_logs x
                       WHERE x.user_id = $1 AND x.description ILIKE 'Mentorat%'), 0)
                AS mentorings,
            EXISTS(SELECT 1 FROM xp_logs x
                    WHERE x.user_id = $1 AND x.source = 'GitHub'
                      AND EXTRACT(HOUR FROM x.created_at AT TIME ZONE 'UTC')
                          BETWEEN $2 AND $3)
                AS has_nocturnal_push,
            EXISTS(
                SELECT 1 FROM (
                    SELECT x.user_id, SUM(x.amount) AS total
                      FROM xp_logs x
                     WHERE x.created_at >= date_trunc('month', NOW())
                     GROUP BY x.user_id
                     ORDER BY total DESC
                     LIMIT 3
                ) top WHERE top.user_id = $1
            ) AS is_month_top3
        "#,
    )
    .bind(user_id)
    .bind(night_from)
    // BETWEEN is inclusive on both ends; the constant is exclusive.
    .bind(night_until - 1)
    .fetch_one(&mut **tx)
    .await?;

    Ok(BadgeSnapshot {
        streak_days,
        active_tracks: usize::try_from(counters.active_tracks).unwrap_or(0),
        gamejam_attendances: counters.gamejam_attendances,
        gamejam_wins: counters.gamejam_wins,
        validations_given: counters.validations_given,
        mentorings: counters.mentorings,
        has_nocturnal_push: counters.has_nocturnal_push,
        is_month_top3: counters.is_month_top3,
        // Roadmap-driven badges need block completion data the roadmap
        // handler owns; they stay false until that lands.
        blocks_completed: 0,
        has_speedrun_block: false,
        has_platform_pr: false,
    })
}

// ---------------------------------------------------------------------------
// Date helpers
// ---------------------------------------------------------------------------

/// Days elapsed since the Unix epoch, so streak arithmetic stays in
/// plain integers (and the shared crate stays chrono-free).
fn days_from_epoch(date: NaiveDate) -> i64 {
    let epoch = NaiveDate::from_ymd_opt(1970, 1, 1).expect("1970-01-01 is a valid date");
    (date - epoch).num_days()
}

fn epoch_to_date(days: i64) -> NaiveDate {
    let epoch = NaiveDate::from_ymd_opt(1970, 1, 1).expect("1970-01-01 is a valid date");
    epoch + chrono::Duration::days(days)
}

/// Whether an instant falls in the "night owl" window. Exposed for
/// tests; the production check is done in SQL.
#[must_use]
pub fn is_nocturnal<T: Timelike>(at: &T) -> bool {
    (badges::NIGHT_OWL_FROM_HOUR..badges::NIGHT_OWL_UNTIL_HOUR).contains(&at.hour())
}

/// Whether a date lands in the current month. Used by the monthly
/// leaderboard reset.
#[must_use]
pub fn is_same_month(a: NaiveDate, b: NaiveDate) -> bool {
    a.year() == b.year() && a.month() == b.month()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epoch_round_trip() {
        for iso in ["1970-01-01", "2026-05-15", "1999-12-31"] {
            let date: NaiveDate = iso.parse().unwrap();
            assert_eq!(epoch_to_date(days_from_epoch(date)), date);
        }
    }

    #[test]
    fn consecutive_dates_are_one_day_apart() {
        let a: NaiveDate = "2026-02-28".parse().unwrap();
        let b: NaiveDate = "2026-03-01".parse().unwrap();
        // 2026 is not a leap year, so these really are consecutive.
        assert_eq!(days_from_epoch(b) - days_from_epoch(a), 1);
    }

    #[test]
    fn unverified_members_never_change_rank() {
        assert_eq!(
            next_rank(GlobalRank::Pending, 999_999, false),
            GlobalRank::Pending
        );
        assert_eq!(
            next_rank(GlobalRank::Initiate, 999_999, false),
            GlobalRank::Initiate
        );
    }

    #[test]
    fn visitors_are_gated_on_onboarding_not_xp() {
        // A verified member who has not picked a track stays a Visitor
        // however much XP they accumulate.
        assert_eq!(
            next_rank(GlobalRank::Visitor, 10_000, true),
            GlobalRank::Visitor
        );
    }

    #[test]
    fn initiate_is_not_skipped() {
        // The audit's finding: the old SQL CASE jumped Visitor -> Apprentice
        // at 150 XP and never assigned Initiate at all.
        assert_eq!(next_rank(GlobalRank::Initiate, 0, true), GlobalRank::Initiate);
        assert_eq!(
            next_rank(GlobalRank::Initiate, 149, true),
            GlobalRank::Initiate
        );
        assert_eq!(
            next_rank(GlobalRank::Initiate, 150, true),
            GlobalRank::Apprentice
        );
    }

    #[test]
    fn rank_never_regresses_below_initiate_for_an_onboarded_member() {
        assert_eq!(next_rank(GlobalRank::Legend, 0, true), GlobalRank::Initiate);
    }

    #[test]
    fn nocturnal_window_is_half_open() {
        use chrono::NaiveTime;
        let at = |h, m| NaiveTime::from_hms_opt(h, m, 0).unwrap();
        assert!(is_nocturnal(&at(0, 0)));
        assert!(is_nocturnal(&at(4, 59)));
        assert!(!is_nocturnal(&at(5, 0)));
        assert!(!is_nocturnal(&at(23, 0)));
    }

    #[test]
    fn same_month_compares_year_too() {
        let a: NaiveDate = "2026-05-01".parse().unwrap();
        let b: NaiveDate = "2026-05-31".parse().unwrap();
        let c: NaiveDate = "2025-05-15".parse().unwrap();
        assert!(is_same_month(a, b));
        assert!(!is_same_month(a, c));
    }

    #[test]
    fn grant_builder_composes() {
        let id = Uuid::nil();
        let g = XpGrant::new(id, 10, XpSource::Github)
            .describe("Push x2")
            .in_track("Engineering")
            .capped(50, Some("Push"))
            .exact();
        assert_eq!(g.base, 10);
        assert_eq!(g.track, Some("Engineering"));
        assert_eq!(g.description, Some("Push x2"));
        assert!(!g.apply_multipliers);
        assert_eq!(g.cap.unwrap().limit, 50);
        assert_eq!(g.cap.unwrap().description_prefix, Some("Push"));
    }
}

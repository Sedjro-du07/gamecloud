//! Involvement — the queries behind `gamecloud_shared::involvement`.
//!
//! A member's level is computed, never stored. For each month of the
//! school year (September to August) the platform counts what every member
//! did, in a handful of aggregate queries, and the shared module turns the
//! counts into a score, a level and credits. The month in progress is
//! measured over its last 30 days, so it is never judged on three days.
//!
//! What is counted, and what is not:
//! * GitHub XP, projects started and released, shares, accepted resources;
//! * sessions attended (scanned on site) out of sessions held;
//! * active days: any XP-earning action, presence, review or share —
//!   except XP granted by hand and the XP carried over from the old Kumo
//!   bot, which say nothing about this month;
//! * reviews of someone else's project, quests completed.

use std::collections::HashMap;

use chrono::{DateTime, Datelike, Duration, NaiveDate, TimeZone, Utc};
use gamecloud_shared::involvement::{
    credits_for, floor_for, walk, year_level, Activity, Breakdown, Month, Responsibilities,
};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    db::queries::audit,
    error::{WebError, WebResult},
};

/// Promotions a member can be in.
pub const PROMOTIONS: [&str; 7] = ["Tek1", "Tek2", "Tek3", "Tek4", "Tek5", "MSc", "Autre"];

/// The school year a moment belongs to, by its first year: 2026 for
/// September 2026 to August 2027.
#[must_use]
pub fn school_year_of(at: DateTime<Utc>) -> i32 {
    if at.month() >= 9 {
        at.year()
    } else {
        at.year() - 1
    }
}

/// "2026-2027".
#[must_use]
pub fn school_year_label(year: i32) -> String {
    format!("{year}-{}", year + 1)
}

fn month_start(year: i32, month: u32) -> DateTime<Utc> {
    let day = NaiveDate::from_ymd_opt(year, month, 1).unwrap_or_default();
    Utc.from_utc_datetime(&day.and_hms_opt(0, 0, 0).unwrap_or_default())
}

fn next_month(at: DateTime<Utc>) -> DateTime<Utc> {
    if at.month() == 12 {
        month_start(at.year() + 1, 1)
    } else {
        month_start(at.year(), at.month() + 1)
    }
}

const MONTH_NAMES: [&str; 12] = [
    "janvier", "février", "mars", "avril", "mai", "juin", "juillet", "août", "septembre",
    "octobre", "novembre", "décembre",
];

/// One month measured: its name and the window it covers.
#[derive(Debug, Clone)]
pub struct Window {
    /// "novembre 2026".
    pub label: String,
    /// Start, inclusive.
    pub start: DateTime<Utc>,
    /// End, exclusive.
    pub end: DateTime<Utc>,
    /// The month in progress, measured over its last 30 days.
    pub current: bool,
}

/// The months of a school year up to now, oldest first.
#[must_use]
pub fn windows(year: i32, now: DateTime<Utc>) -> Vec<Window> {
    let mut out = Vec::new();
    let mut at = month_start(year, 9);
    let end_of_year = month_start(year + 1, 9);
    while at <= now && at < end_of_year {
        let next = next_month(at);
        let current = now < next;
        let label = format!("{} {}", MONTH_NAMES[at.month0() as usize], at.year());
        let (start, end) = if current { (now - Duration::days(30), now) } else { (at, next) };
        out.push(Window { label, start, end, current });
        at = next;
    }
    out
}

/// A member, as the involvement sheet needs them.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Member {
    /// Account.
    pub id: Uuid,
    /// Discord username.
    pub discord_username: Option<String>,
    /// First name, as they typed it.
    pub first_name: Option<String>,
    /// Last name.
    pub last_name: Option<String>,
    /// Tek1…Tek5, MSc, Autre.
    pub promotion: Option<String>,
    /// When the account was created: months before it are not counted.
    pub created_at: DateTime<Utc>,
    /// Bureau offices held.
    pub offices: Vec<String>,
    /// Leads a track.
    pub track_lead: bool,
    /// Co-leads a track.
    pub track_co_lead: bool,
    /// Named by the President or the Vice-President for guaranteed
    /// maximal involvement.
    pub involvement_max: bool,
}

impl Member {
    /// Always shown at maximal involvement: the President and the
    /// Vice-President by their office, and the members they named.
    #[must_use]
    pub fn maximal(&self) -> bool {
        self.involvement_max
            || self.offices.iter().any(|o| matches!(o.as_str(), "President" | "VicePresident"))
    }

    /// What their responsibilities guarantee.
    #[must_use]
    pub fn responsibilities(&self) -> Responsibilities {
        Responsibilities {
            bureau: !self.offices.is_empty(),
            track_lead: self.track_lead,
            track_co_lead: self.track_co_lead,
        }
    }
}

const SELECT_MEMBER: &str = r#"
    SELECT u.id, u.discord_username, u.first_name, u.last_name, u.promotion, u.created_at,
           u.offices, u.involvement_max,
           EXISTS (SELECT 1 FROM track_memberships m
                    WHERE m.user_id = u.id AND m.left_at IS NULL AND m.track_role = 'Lead') AS track_lead,
           EXISTS (SELECT 1 FROM track_memberships m
                    WHERE m.user_id = u.id AND m.left_at IS NULL AND m.track_role = 'CoLead') AS track_co_lead
      FROM users u
"#;

/// Every member (on the server, or admitted), by Discord username.
///
/// # Errors
/// Propagates database errors.
pub async fn members(pool: &PgPool) -> WebResult<Vec<Member>> {
    Ok(sqlx::query_as::<_, Member>(&format!(
        "{SELECT_MEMBER} WHERE u.candidate = FALSE ORDER BY lower(u.discord_username) NULLS LAST"
    ))
    .fetch_all(pool)
    .await?)
}

/// One member.
///
/// # Errors
/// `NotFound` when the account does not exist.
pub async fn member(pool: &PgPool, id: Uuid) -> WebResult<Member> {
    sqlx::query_as::<_, Member>(&format!("{SELECT_MEMBER} WHERE u.id = $1"))
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or(WebError::NotFound)
}

/// One count per member over a window.
async fn counts(pool: &PgPool, sql: &str, w: &Window) -> WebResult<HashMap<Uuid, u32>> {
    let rows: Vec<(Uuid, i64)> = sqlx::query_as(sql)
        .bind(w.start)
        .bind(w.end)
        .fetch_all(pool)
        .await?;
    Ok(rows
        .into_iter()
        .map(|(id, n)| (id, u32::try_from(n.max(0)).unwrap_or(u32::MAX)))
        .collect())
}

/// One aggregate per kind of action, each returning `(member, count)` for
/// the window `[$1, $2)`. The order is the order of [`Counts`].
const COUNT_QUERIES: [&str; 9] = [
    // GitHub XP.
    "SELECT user_id, COALESCE(SUM(amount), 0)::BIGINT FROM xp_logs \
      WHERE source = 'GitHub' AND amount > 0 AND created_at >= $1 AND created_at < $2 GROUP BY user_id",
    // Projects released.
    "SELECT c.user_id, COUNT(DISTINCT p.id) FROM project_contributors c JOIN projects p ON p.id = c.project_id \
      WHERE p.released_at >= $1 AND p.released_at < $2 GROUP BY c.user_id",
    // Projects started.
    "SELECT c.user_id, COUNT(DISTINCT p.id) FROM project_contributors c JOIN projects p ON p.id = c.project_id \
      WHERE p.created_at >= $1 AND p.created_at < $2 GROUP BY c.user_id",
    // Shares.
    "SELECT uploaded_by, COUNT(*) FROM shares WHERE created_at >= $1 AND created_at < $2 GROUP BY uploaded_by",
    // Resources accepted.
    "SELECT submitted_by, COUNT(*) FROM resources \
      WHERE validated_by IS NOT NULL AND created_at >= $1 AND created_at < $2 GROUP BY submitted_by",
    // Sessions attended.
    "SELECT user_id, COUNT(DISTINCT (event_name, scanned_at::date)) FROM attendance \
      WHERE scanned_at >= $1 AND scanned_at < $2 GROUP BY user_id",
    // Active days: hand-granted XP and the XP carried over from Kumo say
    // nothing about this month.
    r#"
    SELECT user_id, COUNT(DISTINCT day) FROM (
        SELECT user_id, created_at::date AS day FROM xp_logs
         WHERE created_at >= $1 AND created_at < $2
           AND source <> 'Manual'
           AND COALESCE(description, '') NOT LIKE 'Report de l''XP %'
        UNION ALL
        SELECT user_id, scanned_at::date FROM attendance WHERE scanned_at >= $1 AND scanned_at < $2
        UNION ALL
        SELECT reviewed_by, reviewed_at::date FROM track_validations
         WHERE reviewed_by IS NOT NULL AND reviewed_at >= $1 AND reviewed_at < $2
        UNION ALL
        SELECT uploaded_by, created_at::date FROM shares WHERE created_at >= $1 AND created_at < $2
    ) actions
    GROUP BY user_id
    "#,
    // Reviews of someone else's project.
    "SELECT v.reviewed_by, COUNT(*) FROM track_validations v JOIN projects p ON p.id = v.project_id \
      WHERE v.reviewed_by IS NOT NULL AND v.reviewed_by <> p.created_by \
        AND v.status IN ('Approved', 'Rejected') AND v.reviewed_at >= $1 AND v.reviewed_at < $2 \
      GROUP BY v.reviewed_by",
    // Quests completed.
    "SELECT user_id, COUNT(*) FROM quest_completions WHERE completed_at >= $1 AND completed_at < $2 GROUP BY user_id",
];

/// Sessions held: those on the calendar, or, where nobody put them on the
/// calendar, those somebody was scanned at.
const SESSIONS_HELD: &str = r#"
    SELECT GREATEST(
      (SELECT COUNT(*) FROM events
        WHERE cancelled_at IS NULL AND kind IN ('Session', 'Workshop', 'Jam', 'Showcase')
          AND starts_at >= $1 AND starts_at < $2),
      (SELECT COUNT(DISTINCT (event_name, scanned_at::date)) FROM attendance
        WHERE scanned_at >= $1 AND scanned_at < $2))
"#;

/// What every member did over a window, and how many sessions were held.
///
/// # Errors
/// Propagates database errors.
pub async fn activity(pool: &PgPool, w: &Window) -> WebResult<(HashMap<Uuid, Activity>, u32)> {
    let mut maps = Vec::with_capacity(COUNT_QUERIES.len());
    for sql in COUNT_QUERIES {
        maps.push(counts(pool, sql, w).await?);
    }
    let held: i64 = sqlx::query_scalar(SESSIONS_HELD)
        .bind(w.start)
        .bind(w.end)
        .fetch_one(pool)
        .await?;
    let held = u32::try_from(held.max(0)).unwrap_or(u32::MAX);

    let everyone: std::collections::HashSet<Uuid> = maps.iter().flat_map(|m| m.keys().copied()).collect();
    let by_member = everyone
        .into_iter()
        .map(|id| {
            let n = |i: usize| maps[i].get(&id).copied().unwrap_or(0);
            let a = Activity {
                github_xp: n(0),
                projects_released: n(1),
                projects_started: n(2),
                shares: n(3),
                resources_accepted: n(4),
                sessions_attended: n(5),
                sessions_held: held,
                active_days: n(6),
                reviews: n(7),
                quests: n(8),
            };
            (id, a)
        })
        .collect();
    Ok((by_member, held))
}

/// What the President or the Vice-President decided for a member's year.
#[derive(Debug, Clone, Default, sqlx::FromRow)]
pub struct Review {
    /// Level forced, if any.
    pub forced_level: Option<i16>,
    /// Why.
    pub force_reason: Option<String>,
    /// Remarks printed on the sheet.
    pub remarks: Option<String>,
}

/// Every review of a school year, by member.
///
/// # Errors
/// Propagates database errors.
pub async fn reviews(pool: &PgPool, school_year: i32) -> WebResult<HashMap<Uuid, Review>> {
    #[derive(sqlx::FromRow)]
    struct Row {
        user_id: Uuid,
        #[sqlx(flatten)]
        review: Review,
    }
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT user_id, forced_level, force_reason, remarks FROM involvement_reviews WHERE school_year = $1",
    )
    .bind(school_year)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|r| (r.user_id, r.review)).collect())
}

/// One month of a member's record, named.
#[derive(Debug, Clone)]
pub struct MonthRecord {
    /// "novembre 2026".
    pub label: String,
    /// Measured over the last 30 days.
    pub current: bool,
    /// The month's score, earned level and kept level.
    pub month: Month,
}

/// A member's involvement for a school year.
#[derive(Debug, Clone)]
pub struct Record {
    /// The member.
    pub member: Member,
    /// 2026 for 2026-2027.
    pub school_year: i32,
    /// Their months, oldest first.
    pub months: Vec<MonthRecord>,
    /// The lowest level their responsibilities allow.
    pub floor: u8,
    /// The average of the months, before any decision.
    pub computed_level: Option<u8>,
    /// The President's or Vice-President's decision, if any.
    pub review: Review,
    /// Guaranteed maximal involvement, whatever the analysis says.
    pub maximal: bool,
    /// The level that counts: maximal, forced, or computed.
    pub level: Option<u8>,
    /// Credits that level earns.
    pub credits: u8,
}

impl Record {
    /// The score of the month in progress, if the member is counted yet.
    #[must_use]
    pub fn current(&self) -> Option<Breakdown> {
        self.months.last().map(|m| m.month.breakdown)
    }
}

/// Everybody's involvement for the school year of `now`.
///
/// # Errors
/// Propagates database errors.
pub async fn records(pool: &PgPool, members: Vec<Member>, now: DateTime<Utc>) -> WebResult<Vec<Record>> {
    let school_year = school_year_of(now);
    let months = windows(school_year, now);
    let mut per_month = Vec::with_capacity(months.len());
    for w in &months {
        per_month.push(activity(pool, w).await?);
    }
    let decided = reviews(pool, school_year).await?;

    Ok(members
        .into_iter()
        .map(|member| {
            // Months before the member arrived are not counted against them.
            let joined = month_start(member.created_at.year(), member.created_at.month());
            let theirs: Vec<(usize, &Window)> = months
                .iter()
                .enumerate()
                .filter(|(_, w)| w.current || w.start >= joined)
                .collect();
            let activities: Vec<Activity> = theirs
                .iter()
                .map(|(i, _)| {
                    let (by_member, held) = &per_month[*i];
                    by_member
                        .get(&member.id)
                        .copied()
                        .unwrap_or(Activity { sessions_held: *held, ..Activity::default() })
                })
                .collect();
            let floor = floor_for(member.responsibilities());
            let walked = walk(&activities, floor);
            let computed_level = year_level(&walked);
            let review = decided.get(&member.id).cloned().unwrap_or_default();
            let maximal = member.maximal();
            let level = if maximal {
                Some(5)
            } else {
                review.forced_level.and_then(|l| u8::try_from(l).ok()).or(computed_level)
            };
            let months = theirs
                .iter()
                .zip(walked)
                .map(|((_, w), month)| MonthRecord { label: w.label.clone(), current: w.current, month })
                .collect();
            Record {
                member,
                school_year,
                months,
                floor,
                computed_level,
                review,
                maximal,
                level,
                credits: level.map_or(0, credits_for),
            }
        })
        .collect())
}

/// Save what a member typed about themselves.
///
/// # Errors
/// `Validation` for a missing name or an unknown promotion.
pub async fn save_identity(
    pool: &PgPool,
    user_id: Uuid,
    first_name: &str,
    last_name: &str,
    promotion: &str,
) -> WebResult<()> {
    let (first, last) = (first_name.trim(), last_name.trim());
    if first.is_empty() || last.is_empty() {
        return Err(WebError::Validation("indique ton prénom et ton nom".into()));
    }
    if first.chars().count() > 80 || last.chars().count() > 80 {
        return Err(WebError::Validation("prénom ou nom trop long".into()));
    }
    if !PROMOTIONS.contains(&promotion) {
        return Err(WebError::Validation("choisis ta promotion".into()));
    }
    sqlx::query("UPDATE users SET first_name = $2, last_name = $3, promotion = $4 WHERE id = $1")
        .bind(user_id)
        .bind(first)
        .bind(last)
        .bind(promotion)
        .execute(pool)
        .await?;
    Ok(())
}

/// Grant or lift a member's guaranteed maximal involvement.
///
/// # Errors
/// `NotFound` when the account does not exist.
pub async fn set_maximal(pool: &PgPool, user_id: Uuid, maximal: bool, by: Uuid) -> WebResult<()> {
    let mut tx = pool.begin().await?;
    let changed = sqlx::query(
        "UPDATE users SET involvement_max = $2 WHERE id = $1 AND involvement_max IS DISTINCT FROM $2",
    )
    .bind(user_id)
    .bind(maximal)
    .execute(&mut *tx)
    .await?;
    if changed.rows_affected() > 0 {
        audit::record_in_tx(
            &mut tx,
            Some(by),
            if maximal { "involvement.max_granted" } else { "involvement.max_lifted" },
            Some("user"),
            Some(user_id),
            serde_json::json!({}),
        )
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

/// Record the President's or Vice-President's decision for a member's
/// year: a forced level (with a reason) or none, and remarks.
///
/// # Errors
/// `Validation` for a level out of range, a forced level without a reason
/// or overlong remarks.
pub async fn save_review(
    pool: &PgPool,
    user_id: Uuid,
    school_year: i32,
    forced_level: Option<u8>,
    reason: Option<&str>,
    remarks: Option<&str>,
    by: Uuid,
) -> WebResult<()> {
    let reason = reason.map(str::trim).filter(|r| !r.is_empty());
    let remarks = remarks.map(str::trim).filter(|r| !r.is_empty());
    if forced_level.is_some_and(|l| !(1..=5).contains(&l)) {
        return Err(WebError::Validation("le niveau va de 1 à 5".into()));
    }
    if forced_level.is_some() && reason.is_none() {
        return Err(WebError::Validation("forcer un niveau demande un motif".into()));
    }
    if remarks.is_some_and(|r| r.chars().count() > 2000) {
        return Err(WebError::Validation("remarques trop longues (2000 caractères maximum)".into()));
    }

    let mut tx = pool.begin().await?;
    sqlx::query(
        r#"
        INSERT INTO involvement_reviews (user_id, school_year, forced_level, force_reason, remarks, updated_by, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, NOW())
        ON CONFLICT (user_id, school_year) DO UPDATE
           SET forced_level = EXCLUDED.forced_level, force_reason = EXCLUDED.force_reason,
               remarks = EXCLUDED.remarks, updated_by = EXCLUDED.updated_by, updated_at = NOW()
        "#,
    )
    .bind(user_id)
    .bind(school_year)
    .bind(forced_level.map(i16::from))
    .bind(forced_level.and(reason))
    .bind(remarks)
    .bind(by)
    .execute(&mut *tx)
    .await?;
    audit::record_in_tx(
        &mut tx,
        Some(by),
        "involvement.reviewed",
        Some("user"),
        Some(user_id),
        serde_json::json!({
            "school_year": school_year_label(school_year),
            "forced_level": forced_level,
            "reason": forced_level.and(reason),
        }),
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(y: i32, m: u32, d: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, m, d, 12, 0, 0).unwrap()
    }

    #[test]
    fn the_school_year_starts_in_september() {
        assert_eq!(school_year_of(at(2026, 9, 1)), 2026);
        assert_eq!(school_year_of(at(2027, 8, 31)), 2026);
        assert_eq!(school_year_of(at(2026, 8, 31)), 2025);
        assert_eq!(school_year_label(2026), "2026-2027");
    }

    #[test]
    fn months_run_from_september_to_now_and_the_last_is_thirty_days() {
        let w = windows(2026, at(2026, 11, 26));
        let labels: Vec<&str> = w.iter().map(|w| w.label.as_str()).collect();
        assert_eq!(labels, ["septembre 2026", "octobre 2026", "novembre 2026"]);
        assert!(!w[0].current && w[2].current);
        assert_eq!(w[2].end - w[2].start, Duration::days(30));
        assert_eq!(w[0].end, month_start(2026, 10));
    }
}

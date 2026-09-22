//! XP economy: sources, multipliers, daily caps, and rank thresholds.
//!
//! Every numerical constant defined in the project brief lives here. If
//! a balancing decision changes (e.g. raising the commit XP from 10 to
//! 12), this is the only file that needs editing.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// XP source
// ---------------------------------------------------------------------------

/// Origin of an XP grant. Stored as the `source` column of `xp_logs`.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::Type))]
#[cfg_attr(feature = "server", sqlx(type_name = "TEXT"))]
#[serde(rename_all = "PascalCase")]
pub enum XpSource {
    /// Webhook from GitHub (commit / PR / review / issue).
    Github,
    /// QR code scan at an event.
    Qr,
    /// Discord activity (DraftBot sync, validated reaction…).
    Discord,
    /// Peer review submitted on a project.
    Review,
    /// Quest completion.
    Quest,
    /// Manual grant by a Bureau member (audited).
    Manual,
    /// XP awarded automatically when a project is Released.
    Project,
}

impl XpSource {
    /// Stable string label. Always matches the `serde` rename.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Github => "GitHub",
            Self::Qr => "QR",
            Self::Discord => "Discord",
            Self::Review => "Review",
            Self::Quest => "Quest",
            Self::Manual => "Manual",
            Self::Project => "Project",
        }
    }
}

// ---------------------------------------------------------------------------
// XP values — GitHub
// ---------------------------------------------------------------------------

/// XP rewarded per commit pushed.
pub const XP_GITHUB_COMMIT: i32 = 10;
/// Daily cap on commit XP (prevents farming).
pub const XP_GITHUB_COMMIT_DAILY_CAP: i32 = 50;
/// XP rewarded for a merged pull request.
pub const XP_GITHUB_PR_MERGED: i32 = 30;
/// XP rewarded for a code review submitted via PR review.
pub const XP_GITHUB_REVIEW: i32 = 15;
/// XP rewarded when an issue is closed/resolved.
pub const XP_GITHUB_ISSUE_RESOLVED: i32 = 20;
/// Bonus for the first commit of the day (streak nudge).
pub const XP_GITHUB_FIRST_COMMIT_BONUS: i32 = 5;

// ---------------------------------------------------------------------------
// XP values — Attendance (QR)
// ---------------------------------------------------------------------------

/// Monday session.
pub const XP_QR_SESSION: i32 = 25;
/// Wednesday office hours.
pub const XP_QR_OFFICE_HOURS: i32 = 15;
/// Friday stand-up.
pub const XP_QR_STANDUP: i32 = 10;
/// Game Jam participation.
pub const XP_QR_GAMEJAM: i32 = 100;
/// Game Jam victory bonus.
pub const XP_QR_GAMEJAM_WIN: i32 = 200;

// ---------------------------------------------------------------------------
// XP values — Projects & validation
// ---------------------------------------------------------------------------

/// Validated Tek1 micro-project.
pub const XP_PROJECT_TEK1: i32 = 50;
/// Validated Tek2 micro-project.
pub const XP_PROJECT_TEK2: i32 = 100;
/// Validated Tek3 micro-project.
pub const XP_PROJECT_TEK3: i32 = 150;
/// Validated Master project.
pub const XP_PROJECT_MASTER: i32 = 250;
/// Released final project.
pub const XP_PROJECT_RELEASED: i32 = 300;
/// Peer review given.
pub const XP_PEER_REVIEW: i32 = 20;
/// Mentoring a teammate's blocker.
pub const XP_MENTORING: i32 = 30;
/// Bonus given to the TrackLead when a project they oversee is Released.
pub const XP_TRACKLEAD_RELEASED: i32 = 50;

// ---------------------------------------------------------------------------
// XP values — Discord
// ---------------------------------------------------------------------------

/// XP awarded when an `Archiviste` validates a help message.
pub const XP_DISCORD_HELP_VALIDATED: i32 = 5;
/// XP awarded when a shared resource is validated.
pub const XP_DISCORD_RESOURCE_VALIDATED: i32 = 10;

// ---------------------------------------------------------------------------
// Multipliers
// ---------------------------------------------------------------------------

/// Streak multiplier applied to XP gains based on consecutive active days.
#[must_use]
pub fn streak_multiplier(streak_days: i32) -> f64 {
    if streak_days >= 30 {
        2.0
    } else if streak_days >= 14 {
        1.5
    } else if streak_days >= 7 {
        1.25
    } else {
        1.0
    }
}

/// Multi-track bonus applied on top of streak. An "active" track is one
/// where the user submitted a project in the last 30 days.
#[must_use]
pub fn multi_track_bonus(active_tracks: usize) -> f64 {
    match active_tracks {
        0 | 1 => 1.0,
        2 => 1.10,
        3 => 1.20,
        _ => 1.30,
    }
}

/// Distribution of track XP: 70% is also credited to the global XP pool.
pub const TRACK_TO_GLOBAL_XP_RATIO: f64 = 0.70;

/// Compute final XP after streak and multi-track multipliers.
///
/// Multipliers compound: `final = base * streak * multi_track`.
#[must_use]
pub fn compute_final_xp(base: i32, streak_days: i32, active_tracks: usize) -> i32 {
    let raw = f64::from(base) * streak_multiplier(streak_days) * multi_track_bonus(active_tracks);
    // Saturating cast: XP is i32, so values >= i32::MAX clamp.
    raw.round() as i32
}

// ---------------------------------------------------------------------------
// Daily caps
// ---------------------------------------------------------------------------

/// Trim an award so that `already_today + result <= cap`.
///
/// **Order matters.** The cap must be applied to the *multiplied* value,
/// not to the base: capping first and multiplying afterwards lets a
/// member with a 30-day streak and three active tracks turn a 50 XP/day
/// cap into 130 XP/day. Callers therefore run
/// [`compute_final_xp`] first and this second.
///
/// Returns `0` when the cap is already met or exceeded.
#[must_use]
pub fn apply_daily_cap(final_xp: i32, already_today: i64, cap: i32) -> i32 {
    let remaining = i64::from(cap).saturating_sub(already_today);
    if remaining <= 0 {
        return 0;
    }
    i32::try_from(i64::from(final_xp).min(remaining)).unwrap_or(i32::MAX)
}

// ---------------------------------------------------------------------------
// Streaks
// ---------------------------------------------------------------------------

/// Outcome of evaluating a member's activity streak on a new XP event.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum StreakOutcome {
    /// The member already earned XP today; the streak is untouched.
    Unchanged,
    /// The member last earned XP yesterday; the streak grows by one.
    Extended(i32),
    /// The streak lapsed (or this is the member's first ever XP); it
    /// restarts at 1.
    Reset,
}

impl StreakOutcome {
    /// The streak value this outcome implies, given the current one.
    #[must_use]
    pub fn resolve(self, current: i32) -> i32 {
        match self {
            StreakOutcome::Unchanged => current,
            StreakOutcome::Extended(next) => next,
            StreakOutcome::Reset => 1,
        }
    }
}

/// Decide how an XP event earned on `today` affects a streak whose last
/// active day was `last_active_day`.
///
/// Days are expressed as a count of days since the Unix epoch so this
/// function stays free of `chrono` and compiles for the WASM frontend.
/// A `None` last day means the member has never earned XP.
///
/// Note that the streak is *not* capped: [`streak_multiplier`] tops out
/// at 30 days, but the raw count keeps climbing so `longest_streak` and
/// the `Streaker` badge have something to read.
#[must_use]
pub fn evaluate_streak(current: i32, last_active_day: Option<i64>, today: i64) -> StreakOutcome {
    match last_active_day {
        Some(last) if last == today => StreakOutcome::Unchanged,
        Some(last) if last + 1 == today => StreakOutcome::Extended(current.saturating_add(1)),
        // A gap of two days or more, a clock skew putting the last
        // activity in the future, or no history at all: start over.
        _ => StreakOutcome::Reset,
    }
}

// ---------------------------------------------------------------------------
// Levels
// ---------------------------------------------------------------------------

/// XP required to advance from level `n` to level `n + 1`.
///
/// Levels are a finer-grained progression than ranks: ranks are the
/// nine milestones a member shows off, levels are the small dopamine
/// tick between them. The curve is quadratic-ish — level 2 costs 100,
/// level 10 costs 900 — so levels stay meaningful deep into the ladder.
#[must_use]
pub const fn xp_for_level(level: i32) -> i64 {
    if level < 1 {
        return 0;
    }
    100 * (level as i64)
}

/// Total cumulative XP needed to *reach* a given level.
#[must_use]
pub fn cumulative_xp_for_level(level: i32) -> i64 {
    if level <= 1 {
        return 0;
    }
    // Sum of 100..=100*(level-1) == 100 * (level-1) * level / 2
    let n = i64::from(level - 1);
    100 * n * (n + 1) / 2
}

/// Level implied by a total XP amount. Always at least 1.
#[must_use]
pub fn level_for_xp(xp_total: i64) -> i32 {
    if xp_total <= 0 {
        return 1;
    }
    let mut level = 1;
    // The ladder is shallow enough (level 100 ≈ 495 000 XP) that a
    // linear scan is cheaper and clearer than the quadratic inverse.
    while cumulative_xp_for_level(level + 1) <= xp_total && level < 999 {
        level += 1;
    }
    level
}

/// Progress towards the next level, as `(xp_into_level, xp_needed)`.
#[must_use]
pub fn level_progress(xp_total: i64) -> (i64, i64) {
    let level = level_for_xp(xp_total);
    let floor = cumulative_xp_for_level(level);
    let needed = xp_for_level(level);
    ((xp_total - floor).max(0), needed.max(1))
}

// ---------------------------------------------------------------------------
// Quests
// ---------------------------------------------------------------------------

/// Condition a quest measures. Mirrors the `quests.condition_type`
/// CHECK constraint.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum QuestCondition {
    /// Number of GitHub pushes credited.
    Push,
    /// Number of event QR scans.
    Attend,
    /// Number of projects submitted for review.
    Submit,
    /// Number of reviews submitted.
    Review,
}

impl QuestCondition {
    /// Stable string identifier, as stored in `quests.condition_type`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            QuestCondition::Push => "Push",
            QuestCondition::Attend => "Attend",
            QuestCondition::Submit => "Submit",
            QuestCondition::Review => "Review",
        }
    }

    /// Parse from the canonical string identifier.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "Push" => Some(QuestCondition::Push),
            "Attend" => Some(QuestCondition::Attend),
            "Submit" => Some(QuestCondition::Submit),
            "Review" => Some(QuestCondition::Review),
            _ => None,
        }
    }

    /// The XP source whose arrival advances this condition. `Submit` has
    /// no XP source of its own — it is advanced explicitly by the
    /// project handler — hence the `Option`.
    #[must_use]
    pub const fn triggered_by(self) -> Option<XpSource> {
        match self {
            QuestCondition::Push => Some(XpSource::Github),
            QuestCondition::Attend => Some(XpSource::Qr),
            QuestCondition::Review => Some(XpSource::Review),
            QuestCondition::Submit => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Global rank thresholds
// ---------------------------------------------------------------------------

/// XP threshold required to *enter* a given rank, in increasing order.
/// Matched by [`crate::roles::GlobalRank::from_xp`].
pub const RANK_THRESHOLDS: &[(i64, &str)] = &[
    // `Pending` and `Visitor` sat below the ladder behind an email
    // gate that no longer exists. They are kept here so the table still
    // lines up with the `GlobalRank` enum and the Discord role ladder,
    // but nothing assigns them: `Initiate` is where every member starts.
    (0, "Pending"),
    (0, "Visitor"),
    (0, "Initiate"),
    (150, "Apprentice"),
    (400, "JuniorDev"),
    (1_000, "SeniorDev"),
    (2_500, "Expert"),
    (5_000, "Veteran"),
    (10_000, "Legend"),
    (25_000, "Myth"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multipliers_compound() {
        // 30-day streak (2.0x) with 3 active tracks (1.20x) => 2.4x.
        assert_eq!(compute_final_xp(10, 30, 3), 24);
        // No streak, single track: untouched.
        assert_eq!(compute_final_xp(10, 0, 1), 10);
    }

    #[test]
    fn cap_is_applied_after_multipliers() {
        // The audit's finding: a 50/day cap must stay a 50/day cap even
        // for a member sitting on a 2.6x multiplier.
        let boosted = compute_final_xp(50, 30, 4);
        assert_eq!(boosted, 130, "sanity: multipliers really do inflate");
        assert_eq!(apply_daily_cap(boosted, 0, 50), 50);
    }

    #[test]
    fn cap_accounts_for_xp_already_earned_today() {
        assert_eq!(apply_daily_cap(30, 40, 50), 10);
        assert_eq!(apply_daily_cap(30, 50, 50), 0);
        assert_eq!(apply_daily_cap(30, 80, 50), 0);
    }

    #[test]
    fn streak_extends_only_on_consecutive_days() {
        assert_eq!(evaluate_streak(3, Some(100), 101), StreakOutcome::Extended(4));
        assert_eq!(evaluate_streak(3, Some(100), 100), StreakOutcome::Unchanged);
        assert_eq!(evaluate_streak(3, Some(100), 102), StreakOutcome::Reset);
        assert_eq!(evaluate_streak(3, None, 100), StreakOutcome::Reset);
    }

    #[test]
    fn streak_reset_starts_at_one_not_zero() {
        // A member earning XP today is on a 1-day streak, not a 0-day one.
        assert_eq!(StreakOutcome::Reset.resolve(9), 1);
        assert_eq!(StreakOutcome::Unchanged.resolve(9), 9);
        assert_eq!(StreakOutcome::Extended(10).resolve(9), 10);
    }

    #[test]
    fn a_clock_skewed_future_last_day_resets_rather_than_panics() {
        assert_eq!(evaluate_streak(5, Some(200), 100), StreakOutcome::Reset);
    }

    #[test]
    fn levels_start_at_one_and_climb() {
        assert_eq!(level_for_xp(0), 1);
        assert_eq!(level_for_xp(-50), 1);
        assert_eq!(level_for_xp(99), 1);
        assert_eq!(level_for_xp(100), 2);
        assert_eq!(level_for_xp(299), 2);
        assert_eq!(level_for_xp(300), 3);
    }

    #[test]
    fn level_progress_is_consistent_with_level() {
        for xp in [0_i64, 1, 99, 100, 250, 300, 5_000, 50_000] {
            let level = level_for_xp(xp);
            let (into, needed) = level_progress(xp);
            assert!(into >= 0 && into < needed, "xp={xp} level={level}");
            assert_eq!(cumulative_xp_for_level(level) + into, xp);
        }
    }

    #[test]
    fn quest_conditions_round_trip() {
        for c in [
            QuestCondition::Push,
            QuestCondition::Attend,
            QuestCondition::Submit,
            QuestCondition::Review,
        ] {
            assert_eq!(QuestCondition::parse(c.as_str()), Some(c));
        }
    }

    #[test]
    fn only_submit_lacks_an_xp_trigger() {
        assert_eq!(QuestCondition::Submit.triggered_by(), None);
        assert_eq!(QuestCondition::Push.triggered_by(), Some(XpSource::Github));
        assert_eq!(QuestCondition::Attend.triggered_by(), Some(XpSource::Qr));
        assert_eq!(QuestCondition::Review.triggered_by(), Some(XpSource::Review));
    }
}

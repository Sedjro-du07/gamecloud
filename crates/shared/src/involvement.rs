//! Involvement: a level from 1 to 5, and the credits it earns (0 to 3).
//!
//! XP measures progression and never goes down. Involvement measures what
//! a member does now, regularly and in several ways, over one month. It
//! is computed from what the platform already records — nobody types it
//! in — and it can be lost: a member who slackens drops, at most one
//! level a month.
//!
//! The score out of 100 has four axes. Production counts most; presence
//! and regularity count almost as much; helping others counts too.
//!
//! | axis        | points | from                                                      |
//! |-------------|--------|-----------------------------------------------------------|
//! | production  | 35     | GitHub XP, projects started and released, shares, resources |
//! | presence    | 30     | sessions attended out of sessions held                    |
//! | regularity  | 25     | distinct active days                                      |
//! | help        | 10     | reviews of others' projects, quests completed             |
//!
//! When no session was held in the month, presence cannot be earned, so
//! it is left out and the other axes are scaled up to 100: nobody loses
//! points for a session that did not exist.
//!
//! Credits must be earned: there is no grace period. A Bureau office or a
//! track lead guarantees a floor; the President and the Vice-President can
//! force any level, with a reason, and that decision wins.

/// Points available on each axis.
pub const PRODUCTION_MAX: u32 = 35;
/// Points available for presence.
pub const PRESENCE_MAX: u32 = 30;
/// Points available for regularity.
pub const REGULARITY_MAX: u32 = 25;
/// Points available for help.
pub const HELP_MAX: u32 = 10;

/// GitHub XP in a month that earns the full GitHub share of production.
pub const GITHUB_XP_FULL: u32 = 300;
/// Share of production that GitHub activity can earn.
pub const GITHUB_POINTS: u32 = 15;
/// Points for a project released in the month.
pub const PROJECT_RELEASED_POINTS: u32 = 8;
/// Points for a project started in the month.
pub const PROJECT_STARTED_POINTS: u32 = 3;
/// Share of production that projects can earn.
pub const PROJECTS_CAP: u32 = 12;
/// Points per share or accepted resource.
pub const SHARING_POINTS: u32 = 2;
/// Share of production that shares and resources can earn.
pub const SHARING_CAP: u32 = 8;
/// Active days in a month that earn the full regularity score.
pub const ACTIVE_DAYS_FULL: u32 = 12;
/// Points per review of someone else's project.
pub const REVIEW_POINTS: u32 = 3;
/// Points per quest completed.
pub const QUEST_POINTS: u32 = 2;

/// Lowest score of each level, from level 2 up.
pub const LEVEL_THRESHOLDS: [u32; 4] = [15, 35, 60, 80];

/// What one member did over one month.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Activity {
    /// XP earned from GitHub (commits, pull requests, reviews, issues).
    pub github_xp: u32,
    /// Projects they contribute to that were released.
    pub projects_released: u32,
    /// Projects they contribute to that were started.
    pub projects_started: u32,
    /// Files or links shared.
    pub shares: u32,
    /// Resources they proposed that were accepted.
    pub resources_accepted: u32,
    /// Sessions they attended (scanned on site).
    pub sessions_attended: u32,
    /// Sessions held for everybody.
    pub sessions_held: u32,
    /// Distinct days with at least one action.
    pub active_days: u32,
    /// Reviews of someone else's project.
    pub reviews: u32,
    /// Quests completed.
    pub quests: u32,
}

/// The score, axis by axis.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Breakdown {
    /// Out of [`PRODUCTION_MAX`].
    pub production: u32,
    /// Out of [`PRESENCE_MAX`]; `None` when no session was held.
    pub presence: Option<u32>,
    /// Out of [`REGULARITY_MAX`].
    pub regularity: u32,
    /// Out of [`HELP_MAX`].
    pub help: u32,
    /// Out of 100.
    pub score: u32,
}

/// `part / whole` of `points`, rounded to the nearest point.
fn share(part: u32, whole: u32, points: u32) -> u32 {
    if whole == 0 {
        return 0;
    }
    let part = part.min(whole);
    (part * points * 2 + whole) / (whole * 2)
}

/// Score one month of activity.
#[must_use]
pub fn score(a: &Activity) -> Breakdown {
    let github = share(a.github_xp, GITHUB_XP_FULL, GITHUB_POINTS);
    let projects = (a.projects_released * PROJECT_RELEASED_POINTS
        + a.projects_started * PROJECT_STARTED_POINTS)
        .min(PROJECTS_CAP);
    let sharing = ((a.shares + a.resources_accepted) * SHARING_POINTS).min(SHARING_CAP);
    let production = (github + projects + sharing).min(PRODUCTION_MAX);

    let presence = (a.sessions_held > 0)
        .then(|| share(a.sessions_attended, a.sessions_held, PRESENCE_MAX));
    let regularity = share(a.active_days, ACTIVE_DAYS_FULL, REGULARITY_MAX);
    let help = (a.reviews * REVIEW_POINTS + a.quests * QUEST_POINTS).min(HELP_MAX);

    let earned = production + presence.unwrap_or(0) + regularity + help;
    let available = PRODUCTION_MAX + REGULARITY_MAX + HELP_MAX
        + if presence.is_some() { PRESENCE_MAX } else { 0 };
    let score = share(earned, available, 100);

    Breakdown { production, presence, regularity, help, score }
}

/// The level a score earns, from 1 to 5.
#[must_use]
pub fn level_for(score: u32) -> u8 {
    let above = LEVEL_THRESHOLDS.iter().filter(|t| score >= **t).count();
    // At most four thresholds: the cast cannot truncate.
    #[allow(clippy::cast_possible_truncation)]
    let above = above as u8;
    1 + above
}

/// Points still needed to reach the next level, or `None` at level 5.
#[must_use]
pub fn points_to_next(score: u32) -> Option<u32> {
    LEVEL_THRESHOLDS.iter().find(|t| score < **t).map(|t| t - score)
}

/// The credits a level earns: none below level 3, then 1, 2 and 3.
#[must_use]
pub const fn credits_for(level: u8) -> u8 {
    match level {
        0..=2 => 0,
        3 => 1,
        4 => 2,
        _ => 3,
    }
}

/// What a member's responsibilities guarantee.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Responsibilities {
    /// Holds a Bureau office.
    pub bureau: bool,
    /// Leads a track.
    pub track_lead: bool,
    /// Co-leads a track.
    pub track_co_lead: bool,
}

/// The lowest level these responsibilities allow: 3 for the Bureau and
/// track leads, 2 for co-leads, otherwise none.
#[must_use]
pub const fn floor_for(r: Responsibilities) -> u8 {
    if r.bureau || r.track_lead {
        3
    } else if r.track_co_lead {
        2
    } else {
        1
    }
}

/// One month of a member's record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Month {
    /// The score that month.
    pub breakdown: Breakdown,
    /// The level the score alone earns.
    pub earned: u8,
    /// The level kept: never more than one below last month's, never
    /// below the floor.
    pub level: u8,
}

/// Walk a member's months in order. A level is reached as soon as it is
/// earned; it is lost at most one level a month; the floor always holds.
#[must_use]
pub fn walk(activities: &[Activity], floor: u8) -> Vec<Month> {
    let mut months = Vec::with_capacity(activities.len());
    let mut previous: Option<u8> = None;
    for activity in activities {
        let breakdown = score(activity);
        let earned = level_for(breakdown.score);
        let kept = previous.map_or(earned, |p| earned.max(p.saturating_sub(1)));
        let level = kept.max(floor).clamp(1, 5);
        previous = Some(level);
        months.push(Month { breakdown, earned, level });
    }
    months
}

/// The year's level: the average of the months, rounded to the nearest
/// level (halves up). `None` before the first month.
#[must_use]
pub fn year_level(months: &[Month]) -> Option<u8> {
    if months.is_empty() {
        return None;
    }
    let sum: u32 = months.iter().map(|m| u32::from(m.level)).sum();
    let count = u32::try_from(months.len()).unwrap_or(u32::MAX);
    let rounded = (sum * 2 + count) / (count * 2);
    u8::try_from(rounded).ok().map(|l| l.clamp(1, 5))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn busy() -> Activity {
        Activity {
            github_xp: 400,
            projects_released: 1,
            projects_started: 2,
            shares: 2,
            resources_accepted: 2,
            sessions_attended: 4,
            sessions_held: 4,
            active_days: 15,
            reviews: 3,
            quests: 1,
        }
    }

    #[test]
    fn doing_nothing_is_level_one_and_no_credit() {
        let b = score(&Activity { sessions_held: 4, ..Activity::default() });
        assert_eq!(b.score, 0);
        assert_eq!(level_for(b.score), 1);
        assert_eq!(credits_for(level_for(b.score)), 0);
    }

    #[test]
    fn maximal_involvement_is_level_five_and_three_credits() {
        let b = score(&busy());
        assert_eq!(b.production, PRODUCTION_MAX);
        assert_eq!(b.presence, Some(PRESENCE_MAX));
        assert_eq!(b.regularity, REGULARITY_MAX);
        assert_eq!(b.help, HELP_MAX);
        assert_eq!(b.score, 100);
        assert_eq!(credits_for(level_for(b.score)), 3);
    }

    #[test]
    fn coming_to_everything_without_producing_is_not_the_top() {
        let a = Activity { sessions_attended: 4, sessions_held: 4, active_days: 12, ..Activity::default() };
        let b = score(&a);
        assert_eq!(b.score, 55);
        assert_eq!(level_for(b.score), 3);
    }

    #[test]
    fn no_session_held_costs_nobody_anything() {
        let a = Activity { sessions_held: 0, ..busy() };
        let b = score(&a);
        assert_eq!(b.presence, None);
        assert_eq!(b.score, 100);
    }

    #[test]
    fn levels_follow_the_thresholds() {
        assert_eq!(level_for(14), 1);
        assert_eq!(level_for(15), 2);
        assert_eq!(level_for(34), 2);
        assert_eq!(level_for(35), 3);
        assert_eq!(level_for(60), 4);
        assert_eq!(level_for(79), 4);
        assert_eq!(level_for(80), 5);
        assert_eq!(points_to_next(70), Some(10));
        assert_eq!(points_to_next(95), None);
    }

    #[test]
    fn caps_stop_one_kind_of_action_from_buying_a_level() {
        let a = Activity { github_xp: 100_000, shares: 50, ..Activity::default() };
        let b = score(&a);
        assert_eq!(b.production, GITHUB_POINTS + SHARING_CAP);
    }

    #[test]
    fn slackening_loses_one_level_a_month() {
        let idle = Activity { sessions_held: 4, ..Activity::default() };
        let months = walk(&[busy(), idle, idle, idle, idle], 1);
        let levels: Vec<u8> = months.iter().map(|m| m.level).collect();
        assert_eq!(levels, [5, 4, 3, 2, 1]);
    }

    #[test]
    fn a_level_is_reached_as_soon_as_it_is_earned() {
        let idle = Activity { sessions_held: 4, ..Activity::default() };
        let levels: Vec<u8> = walk(&[idle, busy()], 1).iter().map(|m| m.level).collect();
        assert_eq!(levels, [1, 5]);
    }

    #[test]
    fn responsibilities_guarantee_a_floor() {
        assert_eq!(floor_for(Responsibilities { bureau: true, ..Responsibilities::default() }), 3);
        assert_eq!(floor_for(Responsibilities { track_lead: true, ..Responsibilities::default() }), 3);
        assert_eq!(floor_for(Responsibilities { track_co_lead: true, ..Responsibilities::default() }), 2);
        assert_eq!(floor_for(Responsibilities::default()), 1);
        let idle = Activity { sessions_held: 4, ..Activity::default() };
        assert!(walk(&[idle, idle], 3).iter().all(|m| m.level == 3));
    }

    #[test]
    fn the_year_is_the_rounded_average() {
        let idle = Activity { sessions_held: 4, ..Activity::default() };
        let months = walk(&[busy(), busy(), idle], 1);
        // 5, 5, 4 → 4.67 → 5.
        assert_eq!(year_level(&months), Some(5));
        assert_eq!(year_level(&[]), None);
    }
}

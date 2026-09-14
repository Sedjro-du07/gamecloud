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
// Global rank thresholds
// ---------------------------------------------------------------------------

/// XP threshold required to *enter* a given rank, in increasing order.
/// Matched by [`crate::roles::GlobalRank::from_xp`].
pub const RANK_THRESHOLDS: &[(i64, &str)] = &[
    (0, "Pending"),
    (0, "Visitor"),       // gated on email_verified, not XP
    (0, "Initiate"),      // gated on onboarding completion
    (150, "Apprentice"),
    (400, "JuniorDev"),
    (1_000, "SeniorDev"),
    (2_500, "Expert"),
    (5_000, "Veteran"),
    (10_000, "Legend"),
    (25_000, "Myth"),
];

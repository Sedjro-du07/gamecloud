//! View models shared by the server and the browser.
//!
//! Everything here compiles for **both** targets: the Axum handlers and
//! Leptos server functions produce these types, and the hydrated WASM
//! bundle consumes them. That is the whole reason the frontend is Rust —
//! a rank threshold or a track colour is defined once and used on both
//! sides of the wire.
//!
//! These are deliberately *flat and owned*: no `chrono` types, no
//! borrowed data, nothing that fails to round-trip through the
//! SSR/hydration boundary. Timestamps cross as pre-formatted strings,
//! because the browser only ever displays them.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// API error.
#[derive(Debug, Clone, Error, Serialize, Deserialize, PartialEq, Eq)]
#[error("{message}")]
pub struct ApiError {
    /// Machine-readable error code (matches `WebError::code()`).
    pub code: String,
    /// Human-readable message.
    pub message: String,
}

/// Error response envelope, mirroring the server's JSON shape.
#[derive(Debug, Deserialize)]
pub struct ErrorEnvelope {
    /// The error itself.
    pub error: ApiError,
}

// ---------------------------------------------------------------------------
// Profile
// ---------------------------------------------------------------------------

/// The signed-in member, as every page needs them.
///
/// `PartialEq` but not `Eq`: `streak_multiplier` is an `f64`, and
/// Leptos only needs structural comparison to decide whether to re-render.
///
/// The several `bool` fields are independent facts the UI branches on
/// (verified? onboarded? may see the admin link?), so
/// `struct_excessive_bools` is not useful advice here.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct MeView {
    /// Member id, as a string so it round-trips without a uuid dependency.
    pub id: String,
    /// Name to display.
    pub display_name: String,
    /// Avatar URL.
    pub avatar_url: Option<String>,
    /// Total XP.
    pub xp_total: i64,
    /// Current level.
    pub level: i32,
    /// XP earned inside the current level.
    pub level_xp_into: i64,
    /// XP needed to finish the current level.
    pub level_xp_needed: i64,
    /// Rank identifier.
    pub global_rank: String,
    /// Rank display title.
    pub rank_title: String,
    /// Rank accent colour.
    pub rank_color: String,
    /// Bureau role identifier, if any.
    pub bureau_role: Option<String>,
    /// Bureau role display title, if any.
    pub bureau_title: Option<String>,
    /// Consecutive active days.
    pub streak_days: i32,
    /// Multiplier the current streak is worth, e.g. `1.25`.
    pub streak_multiplier: f64,
    /// Verified Epitech address.
    pub email: Option<String>,
    /// Whether the address is verified.
    pub email_verified: bool,
    /// Linked GitHub login.
    pub github_username: Option<String>,
    /// Place on the all-time board.
    pub leaderboard_position: Option<i64>,
    /// Whether onboarding (track selection) is still outstanding.
    pub needs_onboarding: bool,
    /// Whether this member can reach the admin panel.
    pub can_access_admin: bool,
    /// Whether this member can mint QR codes.
    pub can_generate_qr: bool,
    /// Whether this member holds `Reviewer` or above in any track, and
    /// therefore has a review queue worth showing.
    pub can_review: bool,
}

// ---------------------------------------------------------------------------
// Tracks and badges
// ---------------------------------------------------------------------------

/// A member's standing in one track.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TrackView {
    /// Canonical identifier.
    pub id: String,
    /// Emoji shown beside the name.
    pub emoji: String,
    /// Theme colour.
    pub color: String,
    /// Chosen specialization.
    pub specialization: Option<String>,
    /// Role held inside the track.
    pub role: String,
    /// XP accumulated in this track.
    pub xp: i64,
}

/// One of the 8 tracks, with its specializations — the onboarding picker.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TrackOption {
    /// Canonical identifier.
    pub id: String,
    /// Emoji.
    pub emoji: String,
    /// Theme colour.
    pub color: String,
    /// Canonical specializations for this track.
    pub specializations: Vec<String>,
    /// Whether the member already belongs to it.
    pub joined: bool,
}

/// A badge, held or merely on offer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BadgeItem {
    /// Canonical identifier.
    pub id: String,
    /// Display label.
    pub title: String,
    /// How it is earned.
    pub description: String,
    /// Whether the member holds it.
    pub held: bool,
}

// ---------------------------------------------------------------------------
// Leaderboard
// ---------------------------------------------------------------------------

/// One row of a leaderboard.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LeaderboardEntry {
    /// 1-based position.
    pub position: usize,
    /// Member id.
    pub user_id: String,
    /// Name to display.
    pub display_name: String,
    /// Avatar.
    pub avatar_url: Option<String>,
    /// XP within the scope shown.
    pub xp: i64,
    /// Rank (or track role, on a track board).
    pub rank: String,
    /// Level.
    pub level: i32,
    /// Streak.
    pub streak_days: i32,
    /// Whether this row is the viewer.
    pub is_me: bool,
}

/// A leaderboard plus what it is scoped to.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct LeaderboardView {
    /// `all`, `season` or `track`.
    pub scope: String,
    /// Season or track name, when relevant.
    pub label: Option<String>,
    /// The rows.
    pub entries: Vec<LeaderboardEntry>,
}

// ---------------------------------------------------------------------------
// Quests
// ---------------------------------------------------------------------------

/// A quest with the viewer's progress folded in.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QuestItem {
    /// Quest id.
    pub id: String,
    /// Title.
    pub title: String,
    /// Description.
    pub description: Option<String>,
    /// XP paid on completion.
    pub xp_reward: i32,
    /// `Weekly`, `Special` or `Hidden`.
    pub quest_type: String,
    /// What is counted, in words.
    pub condition_label: String,
    /// Progress so far.
    pub progress: i32,
    /// Target.
    pub target: i32,
    /// Track restriction, if any.
    pub track: Option<String>,
    /// Whether the viewer has finished it.
    pub completed: bool,
    /// Human-readable time remaining, e.g. "3 j restants".
    pub time_left: String,
}

// ---------------------------------------------------------------------------
// Projects
// ---------------------------------------------------------------------------

/// A project card.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectCard {
    /// Project id.
    pub id: String,
    /// Name.
    pub name: String,
    /// One-liner.
    pub short_description: Option<String>,
    /// Primary track.
    pub track: String,
    /// Track emoji.
    pub track_emoji: String,
    /// Lifecycle status.
    pub status: String,
    /// Rarity tier.
    pub rarity: String,
    /// Rarity colour.
    pub rarity_color: String,
    /// Card image.
    pub thumbnail_url: Option<String>,
    /// Credited contributors.
    pub contributor_count: i64,
    /// Release date, pre-formatted.
    pub released_on: Option<String>,
}

/// One track's verdict on a project.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VerdictItem {
    /// Track concerned.
    pub track: String,
    /// Verdict.
    pub status: String,
    /// Reviewer.
    pub reviewer_name: Option<String>,
    /// Written feedback.
    pub feedback: Option<String>,
}

/// A credited contributor.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContributorItem {
    /// Member id.
    pub user_id: String,
    /// Name.
    pub display_name: String,
    /// Avatar.
    pub avatar_url: Option<String>,
    /// Track they worked in.
    pub track: String,
    /// Role on the project.
    pub role: Option<String>,
}

/// Full project view.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectDetailView {
    /// Card fields.
    pub card: ProjectCard,
    /// Long description.
    pub long_description: Option<String>,
    /// Repository.
    pub github_repo_url: Option<String>,
    /// itch.io page.
    pub itch_url: Option<String>,
    /// Gallery.
    pub screenshots: Vec<String>,
    /// Contributors.
    pub contributors: Vec<ContributorItem>,
    /// Per-track verdicts.
    pub validations: Vec<VerdictItem>,
    /// What the viewer may do here.
    pub rights: ProjectRights,
}

/// A project waiting for the viewer's verdict.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReviewItem {
    /// Project id.
    pub project_id: String,
    /// Project name.
    pub name: String,
    /// One-liner.
    pub short_description: Option<String>,
    /// The track being asked of the viewer.
    pub track: String,
    /// Who submitted it.
    pub author_name: String,
}

/// What the viewer is allowed to do on a project detail page.
///
/// Computed server-side from `Authority`, because the browser must never
/// be the thing deciding what somebody may do — it only decides what to
/// draw. Every action behind these flags is re-checked by the API.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectRights {
    /// May open a review round on this project.
    pub can_submit: bool,
    /// May publish it.
    pub can_release: bool,
    /// Tracks the viewer may render a verdict for, right now.
    pub reviewable_tracks: Vec<String>,
}

// ---------------------------------------------------------------------------
// XP history and attendance
// ---------------------------------------------------------------------------

/// One line of a member's XP ledger.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct XpEntry {
    /// Signed amount.
    pub amount: i32,
    /// Source label.
    pub source: String,
    /// Track, when track-scoped.
    pub track: Option<String>,
    /// Ledger line.
    pub description: Option<String>,
    /// Pre-formatted timestamp.
    pub when: String,
}

/// One attendance record.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AttendanceEntry {
    /// Event name.
    pub event_name: String,
    /// XP credited.
    pub xp: i32,
    /// Pre-formatted timestamp.
    pub when: String,
}

/// A resource-library entry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResourceItem {
    /// Entry id.
    pub id: String,
    /// Title.
    pub title: String,
    /// Link.
    pub url: String,
    /// `Tutorial`, `Tool`, `Asset`, `Doc` or `Video`.
    pub kind: Option<String>,
    /// Tracks it is relevant to.
    pub tracks: Vec<String>,
    /// Difficulty band.
    pub level: Option<String>,
    /// Who proposed it.
    pub submitted_by: String,
    /// Whether a validator signed off.
    pub validated: bool,
    /// Vote tally.
    pub votes: i32,
    /// Whether the viewer has voted.
    pub has_voted: bool,
    /// Whether the viewer may validate entries.
    pub may_validate: bool,
}

/// One line of the audit trail, as the Bureau panel shows it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuditLine {
    /// Who acted. "la plateforme" when the platform acted on its own.
    pub actor: String,
    /// Dotted action name.
    pub action: String,
    /// Free-form details.
    pub detail: String,
    /// Pre-formatted timestamp.
    pub when: String,
}

// ---------------------------------------------------------------------------
// Character sheet
// ---------------------------------------------------------------------------

/// Everything about a member, for the profile page and the exportable
/// sheet.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SheetView {
    /// The member.
    pub me: MeView,
    /// Their tracks.
    pub tracks: Vec<TrackView>,
    /// Badges held.
    pub badges: Vec<BadgeItem>,
    /// Recent XP.
    pub recent_xp: Vec<XpEntry>,
    /// Recent attendance.
    pub attendance: Vec<AttendanceEntry>,
    /// Projects they are credited on.
    pub projects: Vec<ProjectCard>,
}

// ---------------------------------------------------------------------------
// Small display helpers, shared by SSR and the browser
// ---------------------------------------------------------------------------

/// Format an XP number with a thin space every three digits.
///
/// `12345` becomes `12 345`, which is the French convention the rest of
/// the UI uses.
#[must_use]
pub fn format_xp(xp: i64) -> String {
    let negative = xp < 0;
    let digits = xp.abs().to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3 + 1);

    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push('\u{202f}');
        }
        out.push(c);
    }
    if negative {
        format!("-{out}")
    } else {
        out
    }
}

/// Percentage of a level bar that should be filled, clamped to 0-100.
#[must_use]
pub fn level_percent(into: i64, needed: i64) -> f64 {
    if needed <= 0 {
        return 0.0;
    }
    // Both operands are XP counts well under 2^52, so the f64 mantissa
    // represents them exactly; and the result is a bar width, where a
    // sub-pixel error would not be observable anyway.
    #[allow(clippy::cast_precision_loss)]
    let raw = (into as f64 / needed as f64) * 100.0;
    raw.clamp(0.0, 100.0)
}

/// Percentage of a quest bar that should be filled.
#[must_use]
pub fn quest_percent(progress: i32, target: i32) -> f64 {
    if target <= 0 {
        return 100.0;
    }
    let raw = (f64::from(progress) / f64::from(target)) * 100.0;
    raw.clamp(0.0, 100.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_xp_in_groups_of_three() {
        assert_eq!(format_xp(0), "0");
        assert_eq!(format_xp(999), "999");
        assert_eq!(format_xp(1_000), "1\u{202f}000");
        assert_eq!(format_xp(12_345), "12\u{202f}345");
        assert_eq!(format_xp(1_234_567), "1\u{202f}234\u{202f}567");
    }

    #[test]
    fn formats_negative_xp() {
        // Manual revokes are negative and must still read correctly.
        assert_eq!(format_xp(-1_500), "-1\u{202f}500");
    }

    #[test]
    fn level_percent_is_clamped() {
        assert!((level_percent(0, 100) - 0.0).abs() < f64::EPSILON);
        assert!((level_percent(50, 100) - 50.0).abs() < f64::EPSILON);
        assert!((level_percent(150, 100) - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn level_percent_survives_a_zero_denominator() {
        // Never divide by zero just because a level table is odd.
        assert!((level_percent(10, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn quest_percent_treats_a_zero_target_as_done() {
        assert!((quest_percent(0, 0) - 100.0).abs() < f64::EPSILON);
        assert!((quest_percent(1, 4) - 25.0).abs() < f64::EPSILON);
        assert!((quest_percent(9, 4) - 100.0).abs() < f64::EPSILON);
    }
}

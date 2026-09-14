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
    /// Title of the next rank earned by XP; `None` at the top.
    pub next_rank_title: Option<String>,
    /// XP earned since the current rank began.
    pub rank_xp_into: i64,
    /// XP between the current rank and the next.
    pub rank_xp_needed: i64,
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
    /// The title to show for `rank`: the member title, or the track title
    /// on a track board.
    pub rank_title: String,
    /// Accent colour for the row: the rank's ring, or the track's colour.
    pub rank_color: String,
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
    /// May attach a build: credited on the project, or leads its track.
    pub can_upload: bool,
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

/// A build attached to a project.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileItem {
    /// Row id, used to build the download link.
    pub id: String,
    /// Original filename.
    pub filename: String,
    /// Kind, guessed from the extension.
    pub kind: String,
    /// Human-readable size, e.g. "42,3 Mo".
    pub size: String,
    /// Release tag.
    pub version: String,
    /// What changed.
    pub changelog: Option<String>,
    /// Pre-formatted upload date.
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

/// Render a byte count the way a person reads it.
///
/// Builds are megabytes, so the unit matters more than the precision:
/// "42,3 Mo" tells a member whether the download is worth starting on
/// their connection, which `44346573` does not.
#[must_use]
pub fn format_bytes(bytes: i64) -> String {
    const KO: f64 = 1024.0;
    // A file size is bounded by the 500 MB upload cap, far inside the
    // f64 mantissa, and the result is a label rounded to one decimal.
    #[allow(clippy::cast_precision_loss)]
    let b = bytes.max(0) as f64;
    if b < KO {
        return format!("{bytes} o");
    }
    let (value, unit) = if b < KO * KO {
        (b / KO, "Ko")
    } else if b < KO * KO * KO {
        (b / (KO * KO), "Mo")
    } else {
        (b / (KO * KO * KO), "Go")
    };
    format!("{value:.1} {unit}").replace('.', ",")
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

/// What the viewer is allowed to put on the calendar.
///
/// Asked of the server directly rather than inferred from the events
/// already on screen. The page used to decide "you may schedule
/// something" by looking for an existing event you could manage, which
/// meant the very first event of a month could never be created: the
/// form only appeared once an event was already there.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct CalendarRights {
    /// Whether the viewer may schedule association-wide events.
    pub association: bool,
    /// Tracks whose own sessions the viewer may schedule.
    pub tracks: Vec<String>,
    /// Whether the viewer may call a Bureau meeting.
    pub bureau: bool,
    /// Whether the viewer may schedule anything at all.
    pub any: bool,
}

/// An event as the scheduling form submits it.
///
/// The form's nine fields travel as one value rather than nine
/// arguments. Beyond keeping the call sites readable, it means adding a
/// tenth field later is a change in one place instead of four.
///
/// Times arrive as the browser's `datetime-local` strings —
/// `2026-09-16T14:00` — and are parsed on the server, where the
/// association's timezone convention is decided once.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EventDraft {
    /// Empty to create; an id to update that event.
    pub id: String,
    /// What it is called.
    pub title: String,
    /// Longer description; empty for none.
    pub description: String,
    /// `Session`, `Workshop`, `Jam`, `Meeting`, `Deadline`, `Showcase`.
    pub kind: String,
    /// Track identifier. Only meaningful when `audience` is `Track`.
    pub track: String,
    /// `Association`, `Track` or `Bureau`. Empty is read as `Association`.
    pub audience: String,
    /// Start, as `YYYY-MM-DDTHH:MM`.
    pub starts_at: String,
    /// End, as `YYYY-MM-DDTHH:MM`.
    pub ends_at: String,
    /// Room, campus, or a link; empty for none.
    pub location: String,
    /// XP handed to whoever scans in.
    pub xp_reward: i32,
}

/// One entry on the calendar.
///
/// Everything is pre-formatted on the server. The browser should not be
/// deciding how a French date reads, and more importantly it should not
/// be deciding *who may edit this* — `can_manage` is the server's answer
/// to that question, computed from the viewer's authority and the
/// event's track, so the page can hide a control it already knows the
/// server would refuse.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CalendarEvent {
    /// Row id.
    pub id: String,
    /// What it is called.
    pub title: String,
    /// Longer description, if there is one.
    pub description: Option<String>,
    /// `Session`, `Workshop`, `Jam`, `Meeting`, `Deadline`, `Showcase`.
    pub kind: String,
    /// French label for the kind.
    pub kind_label: String,
    /// Track identifier, or `None` when the event is not track-scoped.
    pub track: Option<String>,
    /// `Association`, `Track` or `Bureau`.
    pub audience: String,
    /// French label for the audience, for the badge on the card.
    pub audience_label: String,
    /// Track emoji, when scoped to a track.
    pub track_emoji: Option<String>,
    /// `YYYY-MM-DD`, so the month grid can bucket without parsing a date.
    pub day: String,
    /// `14:00 – 16:00`, or `14:00` when the event has no duration.
    pub time_label: String,
    /// Full date and time, for the detail panel.
    pub when_label: String,
    /// Room, campus, or a link.
    pub location: Option<String>,
    /// XP handed to whoever scans in.
    pub xp_reward: i32,
    /// How many members have scanned in.
    pub attendee_count: i64,
    /// Whether the event has been called off.
    pub cancelled: bool,
    /// Whether it has already finished.
    pub past: bool,
    /// Whether the viewer may edit or cancel it.
    pub can_manage: bool,
}

/// A member who scanned in at an event.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EventAttendee {
    /// Name to show.
    pub display_name: String,
    /// When they scanned in.
    pub when: String,
    /// XP they were credited.
    pub xp_rewarded: i32,
}

/// A freshly minted QR code for an event.
///
/// Carries the rendered SVG rather than the raw token, because the point
/// of generating one is to put it on a projector. The token is included
/// too for the rare case of reading it out loud.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QrTicket {
    /// Inline SVG, ready to drop into the page.
    pub svg: String,
    /// The link the code encodes.
    pub scan_url: String,
    /// When the code stops working, formatted.
    pub expires_label: String,
    /// Ceiling on claims, when one was set.
    pub max_scans: Option<i32>,
}

/// One member of a track, as the track board lists them.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TrackMemberRow {
    /// Name to show.
    pub display_name: String,
    /// `Lead`, `CoLead`, `Mentor`, `Reviewer`, `Contributor`, `Observer`.
    pub role: String,
    /// French label for the role.
    pub role_label: String,
    /// XP earned inside this track.
    pub track_xp: i64,
    /// Chosen specialization, when set.
    pub specialization: Option<String>,
}

/// One project as it stands with respect to a single track.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TrackProjectRow {
    /// Project id.
    pub id: String,
    /// Project name.
    pub name: String,
    /// Project status.
    pub status: String,
    /// This track's verdict: `Pending`, `Approved`, `Rejected`, `NotApplicable`.
    pub verdict: String,
    /// This track's mark out of 100, when one was given.
    pub score: Option<i32>,
    /// Who rendered the verdict.
    pub reviewer_name: Option<String>,
    /// How many builds are attached.
    pub file_count: i64,
}

/// Everything one track's page shows.
// No `Eq`: the average mark is a float, and an average is exactly the
// kind of value that should never be compared for exact equality anyway.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrackBoard {
    /// Canonical identifier.
    pub id: String,
    /// Emoji.
    pub emoji: String,
    /// Theme colour.
    pub color: String,
    /// Canonical specializations.
    pub specializations: Vec<String>,
    /// Whether the viewer belongs to this track.
    pub joined: bool,
    /// The viewer's role in it, when they belong.
    pub my_role: Option<String>,
    /// Whether the viewer may render verdicts for this track.
    pub can_review: bool,
    /// Whether the viewer may schedule this track's sessions.
    pub can_manage_events: bool,
    /// Total XP the track has pooled.
    pub total_xp: i64,
    /// Members, strongest role first.
    pub members: Vec<TrackMemberRow>,
    /// Projects this track has a say in.
    pub projects: Vec<TrackProjectRow>,
    /// The track's own upcoming sessions.
    pub events: Vec<CalendarEvent>,
    /// Average of the marks this track has given, when it has given any.
    pub average_score: Option<f64>,
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
    fn formats_byte_counts_for_people() {
        assert_eq!(format_bytes(0), "0 o");
        assert_eq!(format_bytes(512), "512 o");
        assert_eq!(format_bytes(2048), "2,0 Ko");
        assert_eq!(format_bytes(44_346_573), "42,3 Mo");
        assert_eq!(format_bytes(3_221_225_472), "3,0 Go");
    }

    #[test]
    fn a_negative_size_does_not_produce_nonsense() {
        // The column is BIGINT and constrained positive, but a decode
        // slip should read oddly rather than panic or print "-4,0 Go".
        assert_eq!(format_bytes(-1), "-1 o");
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

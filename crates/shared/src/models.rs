//! Database models — Rust mirrors of the PostgreSQL schema.
//!
//! Each `*Record` struct is a 1:1 reflection of one row in one table.
//! The `server` feature gates the `sqlx::FromRow` derive so the WASM
//! frontend can still depend on this module without pulling in `sqlx`.
//!
//! ## Naming convention
//!
//! - `XRecord`  — exact mirror of one DB row, used by `sqlx::query_as!`.
//! - `NewX`     — payload struct for an `INSERT`, no auto-generated
//!   columns (no `id`, no `created_at`).
//! - `XSummary` — a denormalized read model used by API responses,
//!   typically the union of two or three records.
//!
//! All timestamps use `chrono::Timestamp` (matching `TIMESTAMPTZ`).

use serde::{Deserialize, Serialize};
use uuid::Uuid;

// Timestamp type alias.
//
// On the server side we use `chrono::Timestamp`. On the WASM client
// side we don't need real time arithmetic — every timestamp comes from
// the server already serialized to a JSON string — so we keep it as a
// plain `String` to avoid pulling `chrono` into the WASM bundle.
#[cfg(feature = "server")]
type Timestamp = chrono::DateTime<chrono::Utc>;
#[cfg(not(feature = "server"))]
type Timestamp = String;

use crate::roles::{BureauRole, GlobalRank, SpecialBadge, Track, TrackRole};
use crate::xp::XpSource;

// ===========================================================================
// Users
// ===========================================================================

/// Row of `users`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct UserRecord {
    /// Primary key.
    pub id: Uuid,
    /// Discord user ID (snowflake stored as text).
    pub discord_id: String,
    /// GitHub login, set during onboarding.
    pub github_username: Option<String>,
    /// Epitech email. `None` for `Pending` users.
    pub email: Option<String>,
    /// Whether the OTP has been validated.
    pub email_verified: bool,
    /// Discord avatar URL — refreshed on each login by default.
    pub avatar_url: Option<String>,
    /// User-uploaded custom avatar in Supabase Storage.
    pub avatar_custom_url: Option<String>,
    /// Total XP across all sources.
    pub xp_total: i64,
    /// Current level (cosmetic, derived from XP).
    pub level: i32,
    /// Current global rank, stored as a string (matches [`GlobalRank::as_str`]).
    pub global_rank: String,
    /// Current Bureau role, if any.
    pub bureau_role: Option<String>,
    /// Display title selected by the user.
    pub current_title: Option<String>,
    /// Number of consecutive active days.
    pub streak_days: i32,
    /// Last activity timestamp.
    pub last_activity_at: Option<Timestamp>,
    /// Account creation timestamp.
    pub created_at: Timestamp,
}

impl UserRecord {
    /// Parsed [`GlobalRank`].
    #[must_use]
    #[allow(clippy::match_same_arms)] // explicit catch-all for forward compatibility
    pub fn rank(&self) -> GlobalRank {
        match self.global_rank.as_str() {
            "Pending" => GlobalRank::Pending,
            "Visitor" => GlobalRank::Visitor,
            "Initiate" => GlobalRank::Initiate,
            "Apprentice" => GlobalRank::Apprentice,
            "JuniorDev" => GlobalRank::JuniorDev,
            "SeniorDev" => GlobalRank::SeniorDev,
            "Expert" => GlobalRank::Expert,
            "Veteran" => GlobalRank::Veteran,
            "Legend" => GlobalRank::Legend,
            "Myth" => GlobalRank::Myth,
            // Defensive: an unknown value in the DB is treated as Pending.
            _ => GlobalRank::Pending,
        }
    }

    /// Parsed [`BureauRole`], if any.
    #[must_use]
    pub fn bureau(&self) -> Option<BureauRole> {
        let s = self.bureau_role.as_deref()?;
        Some(match s {
            "President" => BureauRole::President,
            "VicePresident" => BureauRole::VicePresident,
            "Secretary" => BureauRole::Secretary,
            "Treasurer" => BureauRole::Treasurer,
            "VpTech" => BureauRole::VpTech,
            "VpCommunity" => BureauRole::VpCommunity,
            "EventManager" => BureauRole::EventManager,
            "AssistantEventManager" => BureauRole::AssistantEventManager,
            "Archiviste" => BureauRole::Archiviste,
            "AssistantArchiviste" => BureauRole::AssistantArchiviste,
            "CommunityManager" => BureauRole::CommunityManager,
            "SocialMediaManager" => BureauRole::SocialMediaManager,
            "Moderator" => BureauRole::Moderator,
            "AssistantModerator" => BureauRole::AssistantModerator,
            "RecruitmentOfficer" => BureauRole::RecruitmentOfficer,
            "PrManager" => BureauRole::PrManager,
            _ => return None,
        })
    }
}

/// Insert payload for `users`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewUser {
    /// Discord ID (required; we always create users from OAuth).
    pub discord_id: String,
    /// Discord avatar at signup.
    pub avatar_url: Option<String>,
}

// ===========================================================================
// Email OTP
// ===========================================================================

/// Row of `email_otps` — one outstanding code per user at most.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct EmailOtpRecord {
    /// Primary key.
    pub id: Uuid,
    /// Owning user.
    pub user_id: Uuid,
    /// `@epitech.eu` email this code was issued for.
    pub email: String,
    /// Argon2 hash of the 6-digit code (never store the plaintext).
    pub code_hash: String,
    /// When the code expires.
    pub expires_at: Timestamp,
    /// Number of failed attempts so far.
    pub attempts: i32,
    /// Issued-at.
    pub created_at: Timestamp,
}

// ===========================================================================
// Refresh tokens
// ===========================================================================

/// Row of `refresh_tokens` — used to rotate JWT access tokens.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct RefreshTokenRecord {
    /// Primary key.
    pub id: Uuid,
    /// Owning user.
    pub user_id: Uuid,
    /// SHA-256 hash of the actual token (never stored in plaintext).
    pub token_hash: String,
    /// Expiration time.
    pub expires_at: Timestamp,
    /// `true` once rotated/logged-out; row is never deleted (audit).
    pub revoked: bool,
    /// User-Agent at issuance, for display in "Active sessions".
    pub user_agent: Option<String>,
    /// IP address at issuance.
    pub ip_address: Option<String>,
    /// Issued-at.
    pub created_at: Timestamp,
}

// ===========================================================================
// Track memberships
// ===========================================================================

/// Row of `track_memberships`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct TrackMembershipRecord {
    /// Primary key.
    pub id: Uuid,
    /// Owning user.
    pub user_id: Uuid,
    /// Track this membership belongs to. Stored as the track's
    /// canonical string (see [`Track::as_str`]).
    pub track: String,
    /// Specialization label, e.g. `"Gameplay"`.
    pub specialization: Option<String>,
    /// Role inside the track. Stored as [`TrackRole::as_str`].
    pub track_role: String,
    /// XP earned in this track.
    pub track_xp: i64,
    /// When the user joined this track.
    pub joined_at: Timestamp,
    /// Last time the user was active in this track (project submission,
    /// etc.).
    pub last_active_at: Option<Timestamp>,
}

impl TrackMembershipRecord {
    /// Parsed track.
    #[must_use]
    pub fn parsed_track(&self) -> Option<Track> {
        Track::parse(&self.track)
    }

    /// Parsed role.
    #[must_use]
    #[allow(clippy::match_same_arms)] // explicit catch-all for forward compatibility
    pub fn parsed_role(&self) -> TrackRole {
        match self.track_role.as_str() {
            "Contributor" => TrackRole::Contributor,
            "Reviewer" => TrackRole::Reviewer,
            "Mentor" => TrackRole::Mentor,
            "CoLead" => TrackRole::CoLead,
            "Lead" => TrackRole::Lead,
            _ => TrackRole::Observer,
        }
    }
}

// ===========================================================================
// Special badges
// ===========================================================================

/// Row of `special_badges`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct SpecialBadgeRecord {
    /// Primary key.
    pub id: Uuid,
    /// Holder of the badge.
    pub user_id: Uuid,
    /// Type of badge (matches [`SpecialBadge`] string form).
    pub badge_type: String,
    /// When awarded.
    pub awarded_at: Timestamp,
    /// Awarder, `None` if assigned automatically.
    pub awarded_by: Option<Uuid>,
}

impl SpecialBadgeRecord {
    /// Parsed badge type.
    #[must_use]
    pub fn parsed(&self) -> Option<SpecialBadge> {
        match self.badge_type.as_str() {
            "FoundingMember" => Some(SpecialBadge::FoundingMember),
            "Alumni" => Some(SpecialBadge::Alumni),
            "ExternalMentor" => Some(SpecialBadge::ExternalMentor),
            "GameJamWinner" => Some(SpecialBadge::GameJamWinner),
            "GameJamParticipant" => Some(SpecialBadge::GameJamParticipant),
            "BugHunter" => Some(SpecialBadge::BugHunter),
            "Contributor" => Some(SpecialBadge::Contributor),
            "Streaker" => Some(SpecialBadge::Streaker),
            "BlockMaster" => Some(SpecialBadge::BlockMaster),
            "MultiTracker" => Some(SpecialBadge::MultiTracker),
            "Validator" => Some(SpecialBadge::Validator),
            "Mentor" => Some(SpecialBadge::Mentor),
            "TopContributor" => Some(SpecialBadge::TopContributor),
            "NightOwl" => Some(SpecialBadge::NightOwl),
            "SpeedRunner" => Some(SpecialBadge::SpeedRunner),
            _ => None,
        }
    }
}

// ===========================================================================
// Projects
// ===========================================================================

/// Status in the project lifecycle. Stored as text in `projects.status`.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::Type))]
#[cfg_attr(feature = "server", sqlx(type_name = "TEXT"))]
#[serde(rename_all = "PascalCase")]
pub enum ProjectStatus {
    /// Editable by contributors.
    Draft,
    /// Pending track validations.
    InReview,
    /// At least one track has approved, others pending.
    PartialOk,
    /// All required tracks approved.
    Approved,
    /// Published to the Hall of Fame.
    Released,
    /// Archived after release.
    Archived,
    /// Permanently rejected.
    Rejected,
}

impl ProjectStatus {
    /// Stable string identifier.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            ProjectStatus::Draft => "Draft",
            ProjectStatus::InReview => "InReview",
            ProjectStatus::PartialOk => "PartialOK",
            ProjectStatus::Approved => "Approved",
            ProjectStatus::Released => "Released",
            ProjectStatus::Archived => "Archived",
            ProjectStatus::Rejected => "Rejected",
        }
    }

    /// Whether `to` is a valid transition from `self`.
    ///
    /// Used by handlers before persisting a status change. The DB also
    /// enforces a CHECK constraint on the column values, but the legal
    /// transitions are domain logic.
    #[must_use]
    #[allow(clippy::unnested_or_patterns)] // matrix layout is clearer flat
    pub fn can_transition_to(self, to: ProjectStatus) -> bool {
        use ProjectStatus::{Approved, Archived, Draft, InReview, PartialOk, Rejected, Released};
        matches!(
            (self, to),
            (Draft, InReview)
                | (Draft, Rejected)
                | (InReview, PartialOk)
                | (InReview, Rejected)
                | (InReview, Draft)
                | (PartialOk, Approved)
                | (PartialOk, Rejected)
                | (PartialOk, InReview)
                | (Approved, Released)
                | (Approved, Rejected)
                | (Released, Archived)
        )
    }
}

/// Project rarity, derived from level/award status. Cosmetic.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::Type))]
#[cfg_attr(feature = "server", sqlx(type_name = "TEXT"))]
#[serde(rename_all = "PascalCase")]
pub enum Rarity {
    /// Tek1 micro-project.
    Common,
    /// Tek2 micro-project.
    Rare,
    /// Tek3 micro-project.
    Epic,
    /// Master project.
    Legendary,
    /// Game Jam winner / award-winning project.
    Mythic,
}

/// Epitech curriculum level a project belongs to.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::Type))]
#[cfg_attr(feature = "server", sqlx(type_name = "TEXT"))]
#[serde(rename_all = "PascalCase")]
pub enum EpitechLevel {
    /// First year.
    Tek1,
    /// Second year.
    Tek2,
    /// Third year.
    Tek3,
    /// Master cycle.
    Master,
}

/// Row of `projects`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct ProjectRecord {
    /// Primary key.
    pub id: Uuid,
    /// Project name (display).
    pub name: String,
    /// Short tagline.
    pub short_description: Option<String>,
    /// Long markdown description.
    pub long_description: Option<String>,
    /// Primary track (the one driving the project's accent color).
    pub primary_track: String,
    /// Status string.
    pub status: String,
    /// Block number 1..=8.
    pub block_number: Option<i32>,
    /// Curriculum level (string form of [`EpitechLevel`]).
    pub epitech_level: Option<String>,
    /// Rarity (string form of [`Rarity`]).
    pub rarity: String,
    /// Thumbnail URL.
    pub thumbnail_url: Option<String>,
    /// Banner URL.
    pub banner_url: Option<String>,
    /// Up to 10 screenshot URLs.
    pub screenshots: Vec<String>,
    /// Trailer / gameplay video URL.
    pub video_url: Option<String>,
    /// GIF URLs.
    pub gifs: Vec<String>,
    /// GitHub repository URL.
    pub github_repo_url: Option<String>,
    /// itch.io URL.
    pub itch_url: Option<String>,
    /// Original creator.
    pub created_by: Uuid,
    /// Created at.
    pub created_at: Timestamp,
    /// Released at, if status is `Released` or `Archived`.
    pub released_at: Option<Timestamp>,
}

/// Row of `project_contributors`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct ProjectContributorRecord {
    /// Project FK.
    pub project_id: Uuid,
    /// User FK.
    pub user_id: Uuid,
    /// Track this contributor represents on the project.
    pub track: String,
    /// Free-form role on the project ("Lead Programmer", "Composer"…).
    pub role_in_project: Option<String>,
}

/// Row of `project_files`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct ProjectFileRecord {
    /// Primary key.
    pub id: Uuid,
    /// Project FK.
    pub project_id: Uuid,
    /// File name as uploaded.
    pub filename: String,
    /// Type label: Executable, Source, Asset, Doc, Audio, Video.
    pub file_type: String,
    /// File size in bytes (≤ 500MB enforced by handler).
    pub size_bytes: i64,
    /// SHA-256 hex digest, used to detect tampering.
    pub checksum_sha256: String,
    /// Path inside Supabase Storage.
    pub storage_path: String,
    /// Semantic version label.
    pub version: String,
    /// Free-form changelog.
    pub changelog: Option<String>,
    /// Uploader.
    pub uploaded_by: Uuid,
    /// Uploaded at.
    pub uploaded_at: Timestamp,
}

/// Row of `track_validations` — one row per (project, track) pair.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct TrackValidationRecord {
    /// Primary key.
    pub id: Uuid,
    /// Project FK.
    pub project_id: Uuid,
    /// Track being validated.
    pub track: String,
    /// "Pending", "Approved", "Rejected", "NotApplicable".
    pub status: String,
    /// Reviewer FK, set when status changes from Pending.
    pub reviewed_by: Option<Uuid>,
    /// Mandatory feedback when rejecting.
    pub feedback: Option<String>,
    /// Reviewed-at, set when status changes from Pending.
    pub reviewed_at: Option<Timestamp>,
}

// ===========================================================================
// Resources
// ===========================================================================

/// Row of `resources`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct ResourceRecord {
    /// Primary key.
    pub id: Uuid,
    /// Display title.
    pub title: String,
    /// External URL.
    pub url: String,
    /// "Tutorial", "Tool", "Asset", "Doc", "Video".
    pub resource_type: Option<String>,
    /// Tracks this resource is relevant to.
    pub tracks: Vec<String>,
    /// Specializations (subset of the tracks').
    pub specializations: Vec<String>,
    /// Skill level the resource is appropriate for.
    pub level: Option<String>,
    /// Submitter.
    pub submitted_by: Uuid,
    /// Validator (Archiviste or TrackLead).
    pub validated_by: Option<Uuid>,
    /// Net upvotes.
    pub votes: i32,
    /// Whether this is an officially curated resource.
    pub is_official: bool,
    /// Created at.
    pub created_at: Timestamp,
}

// ===========================================================================
// Attendance & QR tokens
// ===========================================================================

/// Row of `attendance`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct AttendanceRecord {
    /// Primary key.
    pub id: Uuid,
    /// User FK.
    pub user_id: Uuid,
    /// Free-form name of the event ("Game Jam Mars 2026").
    pub event_name: String,
    /// "Session", "OfficeHours", "StandUp", "GameJam", "Special".
    pub event_type: String,
    /// XP rewarded by this scan.
    pub xp_rewarded: i32,
    /// Scan timestamp.
    pub scanned_at: Timestamp,
}

/// Row of `qr_tokens`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct QrTokenRecord {
    /// Primary key.
    pub id: Uuid,
    /// Signed JWT (used as the QR payload).
    pub token: String,
    /// Event display name.
    pub event_name: String,
    /// "Session", "OfficeHours", "StandUp", "GameJam", "Special".
    pub event_type: String,
    /// XP value granted on each successful scan.
    pub xp_value: i32,
    /// Generator (an EventManager or executive).
    pub created_by: Uuid,
    /// Expiration time (also encoded inside the JWT).
    pub expires_at: Timestamp,
    /// Whether the token has been consumed.
    pub is_used: bool,
}

// ===========================================================================
// Roadmap
// ===========================================================================

/// Row of `roadmap`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct RoadmapEntryRecord {
    /// Primary key.
    pub id: Uuid,
    /// Display name of the milestone.
    pub milestone_name: String,
    /// Long description.
    pub description: Option<String>,
    /// Target date (date only — no time).
    #[cfg(feature = "server")]
    pub target_date: Option<chrono::NaiveDate>,
    #[cfg(not(feature = "server"))]
    /// Target date as ISO-8601 string in non-server mode.
    pub target_date: Option<String>,
    /// "Todo", "InProgress", "Completed".
    pub status: String,
    /// Block number, when relevant.
    pub block_number: Option<i32>,
    /// Whether this milestone is the major boss of its block.
    pub is_major_boss: bool,
    /// Track-scoped roadmap entry, `None` if transversal.
    pub track: Option<String>,
    /// Completion timestamp.
    pub completed_at: Option<Timestamp>,
}

// ===========================================================================
// Quests
// ===========================================================================

/// Row of `quests`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct QuestRecord {
    /// Primary key.
    pub id: Uuid,
    /// Display title.
    pub title: String,
    /// Description.
    pub description: Option<String>,
    /// XP reward upon completion.
    pub xp_reward: i32,
    /// "Weekly", "Special", "Hidden".
    pub quest_type: String,
    /// Action type measured for completion: "Push", "Attend", "Submit",
    /// "Review".
    pub condition_type: String,
    /// Threshold the user must reach (e.g. 5 commits).
    pub condition_value: i32,
    /// Track-scoped quest, or `None` for global.
    pub track: Option<String>,
    /// Quest start time.
    pub starts_at: Timestamp,
    /// Quest end time.
    pub ends_at: Timestamp,
    /// Author.
    pub created_by: Uuid,
}

/// Row of `quest_completions`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct QuestCompletionRecord {
    /// Quest FK.
    pub quest_id: Uuid,
    /// User FK.
    pub user_id: Uuid,
    /// Completed-at.
    pub completed_at: Timestamp,
}

// ===========================================================================
// XP logs
// ===========================================================================

/// Row of `xp_logs`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct XpLogRecord {
    /// Primary key.
    pub id: Uuid,
    /// Recipient.
    pub user_id: Uuid,
    /// Net XP delta (can be negative if a manual revoke happens).
    pub amount: i32,
    /// Source label (matches [`XpSource::as_str`]).
    pub source: String,
    /// Track-scoped XP, or `None` for global-only XP.
    pub track: Option<String>,
    /// Free-form description for the activity feed.
    pub description: Option<String>,
    /// Logged-at.
    pub created_at: Timestamp,
}

impl XpLogRecord {
    /// Parsed source.
    #[must_use]
    pub fn parsed_source(&self) -> Option<XpSource> {
        match self.source.as_str() {
            "GitHub" => Some(XpSource::Github),
            "QR" => Some(XpSource::Qr),
            "Discord" => Some(XpSource::Discord),
            "Review" => Some(XpSource::Review),
            "Quest" => Some(XpSource::Quest),
            "Manual" => Some(XpSource::Manual),
            "Project" => Some(XpSource::Project),
            _ => None,
        }
    }
}

// ===========================================================================
// Audit logs
// ===========================================================================

/// Row of `audit_logs`. Append-only.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct AuditLogRecord {
    /// Primary key.
    pub id: Uuid,
    /// Actor performing the action.
    pub actor_id: Uuid,
    /// Action label, e.g. "ProjectReleased", "BureauRoleAssigned".
    pub action: String,
    /// "User", "Project", "Role", "Resource".
    pub target_type: Option<String>,
    /// FK on the relevant table.
    pub target_id: Option<Uuid>,
    /// Free-form metadata, kept as JSON.
    pub metadata: Option<serde_json::Value>,
    /// Logged-at.
    pub created_at: Timestamp,
}

// ===========================================================================
// Hall of Fame
// ===========================================================================

/// Row of `hall_of_fame`. Auto-generated when a project is Released.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct HallOfFameEntryRecord {
    /// Primary key.
    pub id: Uuid,
    /// Released project.
    pub project_id: Uuid,
    /// When the project was featured.
    pub featured_at: Timestamp,
    /// Sum of XP distributed to contributors at release.
    pub total_xp_distributed: i32,
    /// Contributors list (denormalized for fast display).
    pub boss_defeated_by: Vec<Uuid>,
}

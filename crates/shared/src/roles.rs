//! Roles, ranks, tracks, badges, and permission logic.
//!
//! This module is the single source of truth for "who can do what" on
//! GameCloud OS. Three orthogonal dimensions of role exist:
//!
//! 1. **Bureau role** — elected/appointed leadership (President, VPTech,
//!    EventManager…). One per user (or none). See [`BureauRole`].
//! 2. **Track membership** — for each of the 8 tracks the user has joined,
//!    a sub-role from `Observer` to `Lead`. See [`TrackRole`] and
//!    [`Track`].
//! 3. **Global rank** — derived from total XP (with a special
//!    `email_verified` gate between `Pending` and `Visitor`). See
//!    [`GlobalRank`].
//!
//! Permissions are computed by [`Authority::can`] which takes an
//! [`Authority`] (the actor's full role envelope) and an [`Action`].

use serde::{Deserialize, Serialize};

use crate::xp::RANK_THRESHOLDS;

// ===========================================================================
// Tracks
// ===========================================================================

/// One of the 8 production tracks of the association.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::Type))]
#[cfg_attr(feature = "server", sqlx(type_name = "TEXT"))]
#[serde(rename_all = "PascalCase")]
pub enum Track {
    /// Engineering — gameplay code, tools, engine, devops.
    Engineering,
    /// Game Design — systems, levels, UX, monetization.
    GameDesign,
    /// Narrative — writing, world-building, quests, localization.
    Narrative,
    /// Visual Art — 2D/3D, UI, VFX, concept art.
    VisualArt,
    /// Audio — composition, sound design, audio programming.
    Audio,
    /// Production — producing, PM, scrum, QA leadership.
    Production,
    /// QA & Testing — functional, automated, balance, accessibility.
    Qa,
    /// Marketing — community, social, press, video, copy.
    Marketing,
}

impl Track {
    /// All 8 tracks in canonical order.
    pub const ALL: [Track; 8] = [
        Track::Engineering,
        Track::GameDesign,
        Track::Narrative,
        Track::VisualArt,
        Track::Audio,
        Track::Production,
        Track::Qa,
        Track::Marketing,
    ];

    /// Stable string identifier used in URLs, channel names, and the
    /// `track` column of several tables.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Track::Engineering => "Engineering",
            Track::GameDesign => "GameDesign",
            Track::Narrative => "Narrative",
            Track::VisualArt => "VisualArt",
            Track::Audio => "Audio",
            Track::Production => "Production",
            Track::Qa => "QA",
            Track::Marketing => "Marketing",
        }
    }

    /// Display emoji used in Discord embeds and the HUD.
    #[must_use]
    pub const fn emoji(self) -> &'static str {
        match self {
            Track::Engineering => "⚙️",
            Track::GameDesign => "🎮",
            Track::Narrative => "📖",
            Track::VisualArt => "🎨",
            Track::Audio => "🎵",
            Track::Production => "📊",
            Track::Qa => "🐛",
            Track::Marketing => "📣",
        }
    }

    /// Track theme color, hex including leading `#`.
    #[must_use]
    pub const fn color_hex(self) -> &'static str {
        match self {
            Track::Engineering => "#00f2ff",
            Track::GameDesign => "#bf00ff",
            Track::Narrative => "#ffaa00",
            Track::VisualArt => "#ff007f",
            Track::Audio => "#00ff88",
            Track::Production => "#ff6600",
            Track::Qa => "#ff003c",
            Track::Marketing => "#0088ff",
        }
    }

    /// Parse from the canonical string identifier.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|t| t.as_str() == s)
    }
}

// ===========================================================================
// Track sub-roles
// ===========================================================================

/// Sub-role inside a single track. A user has a separate `TrackRole`
/// per track they have joined.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::Type))]
#[cfg_attr(feature = "server", sqlx(type_name = "TEXT"))]
#[serde(rename_all = "PascalCase")]
pub enum TrackRole {
    /// Default on entry. Read-only access to the track's resources.
    Observer = 0,
    /// 100 XP earned in this track. Can submit projects.
    Contributor = 1,
    /// 300 XP. Can submit reviews on the track's projects.
    Reviewer = 2,
    /// 500 XP. Can mentor and validate junior reviews.
    Mentor = 3,
    /// Appointed by the track Lead. Co-leads the track.
    CoLead = 4,
    /// Appointed by the Bureau. Leads the track and publishes Released
    /// status.
    Lead = 5,
}

impl TrackRole {
    /// Compute the auto-assignable role from a user's track XP. Manual
    /// roles (`CoLead`, `Lead`) are *never* auto-assigned and are returned
    /// only by explicit appointment.
    #[must_use]
    pub fn from_track_xp(track_xp: i64) -> TrackRole {
        if track_xp >= 500 {
            TrackRole::Mentor
        } else if track_xp >= 300 {
            TrackRole::Reviewer
        } else if track_xp >= 100 {
            TrackRole::Contributor
        } else {
            TrackRole::Observer
        }
    }

    /// Stable string identifier. Stored verbatim in the
    /// `track_memberships.track_role` column.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            TrackRole::Observer => "Observer",
            TrackRole::Contributor => "Contributor",
            TrackRole::Reviewer => "Reviewer",
            TrackRole::Mentor => "Mentor",
            TrackRole::CoLead => "CoLead",
            TrackRole::Lead => "Lead",
        }
    }
}

// ===========================================================================
// Bureau roles
// ===========================================================================

/// Elected/appointed leadership role within the association.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::Type))]
#[cfg_attr(feature = "server", sqlx(type_name = "TEXT"))]
#[serde(rename_all = "PascalCase")]
pub enum BureauRole {
    /// Grand Archonte — president of the association.
    President,
    /// Archonte Adjoint — vice-president.
    VicePresident,
    /// Scribe Royal — secretary.
    Secretary,
    /// Intendant du Trésor — treasurer.
    Treasurer,
    /// Forgeron Suprême — VP Tech.
    VpTech,
    /// Héraut en Chef — VP Community.
    VpCommunity,
    /// Maître des Arènes — event manager.
    EventManager,
    /// Écuyer des Arènes — assistant event manager.
    AssistantEventManager,
    /// Gardien des Chroniques — archivist.
    Archiviste,
    /// Apprenti Chroniqueur — assistant archivist.
    AssistantArchiviste,
    /// Éclaireur en Chef — community manager.
    CommunityManager,
    /// Barde Numérique — social media manager.
    SocialMediaManager,
    /// Sentinelle — moderator.
    Moderator,
    /// Garde — assistant moderator.
    AssistantModerator,
    /// Chasseur de Talents — recruitment officer.
    RecruitmentOfficer,
    /// Ambassadeur Suprême — PR manager.
    PrManager,
}

impl BureauRole {
    /// Gamified display title shown in the HUD.
    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            BureauRole::President => "⚜️ Grand Archonte",
            BureauRole::VicePresident => "🛡️ Archonte Adjoint",
            BureauRole::Secretary => "📋 Scribe Royal",
            BureauRole::Treasurer => "💰 Intendant du Trésor",
            BureauRole::VpTech => "🔧 Forgeron Suprême",
            BureauRole::VpCommunity => "🌐 Héraut en Chef",
            BureauRole::EventManager => "🎪 Maître des Arènes",
            BureauRole::AssistantEventManager => "🎭 Écuyer des Arènes",
            BureauRole::Archiviste => "📜 Gardien des Chroniques",
            BureauRole::AssistantArchiviste => "📂 Apprenti Chroniqueur",
            BureauRole::CommunityManager => "📣 Éclaireur en Chef",
            BureauRole::SocialMediaManager => "📸 Barde Numérique",
            BureauRole::Moderator => "⚔️ Sentinelle",
            BureauRole::AssistantModerator => "🗡️ Garde",
            BureauRole::RecruitmentOfficer => "🎯 Chasseur de Talents",
            BureauRole::PrManager => "🤝 Ambassadeur Suprême",
        }
    }

    /// Every bureau role.
    pub const ALL: [BureauRole; 16] = [
        BureauRole::President,
        BureauRole::VicePresident,
        BureauRole::Secretary,
        BureauRole::Treasurer,
        BureauRole::VpTech,
        BureauRole::VpCommunity,
        BureauRole::EventManager,
        BureauRole::AssistantEventManager,
        BureauRole::Archiviste,
        BureauRole::AssistantArchiviste,
        BureauRole::CommunityManager,
        BureauRole::SocialMediaManager,
        BureauRole::Moderator,
        BureauRole::AssistantModerator,
        BureauRole::RecruitmentOfficer,
        BureauRole::PrManager,
    ];

    /// Stable string identifier, as stored in `users.bureau_role`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            BureauRole::President => "President",
            BureauRole::VicePresident => "VicePresident",
            BureauRole::Secretary => "Secretary",
            BureauRole::Treasurer => "Treasurer",
            BureauRole::VpTech => "VpTech",
            BureauRole::VpCommunity => "VpCommunity",
            BureauRole::EventManager => "EventManager",
            BureauRole::AssistantEventManager => "AssistantEventManager",
            BureauRole::Archiviste => "Archiviste",
            BureauRole::AssistantArchiviste => "AssistantArchiviste",
            BureauRole::CommunityManager => "CommunityManager",
            BureauRole::SocialMediaManager => "SocialMediaManager",
            BureauRole::Moderator => "Moderator",
            BureauRole::AssistantModerator => "AssistantModerator",
            BureauRole::RecruitmentOfficer => "RecruitmentOfficer",
            BureauRole::PrManager => "PrManager",
        }
    }

    /// Parse from the canonical string identifier.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|r| r.as_str() == s)
    }

    /// Whether this role belongs to the executive tier (President / VP /
    /// Treasurer / Secretary). Some destructive operations require it.
    #[must_use]
    pub const fn is_executive(self) -> bool {
        matches!(
            self,
            BureauRole::President
                | BureauRole::VicePresident
                | BureauRole::Secretary
                | BureauRole::Treasurer
                | BureauRole::VpTech
                | BureauRole::VpCommunity
        )
    }
}

// ===========================================================================
// Global rank
// ===========================================================================

/// Global rank derived from total XP, gated by account verification.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::Type))]
#[cfg_attr(feature = "server", sqlx(type_name = "TEXT"))]
#[serde(rename_all = "PascalCase")]
pub enum GlobalRank {
    /// No email submitted yet.
    Pending = 0,
    /// Email verified, has not completed onboarding.
    Visitor = 1,
    /// Onboarding complete.
    Initiate = 2,
    /// 150 XP.
    Apprentice = 3,
    /// 400 XP.
    JuniorDev = 4,
    /// 1 000 XP.
    SeniorDev = 5,
    /// 2 500 XP.
    Expert = 6,
    /// 5 000 XP.
    Veteran = 7,
    /// 10 000 XP.
    Legend = 8,
    /// 25 000 XP — secret rank.
    Myth = 9,
}

impl GlobalRank {
    /// Compute the rank from total XP for a user that is at least
    /// `Initiate` (i.e. email-verified and onboarding done).
    ///
    /// Callers must apply the `Pending` / `Visitor` / `Initiate` gating
    /// themselves: passing 0 here returns `Initiate`, not `Pending`.
    #[must_use]
    pub fn from_xp(xp_total: i64) -> GlobalRank {
        // Match order matters: descending thresholds.
        if xp_total >= 25_000 {
            GlobalRank::Myth
        } else if xp_total >= 10_000 {
            GlobalRank::Legend
        } else if xp_total >= 5_000 {
            GlobalRank::Veteran
        } else if xp_total >= 2_500 {
            GlobalRank::Expert
        } else if xp_total >= 1_000 {
            GlobalRank::SeniorDev
        } else if xp_total >= 400 {
            GlobalRank::JuniorDev
        } else if xp_total >= 150 {
            GlobalRank::Apprentice
        } else {
            GlobalRank::Initiate
        }
    }

    /// Gamified display title.
    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            GlobalRank::Pending => "⏳ L'Aspirant",
            GlobalRank::Visitor => "👁️ Observateur des Ombres",
            GlobalRank::Initiate => "🌱 L'Initié",
            GlobalRank::Apprentice => "📖 Apprenti Forgeron",
            GlobalRank::JuniorDev => "⚡ Compagnon de Guilde",
            GlobalRank::SeniorDev => "🔥 Vétéran des Arènes",
            GlobalRank::Expert => "💎 Maître Artisan",
            GlobalRank::Veteran => "🌙 Ancien de la Forge",
            GlobalRank::Legend => "🌟 Légende Vivante",
            GlobalRank::Myth => "👑⚡ Mythe de la Guilde",
        }
    }

    /// Color of the avatar ring in the HUD.
    #[must_use]
    pub const fn ring_color(self) -> &'static str {
        match self {
            GlobalRank::Pending => "#6e6e80",
            GlobalRank::Visitor => "#9aa0b3",
            GlobalRank::Initiate => "#a8e6a3",
            GlobalRank::Apprentice => "#34d058",
            GlobalRank::JuniorDev => "#3fa9ff",
            GlobalRank::SeniorDev => "#00f2ff",
            GlobalRank::Expert => "#9c4dff",
            GlobalRank::Veteran => "#5c1a9c",
            GlobalRank::Legend => "#ffd700",
            GlobalRank::Myth => "#ff003c",
        }
    }

    /// All ranks, lowest first.
    pub const ALL: [GlobalRank; 10] = [
        GlobalRank::Pending,
        GlobalRank::Visitor,
        GlobalRank::Initiate,
        GlobalRank::Apprentice,
        GlobalRank::JuniorDev,
        GlobalRank::SeniorDev,
        GlobalRank::Expert,
        GlobalRank::Veteran,
        GlobalRank::Legend,
        GlobalRank::Myth,
    ];

    /// Parse from the canonical string identifier, falling back to
    /// `Pending` for anything unrecognised. Stored ranks are constrained
    /// by a CHECK, so an unknown value means corrupt data — treating it
    /// as the least-privileged rank fails closed.
    #[must_use]
    pub fn parse(s: &str) -> Self {
        Self::ALL
            .iter()
            .copied()
            .find(|r| r.as_str() == s)
            .unwrap_or(GlobalRank::Pending)
    }

    /// Stable string identifier (matches the value stored in
    /// `users.global_rank`).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        // RANK_THRESHOLDS uses the same names; reusing it would force a
        // runtime lookup. We duplicate here for `const` correctness.
        match self {
            GlobalRank::Pending => "Pending",
            GlobalRank::Visitor => "Visitor",
            GlobalRank::Initiate => "Initiate",
            GlobalRank::Apprentice => "Apprentice",
            GlobalRank::JuniorDev => "JuniorDev",
            GlobalRank::SeniorDev => "SeniorDev",
            GlobalRank::Expert => "Expert",
            GlobalRank::Veteran => "Veteran",
            GlobalRank::Legend => "Legend",
            GlobalRank::Myth => "Myth",
        }
    }
}

// Compile-time sanity: rank constants line up with [`crate::xp::RANK_THRESHOLDS`].
const _: () = {
    let table = RANK_THRESHOLDS;
    assert!(table.len() == 10);
};

// ===========================================================================
// Specializations (badges, not technical roles)
// ===========================================================================

/// Specialization within a track. Each `(Track, &str)` pair maps to one
/// of the canonical specializations listed in the brief.
#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct Specialization {
    /// The track this specialization belongs to.
    pub track: Track,
    /// The specialization label (e.g. `"Gameplay"`).
    pub name: String,
}

/// Canonical specialization list per track. Source of truth for input
/// validation when a user picks a specialization during onboarding.
#[must_use]
pub const fn specializations_for(track: Track) -> &'static [&'static str] {
    match track {
        Track::Engineering => &[
            "Game Engineer",
            "Gameplay",
            "Tools",
            "Engine",
            "DevOps",
            "Networking",
        ],
        Track::GameDesign => &[
            "Game Designer",
            "Level",
            "Systems",
            "UX",
            "Monetization",
            "Puzzle",
        ],
        Track::Narrative => &[
            "Narrative Designer",
            "Writer",
            "World Builder",
            "Quest",
            "Localization",
        ],
        Track::VisualArt => &[
            "2D/Pixel",
            "3D",
            "UI/HUD",
            "VFX",
            "Concept Art",
            "Animation",
            "Technical Art",
        ],
        Track::Audio => &[
            "Composer",
            "Sound Designer",
            "Audio Programmer",
            "Voice",
            "Music Producer",
        ],
        Track::Production => &[
            "Producer",
            "PM",
            "Scrum Master",
            "Product Owner",
            "QA Lead",
        ],
        Track::Qa => &[
            "Functional",
            "Automated",
            "Balance",
            "Accessibility",
            "Performance",
        ],
        Track::Marketing => &[
            "CM",
            "Social Media",
            "Press",
            "Trailer/Video",
            "Copywriter",
            "Data Analyst",
        ],
    }
}

// ===========================================================================
// Special badges
// ===========================================================================

/// Special badge — orthogonal to track / bureau / rank.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::Type))]
#[cfg_attr(feature = "server", sqlx(type_name = "TEXT"))]
#[serde(rename_all = "PascalCase")]
pub enum SpecialBadge {
    /// Founding member of the association.
    FoundingMember,
    /// Former member who has left.
    Alumni,
    /// External professional invited as mentor.
    ExternalMentor,
    /// Game Jam winner.
    GameJamWinner,
    /// Game Jam participant.
    GameJamParticipant,
    /// Reported a confirmed bug on the platform.
    BugHunter,
    /// PR merged on the platform repository.
    Contributor,
    /// 7 consecutive active days.
    Streaker,
    /// Completed a full Block.
    BlockMaster,
    /// 3 or more active tracks simultaneously.
    MultiTracker,
    /// Submitted 10 valid validations.
    Validator,
    /// 5 confirmed mentorings.
    Mentor,
    /// Top 3 XP of the month.
    TopContributor,
    /// Pushed code after midnight.
    NightOwl,
    /// Block completed in under 48 hours.
    SpeedRunner,
}

// ===========================================================================
// Authority and permissions
// ===========================================================================

/// Per-track membership row, summarized.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackMembership {
    /// Track this membership applies to.
    pub track: Track,
    /// Role inside this track.
    pub role: TrackRole,
}

/// All the role information needed to evaluate a permission check.
///
/// This is the typed envelope we pass around in middleware and into
/// handlers. It is built once per request from the database and then
/// consulted via [`Authority::can`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Authority {
    /// Global rank.
    pub rank: GlobalRank,
    /// Bureau role, if any.
    pub bureau: Option<BureauRole>,
    /// Per-track memberships.
    pub tracks: Vec<TrackMembership>,
}

impl Authority {
    /// Convenience: an unauthenticated viewer (no rank, no roles).
    #[must_use]
    pub fn anonymous() -> Self {
        Self {
            rank: GlobalRank::Pending,
            bureau: None,
            tracks: Vec::new(),
        }
    }

    /// Whether the user has at least the given role inside the given
    /// track.
    #[must_use]
    pub fn has_track_role(&self, track: Track, min: TrackRole) -> bool {
        self.tracks
            .iter()
            .any(|m| m.track == track && m.role >= min)
    }

    /// Whether the user is the Lead of any track.
    #[must_use]
    pub fn is_any_track_lead(&self) -> bool {
        self.tracks.iter().any(|m| m.role == TrackRole::Lead)
    }

    /// Whether the user has any executive bureau role.
    #[must_use]
    pub fn is_executive(&self) -> bool {
        self.bureau.is_some_and(BureauRole::is_executive)
    }

    /// Top-level permission check.
    #[must_use]
    // Each Action gets its own arm even when several share an
    // implementation — the permission table is meant to read like a spec,
    // and merging arms loses that property.
    #[allow(clippy::match_same_arms)]
    pub fn can(&self, action: Action) -> bool {
        // Pending users can do effectively nothing.
        if self.rank == GlobalRank::Pending {
            return matches!(action, Action::SubmitEpitechEmail);
        }

        match action {
            Action::ViewPublicProjects => self.rank >= GlobalRank::Visitor,
            Action::ViewTrackInternalProjects(t) => self.has_track_role(t, TrackRole::Observer),
            Action::SubmitEpitechEmail => self.rank == GlobalRank::Pending,
            Action::CompleteOnboarding => self.rank == GlobalRank::Visitor,
            Action::CreateProject => self.rank >= GlobalRank::JuniorDev,
            Action::SubmitProjectForReview => self.rank >= GlobalRank::JuniorDev,
            Action::ReviewProjectForTrack(t) => self.has_track_role(t, TrackRole::Reviewer),
            Action::PublishProjectAsReleased(t) => self.has_track_role(t, TrackRole::Lead),
            Action::AppointTrackCoLead(t) => self.has_track_role(t, TrackRole::Lead),
            Action::AppointTrackLead(_) => self.is_executive(),
            Action::AssignBureauRole => matches!(
                self.bureau,
                Some(BureauRole::President | BureauRole::VicePresident)
            ),
            Action::GenerateQrToken => self.is_event_manager() || self.is_executive(),
            Action::GrantManualXp => self.is_executive(),
            Action::ValidateResource => matches!(
                self.bureau,
                Some(BureauRole::Archiviste | BureauRole::AssistantArchiviste)
            ) || self.is_any_track_lead(),
            Action::ModerateContent => matches!(
                self.bureau,
                Some(BureauRole::Moderator | BureauRole::AssistantModerator)
            ) || self.is_executive(),
            Action::AccessAdminPanel => self.bureau.is_some(),
            Action::ViewAuditLogs => self.is_executive(),
        }
    }

    /// Helper used by [`Self::can`] for QR generation.
    fn is_event_manager(&self) -> bool {
        matches!(
            self.bureau,
            Some(BureauRole::EventManager | BureauRole::AssistantEventManager)
        )
    }
}

/// Discrete actions checked against [`Authority`].
///
/// Keep this list narrow: a permission check is "can the actor do this
/// kind of thing", not "can the actor edit this specific row". Per-row
/// ownership checks (e.g. "can I edit *my own* profile") happen at the
/// handler layer, after the coarse-grained gate has passed.
#[derive(Debug, Clone, Copy)]
pub enum Action {
    /// Read public-facing project pages (Released projects).
    ViewPublicProjects,
    /// Read internal project pages (Draft / InReview / PartialOK) for the
    /// given track.
    ViewTrackInternalProjects(Track),
    /// Submit an `@epitech.eu` email after Discord OAuth.
    SubmitEpitechEmail,
    /// Complete the 3-step onboarding (track choice + profile).
    CompleteOnboarding,
    /// Create a new project in `Draft` status.
    CreateProject,
    /// Move a project from `Draft` to `InReview`.
    SubmitProjectForReview,
    /// Submit an Approve/Reject/NotApplicable verdict for a specific
    /// track on a project.
    ReviewProjectForTrack(Track),
    /// Move a project from `Approved` to `Released`.
    PublishProjectAsReleased(Track),
    /// Appoint a track CoLead.
    AppointTrackCoLead(Track),
    /// Appoint a track Lead (Bureau-only).
    AppointTrackLead(Track),
    /// Assign or revoke a Bureau role.
    AssignBureauRole,
    /// Create QR codes for events.
    GenerateQrToken,
    /// Manually grant XP (audited).
    GrantManualXp,
    /// Validate a community-submitted resource.
    ValidateResource,
    /// Moderate messages, ban from features, etc.
    ModerateContent,
    /// Access the admin panel.
    AccessAdminPanel,
    /// Read the audit log.
    ViewAuditLogs,
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn track_role_ordering() {
        assert!(TrackRole::Lead > TrackRole::Mentor);
        assert!(TrackRole::Mentor > TrackRole::Reviewer);
        assert!(TrackRole::Reviewer > TrackRole::Contributor);
        assert!(TrackRole::Contributor > TrackRole::Observer);
    }

    #[test]
    fn track_role_from_xp() {
        assert_eq!(TrackRole::from_track_xp(0), TrackRole::Observer);
        assert_eq!(TrackRole::from_track_xp(99), TrackRole::Observer);
        assert_eq!(TrackRole::from_track_xp(100), TrackRole::Contributor);
        assert_eq!(TrackRole::from_track_xp(299), TrackRole::Contributor);
        assert_eq!(TrackRole::from_track_xp(300), TrackRole::Reviewer);
        assert_eq!(TrackRole::from_track_xp(499), TrackRole::Reviewer);
        assert_eq!(TrackRole::from_track_xp(500), TrackRole::Mentor);
        assert_eq!(TrackRole::from_track_xp(50_000), TrackRole::Mentor);
    }

    #[test]
    fn global_rank_thresholds() {
        assert_eq!(GlobalRank::from_xp(0), GlobalRank::Initiate);
        assert_eq!(GlobalRank::from_xp(149), GlobalRank::Initiate);
        assert_eq!(GlobalRank::from_xp(150), GlobalRank::Apprentice);
        assert_eq!(GlobalRank::from_xp(400), GlobalRank::JuniorDev);
        assert_eq!(GlobalRank::from_xp(2_500), GlobalRank::Expert);
        assert_eq!(GlobalRank::from_xp(10_000), GlobalRank::Legend);
        assert_eq!(GlobalRank::from_xp(25_000), GlobalRank::Myth);
        assert_eq!(GlobalRank::from_xp(999_999), GlobalRank::Myth);
    }

    #[test]
    fn pending_user_only_submits_email() {
        let authority = Authority::anonymous();
        assert!(authority.can(Action::SubmitEpitechEmail));
        assert!(!authority.can(Action::ViewPublicProjects));
        assert!(!authority.can(Action::CreateProject));
        assert!(!authority.can(Action::AccessAdminPanel));
    }

    #[test]
    fn track_lead_can_publish_only_their_track() {
        let authority = Authority {
            rank: GlobalRank::Legend,
            bureau: None,
            tracks: vec![TrackMembership {
                track: Track::Engineering,
                role: TrackRole::Lead,
            }],
        };
        assert!(authority.can(Action::PublishProjectAsReleased(Track::Engineering)));
        assert!(!authority.can(Action::PublishProjectAsReleased(Track::Audio)));
    }

    #[test]
    fn executive_can_grant_manual_xp() {
        let auth = Authority {
            rank: GlobalRank::Legend,
            bureau: Some(BureauRole::President),
            tracks: vec![],
        };
        assert!(auth.can(Action::GrantManualXp));

        let mod_auth = Authority {
            rank: GlobalRank::Legend,
            bureau: Some(BureauRole::Moderator),
            tracks: vec![],
        };
        assert!(!mod_auth.can(Action::GrantManualXp));
    }

    #[test]
    fn all_tracks_have_specializations() {
        for &t in &Track::ALL {
            assert!(!specializations_for(t).is_empty(), "track {t:?} empty");
        }
    }
}

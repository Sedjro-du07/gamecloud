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
//! 3. **Global rank** — derived from total XP alone, with `Initiate` as
//!    the floor. See [`GlobalRank`]. There used to be an
//!    `email_verified` gate below it, pinning members at `Pending`
//!    until they produced an `@epitech.eu` address; being on the
//!    Discord server is the membership check the association actually
//!    applies, so the gate was removed and `Pending` / `Visitor` are no
//!    longer assigned to anyone.
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

    /// The role in French, as a member reads it on their profile.
    #[must_use]
    pub const fn title_fr(self) -> &'static str {
        match self {
            TrackRole::Observer => "Observateur·ice",
            TrackRole::Contributor => "Contributeur·ice",
            TrackRole::Reviewer => "Relecteur·ice",
            TrackRole::Mentor => "Mentor",
            TrackRole::CoLead => "Co-responsable",
            TrackRole::Lead => "Responsable",
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

    /// The title shown for a stored `track_role` value.
    ///
    /// Progress inside a track is a title, not a number: "Relecteur en
    /// Audio" says what somebody may do there; "niveau 7" says nothing.
    /// Takes the stored string because every caller reads it straight
    /// out of `track_memberships`.
    #[must_use]
    pub fn title_of(stored: &str) -> &'static str {
        match stored {
            "Lead" => "👑 Responsable",
            "CoLead" => "⚔️ Co-responsable",
            "Mentor" => "🧭 Mentor",
            "Reviewer" => "🔍 Relecteur",
            "Contributor" => "🔨 Contributeur",
            _ => "👁️ Observateur",
        }
    }

    /// The next title earned by track XP for a stored `track_role`.
    ///
    /// Returns `(next title, XP the current title starts at, XP the next
    /// one starts at)`, which is exactly what a progress bar needs.
    /// `None` from Mentor up: past Mentor, titles are appointments, not XP.
    #[must_use]
    pub fn next_milestone(stored: &str) -> Option<(&'static str, i64, i64)> {
        match stored {
            "Observer" => Some((Self::title_of("Contributor"), 0, 100)),
            "Contributor" => Some((Self::title_of("Reviewer"), 100, 300)),
            "Reviewer" => Some((Self::title_of("Mentor"), 300, 500)),
            _ => None,
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
    /// Membre du Bureau — a Bureau member whose office is not decided yet.
    ///
    /// It keeps the Bureau's channels open to them and nothing else: no
    /// admin panel, no Bureau meetings, no executive right. It exists so
    /// that assigning offices is not a race against people losing access.
    Provisional,
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
            BureauRole::Provisional => "🏛 Membre du Bureau",
        }
    }

    /// Every bureau role, the provisional one last.
    pub const ALL: [BureauRole; 17] = [
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
        BureauRole::Provisional,
    ];

    /// Position in protocol order — the order [`BureauRole::ALL`] lists
    /// the offices in, President first. Offices are stored sorted by it,
    /// so the first one held is always the principal one.
    #[must_use]
    pub fn protocol_rank(self) -> usize {
        BureauRole::ALL.iter().position(|b| *b == self).unwrap_or(usize::MAX)
    }

    /// What the office *is*, in plain French — the gamified title says
    /// who you are in the guild, this says what you are responsible for.
    /// Shown next to the title wherever somebody is being appointed.
    #[must_use]
    pub const fn plain(self) -> &'static str {
        match self {
            BureauRole::President => "Président·e",
            BureauRole::VicePresident => "Vice-président·e",
            BureauRole::Secretary => "Secrétaire",
            BureauRole::Treasurer => "Trésorier·e",
            BureauRole::VpTech => "Vice-président·e technique",
            BureauRole::VpCommunity => "Vice-président·e communauté",
            BureauRole::EventManager => "Responsable des événements",
            BureauRole::AssistantEventManager => "Adjoint·e aux événements",
            BureauRole::Archiviste => "Archiviste",
            BureauRole::AssistantArchiviste => "Archiviste adjoint·e",
            BureauRole::CommunityManager => "Community manager",
            BureauRole::SocialMediaManager => "Réseaux sociaux",
            BureauRole::Moderator => "Modérateur·ice",
            BureauRole::AssistantModerator => "Modérateur·ice adjoint·e",
            BureauRole::RecruitmentOfficer => "Recrutement",
            BureauRole::PrManager => "Relations publiques",
            BureauRole::Provisional => "Membre du Bureau, sans office",
        }
    }

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
            BureauRole::Provisional => "Provisional",
        }
    }

    /// Whether this is an actual office, as opposed to the provisional
    /// placeholder that only keeps the Bureau's channels open.
    #[must_use]
    pub const fn holds_office(self) -> bool {
        !matches!(self, BureauRole::Provisional)
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

    /// The next rank earned by XP, and the total XP it starts at.
    ///
    /// `None` at the top. The gated ranks (`Pending`, `Visitor`,
    /// `Initiate`) all lead to `Apprentice`: what they wait on is not XP,
    /// but XP is what a progress bar can show.
    #[must_use]
    pub fn next_milestone(self) -> Option<(GlobalRank, i64)> {
        let i = Self::ALL.iter().position(|r| *r == self)?;
        Self::ALL[i + 1..]
            .iter()
            .zip(&RANK_THRESHOLDS[i + 1..])
            .find(|(_, (xp, _))| *xp > 0)
            .map(|(rank, (xp, _))| (*rank, *xp))
    }

    /// Total XP at which this rank starts.
    #[must_use]
    pub fn floor_xp(self) -> i64 {
        Self::ALL
            .iter()
            .position(|r| *r == self)
            .map_or(0, |i| RANK_THRESHOLDS[i].0)
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

#[cfg(test)]
mod provisional_tests {
    use super::BureauRole;

    #[test]
    fn the_provisional_role_round_trips_and_is_not_an_office() {
        assert_eq!(BureauRole::parse("Provisional"), Some(BureauRole::Provisional));
        assert!(!BureauRole::Provisional.holds_office());
        assert!(!BureauRole::Provisional.is_executive());
        assert!(BureauRole::Secretary.holds_office());
    }

    #[test]
    fn office_titles_stay_distinct() {
        // Discord roles are found by title, so two sharing one would be
        // indistinguishable.
        let mut titles: Vec<&str> = BureauRole::ALL.iter().map(|b| b.title()).collect();
        let before = titles.len();
        titles.sort_unstable();
        titles.dedup();
        assert_eq!(titles.len(), before);
    }
}

#[cfg(test)]
mod milestone_tests {
    use super::{GlobalRank, TrackRole};

    #[test]
    fn the_next_title_skips_the_gated_ranks() {
        assert_eq!(
            GlobalRank::Pending.next_milestone(),
            Some((GlobalRank::Apprentice, 150))
        );
        assert_eq!(
            GlobalRank::Apprentice.next_milestone(),
            Some((GlobalRank::JuniorDev, 400))
        );
        assert_eq!(GlobalRank::Myth.next_milestone(), None);
    }

    #[test]
    fn a_rank_starts_at_its_threshold() {
        assert_eq!(GlobalRank::Initiate.floor_xp(), 0);
        assert_eq!(GlobalRank::Expert.floor_xp(), 2_500);
    }

    #[test]
    fn every_stored_track_role_has_a_title() {
        for stored in ["Observer", "Contributor", "Reviewer", "Mentor", "CoLead", "Lead"] {
            assert!(!TrackRole::title_of(stored).is_empty());
        }
        assert_eq!(TrackRole::title_of("Reviewer"), "🔍 Relecteur");
    }

    #[test]
    fn track_milestones_match_the_auto_thresholds() {
        // The bar must agree with `from_track_xp`, or it would promise a
        // title at an XP total that does not grant it.
        for stored in ["Observer", "Contributor", "Reviewer"] {
            let (_, floor, target) = TrackRole::next_milestone(stored).unwrap();
            assert_eq!(TrackRole::from_track_xp(floor).as_str(), stored);
            assert_ne!(TrackRole::from_track_xp(target).as_str(), stored);
        }
        assert_eq!(TrackRole::next_milestone("Mentor"), None);
        assert_eq!(TrackRole::next_milestone("Lead"), None);
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
    /// Bureau offices held, in protocol order. A member may hold several
    /// — the same person keeping the minutes and the accounts is common
    /// in a small association — and every right any of them grants
    /// applies.
    pub offices: Vec<BureauRole>,
    /// Per-track memberships.
    pub tracks: Vec<TrackMembership>,
}

impl Authority {
    /// Convenience: an unauthenticated viewer (no rank, no roles).
    #[must_use]
    pub fn anonymous() -> Self {
        Self {
            rank: GlobalRank::Pending,
            offices: Vec::new(),
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

    /// Whether any office held satisfies `test`.
    #[must_use]
    pub fn holds(&self, test: impl Fn(BureauRole) -> bool) -> bool {
        self.offices.iter().copied().any(test)
    }

    /// The first office held that satisfies `test`, in protocol order —
    /// used to say *which* office grants a right.
    #[must_use]
    pub fn office_where(&self, test: impl Fn(BureauRole) -> bool) -> Option<BureauRole> {
        self.offices.iter().copied().find(|b| test(*b))
    }

    /// Whether the user holds any executive bureau office.
    #[must_use]
    pub fn is_executive(&self) -> bool {
        self.holds(BureauRole::is_executive)
    }

    /// Top-level permission check.
    #[must_use]
    // Each Action gets its own arm even when several share an
    // implementation — the permission table is meant to read like a spec,
    // and merging arms loses that property.
    #[allow(clippy::match_same_arms)]
    pub fn can(&self, action: Action) -> bool {
        // There used to be a `Pending` dead end here: a member without a
        // verified `@epitech.eu` address could do nothing at all but
        // submit one. Getting into the Discord server is the membership
        // check the association actually applies, so the gate is gone
        // and the mail is optional. `Pending` and `Visitor` are no
        // longer assigned to anyone; the variants survive only for
        // historic rows and the Discord role ladder.
        match action {
            Action::ViewPublicProjects => self.rank >= GlobalRank::Visitor,
            Action::ViewTrackInternalProjects(t) => self.has_track_role(t, TrackRole::Observer),
            // Both of these were gates. They are now simply available:
            // recording an address is optional, and picking a track is
            // something a member may do at any point, not a step they
            // must clear once.
            Action::SubmitEpitechEmail | Action::CompleteOnboarding => true,
            Action::CreateProject => self.rank >= GlobalRank::JuniorDev,
            Action::SubmitProjectForReview => self.rank >= GlobalRank::JuniorDev,
            Action::ReviewProjectForTrack(t) => self.has_track_role(t, TrackRole::Reviewer),
            Action::PublishProjectAsReleased(t) => self.has_track_role(t, TrackRole::Lead),
            Action::AppointTrackCoLead(t) => self.has_track_role(t, TrackRole::Lead),
            Action::AppointTrackLead(_) => self.is_executive(),
            Action::AssignBureauRole => self.holds(|b| {
                matches!(b, BureauRole::President | BureauRole::VicePresident)
            }),
            Action::GenerateQrToken => self.is_event_manager() || self.is_executive(),
            // A track Lead owns their track's sessions; anything that
            // concerns the whole association stays with the people whose
            // job it is to run it.
            Action::ManageEvents(scope) => match scope {
                EventScope::Association => self.is_event_manager() || self.is_executive(),
                EventScope::Track(t) => {
                    self.is_event_manager()
                        || self.is_executive()
                        || self.has_track_role(t, TrackRole::Lead)
                }
                // Any office-holder may call a Bureau meeting. The
                // Bureau is small and self-governing; making its own
                // members ask the President to book a room would be
                // ceremony, not control — and nobody outside it can see
                // these events anyway.
                EventScope::Bureau => self.holds(BureauRole::holds_office),
            },
            Action::GrantManualXp => self.is_executive(),
            Action::ValidateResource => {
                self.holds(|b| {
                    matches!(b, BureauRole::Archiviste | BureauRole::AssistantArchiviste)
                }) || self.is_any_track_lead()
            }
            Action::ModerateContent => {
                self.holds(|b| {
                    matches!(b, BureauRole::Moderator | BureauRole::AssistantModerator)
                }) || self.is_executive()
            }
            Action::AccessAdminPanel => self.holds(BureauRole::holds_office),
            Action::ViewAuditLogs => self.is_executive(),
        }
    }

    /// Helper used by [`Self::can`] for QR generation.
    fn is_event_manager(&self) -> bool {
        self.holds(|b| matches!(b, BureauRole::EventManager | BureauRole::AssistantEventManager))
    }
}

/// One thing a member may do, said the way they would say it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Right {
    /// What they may do.
    pub what: String,
    /// Why they may — the rank, office or track role that grants it.
    pub because: String,
}

impl Authority {
    /// Everything this member may do beyond reading, and why.
    ///
    /// Built by asking [`Authority::can`] rather than restating the rules,
    /// so the list a member reads on their profile cannot disagree with
    /// what the platform will actually let them do. An appointment made
    /// a minute ago shows up here on their next page load, because the
    /// authority is rebuilt from the database on every request.
    #[must_use]
    pub fn rights(&self) -> Vec<Right> {
        let mut out = Vec::new();
        let mut push = |what: String, because: String| out.push(Right { what, because });

        if self.can(Action::CreateProject) {
            push(
                "Créer un projet et le soumettre à la relecture".into(),
                format!("ton rang, {}", self.rank.title()),
            );
        }

        for t in Track::ALL {
            let Some(m) = self.tracks.iter().find(|m| m.track == t) else {
                continue;
            };
            let role = m.role.title_fr();
            if self.can(Action::PublishProjectAsReleased(t)) {
                push(
                    format!("Publier les projets de la track {}", t.as_str()),
                    format!("{role} de la track"),
                );
            }
            if self.can(Action::AppointTrackCoLead(t)) {
                push(
                    format!("Nommer les co-responsables de la track {}", t.as_str()),
                    format!("{role} de la track"),
                );
            }
            if self.can(Action::ReviewProjectForTrack(t)) {
                push(
                    format!("Relire et noter les projets pour la track {}", t.as_str()),
                    format!("{role} de la track"),
                );
            }
            if self.has_track_role(t, TrackRole::Lead)
                && self.can(Action::ManageEvents(EventScope::Track(t)))
            {
                push(
                    format!("Programmer les séances de la track {}", t.as_str()),
                    format!("{role} de la track"),
                );
            }
        }

        // Each office-granted right names the office that grants it — the
        // first, in protocol order, that does. With several offices a
        // blanket "ton office" would be wrong half the time: a Secretary
        // who is also Event Manager generates QR codes as Event Manager.
        let named = |b: BureauRole| format!("{} ({})", b.title(), b.plain());
        let by = |test: &dyn Fn(BureauRole) -> bool| {
            self.office_where(test).map_or_else(|| "ton office".to_string(), named)
        };
        let exec = |b: BureauRole| b.is_executive();
        let events = |b: BureauRole| {
            b.is_executive()
                || matches!(b, BureauRole::EventManager | BureauRole::AssistantEventManager)
        };

        if self.can(Action::AccessAdminPanel) {
            push("Ouvrir le panneau du Bureau".into(), by(&BureauRole::holds_office));
        }
        if self.can(Action::ManageEvents(EventScope::Bureau)) {
            push("Convoquer une réunion du Bureau".into(), by(&BureauRole::holds_office));
        }
        if self.can(Action::ManageEvents(EventScope::Association)) {
            push("Programmer les événements de toute l'association".into(), by(&events));
        }
        if self.can(Action::GenerateQrToken) {
            push("Générer les codes QR de présence".into(), by(&events));
        }
        if self.can(Action::GrantManualXp) {
            push("Attribuer ou retirer de l'XP, et ouvrir des quêtes".into(), by(&exec));
        }
        if self.can(Action::AssignBureauRole) {
            push(
                "Nommer aux offices du Bureau".into(),
                by(&|b| matches!(b, BureauRole::President | BureauRole::VicePresident)),
            );
        }
        if self.is_executive() {
            push("Nommer les responsables de track".into(), by(&exec));
        }
        if self.can(Action::ValidateResource) {
            // Two different things grant this: the archivist offices, or
            // leading any track. Naming whichever one actually applies —
            // a track Lead who also sits on the Bureau without an office
            // must not be told their seat is what lets them.
            let archivist = self.office_where(|b| {
                matches!(b, BureauRole::Archiviste | BureauRole::AssistantArchiviste)
            });
            push(
                "Valider les ressources proposées".into(),
                archivist.map_or_else(|| "responsable d'une track".to_string(), named),
            );
        }
        if self.can(Action::ModerateContent) {
            push(
                "Modérer les contenus".into(),
                by(&|b| {
                    b.is_executive()
                        || matches!(b, BureauRole::Moderator | BureauRole::AssistantModerator)
                }),
            );
        }
        if self.can(Action::ViewAuditLogs) {
            push("Consulter le journal d'audit".into(), by(&exec));
        }
        out
    }
}

/// Who an event is for.
///
/// Not `Option<Track>`: "the whole association" and "the Bureau only"
/// are both track-less, but they are opposites — one is the most public
/// thing on the calendar and the other must not appear on an ordinary
/// member's calendar at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventScope {
    /// Everybody in the association.
    Association,
    /// One discipline.
    Track(Track),
    /// The Bureau, and nobody else.
    Bureau,
}

impl EventScope {
    /// Parse the stored `audience` column plus its optional track.
    ///
    /// Mirrors the `events_track_matches_audience` CHECK exactly, in
    /// both directions: a `Track` scope must name a track, and every
    /// other scope must not. Accepting a stray track on a Bureau meeting
    /// would let it be filed under a discipline and surface on that
    /// track's agenda — precisely the leak the column exists to prevent.
    ///
    /// Returns `None` for any combination the schema forbids, so a row
    /// that somehow broke its constraint is refused rather than silently
    /// treated as public.
    #[must_use]
    pub fn parse(audience: &str, track: Option<&str>) -> Option<Self> {
        let named = track.map(str::trim).filter(|t| !t.is_empty());
        match audience {
            "Association" if named.is_none() => Some(Self::Association),
            "Bureau" if named.is_none() => Some(Self::Bureau),
            "Track" => Track::parse(named?).map(Self::Track),
            _ => None,
        }
    }

    /// The `audience` value this scope stores as.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Association => "Association",
            Self::Track(_) => "Track",
            Self::Bureau => "Bureau",
        }
    }

    /// The track, when the scope has one.
    #[must_use]
    pub const fn track(self) -> Option<Track> {
        match self {
            Self::Track(t) => Some(t),
            _ => None,
        }
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
    /// Put an event on the calendar, change it, or call it off.
    ///
    /// Carries the event's scope, because who may touch an event depends
    /// entirely on who it is for.
    ManageEvents(EventScope),
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
    fn a_scope_round_trips_through_its_stored_form() {
        for scope in [
            EventScope::Association,
            EventScope::Bureau,
            EventScope::Track(Track::Audio),
        ] {
            let track = scope.track().map(Track::as_str);
            assert_eq!(EventScope::parse(scope.as_str(), track), Some(scope));
        }
    }

    #[test]
    fn a_scope_refuses_the_combinations_the_schema_refuses() {
        // A track on something that is not a track event, and a track
        // event with no track: both are rows the CHECK constraint would
        // reject, and both must be rejected here too.
        assert_eq!(EventScope::parse("Association", Some("Audio")), None);
        assert_eq!(EventScope::parse("Bureau", Some("Audio")), None);
        assert_eq!(EventScope::parse("Track", None), None);
        assert_eq!(EventScope::parse("Track", Some("Cooking")), None);
        assert_eq!(EventScope::parse("Everyone", None), None);
    }

    #[test]
    fn a_blank_track_counts_as_no_track() {
        // The form submits an empty string rather than omitting the
        // field, so "   " must read as absent and not as a track named
        // three spaces.
        assert_eq!(
            EventScope::parse("Association", Some("   ")),
            Some(EventScope::Association)
        );
        assert_eq!(EventScope::parse("Track", Some("  ")), None);
    }

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
    fn a_treasurer_is_told_what_the_office_opens() {
        let treasurer = Authority {
            rank: GlobalRank::Initiate,
            offices: vec![BureauRole::Treasurer],
            tracks: vec![],
        };
        let rights = treasurer.rights();
        let has = |w: &str| rights.iter().any(|r| r.what.contains(w));
        assert!(has("panneau du Bureau"));
        assert!(has("réunion du Bureau"));
        assert!(has("Attribuer ou retirer de l'XP"));
        // The Treasurer is executive but not President or VP: offices
        // are not theirs to hand out.
        assert!(!has("Nommer aux offices"));
        // And every right names the office that grants it.
        assert!(rights.iter().all(|r| !r.because.is_empty()));
        assert!(rights.iter().any(|r| r.because.contains("Trésorier")));
    }

    #[test]
    fn a_track_lead_is_told_about_their_track_and_nothing_else() {
        let lead = Authority {
            rank: GlobalRank::Initiate,
            offices: vec![],
            tracks: vec![TrackMembership {
                track: Track::VisualArt,
                role: TrackRole::Lead,
            }],
        };
        let rights = lead.rights();
        let has = |w: &str| rights.iter().any(|r| r.what.contains(w));
        assert!(has("Publier les projets de la track VisualArt"));
        assert!(has("Nommer les co-responsables de la track VisualArt"));
        assert!(has("Relire et noter les projets pour la track VisualArt"));
        assert!(has("Programmer les séances de la track VisualArt"));
        assert!(!has("track Audio"));
        assert!(!has("panneau du Bureau"));
    }

    #[test]
    fn a_second_office_adds_its_rights_to_the_first() {
        // An Event Manager may mint QR codes but not moderate; a
        // Moderator may moderate but not mint codes. Holding both must
        // give both — the old single `bureau` field kept only one.
        let both = Authority {
            rank: GlobalRank::Initiate,
            offices: vec![BureauRole::EventManager, BureauRole::Moderator],
            tracks: vec![],
        };
        assert!(both.can(Action::GenerateQrToken));
        assert!(both.can(Action::ModerateContent));

        let only_events = Authority {
            offices: vec![BureauRole::EventManager],
            ..both.clone()
        };
        assert!(!only_events.can(Action::ModerateContent));
    }

    #[test]
    fn with_several_offices_each_right_names_the_one_that_grants_it() {
        let both = Authority {
            rank: GlobalRank::Initiate,
            offices: vec![BureauRole::Archiviste, BureauRole::Moderator],
            tracks: vec![],
        };
        let because = |what: &str| {
            both.rights()
                .into_iter()
                .find(|r| r.what.contains(what))
                .map(|r| r.because)
                .unwrap_or_default()
        };
        assert!(because("ressources").contains("Archiviste"));
        assert!(because("Modérer").contains("Modérateur"));
    }

    #[test]
    fn protocol_order_puts_the_president_first() {
        assert_eq!(BureauRole::President.protocol_rank(), 0);
        assert!(BureauRole::Secretary.protocol_rank() < BureauRole::Treasurer.protocol_rank());
        assert!(BureauRole::Treasurer.protocol_rank() < BureauRole::Provisional.protocol_rank());
    }

    #[test]
    fn each_right_names_what_actually_grants_it() {
        // A track Lead sitting on the Bureau without an office validates
        // resources as a Lead — the seat has nothing to do with it.
        let lead_on_bureau = Authority {
            rank: GlobalRank::Initiate,
            offices: vec![BureauRole::Provisional],
            tracks: vec![TrackMembership {
                track: Track::VisualArt,
                role: TrackRole::Lead,
            }],
        };
        let validate = lead_on_bureau
            .rights()
            .into_iter()
            .find(|r| r.what.contains("ressources"))
            .expect("a Lead validates resources");
        assert_eq!(validate.because, "responsable d'une track");

        let archivist = Authority {
            rank: GlobalRank::Initiate,
            offices: vec![BureauRole::Archiviste],
            tracks: vec![],
        };
        let validate = archivist
            .rights()
            .into_iter()
            .find(|r| r.what.contains("ressources"))
            .expect("the archivist validates resources");
        assert!(validate.because.contains("Archiviste"));
    }

    #[test]
    fn the_rights_list_never_claims_what_can_refuses() {
        // Everybody on the list must be allowed, by construction. This
        // pins it: if somebody edits `rights` to add a line without the
        // matching `can` check, a plain member would start being told
        // they can open the Bureau panel.
        let plain = Authority {
            rank: GlobalRank::Initiate,
            offices: vec![],
            tracks: vec![],
        };
        assert!(plain.rights().is_empty(), "{:?}", plain.rights());
    }

    #[test]
    fn every_office_has_a_plain_name() {
        for b in BureauRole::ALL {
            assert!(!b.plain().is_empty(), "{b:?}");
        }
    }

    #[test]
    fn an_anonymous_visitor_still_holds_nothing() {
        // The rank gate is gone, but that only stopped the mailbox
        // deciding who counts — it did not make everybody an officer.
        // Anonymous means no rank, no track and no office, so the
        // rights that depend on those are all still refused.
        let authority = Authority::anonymous();
        assert!(!authority.can(Action::ViewPublicProjects));
        assert!(!authority.can(Action::CreateProject));
        assert!(!authority.can(Action::AccessAdminPanel));
        assert!(!authority.can(Action::GrantManualXp));
    }

    #[test]
    fn a_member_is_no_longer_held_back_by_an_unverified_mailbox() {
        // The regression this guards: `can` used to return `false` for
        // everything except `SubmitEpitechEmail` whenever the rank was
        // `Pending`, and `Pending` was where every unverified member
        // was pinned. A member's rights now follow their XP, their
        // tracks and their office — never their email.
        let member = Authority {
            rank: GlobalRank::Legend,
            offices: vec![],
            tracks: vec![],
        };
        assert!(member.can(Action::ViewPublicProjects));
        assert!(member.can(Action::CreateProject));
        assert!(member.can(Action::SubmitProjectForReview));
    }

    #[test]
    fn recording_an_address_and_picking_a_track_are_always_open() {
        // Both were one-shot gates tied to `Pending` / `Visitor`. They
        // are now ordinary things a member may do whenever they like.
        for authority in [
            Authority::anonymous(),
            Authority {
                rank: GlobalRank::Initiate,
                offices: vec![],
                tracks: vec![],
            },
        ] {
            assert!(authority.can(Action::SubmitEpitechEmail));
            assert!(authority.can(Action::CompleteOnboarding));
        }
    }

    #[test]
    fn track_lead_can_publish_only_their_track() {
        let authority = Authority {
            rank: GlobalRank::Legend,
            offices: vec![],
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
            offices: vec![BureauRole::President],
            tracks: vec![],
        };
        assert!(auth.can(Action::GrantManualXp));

        let mod_auth = Authority {
            rank: GlobalRank::Legend,
            offices: vec![BureauRole::Moderator],
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

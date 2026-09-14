//! Badge award rules.
//!
//! [`crate::roles::SpecialBadge`] enumerates the badges; this module
//! says *when* each one is earned. Badges split into two families:
//!
//! - **Automatic** — derived from counters the platform already keeps
//!   (streak length, active tracks, jam attendance, review volume).
//!   [`evaluate`] turns a [`BadgeSnapshot`] into the set the member
//!   should hold, and the caller awards whatever is missing. Awarding
//!   is idempotent: `special_badges` has a unique `(user_id,
//!   badge_type)` index, so re-evaluating on every XP event is safe
//!   and cheap.
//!
//! - **Manual** — conferred by the Bureau (`FoundingMember`, `Alumni`,
//!   `ExternalMentor`, `BugHunter`). [`SpecialBadge::is_automatic`]
//!   distinguishes them so the admin endpoint can refuse to hand-award
//!   something the engine owns, and vice versa.

use serde::{Deserialize, Serialize};

use crate::roles::SpecialBadge;

// ---------------------------------------------------------------------------
// Badge metadata
// ---------------------------------------------------------------------------

impl SpecialBadge {
    /// Stable string identifier, as stored in `special_badges.badge_type`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            SpecialBadge::FoundingMember => "FoundingMember",
            SpecialBadge::Alumni => "Alumni",
            SpecialBadge::ExternalMentor => "ExternalMentor",
            SpecialBadge::GameJamWinner => "GameJamWinner",
            SpecialBadge::GameJamParticipant => "GameJamParticipant",
            SpecialBadge::BugHunter => "BugHunter",
            SpecialBadge::Contributor => "Contributor",
            SpecialBadge::Streaker => "Streaker",
            SpecialBadge::BlockMaster => "BlockMaster",
            SpecialBadge::MultiTracker => "MultiTracker",
            SpecialBadge::Validator => "Validator",
            SpecialBadge::Mentor => "Mentor",
            SpecialBadge::TopContributor => "TopContributor",
            SpecialBadge::NightOwl => "NightOwl",
            SpecialBadge::SpeedRunner => "SpeedRunner",
        }
    }

    /// Every badge, in display order.
    pub const ALL: [SpecialBadge; 15] = [
        SpecialBadge::FoundingMember,
        SpecialBadge::Alumni,
        SpecialBadge::ExternalMentor,
        SpecialBadge::GameJamWinner,
        SpecialBadge::GameJamParticipant,
        SpecialBadge::BugHunter,
        SpecialBadge::Contributor,
        SpecialBadge::Streaker,
        SpecialBadge::BlockMaster,
        SpecialBadge::MultiTracker,
        SpecialBadge::Validator,
        SpecialBadge::Mentor,
        SpecialBadge::TopContributor,
        SpecialBadge::NightOwl,
        SpecialBadge::SpeedRunner,
    ];

    /// Parse from the canonical string identifier.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|b| b.as_str() == s)
    }

    /// Gamified display label shown on the character sheet.
    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            SpecialBadge::FoundingMember => "🏛️ Fondateur",
            SpecialBadge::Alumni => "🎓 Ancien",
            SpecialBadge::ExternalMentor => "🧭 Mentor Externe",
            SpecialBadge::GameJamWinner => "🏆 Vainqueur de Jam",
            SpecialBadge::GameJamParticipant => "🎲 Jammeur",
            SpecialBadge::BugHunter => "🐛 Chasseur de Bugs",
            SpecialBadge::Contributor => "🔧 Contributeur",
            SpecialBadge::Streaker => "🔥 Increvable",
            SpecialBadge::BlockMaster => "🧱 Maître de Block",
            SpecialBadge::MultiTracker => "🎯 Touche-à-tout",
            SpecialBadge::Validator => "⚖️ Validateur",
            SpecialBadge::Mentor => "🤲 Mentor",
            SpecialBadge::TopContributor => "👑 Top du Mois",
            SpecialBadge::NightOwl => "🦉 Oiseau de Nuit",
            SpecialBadge::SpeedRunner => "⚡ Speedrunner",
        }
    }

    /// One-line explanation of how the badge is obtained.
    #[must_use]
    pub const fn description(self) -> &'static str {
        match self {
            SpecialBadge::FoundingMember => "Membre fondateur de l'association.",
            SpecialBadge::Alumni => "Ancien membre, parti vers d'autres aventures.",
            SpecialBadge::ExternalMentor => "Professionnel invité comme mentor.",
            SpecialBadge::GameJamWinner => "A remporté une Game Jam.",
            SpecialBadge::GameJamParticipant => "A participé à une Game Jam.",
            SpecialBadge::BugHunter => "A signalé un bug confirmé sur la plateforme.",
            SpecialBadge::Contributor => "A une PR mergée sur le dépôt de la plateforme.",
            SpecialBadge::Streaker => "7 jours d'activité consécutifs.",
            SpecialBadge::BlockMaster => "A terminé un Block complet.",
            SpecialBadge::MultiTracker => "Actif sur 3 tracks ou plus simultanément.",
            SpecialBadge::Validator => "A rendu 10 validations de projet.",
            SpecialBadge::Mentor => "5 mentorats confirmés.",
            SpecialBadge::TopContributor => "Top 3 XP du mois.",
            SpecialBadge::NightOwl => "A poussé du code après minuit.",
            SpecialBadge::SpeedRunner => "Block bouclé en moins de 48 heures.",
        }
    }

    /// Whether the badge engine owns this badge. Manual badges are
    /// conferred by the Bureau through the admin endpoint instead.
    #[must_use]
    pub const fn is_automatic(self) -> bool {
        match self {
            SpecialBadge::Streaker
            | SpecialBadge::MultiTracker
            | SpecialBadge::GameJamParticipant
            | SpecialBadge::GameJamWinner
            | SpecialBadge::Validator
            | SpecialBadge::Mentor
            | SpecialBadge::NightOwl
            | SpecialBadge::TopContributor
            | SpecialBadge::BlockMaster
            | SpecialBadge::SpeedRunner
            | SpecialBadge::Contributor => true,

            SpecialBadge::FoundingMember
            | SpecialBadge::Alumni
            | SpecialBadge::ExternalMentor
            | SpecialBadge::BugHunter => false,
        }
    }
}

// ---------------------------------------------------------------------------
// Thresholds
// ---------------------------------------------------------------------------

/// Consecutive active days required for `Streaker`.
pub const STREAKER_DAYS: i32 = 7;
/// Simultaneously active tracks required for `MultiTracker`.
pub const MULTITRACKER_TRACKS: usize = 3;
/// Validations required for `Validator`.
pub const VALIDATOR_COUNT: i64 = 10;
/// Confirmed mentorings required for `Mentor`.
pub const MENTOR_COUNT: i64 = 5;
/// Hour (UTC, inclusive) from which a push counts as nocturnal.
pub const NIGHT_OWL_FROM_HOUR: u32 = 0;
/// Hour (UTC, exclusive) until which a push counts as nocturnal.
pub const NIGHT_OWL_UNTIL_HOUR: u32 = 5;

// ---------------------------------------------------------------------------
// Evaluation
// ---------------------------------------------------------------------------

/// Everything the badge engine needs to know about one member at one
/// instant. The web crate assembles this with a single aggregate query
/// after each XP grant.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct BadgeSnapshot {
    /// Current consecutive-day streak.
    pub streak_days: i32,
    /// Tracks with activity in the last 30 days.
    pub active_tracks: usize,
    /// Game Jam events attended.
    pub gamejam_attendances: i64,
    /// Game Jam victories recorded.
    pub gamejam_wins: i64,
    /// Project track-validations rendered by this member.
    pub validations_given: i64,
    /// Mentoring XP events credited to this member.
    pub mentorings: i64,
    /// Whether the member has ever been credited XP between midnight
    /// and 05:00 UTC.
    pub has_nocturnal_push: bool,
    /// Whether the member placed top-3 by XP in the current month.
    pub is_month_top3: bool,
    /// Blocks completed (all roadmap milestones of a block done).
    pub blocks_completed: i64,
    /// Whether any completed block was finished within 48 hours.
    pub has_speedrun_block: bool,
    /// Whether the member has a merged PR on the platform repository.
    pub has_platform_pr: bool,
}

/// Return every automatic badge the snapshot qualifies for.
///
/// The caller diffs this against the badges already held and inserts
/// the difference. Nothing here is order-dependent or stateful, which
/// makes the engine safe to run after every single XP event.
#[must_use]
pub fn evaluate(snapshot: &BadgeSnapshot) -> Vec<SpecialBadge> {
    let mut earned = Vec::new();

    if snapshot.streak_days >= STREAKER_DAYS {
        earned.push(SpecialBadge::Streaker);
    }
    if snapshot.active_tracks >= MULTITRACKER_TRACKS {
        earned.push(SpecialBadge::MultiTracker);
    }
    if snapshot.gamejam_attendances > 0 {
        earned.push(SpecialBadge::GameJamParticipant);
    }
    if snapshot.gamejam_wins > 0 {
        earned.push(SpecialBadge::GameJamWinner);
    }
    if snapshot.validations_given >= VALIDATOR_COUNT {
        earned.push(SpecialBadge::Validator);
    }
    if snapshot.mentorings >= MENTOR_COUNT {
        earned.push(SpecialBadge::Mentor);
    }
    if snapshot.has_nocturnal_push {
        earned.push(SpecialBadge::NightOwl);
    }
    if snapshot.is_month_top3 {
        earned.push(SpecialBadge::TopContributor);
    }
    if snapshot.blocks_completed > 0 {
        earned.push(SpecialBadge::BlockMaster);
    }
    if snapshot.has_speedrun_block {
        earned.push(SpecialBadge::SpeedRunner);
    }
    if snapshot.has_platform_pr {
        earned.push(SpecialBadge::Contributor);
    }

    earned
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_snapshot_earns_nothing() {
        assert!(evaluate(&BadgeSnapshot::default()).is_empty());
    }

    #[test]
    fn streak_threshold_is_inclusive() {
        let below = BadgeSnapshot {
            streak_days: STREAKER_DAYS - 1,
            ..Default::default()
        };
        let at = BadgeSnapshot {
            streak_days: STREAKER_DAYS,
            ..Default::default()
        };
        assert!(!evaluate(&below).contains(&SpecialBadge::Streaker));
        assert!(evaluate(&at).contains(&SpecialBadge::Streaker));
    }

    #[test]
    fn jam_win_implies_both_jam_badges() {
        let snap = BadgeSnapshot {
            gamejam_attendances: 1,
            gamejam_wins: 1,
            ..Default::default()
        };
        let earned = evaluate(&snap);
        assert!(earned.contains(&SpecialBadge::GameJamParticipant));
        assert!(earned.contains(&SpecialBadge::GameJamWinner));
    }

    #[test]
    fn every_badge_round_trips_through_parse() {
        for badge in SpecialBadge::ALL {
            assert_eq!(SpecialBadge::parse(badge.as_str()), Some(badge));
        }
    }

    #[test]
    fn manual_badges_are_never_emitted_by_the_engine() {
        // A maximal snapshot must still not confer a Bureau-only badge.
        let snap = BadgeSnapshot {
            streak_days: 999,
            active_tracks: 8,
            gamejam_attendances: 9,
            gamejam_wins: 9,
            validations_given: 999,
            mentorings: 999,
            has_nocturnal_push: true,
            is_month_top3: true,
            blocks_completed: 8,
            has_speedrun_block: true,
            has_platform_pr: true,
        };
        for badge in evaluate(&snap) {
            assert!(
                badge.is_automatic(),
                "engine emitted manual badge {badge:?}"
            );
        }
    }
}

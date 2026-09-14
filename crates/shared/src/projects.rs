//! Project lifecycle.
//!
//! A project is the unit of work the association actually ships, and
//! its review pipeline is the part of GameCloud OS that makes members
//! better rather than merely measuring them: a build is judged by the
//! specialists of every track it touches, and a rejection is required
//! to carry written feedback.
//!
//! ```text
//!            submit            all verdicts in
//!   Draft ──────────► InReview ───────────────┬──► Approved ──► Released ──► Archived
//!     ▲                  │                    │
//!     │                  │                    ├──► PartialOK ──┐
//!     └──── revise ──────┴────────────────────┴──► Rejected ◄──┘
//! ```
//!
//! The status is stored as a string constrained by a CHECK; this module
//! is the single place that decides what may follow what.

use serde::{Deserialize, Serialize};

use crate::errors::{DomainError, DomainResult};

// ---------------------------------------------------------------------------
// Status
// ---------------------------------------------------------------------------

/// Where a project sits in its lifecycle.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::Type))]
#[cfg_attr(feature = "server", sqlx(type_name = "TEXT"))]
#[serde(rename_all = "PascalCase")]
pub enum ProjectStatus {
    /// Being worked on; not yet visible outside the team.
    Draft,
    /// Submitted; every concerned track owes a verdict.
    InReview,
    /// Some tracks approved, others have not — the project needs work
    /// in a specific discipline before it can advance.
    PartialOk,
    /// Every concerned track approved.
    Approved,
    /// Published. Appears in the Hall of Fame and pays out XP.
    Released,
    /// Retired from the active listing but kept in the record.
    Archived,
    /// Turned down. Carries feedback explaining why.
    Rejected,
}

impl ProjectStatus {
    /// Every status.
    pub const ALL: [ProjectStatus; 7] = [
        ProjectStatus::Draft,
        ProjectStatus::InReview,
        ProjectStatus::PartialOk,
        ProjectStatus::Approved,
        ProjectStatus::Released,
        ProjectStatus::Archived,
        ProjectStatus::Rejected,
    ];

    /// Stable string identifier, as stored in `projects.status`.
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

    /// Parse from the canonical string identifier.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|st| st.as_str() == s)
    }

    /// Whether the project is visible to members outside its team.
    #[must_use]
    pub const fn is_public(self) -> bool {
        matches!(self, ProjectStatus::Released | ProjectStatus::Archived)
    }

    /// Whether the project is still open to edits by its team.
    #[must_use]
    pub const fn is_editable(self) -> bool {
        matches!(
            self,
            ProjectStatus::Draft | ProjectStatus::PartialOk | ProjectStatus::Rejected
        )
    }

    /// Statuses reachable from this one.
    #[must_use]
    pub fn allowed_next(self) -> &'static [ProjectStatus] {
        match self {
            ProjectStatus::Draft => &[ProjectStatus::InReview, ProjectStatus::Archived],
            // A review round ends in exactly one of three verdicts.
            ProjectStatus::InReview => &[
                ProjectStatus::Approved,
                ProjectStatus::PartialOk,
                ProjectStatus::Rejected,
            ],
            // Both of these bounce back for another round once the team
            // has addressed the feedback.
            ProjectStatus::PartialOk | ProjectStatus::Rejected => {
                &[ProjectStatus::InReview, ProjectStatus::Archived]
            }
            ProjectStatus::Approved => &[ProjectStatus::Released, ProjectStatus::Archived],
            ProjectStatus::Released => &[ProjectStatus::Archived],
            // Archival is terminal: the record stops changing.
            ProjectStatus::Archived => &[],
        }
    }

    /// Check a transition.
    ///
    /// # Errors
    /// [`DomainError::InvalidProjectTransition`] when `to` is not
    /// reachable from `self`.
    pub fn transition_to(self, to: ProjectStatus) -> DomainResult<ProjectStatus> {
        if self.allowed_next().contains(&to) {
            Ok(to)
        } else {
            Err(DomainError::InvalidProjectTransition {
                from: self.as_str(),
                to: to.as_str(),
            })
        }
    }
}

// ---------------------------------------------------------------------------
// Verdicts
// ---------------------------------------------------------------------------

/// One track's verdict on a project.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum Verdict {
    /// No verdict yet.
    Pending,
    /// This track is satisfied.
    Approved,
    /// This track is not satisfied. Requires feedback.
    Rejected,
    /// This track has nothing to judge here (a text adventure has no
    /// Audio deliverable, say).
    NotApplicable,
}

impl Verdict {
    /// Every verdict.
    pub const ALL: [Verdict; 4] = [
        Verdict::Pending,
        Verdict::Approved,
        Verdict::Rejected,
        Verdict::NotApplicable,
    ];

    /// Stable string identifier, as stored in `track_validations.status`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Verdict::Pending => "Pending",
            Verdict::Approved => "Approved",
            Verdict::Rejected => "Rejected",
            Verdict::NotApplicable => "NotApplicable",
        }
    }

    /// Parse from the canonical string identifier.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|v| v.as_str() == s)
    }

    /// Whether this verdict requires written feedback. The database
    /// enforces the same rule with a CHECK constraint.
    #[must_use]
    pub const fn requires_feedback(self) -> bool {
        matches!(self, Verdict::Rejected)
    }
}

/// Fold every track's verdict into the project's next status.
///
/// The rules, in priority order:
///
/// 1. any `Rejected` → `Rejected` (one blocking discipline is enough);
/// 2. any `Pending` → `InReview` (the round is not over);
/// 3. at least one `Approved` → `Approved`;
/// 4. everything `NotApplicable` → `PartialOk`.
///
/// Rule 4 is the interesting one: a project nobody had anything to
/// judge on has not actually been reviewed, so it must not sail through
/// to `Approved`. It goes to `PartialOk`, which sends it back for a
/// round with the right tracks attached.
#[must_use]
pub fn aggregate(verdicts: &[Verdict]) -> ProjectStatus {
    if verdicts.is_empty() {
        return ProjectStatus::InReview;
    }
    if verdicts.contains(&Verdict::Rejected) {
        return ProjectStatus::Rejected;
    }
    if verdicts.contains(&Verdict::Pending) {
        return ProjectStatus::InReview;
    }
    if verdicts.contains(&Verdict::Approved) {
        return ProjectStatus::Approved;
    }
    ProjectStatus::PartialOk
}

// ---------------------------------------------------------------------------
// Rarity
// ---------------------------------------------------------------------------

/// Cosmetic tier awarded to a released project.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum Rarity {
    /// Default.
    Common,
    /// Two tracks approved.
    Rare,
    /// Three tracks approved.
    Epic,
    /// Four tracks approved.
    Legendary,
    /// Five or more tracks approved.
    Mythic,
}

impl Rarity {
    /// Stable string identifier, as stored in `projects.rarity`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Rarity::Common => "Common",
            Rarity::Rare => "Rare",
            Rarity::Epic => "Epic",
            Rarity::Legendary => "Legendary",
            Rarity::Mythic => "Mythic",
        }
    }

    /// Colour used for the card border in the Hall of Fame.
    #[must_use]
    pub const fn color_hex(self) -> &'static str {
        match self {
            Rarity::Common => "#9aa0b3",
            Rarity::Rare => "#3fa9ff",
            Rarity::Epic => "#9c4dff",
            Rarity::Legendary => "#ffd700",
            Rarity::Mythic => "#ff003c",
        }
    }

    /// Derive rarity from how many tracks signed off.
    ///
    /// A project that satisfied five disciplines is a genuinely harder
    /// thing than one that satisfied one, and this is where that shows.
    #[must_use]
    pub const fn from_approved_tracks(approved: usize) -> Self {
        match approved {
            0 | 1 => Rarity::Common,
            2 => Rarity::Rare,
            3 => Rarity::Epic,
            4 => Rarity::Legendary,
            _ => Rarity::Mythic,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statuses_round_trip() {
        for status in ProjectStatus::ALL {
            assert_eq!(ProjectStatus::parse(status.as_str()), Some(status));
        }
    }

    #[test]
    fn verdicts_round_trip() {
        for verdict in Verdict::ALL {
            assert_eq!(Verdict::parse(verdict.as_str()), Some(verdict));
        }
    }

    #[test]
    fn the_happy_path_is_walkable() {
        let s = ProjectStatus::Draft;
        let s = s.transition_to(ProjectStatus::InReview).unwrap();
        let s = s.transition_to(ProjectStatus::Approved).unwrap();
        let s = s.transition_to(ProjectStatus::Released).unwrap();
        let s = s.transition_to(ProjectStatus::Archived).unwrap();
        assert_eq!(s, ProjectStatus::Archived);
    }

    #[test]
    fn a_draft_cannot_be_released_directly() {
        assert!(ProjectStatus::Draft
            .transition_to(ProjectStatus::Released)
            .is_err());
    }

    #[test]
    fn archived_is_terminal() {
        assert!(ProjectStatus::Archived.allowed_next().is_empty());
        for target in ProjectStatus::ALL {
            assert!(ProjectStatus::Archived.transition_to(target).is_err());
        }
    }

    #[test]
    fn rejected_and_partial_can_go_back_for_another_round() {
        assert!(ProjectStatus::Rejected
            .transition_to(ProjectStatus::InReview)
            .is_ok());
        assert!(ProjectStatus::PartialOk
            .transition_to(ProjectStatus::InReview)
            .is_ok());
    }

    #[test]
    fn a_released_project_cannot_be_un_released() {
        assert!(ProjectStatus::Released
            .transition_to(ProjectStatus::InReview)
            .is_err());
        assert!(ProjectStatus::Released
            .transition_to(ProjectStatus::Draft)
            .is_err());
    }

    #[test]
    fn one_rejection_sinks_the_whole_project() {
        let verdicts = [Verdict::Approved, Verdict::Approved, Verdict::Rejected];
        assert_eq!(aggregate(&verdicts), ProjectStatus::Rejected);
    }

    #[test]
    fn a_pending_track_keeps_the_round_open() {
        let verdicts = [Verdict::Approved, Verdict::Pending];
        assert_eq!(aggregate(&verdicts), ProjectStatus::InReview);
    }

    #[test]
    fn rejection_outranks_pending() {
        // A blocking verdict is actionable now; there is no point making
        // the team wait for the remaining tracks to weigh in.
        let verdicts = [Verdict::Pending, Verdict::Rejected];
        assert_eq!(aggregate(&verdicts), ProjectStatus::Rejected);
    }

    #[test]
    fn all_approved_is_approved() {
        let verdicts = [Verdict::Approved, Verdict::NotApplicable];
        assert_eq!(aggregate(&verdicts), ProjectStatus::Approved);
    }

    #[test]
    fn nothing_but_not_applicable_is_not_an_approval() {
        let verdicts = [Verdict::NotApplicable, Verdict::NotApplicable];
        assert_eq!(aggregate(&verdicts), ProjectStatus::PartialOk);
    }

    #[test]
    fn no_verdicts_at_all_means_the_review_has_not_happened() {
        assert_eq!(aggregate(&[]), ProjectStatus::InReview);
    }

    #[test]
    fn only_rejection_demands_feedback() {
        assert!(Verdict::Rejected.requires_feedback());
        for v in [Verdict::Pending, Verdict::Approved, Verdict::NotApplicable] {
            assert!(!v.requires_feedback());
        }
    }

    #[test]
    fn rarity_climbs_with_breadth() {
        assert_eq!(Rarity::from_approved_tracks(1), Rarity::Common);
        assert_eq!(Rarity::from_approved_tracks(2), Rarity::Rare);
        assert_eq!(Rarity::from_approved_tracks(3), Rarity::Epic);
        assert_eq!(Rarity::from_approved_tracks(4), Rarity::Legendary);
        assert_eq!(Rarity::from_approved_tracks(8), Rarity::Mythic);
    }

    #[test]
    fn only_published_statuses_are_public() {
        assert!(ProjectStatus::Released.is_public());
        assert!(ProjectStatus::Archived.is_public());
        assert!(!ProjectStatus::Draft.is_public());
        assert!(!ProjectStatus::InReview.is_public());
    }

    #[test]
    fn a_project_under_review_is_frozen() {
        assert!(!ProjectStatus::InReview.is_editable());
        assert!(ProjectStatus::Draft.is_editable());
        assert!(ProjectStatus::Rejected.is_editable());
    }
}

//! Shared domain errors.
//!
//! These errors describe *business-rule violations* — never transport,
//! database, or HTTP errors. Web and bot crates wrap [`DomainError`] in
//! their own infrastructure error types.

use thiserror::Error;

/// Result alias for fallible domain operations.
pub type DomainResult<T> = Result<T, DomainError>;

/// Top-level domain error type.
#[derive(Debug, Error)]
pub enum DomainError {
    /// The action is for members of the association — accounts on its
    /// Discord server, holding an office, or admitted by the Bureau.
    #[error("réservé aux membres de l'association")]
    NotAMember,

    /// The actor lacks the permissions for the requested action.
    #[error("permission denied: {0}")]
    Forbidden(&'static str),

    /// A track-related operation references an unknown track.
    #[error("unknown track: {0}")]
    UnknownTrack(String),

    /// A specialization name is not part of the canonical list for its track.
    #[error("unknown specialization '{spec}' for track {track}")]
    UnknownSpecialization {
        /// The track this specialization should belong to.
        track: String,
        /// The specialization that could not be matched.
        spec: String,
    },

    /// XP cap reached for a given source on a given day.
    #[error("daily XP cap reached for source {0}")]
    DailyXpCapReached(&'static str),

    /// A QR token has expired or already been consumed.
    #[error("QR token is no longer valid")]
    InvalidQrToken,

    /// A project transition is not allowed from the current state.
    #[error("invalid project transition: {from} -> {to}")]
    InvalidProjectTransition {
        /// Current status.
        from: &'static str,
        /// Requested status.
        to: &'static str,
    },

    /// The member already claimed this QR token.
    #[error("you have already scanned this code")]
    QrAlreadyClaimed,

    /// The QR token hit its attendance ceiling.
    #[error("this code has reached its scan limit")]
    QrCapacityReached,

    /// A quest was acted on outside its open window.
    #[error("this quest is not currently active")]
    QuestNotActive,

    /// A member tried to vote twice on the same resource.
    #[error("you have already voted for this resource")]
    AlreadyVoted,

    /// A member tried to join a track they already belong to.
    #[error("you already belong to this track")]
    AlreadyInTrack,

    /// Generic invariant violation; prefer a specific variant when possible.
    #[error("invariant violated: {0}")]
    Invariant(&'static str),
}

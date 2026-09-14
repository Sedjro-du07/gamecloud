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
    /// The provided email address does not match the `@epitech.eu` policy.
    #[error("invalid Epitech email: {0}")]
    InvalidEpitechEmail(String),

    /// An action requires a verified email but the user has not completed
    /// OTP verification.
    #[error("email verification required")]
    EmailNotVerified,

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

    /// Generic invariant violation; prefer a specific variant when possible.
    #[error("invariant violated: {0}")]
    Invariant(&'static str),
}

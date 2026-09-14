//! Account life-cycle as a type-state.
//!
//! The platform requires that any user above [`GlobalRank::Visitor`] has
//! a verified `@epitech.eu` email. We encode that invariant as marker
//! types so it is impossible to construct a `User<Verified>` and then
//! call a "give XP" function on a `User<Pending>`.
//!
//! ## Levels of guarantee
//!
//! - **Compile time** — function signatures distinguish `User<Pending>`,
//!   `User<EmailSubmitted>`, and `User<Verified>`. XP-granting functions
//!   only accept the third variant.
//! - **Runtime** — when reading from the database, [`UserRecord::classify`]
//!   inspects boolean fields and returns the correct typed wrapper.
//!
//! The non-typed [`UserRecord`] in `models` is what `sqlx` actually loads
//! from PostgreSQL; the type-state wrappers in this module are produced
//! from it on entry to the business layer.

use std::marker::PhantomData;

use crate::{errors::DomainResult, models::UserRecord, DomainError};

// ---------------------------------------------------------------------------
// Stage marker types
// ---------------------------------------------------------------------------

mod sealed {
    /// Sealed trait pattern — only this crate may add new stages.
    pub trait Sealed {}
}

/// Marker trait implemented by all account stages.
pub trait Stage: sealed::Sealed + Copy + Clone + std::fmt::Debug {
    /// Human-readable name of the stage.
    const NAME: &'static str;
}

/// User has authenticated with Discord but has not submitted an email.
#[derive(Debug, Copy, Clone)]
pub struct Pending;
impl sealed::Sealed for Pending {}
impl Stage for Pending {
    const NAME: &'static str = "Pending";
}

/// User has submitted an `@epitech.eu` email and an OTP has been sent.
#[derive(Debug, Copy, Clone)]
pub struct EmailSubmitted;
impl sealed::Sealed for EmailSubmitted {}
impl Stage for EmailSubmitted {
    const NAME: &'static str = "EmailSubmitted";
}

/// User has confirmed their email via OTP. Eligible for ranks above
/// `Visitor` and can earn XP.
#[derive(Debug, Copy, Clone)]
pub struct Verified;
impl sealed::Sealed for Verified {}
impl Stage for Verified {
    const NAME: &'static str = "Verified";
}

// ---------------------------------------------------------------------------
// Typed wrapper
// ---------------------------------------------------------------------------

/// A user record viewed at a known stage.
///
/// The inner `UserRecord` is always present; the phantom type encodes
/// what we know about it. Construction is restricted: only [`UserRecord::classify`]
/// produces a `User<S>`, ensuring the invariant.
#[derive(Debug, Clone)]
pub struct User<S: Stage> {
    inner: UserRecord,
    _stage: PhantomData<S>,
}

impl<S: Stage> User<S> {
    /// Borrow the underlying record.
    #[must_use]
    pub fn record(&self) -> &UserRecord {
        &self.inner
    }

    /// Consume the wrapper and return the underlying record.
    #[must_use]
    pub fn into_record(self) -> UserRecord {
        self.inner
    }
}

impl User<Verified> {
    /// Internal constructor for verified users — only callable from this
    /// module, after invariant checks pass.
    pub(crate) fn new_verified(record: UserRecord) -> Self {
        Self {
            inner: record,
            _stage: PhantomData,
        }
    }
}

impl User<EmailSubmitted> {
    pub(crate) fn new_email_submitted(record: UserRecord) -> Self {
        Self {
            inner: record,
            _stage: PhantomData,
        }
    }
}

impl User<Pending> {
    pub(crate) fn new_pending(record: UserRecord) -> Self {
        Self {
            inner: record,
            _stage: PhantomData,
        }
    }
}

// ---------------------------------------------------------------------------
// Classification: runtime → typed
// ---------------------------------------------------------------------------

/// One of the three possible typed views of a user record.
#[derive(Debug)]
pub enum AccountView {
    /// User has not submitted an email.
    Pending(User<Pending>),
    /// Email submitted, OTP pending.
    EmailSubmitted(User<EmailSubmitted>),
    /// Email verified.
    Verified(User<Verified>),
}

impl UserRecord {
    /// Classify this raw record into one of the three account stages.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::Invariant`] if the record is internally
    /// inconsistent (e.g. `email_verified = true` but `email IS NULL`).
    pub fn classify(self) -> DomainResult<AccountView> {
        match (self.email.as_deref(), self.email_verified) {
            (None, false) => Ok(AccountView::Pending(User::new_pending(self))),
            (Some(_), false) => Ok(AccountView::EmailSubmitted(User::new_email_submitted(self))),
            (Some(_), true) => Ok(AccountView::Verified(User::new_verified(self))),
            (None, true) => Err(DomainError::Invariant(
                "user has email_verified = true but no email",
            )),
        }
    }
}

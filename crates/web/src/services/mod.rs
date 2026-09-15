//! Application services — cross-cutting helpers used by handlers.
//!
//! Each module owns a single concern. They never read environment
//! variables directly; the `Config` is passed in.

pub mod email_validator;
pub mod github;
pub mod jwt;
pub mod mailer;
pub mod notifications;
pub mod otp;
pub mod password;
pub mod tokens;
pub mod uploads;

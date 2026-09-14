//! Application services — cross-cutting helpers used by handlers.
//!
//! Each module owns a single concern. They never read environment
//! variables directly; the `Config` is passed in.

pub mod email_validator;
pub mod jwt;
pub mod mailer;
pub mod otp;
pub mod password;
pub mod tokens;

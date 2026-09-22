//! Application services — cross-cutting helpers used by handlers.
//!
//! Each module owns a single concern. They never read environment
//! variables directly; the `Config` is passed in.

pub mod discord;
pub mod github;
pub mod jwt;
pub mod notifications;
pub mod tokens;
pub mod uploads;

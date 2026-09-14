//! # GameCloud Shared
//!
//! Domain types, database models, role and permission logic, and XP
//! constants shared between the Axum/Leptos web crate and the Serenity bot
//! crate.
//!
//! ## Feature flags
//!
//! - `server` — pulls in `sqlx` derives so models can be queried from a
//!   PostgreSQL pool. The WASM frontend builds without this feature.
//!
//! ## Design principles
//!
//! 1. **Make invalid states unrepresentable.** Account stages, validation
//!    statuses, and rank progression are encoded as enums and (where
//!    practical) phantom-typed wrappers. See [`account`].
//! 2. **One source of truth.** XP source values, rank thresholds, and role
//!    titles are defined once here and consumed by both the web crate and
//!    the bot. See [`xp`] and [`roles`].
//! 3. **Frontend-friendly.** Every public type that crosses the SSR/CSR
//!    boundary derives `Serialize` + `Deserialize` and avoids server-only
//!    fields.

#![warn(missing_docs)]
#![warn(clippy::pedantic)]
// `module_name_repetitions` is a stylistic nit that fights our naming scheme
// (XRecord / XSummary inside `models`).
#![allow(clippy::module_name_repetitions)]
// `doc_markdown` flags every CamelCase domain term ("DraftBot", "TrackLead") in
// doc comments. Our domain vocabulary is full of these; turning them on would
// drown real warnings.
#![allow(clippy::doc_markdown)]
// `cast_possible_truncation` triggers on the controlled f64→i32 in xp.rs;
// we opt-in case-by-case rather than at module level.
#![allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]

pub mod account;
pub mod badges;
pub mod errors;
pub mod models;
pub mod projects;
pub mod roles;
pub mod xp;

pub use errors::{DomainError, DomainResult};

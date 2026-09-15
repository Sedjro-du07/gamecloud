//! Axum route handlers.
//!
//! Each module here exposes a `router()` function that returns a
//! `axum::Router<AppState>`. The top-level `router::build` function in
//! `lib.rs` composes them all under the right paths.

pub mod admin;
pub mod auth;
pub mod contact;
pub mod entrance;
pub mod health;
pub mod projects;
pub mod qr;
pub mod quests;
pub mod resources;
pub mod seasons;
pub mod shares;
pub mod users;
pub mod webhooks;

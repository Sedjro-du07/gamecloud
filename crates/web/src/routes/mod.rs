//! Axum route handlers.
//!
//! Each module here exposes a `router()` function that returns a
//! `axum::Router<AppState>`. The top-level `router::build` function in
//! `lib.rs` composes them all under the right paths.

pub mod auth;
pub mod health;
pub mod qr;
pub mod webhooks;

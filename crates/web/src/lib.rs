//! GameCloud OS web crate.
//!
//! Hybrid SSR / hydration crate built by `cargo-leptos`. The native
//! server target compiles with the `ssr` feature and pulls in Axum,
//! sqlx, OAuth, JWT, and SMTP support. The WASM client target
//! compiles with the `hydrate` feature and stays pure-frontend.

#![warn(missing_docs)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions, clippy::doc_markdown)]
#![allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
// SQL queries use `r#"…"#` by convention even when no `"` is present,
// because the hash form is what we use everywhere and clippy's "you
// don't need them here" advice is busywork.
#![allow(clippy::needless_raw_string_hashes)]
// Leptos `#[component]` generates `pub fn` items that don't have an
// observable return value to "use" — they are mounted by the framework.
// `must_use_candidate` fires on every component, which is noise.
#![allow(clippy::must_use_candidate)]

// --- Frontend (always compiled; both ssr and hydrate use them) -----
pub mod api;
pub mod app;
pub mod components;
pub mod pages;

// --- Server-only ----------------------------------------------------
#[cfg(feature = "ssr")]
pub mod config;
#[cfg(feature = "ssr")]
pub mod db;
#[cfg(feature = "ssr")]
pub mod error;
#[cfg(feature = "ssr")]
pub mod middleware;
#[cfg(feature = "ssr")]
pub mod router;
#[cfg(feature = "ssr")]
pub mod routes;
#[cfg(feature = "ssr")]
pub mod services;
#[cfg(feature = "ssr")]
pub mod state;

/// WASM hydrate entry-point — invoked by `gamecloud.js` injected by
/// `cargo-leptos`. Mounts the Leptos `App` to the existing SSRed DOM.
#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    console_error_panic_hook::set_once();
    leptos::mount::hydrate_body(crate::app::App);
}

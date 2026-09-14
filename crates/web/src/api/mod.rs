//! Browser-side HTTP API client.
//!
//! These thin wrappers around `fetch` (in WASM) or `reqwest` (in SSR)
//! call the server's REST endpoints documented in `API_REFERENCE.md`.
//! All functions return a `Result<T, ApiError>` with serializable
//! payloads so they can be invoked from `leptos::Resource`.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// API error.
#[derive(Debug, Clone, Error, Serialize, Deserialize)]
#[error("{message}")]
pub struct ApiError {
    /// Machine-readable error code (matches `WebError::code()`).
    pub code: String,
    /// Human-readable message.
    pub message: String,
}

/// Error response envelope, mirrors the server's JSON shape. Reserved
/// for use by the WASM `fetch` wrappers that will land in phase 5.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub(crate) struct ErrorEnvelope {
    pub(crate) error: ApiError,
}

/// Public profile shape returned by `/api/auth/me`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MeUser {
    /// User UUID, encoded as string for stable JSON shape.
    pub id: String,
    /// Discord ID.
    pub discord_id: String,
    /// GitHub username, if linked.
    pub github_username: Option<String>,
    /// Verified Epitech email, if any.
    pub email: Option<String>,
    /// Whether the email is verified.
    pub email_verified: bool,
    /// Avatar URL.
    pub avatar_url: Option<String>,
    /// Total XP.
    pub xp_total: i64,
    /// Level.
    pub level: i32,
    /// Global rank string (matches `GlobalRank::as_str`).
    pub global_rank: String,
    /// Bureau role string, if any.
    pub bureau_role: Option<String>,
    /// Display title.
    pub current_title: Option<String>,
    /// Streak in days.
    pub streak_days: i32,
}

/// Response body for `/api/auth/me`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeResponse {
    /// The current user.
    pub user: MeUser,
}

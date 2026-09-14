//! Web crate error type.
//!
//! Wraps domain errors, sqlx errors, and miscellaneous infrastructure
//! failures, then converts them into a uniform JSON HTTP response. The
//! conversion never leaks internal details (sql query text, panics,
//! etc.) to the client; it logs them at WARN/ERROR and returns a
//! generic message.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use gamecloud_shared::DomainError;
use serde::Serialize;
use thiserror::Error;

/// Web-layer result alias.
pub type WebResult<T> = Result<T, WebError>;

/// Application-wide error.
#[derive(Debug, Error)]
pub enum WebError {
    /// Domain-rule violation.
    #[error(transparent)]
    Domain(#[from] DomainError),

    /// 401 — credentials missing or invalid.
    #[error("unauthorized")]
    Unauthorized,

    /// 403 — credentials valid but insufficient.
    #[error("forbidden")]
    Forbidden,

    /// 404 — resource not found.
    #[error("not found")]
    NotFound,

    /// 409 — write conflict.
    #[error("conflict: {0}")]
    Conflict(&'static str),

    /// 422 — request body / query is structurally OK but semantically
    /// invalid.
    #[error("validation error: {0}")]
    Validation(String),

    /// 429 — rate limited.
    #[error("rate limited")]
    RateLimited,

    /// 502 — upstream error (Discord, GitHub, SMTP).
    #[error("upstream error: {0}")]
    Upstream(String),

    /// 500 — database error. Mapped to a generic message.
    #[error("database error")]
    Database(#[from] sqlx::Error),

    /// 500 — anything else.
    #[error("internal error")]
    Internal(#[from] anyhow::Error),
}

#[derive(Serialize)]
struct ErrorBody {
    error: ErrorBodyInner,
}

#[derive(Serialize)]
struct ErrorBodyInner {
    code: &'static str,
    message: String,
}

impl WebError {
    // Each variant maps to its own arm even when several share an
    // implementation — the table is meant to read like a spec.
    #[allow(clippy::match_same_arms, clippy::unnested_or_patterns)]
    fn status(&self) -> StatusCode {
        match self {
            WebError::Domain(DomainError::InvalidEpitechEmail(_))
            | WebError::Domain(DomainError::UnknownTrack(_))
            | WebError::Domain(DomainError::UnknownSpecialization { .. })
            | WebError::Domain(DomainError::InvalidProjectTransition { .. })
            | WebError::Validation(_) => StatusCode::UNPROCESSABLE_ENTITY,

            WebError::Domain(DomainError::EmailNotVerified)
            | WebError::Domain(DomainError::EmailAlreadyVerified) => StatusCode::FORBIDDEN,

            WebError::Domain(DomainError::EmailAlreadyTaken)
            | WebError::Domain(DomainError::QrAlreadyClaimed)
            | WebError::Domain(DomainError::AlreadyVoted)
            | WebError::Domain(DomainError::AlreadyInTrack) => StatusCode::CONFLICT,

            WebError::Domain(DomainError::QrCapacityReached)
            | WebError::Domain(DomainError::QuestNotActive) => StatusCode::GONE,

            WebError::Domain(DomainError::OtpCooldown) => StatusCode::TOO_MANY_REQUESTS,
            WebError::Domain(DomainError::Forbidden(_)) | WebError::Forbidden => {
                StatusCode::FORBIDDEN
            }
            WebError::Domain(DomainError::DailyXpCapReached(_)) => StatusCode::TOO_MANY_REQUESTS,
            WebError::Domain(DomainError::InvalidQrToken) => StatusCode::GONE,
            WebError::Domain(DomainError::Invariant(_)) => StatusCode::INTERNAL_SERVER_ERROR,

            WebError::Unauthorized => StatusCode::UNAUTHORIZED,
            WebError::NotFound => StatusCode::NOT_FOUND,
            WebError::Conflict(_) => StatusCode::CONFLICT,
            WebError::RateLimited => StatusCode::TOO_MANY_REQUESTS,
            WebError::Upstream(_) => StatusCode::BAD_GATEWAY,
            WebError::Database(_) | WebError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    #[allow(clippy::match_same_arms)]
    fn code(&self) -> &'static str {
        match self {
            WebError::Domain(DomainError::InvalidEpitechEmail(_)) => "invalid_email",
            WebError::Domain(DomainError::EmailNotVerified) => "email_not_verified",
            WebError::Domain(DomainError::EmailAlreadyVerified) => "email_already_verified",
            WebError::Domain(DomainError::EmailAlreadyTaken) => "email_taken",
            WebError::Domain(DomainError::QrAlreadyClaimed) => "qr_already_claimed",
            WebError::Domain(DomainError::QrCapacityReached) => "qr_capacity_reached",
            WebError::Domain(DomainError::OtpCooldown) => "otp_cooldown",
            WebError::Domain(DomainError::QuestNotActive) => "quest_not_active",
            WebError::Domain(DomainError::AlreadyVoted) => "already_voted",
            WebError::Domain(DomainError::AlreadyInTrack) => "already_in_track",
            WebError::Domain(DomainError::Forbidden(_)) | WebError::Forbidden => "forbidden",
            WebError::Domain(DomainError::UnknownTrack(_)) => "unknown_track",
            WebError::Domain(DomainError::UnknownSpecialization { .. }) => "unknown_specialization",
            WebError::Domain(DomainError::DailyXpCapReached(_)) => "daily_xp_cap",
            WebError::Domain(DomainError::InvalidQrToken) => "invalid_qr_token",
            WebError::Domain(DomainError::InvalidProjectTransition { .. }) => "invalid_transition",
            WebError::Domain(DomainError::Invariant(_)) => "invariant",
            WebError::Unauthorized => "unauthorized",
            WebError::NotFound => "not_found",
            WebError::Conflict(_) => "conflict",
            WebError::Validation(_) => "validation",
            WebError::RateLimited => "rate_limited",
            WebError::Upstream(_) => "upstream",
            WebError::Database(_) => "internal",
            WebError::Internal(_) => "internal",
        }
    }

    /// Whether this error variant is "expected": something the client
    /// did wrong rather than a bug. Drives whether the server logs at
    /// INFO or WARN.
    fn is_client_error(&self) -> bool {
        let s = self.status().as_u16();
        (400..500).contains(&s)
    }
}

impl IntoResponse for WebError {
    fn into_response(self) -> Response {
        let status = self.status();
        let code = self.code();

        if self.is_client_error() {
            tracing::info!(status = %status, code, error = %self, "client error");
        } else {
            tracing::error!(status = %status, code, error = ?self, "internal error");
        }

        // Public-facing message — for 5xx we deliberately don't leak
        // the inner error string.
        let message = if self.is_client_error() {
            self.to_string()
        } else {
            "internal server error".to_string()
        };

        let body = Json(ErrorBody {
            error: ErrorBodyInner { code, message },
        });

        (status, body).into_response()
    }
}

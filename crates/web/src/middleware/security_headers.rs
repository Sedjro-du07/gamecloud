//! Security headers on every response.
//!
//! The pages are served with a content security policy that only lets the
//! platform's own code, styles and data run — plus the font service the
//! stylesheet names and images over HTTPS (avatars, project screenshots).
//! Framing is refused, MIME sniffing is off, referrers stay on the origin,
//! and in production browsers are told to use HTTPS only.
//!
//! `'unsafe-inline'` stays in `script-src` because Leptos boots hydration
//! with an inline module script; `'wasm-unsafe-eval'` lets the WASM bundle
//! compile. Nothing else is inlined.

use axum::{
    extract::{Request, State},
    http::{header, HeaderName, HeaderValue},
    middleware::Next,
    response::Response,
};

use crate::state::AppState;

/// What a page may load and where it may send data.
const CONTENT_SECURITY_POLICY: &str = "default-src 'self'; \
    script-src 'self' 'unsafe-inline' 'wasm-unsafe-eval'; \
    style-src 'self' 'unsafe-inline' https://fonts.googleapis.com; \
    font-src 'self' https://fonts.gstatic.com; \
    img-src 'self' data: https:; \
    connect-src 'self'; \
    media-src 'self'; \
    object-src 'none'; \
    frame-ancestors 'none'; \
    base-uri 'self'; \
    form-action 'self'";

/// Headers every response carries.
const FIXED: [(HeaderName, &str); 6] = [
    (header::CONTENT_SECURITY_POLICY, CONTENT_SECURITY_POLICY),
    (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
    (header::X_FRAME_OPTIONS, "DENY"),
    (header::REFERRER_POLICY, "strict-origin-when-cross-origin"),
    (
        HeaderName::from_static("permissions-policy"),
        "camera=(), microphone=(), geolocation=(), payment=(), usb=()",
    ),
    (HeaderName::from_static("cross-origin-opener-policy"), "same-origin"),
];

/// Add the security headers.
pub async fn layer(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    for (name, value) in FIXED {
        headers.insert(name, HeaderValue::from_static(value));
    }
    if state.config().is_production {
        headers.insert(
            header::STRICT_TRANSPORT_SECURITY,
            HeaderValue::from_static("max-age=31536000; includeSubDomains"),
        );
    }
    response
}

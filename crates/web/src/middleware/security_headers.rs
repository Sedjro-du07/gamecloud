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
//!
//! It also decides how long a browser may keep a response. The WASM bundle
//! and its JavaScript loader keep the same file names from one release to
//! the next, and were served with no `Cache-Control` at all, so browsers
//! fell back on heuristic caching and could run last week's code against
//! this week's pages. Hydration then breaks, links and buttons stop
//! responding, and only a manual refresh fetched the new bundle. The
//! bundle is now always revalidated (a cheap `304` when unchanged), and
//! pages — which carry the signed-in member's own data — are never stored.

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

/// How long a browser may keep the response to `path`.
///
/// Applied only when the handler set nothing itself, so a download that
/// chooses its caching deliberately is not overridden.
fn cache_policy(path: &str) -> &'static str {
    if path.starts_with("/pkg/") {
        // Same file names every release: always ask before reusing.
        "no-cache"
    } else if path.starts_with("/api/") || path.starts_with("/_fn/") {
        "no-store"
    } else if path.contains('.') {
        // Other static assets (favicon, sounds): revalidate as well.
        "no-cache"
    } else {
        // A page: rendered for one member, and the bundle it names may
        // have changed since.
        "private, no-store"
    }
}

/// Add the security headers.
pub async fn layer(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let policy = cache_policy(request.uri().path());
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    for (name, value) in FIXED {
        headers.insert(name, HeaderValue::from_static(value));
    }
    if !headers.contains_key(header::CACHE_CONTROL) {
        headers.insert(header::CACHE_CONTROL, HeaderValue::from_static(policy));
    }
    if state.config().is_production {
        headers.insert(
            header::STRICT_TRANSPORT_SECURITY,
            HeaderValue::from_static("max-age=31536000; includeSubDomains"),
        );
    }
    response
}

#[cfg(test)]
mod tests {
    use super::cache_policy;

    #[test]
    fn the_bundle_is_always_revalidated() {
        // The regression: an old `gamecloud.wasm` reused against new
        // pages, so nothing responded until the member refreshed.
        assert_eq!(cache_policy("/pkg/gamecloud.wasm"), "no-cache");
        assert_eq!(cache_policy("/pkg/gamecloud.js"), "no-cache");
    }

    #[test]
    fn pages_and_data_are_never_stored() {
        assert_eq!(cache_policy("/profile"), "private, no-store");
        assert_eq!(cache_policy("/tracks/VisualArt"), "private, no-store");
        assert_eq!(cache_policy("/_fn/get_me123"), "no-store");
        assert_eq!(cache_policy("/api/auth/me"), "no-store");
    }
}

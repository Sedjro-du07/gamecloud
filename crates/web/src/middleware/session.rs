//! Keep a signed-in member signed in.
//!
//! The access token lives an hour; the refresh token a week. Nothing used
//! to connect the two: no page ever called `/api/auth/refresh`, so an hour
//! after signing in every member was quietly signed out — buttons failed
//! with "not signed in", pages fell back to their members-only notice, and
//! the only way back was to sign in with Discord again.
//!
//! This layer renews the access token on the way in, for every request —
//! page renders and server functions alike — whenever it is missing,
//! invalid or within [`RENEW_BEFORE`] of expiring, provided the refresh
//! token is still good. The fresh token is written into the request's
//! `Cookie` header, so the handlers and extractors downstream see a live
//! session without knowing anything happened, and returned to the browser
//! with `Set-Cookie`.
//!
//! The refresh token itself is **not** rotated here. A page fires several
//! server functions at once; rotating would let the first request revoke
//! the token the others are still presenting, and the reuse detection in
//! `/api/auth/refresh` would then treat that as theft and sign the member
//! out everywhere. Renewing only the access token is idempotent, so any
//! number of concurrent requests can do it.

use axum::{
    extract::{Request, State},
    http::{header, HeaderValue},
    middleware::Next,
    response::Response,
};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use chrono::Utc;

use crate::{
    db::queries::{auth as auth_q, users},
    middleware::auth::{ACCESS_COOKIE, REFRESH_COOKIE},
    services::{jwt, tokens},
    state::AppState,
};

/// Renew this long before the access token expires, so a page left open
/// does not hold a token that dies between load and click.
const RENEW_BEFORE: i64 = 10 * 60;

/// Renew the access token when needed.
pub async fn layer(State(state): State<AppState>, mut request: Request, next: Next) -> Response {
    // A copy of the headers, not a borrow of the request: the request body
    // is not `Sync`, so holding `&Request` across an `.await` would make
    // this future unsendable.
    let headers = request.headers().clone();
    let renewed = renewal(&state, &headers).await;

    if let Some(token) = &renewed {
        rewrite_access_cookie(&mut request, token);
    }
    let mut response = next.run(request).await;
    if let Some(token) = renewed {
        let cookie = access_cookie(&state, token);
        if let Ok(value) = HeaderValue::from_str(&cookie.to_string()) {
            response.headers_mut().append(header::SET_COOKIE, value);
        }
    }
    response
}

/// A fresh access token, when the request needs one and deserves one.
async fn renewal(state: &AppState, headers: &axum::http::HeaderMap) -> Option<String> {
    let jar = CookieJar::from_headers(headers);
    let secret = &state.config().jwt_secret;

    if let Some(current) = jar.get(ACCESS_COOKIE) {
        if let Ok(claims) = jwt::verify_access_claims(secret, current.value()) {
            if claims.exp - Utc::now().timestamp() > RENEW_BEFORE {
                return None;
            }
        }
    }

    let refresh = jar.get(REFRESH_COOKIE)?;
    let row = auth_q::lookup_refresh_token(state.pool(), &tokens::hash(refresh.value()))
        .await
        .ok()?;
    if row.revoked || row.expires_at <= Utc::now() {
        return None;
    }
    // The account may have been deleted since.
    users::find_by_id(state.pool(), row.user_id).await.ok()??;

    jwt::issue_access(secret, row.user_id, state.config().jwt_access_ttl_seconds)
        .ok()
        .map(|(token, _)| token)
}

/// Put the fresh token in place of the stale one in the request.
fn rewrite_access_cookie(request: &mut Request, token: &str) {
    let jar = CookieJar::from_headers(request.headers());
    let mut pairs: Vec<String> = jar
        .iter()
        .filter(|c| c.name() != ACCESS_COOKIE)
        .map(|c| format!("{}={}", c.name(), c.value()))
        .collect();
    pairs.push(format!("{ACCESS_COOKIE}={token}"));
    if let Ok(value) = HeaderValue::from_str(&pairs.join("; ")) {
        request.headers_mut().insert(header::COOKIE, value);
    }
}

/// The access cookie, exactly as sign-in sets it.
pub fn access_cookie(state: &AppState, token: String) -> Cookie<'static> {
    let cfg = state.config();
    Cookie::build((ACCESS_COOKIE, token))
        .http_only(true)
        .secure(cfg.is_production)
        .same_site(SameSite::Lax)
        .path("/")
        .max_age(time::Duration::seconds(
            cfg.jwt_access_ttl_seconds.try_into().unwrap_or(3600),
        ))
        .build()
}

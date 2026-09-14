//! Authentication routes.
//!
//! - `GET  /api/auth/login`         — start Discord OAuth, set state cookie, redirect.
//! - `GET  /api/auth/callback`      — Discord OAuth callback, issue tokens.
//! - `POST /api/auth/email`         — submit `@epitech.eu` email, send OTP. Accepts both JSON and form-urlencoded.
//! - `POST /api/auth/verify`        — submit OTP code, finalize verification. Accepts both JSON and form-urlencoded.
//! - `POST /api/auth/refresh`       — rotate access + refresh tokens.
//! - `POST /api/auth/logout`        — revoke current refresh token.
//! - `GET  /api/auth/me`            — return the current user record.
//!
//! ## Form vs JSON content negotiation
//!
//! `submit_email` and `verify_otp` both accept either a JSON body or a
//! `application/x-www-form-urlencoded` body. This lets browser
//! `<form>` POSTs work end-to-end without JavaScript, while JSON-based
//! API clients keep working too. On a successful form submit we
//! `303 See Other` redirect to the next step in the flow; on a
//! successful JSON submit we return a tiny JSON ack.

use axum::{
    body::Bytes,
    extract::{Query, Request, State},
    http::{header, StatusCode},
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
    Json, Router,
};
use axum_extra::extract::{
    cookie::{Cookie, SameSite},
    CookieJar,
};
use chrono::{Duration, Utc};
use gamecloud_shared::models::UserRecord;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    db::queries::{auth as auth_q, users},
    error::{WebError, WebResult},
    middleware::auth::{CurrentUser, ACCESS_COOKIE, REFRESH_COOKIE},
    services::{email_validator, jwt, otp, password, tokens},
    state::AppState,
};

/// Build the auth router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/login", get(login_redirect))
        .route("/callback", get(callback))
        .route("/email", post(submit_email))
        .route("/verify", post(verify_otp))
        .route("/refresh", post(refresh))
        // Allow both GET and POST: GET is convenient for plain `<a>`
        // links in the navigation; POST is the canonical CSRF-safe
        // verb. Both perform the same operation (revoke + clear
        // cookies) and are idempotent.
        .route("/logout", get(logout).post(logout))
        .route("/me", get(me))
}

// ---------------------------------------------------------------------------
// Discord OAuth
// ---------------------------------------------------------------------------

const DISCORD_OAUTH_AUTHORIZE: &str = "https://discord.com/api/oauth2/authorize";
const DISCORD_OAUTH_TOKEN: &str = "https://discord.com/api/oauth2/token";
const DISCORD_API_USER: &str = "https://discord.com/api/users/@me";
const OAUTH_STATE_COOKIE: &str = "gc_oauth_state";

#[derive(Deserialize)]
struct CallbackQuery {
    code: String,
    state: String,
}

#[derive(Deserialize)]
struct DiscordTokenResponse {
    access_token: String,
}

#[derive(Deserialize)]
struct DiscordUser {
    id: String,
    avatar: Option<String>,
}

/// Where to send the user after a successful OAuth callback. If the
/// account is already verified, jump straight to the profile; if it
/// has submitted an email but not verified, jump to /onboarding/verify;
/// otherwise (`Pending`), show /onboarding/email.
fn post_login_target(record: &UserRecord) -> &'static str {
    if record.email_verified {
        "/profile"
    } else if record.email.is_some() {
        "/onboarding/verify"
    } else {
        "/onboarding/email"
    }
}

async fn login_redirect(State(state): State<AppState>, jar: CookieJar) -> Response {
    let cfg = state.config();
    let csrf_state = tokens::generate();

    let url = format!(
        "{base}?client_id={client_id}&redirect_uri={redirect}&response_type=code&scope=identify&state={state}&prompt=consent",
        base = DISCORD_OAUTH_AUTHORIZE,
        client_id = cfg.discord_client_id,
        redirect = urlencoding_minimal(&cfg.discord_redirect_uri),
        state = csrf_state,
    );

    let cookie = Cookie::build((OAUTH_STATE_COOKIE, csrf_state))
        .http_only(true)
        .secure(cfg.is_production)
        .same_site(SameSite::Lax)
        .path("/")
        .max_age(time::Duration::minutes(10))
        .build();

    (jar.add(cookie), Redirect::to(&url)).into_response()
}

async fn callback(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(q): Query<CallbackQuery>,
) -> WebResult<Response> {
    let cfg = state.config();

    let saved = jar
        .get(OAUTH_STATE_COOKIE)
        .map(|c| c.value().to_string())
        .ok_or(WebError::Unauthorized)?;
    if saved != q.state {
        return Err(WebError::Unauthorized);
    }

    // Exchange code for token.
    let client = reqwest::Client::new();
    let form = [
        ("client_id", cfg.discord_client_id.as_str()),
        ("client_secret", cfg.discord_client_secret.as_str()),
        ("grant_type", "authorization_code"),
        ("code", q.code.as_str()),
        ("redirect_uri", cfg.discord_redirect_uri.as_str()),
    ];
    let token: DiscordTokenResponse = client
        .post(DISCORD_OAUTH_TOKEN)
        .form(&form)
        .send()
        .await
        .map_err(|e| WebError::Upstream(format!("discord token: {e}")))?
        .error_for_status()
        .map_err(|e| WebError::Upstream(format!("discord token status: {e}")))?
        .json()
        .await
        .map_err(|e| WebError::Upstream(format!("discord token decode: {e}")))?;

    // Fetch identity.
    let me: DiscordUser = client
        .get(DISCORD_API_USER)
        .bearer_auth(&token.access_token)
        .send()
        .await
        .map_err(|e| WebError::Upstream(format!("discord me: {e}")))?
        .error_for_status()
        .map_err(|e| WebError::Upstream(format!("discord me status: {e}")))?
        .json()
        .await
        .map_err(|e| WebError::Upstream(format!("discord me decode: {e}")))?;

    let avatar_url = me.avatar.as_ref().map(|hash| {
        format!(
            "https://cdn.discordapp.com/avatars/{user}/{hash}.png",
            user = me.id,
            hash = hash
        )
    });

    let user = users::upsert_from_discord(state.pool(), &me.id, avatar_url.as_deref()).await?;
    let target = post_login_target(&user);

    let (jar, _exp) = issue_session(&state, jar, user.id, None, None).await?;
    let jar = jar.remove(Cookie::from(OAUTH_STATE_COOKIE));

    Ok((jar, Redirect::to(target)).into_response())
}

// ---------------------------------------------------------------------------
// Email submission — accepts JSON or form-urlencoded
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct SubmitEmailBody {
    email: String,
}

/// Whether a request claims to be a browser form submission.
fn is_form_request(request: &Request) -> bool {
    request
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|ct| ct.starts_with("application/x-www-form-urlencoded"))
}

/// Best-effort decoder accepting both JSON and form-urlencoded bodies.
fn decode_body<T: serde::de::DeserializeOwned>(
    is_form: bool,
    bytes: &[u8],
) -> Result<T, WebError> {
    if is_form {
        serde_urlencoded::from_bytes(bytes)
            .map_err(|e| WebError::Validation(format!("invalid form: {e}")))
    } else {
        serde_json::from_slice(bytes)
            .map_err(|e| WebError::Validation(format!("invalid json: {e}")))
    }
}

async fn submit_email(
    State(state): State<AppState>,
    user: CurrentUser,
    request: Request,
) -> WebResult<Response> {
    let is_form = is_form_request(&request);
    let bytes = read_body(request).await?;
    let body: SubmitEmailBody = decode_body(is_form, &bytes)?;

    let canonical = email_validator::validate(&body.email)?;
    let code = otp::generate();
    let hash = password::hash(&code)?;
    let expires = Utc::now() + Duration::minutes(15);

    auth_q::upsert_otp(state.pool(), user.id, &canonical, &hash, expires).await?;
    state.mailer().send_otp(&canonical, &code).await?;

    if is_form {
        Ok(Redirect::to("/onboarding/verify").into_response())
    } else {
        Ok(Json(EmptyAck { ok: true }).into_response())
    }
}

// ---------------------------------------------------------------------------
// OTP verification — accepts JSON or form-urlencoded
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct VerifyOtpBody {
    code: String,
}

async fn verify_otp(
    State(state): State<AppState>,
    user: CurrentUser,
    request: Request,
) -> WebResult<Response> {
    let is_form = is_form_request(&request);
    let bytes = read_body(request).await?;
    let body: VerifyOtpBody = decode_body(is_form, &bytes)?;

    let otp_row = auth_q::fetch_otp(state.pool(), user.id)
        .await?
        .ok_or(WebError::Unauthorized)?;

    if otp_row.expires_at <= Utc::now() {
        return Err(WebError::Unauthorized);
    }
    if otp_row.attempts >= 5 {
        return Err(WebError::RateLimited);
    }
    if !password::verify(&otp_row.code_hash, &body.code)? {
        auth_q::increment_otp_attempts(state.pool(), user.id).await?;
        return Err(WebError::Unauthorized);
    }

    auth_q::finalize_email_verification(state.pool(), user.id, &otp_row.email).await?;

    if is_form {
        Ok(Redirect::to("/profile").into_response())
    } else {
        Ok(Json(EmptyAck { ok: true }).into_response())
    }
}

async fn read_body(request: Request) -> WebResult<Bytes> {
    use http_body_util::BodyExt;
    let body = request.into_body();
    body.collect()
        .await
        .map(http_body_util::Collected::to_bytes)
        .map_err(|e| WebError::Validation(format!("read body: {e}")))
}

// ---------------------------------------------------------------------------
// Refresh
// ---------------------------------------------------------------------------

async fn refresh(State(state): State<AppState>, jar: CookieJar) -> WebResult<Response> {
    let token = jar
        .get(REFRESH_COOKIE)
        .map(|c| c.value().to_string())
        .ok_or(WebError::Unauthorized)?;

    let hash = tokens::hash(&token);
    let row = auth_q::lookup_refresh_token(state.pool(), &hash).await?;

    if row.expires_at <= Utc::now() {
        auth_q::revoke_refresh_token(state.pool(), &hash).await?;
        return Err(WebError::Unauthorized);
    }
    if row.revoked {
        auth_q::revoke_all_for_user(state.pool(), row.user_id).await?;
        return Err(WebError::Unauthorized);
    }

    auth_q::revoke_refresh_token(state.pool(), &hash).await?;
    let (jar, _) = issue_session(&state, jar, row.user_id, None, None).await?;
    Ok((jar, Json(EmptyAck { ok: true })).into_response())
}

// ---------------------------------------------------------------------------
// Logout — accepts both POST (CSRF-safe with cookie sameSite) and GET
// (handy for browser links). Always redirects to home.
// ---------------------------------------------------------------------------

async fn logout(State(state): State<AppState>, jar: CookieJar) -> WebResult<Response> {
    if let Some(c) = jar.get(REFRESH_COOKIE) {
        let hash = tokens::hash(c.value());
        let _ = auth_q::revoke_refresh_token(state.pool(), &hash).await;
    }
    let jar = jar
        .remove(Cookie::from(ACCESS_COOKIE))
        .remove(Cookie::from(REFRESH_COOKIE));
    Ok((jar, Redirect::to("/")).into_response())
}

// ---------------------------------------------------------------------------
// Me
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct MeBody {
    user: UserRecord,
}

async fn me(user: CurrentUser) -> WebResult<Json<MeBody>> {
    Ok(Json(MeBody { user: user.record }))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct EmptyAck {
    ok: bool,
}

/// Issue access + refresh cookies for the given user. The refresh token
/// row is also persisted (hashed).
async fn issue_session(
    state: &AppState,
    jar: CookieJar,
    user_id: Uuid,
    user_agent: Option<&str>,
    ip_address: Option<&str>,
) -> WebResult<(CookieJar, chrono::DateTime<Utc>)> {
    let cfg = state.config();
    let (access_jwt, access_exp) =
        jwt::issue_access(&cfg.jwt_secret, user_id, cfg.jwt_access_ttl_seconds)?;

    let refresh_plain = tokens::generate();
    let refresh_hash = tokens::hash(&refresh_plain);
    let refresh_exp = Utc::now()
        + Duration::seconds(cfg.jwt_refresh_ttl_seconds.try_into().unwrap_or(604_800));
    auth_q::insert_refresh_token(
        state.pool(),
        user_id,
        &refresh_hash,
        refresh_exp,
        user_agent,
        ip_address,
    )
    .await?;

    let secure = cfg.is_production;

    let access_cookie = Cookie::build((ACCESS_COOKIE, access_jwt))
        .http_only(true)
        .secure(secure)
        .same_site(SameSite::Lax)
        .path("/")
        .max_age(
            time::Duration::seconds(cfg.jwt_access_ttl_seconds.try_into().unwrap_or(3600)),
        )
        .build();
    let refresh_cookie = Cookie::build((REFRESH_COOKIE, refresh_plain))
        .http_only(true)
        .secure(secure)
        .same_site(SameSite::Strict)
        .path("/api/auth")
        .max_age(time::Duration::seconds(
            cfg.jwt_refresh_ttl_seconds.try_into().unwrap_or(604_800),
        ))
        .build();

    Ok((jar.add(access_cookie).add(refresh_cookie), access_exp))
}

/// Minimal URL-encoder for the OAuth redirect URI. We deliberately
/// percent-encode `:` and `/` because the redirect_uri is a *value*
/// inside another URL — its slashes must NOT be left raw.
fn urlencoding_minimal(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for b in input.bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(b as char);
            }
            _ => {
                use std::fmt::Write;
                let _ = write!(&mut out, "%{b:02X}");
            }
        }
    }
    out
}

// Anchor StatusCode import (used by future endpoints; kept here so we
// don't lose the import on refactor).
#[allow(dead_code)]
const _STATUS_ANCHOR: StatusCode = StatusCode::OK;

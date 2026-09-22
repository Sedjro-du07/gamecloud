//! Authentication routes.
//!
//! Sign-in is Discord OAuth and nothing else: being on the association's
//! server is the membership check, so there is no email step.
//!
//! - `GET  /api/auth/login`         — start Discord OAuth, set state cookie, redirect.
//! - `GET  /api/auth/callback`      — Discord OAuth callback, issue tokens.
//! - `POST /api/auth/refresh`       — rotate access + refresh tokens.
//! - `POST /api/auth/logout`        — revoke current refresh token.
//! - `GET  /api/auth/me`            — return the current user record.

use axum::{
    extract::{Query, State},
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
    services::{jwt, tokens},
    state::AppState,
};

/// Build the auth router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/login", get(login_redirect))
        .route("/callback", get(callback))
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
    /// Globally unique handle. Always present on `/users/@me`.
    username: Option<String>,
    /// Display name, `null` for accounts that never set one.
    global_name: Option<String>,
    avatar: Option<String>,
}

/// Where to send the member after a successful OAuth callback.
///
/// A candidate — not on the server, not admitted — goes to the entrance
/// tests. Everybody else lands on their profile.
///
/// This used to route on the email flow: unverified accounts were sent
/// to `/onboarding/email` and could go nowhere else. Since nothing
/// depends on a verified address any more, a member who has just signed
/// in should see the platform, not a form.
fn post_login_target(_record: &UserRecord, candidate: bool) -> &'static str {
    if candidate {
        "/tests"
    } else {
        "/profile"
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

    let user = users::upsert_from_discord(
        state.pool(),
        &users::DiscordIdentity {
            discord_id: &me.id,
            username: me.username.as_deref(),
            global_name: me.global_name.as_deref(),
            avatar_url: avatar_url.as_deref(),
        },
    )
    .await?;

    // DraftBot level-ups earned on the server before this member ever
    // signed in were queued rather than dropped; pay them now. Best
    // effort: a failure here must not stop somebody logging in.
    if let Err(e) =
        super::webhooks::credit_draftbot_backlog(&state, user.id, &user.discord_id).await
    {
        tracing::warn!(error = %e, user = %user.id, "draftbot backlog not credited");
    }
    // Likewise the XP earned with Kumo before the platform existed.
    if let Err(e) =
        crate::db::queries::xp::credit_legacy_xp(state.pool(), user.id, &user.discord_id).await
    {
        tracing::warn!(error = %e, user = %user.id, "legacy XP not credited");
    }

    let candidate = settle_admission(&state, &user).await?;
    let target = post_login_target(&user, candidate);

    let (jar, _exp) = issue_session(&state, jar, user.id, None, None).await?;
    let jar = jar.remove(Cookie::from(OAUTH_STATE_COOKIE));

    Ok((jar, Redirect::to(target)).into_response())
}

/// Decide at login whether an account is a candidate.
///
/// Somebody already on the association's Discord server signs up as
/// before. Somebody who is not must pass an entrance test first: until
/// the Bureau admits them they are a candidate, and signing up waits.
/// Members, office holders (an office can be given before its holder
/// signs up) and admitted accounts are never candidates.
///
/// When Discord cannot say — no bot token configured, or no answer —
/// the stored status stands, so a Discord outage neither locks members
/// out nor lets candidates skip the test.
async fn settle_admission(state: &AppState, user: &UserRecord) -> WebResult<bool> {
    use crate::db::queries::entrance;

    let admission = entrance::admission(state.pool(), user.id).await?;
    if user.bureau_role.is_some() || admission.admitted_at.is_some() {
        if admission.candidate {
            entrance::set_candidate(state.pool(), user.id, false).await?;
        }
        return Ok(false);
    }

    match crate::services::discord::is_guild_member(state.config(), &user.discord_id).await {
        Some(on_server) => {
            let candidate = !on_server;
            entrance::set_candidate(state.pool(), user.id, candidate).await?;
            Ok(candidate)
        }
        None => Ok(admission.candidate),
    }
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

async fn logout(
    State(state): State<AppState>,
    user: Option<CurrentUser>,
    jar: CookieJar,
) -> WebResult<Response> {
    if let Some(c) = jar.get(REFRESH_COOKIE) {
        let hash = tokens::hash(c.value());
        let _ = auth_q::revoke_refresh_token(state.pool(), &hash).await;
    }

    // Revoke every access token already issued to this member. Without
    // this, logout only clears the browser's copy: the JWT itself stays
    // valid until `exp`, so a restored cookie keeps working for an hour.
    if let Some(user) = user {
        let _ = auth_q::revoke_sessions(state.pool(), user.id).await;
    }

    // A cookie is identified by (name, domain, path). The removal cookie
    // must therefore carry the *same* path it was set with, or the
    // browser keeps the original — which is exactly what used to happen:
    // the session cookie was set with `Path=/` and removed with no path
    // at all, so it survived and the member stayed signed in.
    // Both refresh-cookie paths: sessions issued before it moved to `/`
    // still carry the old `/api/auth` one.
    let jar = jar
        .remove(Cookie::build((ACCESS_COOKIE, "")).path("/").build())
        .remove(Cookie::build((REFRESH_COOKIE, "")).path("/").build())
        .remove(
            Cookie::build((REFRESH_COOKIE, ""))
                .path("/api/auth")
                .build(),
        );

    Ok((jar, Redirect::to("/")).into_response())
}

// ---------------------------------------------------------------------------
// Me
// ---------------------------------------------------------------------------

/// The signed-in member, without identifiers: the same profile the rest of
/// the API returns.
async fn me(
    State(state): State<AppState>,
    user: CurrentUser,
) -> WebResult<Json<super::users::ProfileResponse>> {
    Ok(Json(super::users::build_profile(&state, user.id).await?))
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

    let access_cookie = crate::middleware::session::access_cookie(state, access_jwt);
    // Sent on every path, not only `/api/auth`: the session layer renews
    // the access token from it on ordinary page and server-function
    // requests, which is what keeps a member signed in past the hour.
    // `Lax` rather than `Strict` so that following a link from Discord
    // still renews; it is `HttpOnly`, and only ever mints access tokens.
    let refresh_cookie = Cookie::build((REFRESH_COOKIE, refresh_plain))
        .http_only(true)
        .secure(secure)
        .same_site(SameSite::Lax)
        .path("/")
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

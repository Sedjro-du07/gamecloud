//! Authentication extractors.
//!
//! Two extractors are provided:
//!
//! - `CurrentUser` resolves the JWT from the `access_token` cookie and
//!   loads the matching `UserRecord`. Errors with `Unauthorized` if no
//!   valid cookie is present.
//! - `Authorized(Action)` (built on top of `CurrentUser`) additionally
//!   loads the `Authority` envelope and checks `can(action)`. Use it as
//!   a guard on protected routes.
//!
//! For routes that are *optionally* authenticated (public projects with
//! a "your XP" sidebar, etc.) use `Option<CurrentUser>` from the handler
//! signature directly — the extractor implementation handles that case.

use axum::{
    extract::{FromRef, FromRequestParts, OptionalFromRequestParts},
    http::request::Parts,
};
use axum_extra::extract::CookieJar;
use gamecloud_shared::{models::UserRecord, roles::Authority};
use uuid::Uuid;

use crate::{
    db::queries::users,
    error::{WebError, WebResult},
    services::jwt,
    state::AppState,
};

/// Cookie name that carries the access JWT.
pub const ACCESS_COOKIE: &str = "gc_access";
/// Cookie name that carries the refresh token (opaque, hashed in DB).
pub const REFRESH_COOKIE: &str = "gc_refresh";

/// Authenticated user.
#[derive(Debug, Clone)]
pub struct CurrentUser {
    /// The user ID extracted from the JWT.
    pub id: Uuid,
    /// The full user record.
    pub record: UserRecord,
}

impl<S> FromRequestParts<S> for CurrentUser
where
    AppState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = WebError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let app: AppState = AppState::from_ref(state);
        let jar = CookieJar::from_headers(&parts.headers);
        let token = jar
            .get(ACCESS_COOKIE)
            .ok_or(WebError::Unauthorized)?
            .value()
            .to_string();
        let user_id = jwt::verify_access(&app.config().jwt_secret, &token)?;
        let record = users::find_by_id(app.pool(), user_id)
            .await?
            .ok_or(WebError::Unauthorized)?;
        Ok(Self {
            id: user_id,
            record,
        })
    }
}

/// Optionally-authenticated variant.
///
/// The module documentation always claimed a handler could take
/// `Option<CurrentUser>` for routes that are public but richer when
/// signed in — but axum 0.8 requires an explicit
/// `OptionalFromRequestParts` implementation, and there was none, so
/// any handler that tried it failed to compile.
///
/// A missing or invalid cookie yields `None` rather than an error. A
/// *database* failure while resolving a valid token is still an error:
/// silently downgrading a signed-in member to anonymous because the
/// pool hiccupped would hide an outage behind a permissions puzzle.
impl<S> OptionalFromRequestParts<S> for CurrentUser
where
    AppState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = WebError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &S,
    ) -> Result<Option<Self>, Self::Rejection> {
        let app: AppState = AppState::from_ref(state);
        let jar = CookieJar::from_headers(&parts.headers);
        let Some(cookie) = jar.get(ACCESS_COOKIE) else {
            return Ok(None);
        };
        let Ok(user_id) = jwt::verify_access(&app.config().jwt_secret, cookie.value()) else {
            return Ok(None);
        };
        let record = users::find_by_id(app.pool(), user_id).await?;
        Ok(record.map(|record| Self {
            id: user_id,
            record,
        }))
    }
}

/// Resolve the full `Authority` envelope from the database.
///
/// Use inside a handler when the route needs both the user *and* the
/// permission envelope:
///
/// ```ignore
/// async fn handler(State(state): State<AppState>, user: CurrentUser) -> WebResult<Json<()>> {
///     let auth = load_authority_for(&state, &user).await?;
///     if !auth.can(Action::CreateProject) { return Err(WebError::Forbidden); }
///     // …
/// }
/// ```
///
/// # Errors
/// Propagates database errors.
pub async fn load_authority_for(state: &AppState, user: &CurrentUser) -> WebResult<Authority> {
    users::load_authority(state.pool(), user.id).await
}

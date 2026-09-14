//! Profile page.
//!
//! Displays the `CharacterCard` for the current user. The data is
//! fetched via a Leptos server function that runs on the server side
//! and reads the JWT from the `gc_access` cookie via the request
//! parts injected into the Leptos render context.

use leptos::prelude::*;
use serde::{Deserialize, Serialize};

use crate::components::character_card::CharacterCard;

/// Minimal subset of the user record exposed to the client. We
/// re-define this here (rather than reusing `UserRecord`) because
/// `UserRecord` pulls in `chrono::DateTime` types that don't
/// round-trip cleanly through the WASM boundary in all configurations.
#[derive(Clone, Serialize, Deserialize)]
pub struct CurrentUserView {
    /// Discord ID — used as a stable display name when the user has
    /// no preferred title.
    pub display_name: String,
    /// Avatar URL.
    pub avatar_url: Option<String>,
    /// Total XP across all sources.
    pub xp_total: i64,
    /// Global rank (string form).
    pub global_rank: String,
    /// User-selected title; falls back to the rank title.
    pub title: Option<String>,
    /// Whether the email has been verified.
    pub email_verified: bool,
    /// The user's Epitech email if any.
    pub email: Option<String>,
}

/// Server function that returns the current user, or `None` if the
/// caller is not signed in.
#[server(GetCurrentUser, "/api")]
pub async fn get_current_user() -> Result<Option<CurrentUserView>, ServerFnError> {
    use crate::{
        middleware::auth::ACCESS_COOKIE,
        services::jwt,
        state::AppState,
    };
    use axum::http::request::Parts;

    // Pull the AppState and the request parts out of the Leptos render
    // context. Both were injected by `leptos_routes_with_context` in
    // `router.rs`.
    let Some(state) = use_context::<AppState>() else {
        return Ok(None);
    };
    let Some(parts) = use_context::<Parts>() else {
        return Ok(None);
    };

    let cookie_header = match parts.headers.get(axum::http::header::COOKIE) {
        Some(h) => h.to_str().unwrap_or("").to_string(),
        None => return Ok(None),
    };
    let access_token = cookie_header
        .split(';')
        .map(str::trim)
        .find_map(|kv| kv.strip_prefix(&format!("{ACCESS_COOKIE}=")));
    let Some(token) = access_token else {
        return Ok(None);
    };

    let Ok(user_id) = jwt::verify_access(&state.config().jwt_secret, token) else {
        return Ok(None);
    };

    match crate::db::queries::users::find_by_id(state.pool(), user_id).await {
        Ok(Some(record)) => Ok(Some(CurrentUserView {
            display_name: record
                .current_title
                .clone()
                .unwrap_or_else(|| record.discord_id.clone()),
            avatar_url: record
                .avatar_custom_url
                .clone()
                .or(record.avatar_url.clone()),
            xp_total: record.xp_total,
            global_rank: record.global_rank.clone(),
            title: record.current_title.clone(),
            email_verified: record.email_verified,
            email: record.email.clone(),
        })),
        Ok(None) => Ok(None),
        Err(e) => {
            tracing::warn!(error = ?e, "profile fetch failed");
            Err(ServerFnError::ServerError(
                "could not load profile".to_string(),
            ))
        }
    }
}

/// Profile page.
#[component]
pub fn ProfilePage() -> impl IntoView {
    let me = Resource::new(|| (), |()| async { get_current_user().await });

    view! {
        <section class="gc-profile">
            <Suspense fallback=move || view! { <p class="gc-empty">"Chargement…"</p> }>
                {move || match me.get() {
                    None => view! { <p class="gc-empty">"Chargement…"</p> }.into_any(),
                    Some(Err(_)) => view! {
                        <p class="gc-empty">"Erreur lors du chargement du profil."</p>
                    }
                    .into_any(),
                    Some(Ok(None)) => view! {
                        <p class="gc-empty">
                            "Pas encore connecté. "
                            <a href="/api/auth/login" rel="external">
                                "Se connecter avec Discord"
                            </a>
                            "."
                        </p>
                    }
                    .into_any(),
                    Some(Ok(Some(user))) => {
                        let card = view! {
                            <CharacterCard
                                name=user.display_name.clone()
                                avatar_url=user.avatar_url.clone()
                                xp_total=user.xp_total
                                global_rank=user.global_rank.clone()
                                title=user.title.clone()
                            />
                        };
                        let banner = (!user.email_verified).then(|| view! {
                            <div class="gc-banner gc-banner--warning">
                                "Email Epitech pas encore validé. "
                                <a href="/onboarding/email" rel="external">"Compléter"</a>
                                "."
                            </div>
                        });
                        view! { <>{banner} {card}</> }.into_any()
                    }
                }}
            </Suspense>
        </section>
    }
}

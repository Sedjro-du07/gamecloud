//! `GET /api/contact/kumo` — write to Kumo.
//!
//! Anybody may reach Kumo, signed in or not. The link opens a private
//! conversation with the GameCloud OS bot on Discord, which relays the
//! message to Kumo and sends Kumo's answer back (see the bot's
//! `events::kumo`). A bot's user id is its application id, which the
//! platform already knows from the OAuth configuration.

use axum::{extract::State, response::Redirect, routing::get, Router};

use crate::state::AppState;

/// Build the contact router.
pub fn router() -> Router<AppState> {
    Router::new().route("/kumo", get(kumo))
}

async fn kumo(State(state): State<AppState>) -> Redirect {
    Redirect::temporary(&format!(
        "https://discord.com/users/{}",
        state.config().discord_client_id
    ))
}

//! What a page shows somebody who is not signed in.
//!
//! Signing in is offered first; writing to Kumo stays available to
//! everyone, signed in or not.

use leptos::prelude::*;

/// Sign-in prompt with the way to contact Kumo.
#[component]
pub fn SignInPrompt(
    /// What signing in lets them do, e.g. "voir le calendrier".
    what: &'static str,
) -> impl IntoView {
    view! {
        <div class="gc-banner gc-signin">
            <p>{format!("Connecte-toi avec Discord pour {what}.")}</p>
            <div class="gc-signin__actions">
                <crate::components::discord_login::DiscordLogin />
                <a class="gc-btn gc-btn--ghost" href="/kumo">
                    "💬 Contacter Kumo"
                </a>
            </div>
        </div>
    }
}

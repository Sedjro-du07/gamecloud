//! Leaderboard — placeholder.
//!
//! Phase 6 will fetch live data from `/api/users` (top by XP) and add
//! per-track tabs. For now it points users to the Discord
//! `/leaderboard` slash command which already works.

use leptos::prelude::*;

/// Leaderboard placeholder page.
#[component]
pub fn LeaderboardPage() -> impl IntoView {
    view! {
        <section class="gc-placeholder">
            <h1>"Leaderboard"</h1>
            <p>
                "Le classement live arrive avec la prochaine release.
                En attendant, tape "
                <code>"/leaderboard"</code>
                " dans le serveur Discord pour voir le top 10."
            </p>
        </section>
    }
}

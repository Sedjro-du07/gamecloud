//! Projects index — placeholder.
//!
//! Phase 6 will turn this into a real list of Released projects with
//! filters by track and rarity. For now it gives the user a friendly
//! message rather than a 404 from the navigation menu.

use leptos::prelude::*;

/// Projects placeholder page.
#[component]
pub fn ProjectsPage() -> impl IntoView {
    view! {
        <section class="gc-placeholder">
            <h1>"Projets"</h1>
            <p>
                "Cette section accueillera bientôt le Hall of Fame des
                projets validés. Pour l'instant, soumets ton prochain
                projet via le bot Discord ou attends la prochaine release
                de la plateforme."
            </p>
        </section>
    }
}

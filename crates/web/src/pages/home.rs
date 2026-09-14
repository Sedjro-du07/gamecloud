//! Public landing page.

use leptos::prelude::*;

/// Home page — Cyberpunk pitch + login CTA.
#[component]
pub fn HomePage() -> impl IntoView {
    view! {
        <section class="gc-home">
            <h1 class="gc-home__title">"GameCloud OS"</h1>
            <p class="gc-home__tagline">
                "L'OS gamifié de l'association GameCloud — Epitech Bénin."
            </p>
            <div class="gc-home__cta">
                // `rel="external"` tells the Leptos client-side router
                // not to intercept the click; the browser performs a
                // real navigation to the Axum endpoint, which redirects
                // to Discord. Without it, the first click is swallowed
                // by the router and only a refresh actually navigates.
                <a class="gc-btn gc-btn--primary"
                   href="/api/auth/login"
                   rel="external">
                    "Se connecter avec Discord"
                </a>
            </div>
            <ul class="gc-home__bullets">
                <li>"8 tracks · 16 rôles bureau · 10 rangs globaux"</li>
                <li>"XP gagnée via GitHub, présence aux events, et activité Discord"</li>
                <li>"Hall of Fame des projets validés"</li>
            </ul>
        </section>
    }
}

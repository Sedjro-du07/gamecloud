//! Top-level Leptos `App` component and document `<head>` shell.

use leptos::prelude::*;
use leptos_meta::{provide_meta_context, Meta, MetaTags, Stylesheet, Title};
use leptos_router::{
    components::{Route, Router, Routes},
    StaticSegment,
};

use crate::{
    components::hud_shell::HudShell,
    pages::{
        home::HomePage,
        leaderboard::LeaderboardPage,
        onboarding::{OnboardingEmailPage, OnboardingVerifyPage},
        profile::ProfilePage,
        projects::ProjectsPage,
    },
};

/// Document `<head>` shared by SSR + CSR. `cargo-leptos` injects
/// `pkg/gamecloud.css` and `pkg/gamecloud.js` automatically; the
/// `HydrationScripts` component renders the `<script>` tag that boots
/// the WASM bundle.
#[component]
pub fn Shell(
    /// Leptos options injected by `leptos_axum`. Carries the output
    /// name and the path of the WASM bundle.
    options: leptos::config::LeptosOptions,
) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="fr">
            <head>
                <meta charset="utf-8" />
                <meta name="viewport" content="width=device-width, initial-scale=1" />
                <HydrationScripts options />
                <MetaTags />
            </head>
            <body>
                <App />
            </body>
        </html>
    }
}

/// Application root. Provides meta context and declares the route table.
#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();

    view! {
        <Stylesheet id="leptos" href="/pkg/gamecloud.css" />
        <Title text="GameCloud OS" />
        <Meta name="description"
              content="GameCloud OS — l'OS gamifié de l'association game-dev d'Epitech Bénin." />

        <Router>
            <HudShell>
                <Routes fallback=|| view! { <p class="gc-empty">"404"</p> }>
                    <Route path=StaticSegment("") view=HomePage />
                    <Route path=StaticSegment("profile") view=ProfilePage />
                    <Route path=StaticSegment("projects") view=ProjectsPage />
                    <Route path=StaticSegment("leaderboard") view=LeaderboardPage />
                    <Route path=(StaticSegment("onboarding"), StaticSegment("email"))
                           view=OnboardingEmailPage />
                    <Route path=(StaticSegment("onboarding"), StaticSegment("verify"))
                           view=OnboardingVerifyPage />
                </Routes>
            </HudShell>
        </Router>
    }
}

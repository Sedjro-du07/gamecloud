//! Top-level Leptos `App` component and document `<head>` shell.

use leptos::prelude::*;
use leptos_meta::{provide_meta_context, Link, Meta, MetaTags, Stylesheet, Title};
use leptos_router::{
    components::{Route, Router, Routes},
    ParamSegment, StaticSegment,
};

use crate::{
    components::hud_shell::HudShell,
    pages::{
        admin::AdminPage,
        calendar::CalendarPage,
        home::HomePage,
        new_project::NewProjectPage,
        leaderboard::LeaderboardPage,
        onboarding::{OnboardingEmailPage, OnboardingVerifyPage},
        profile::ProfilePage,
        project_detail::ProjectDetailPage,
        projects::ProjectsPage,
        quests::QuestsPage,
        resources::ResourcesPage,
        reviews::ReviewsPage,
        scan::ScanPage,
        track_detail::TrackDetailPage,
        tracks::TrackPickerPage,
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
        // The display, body and mono faces the stylesheet names. Without
        // them every heading fell back to the system font.
        <Link rel="preconnect" href="https://fonts.googleapis.com" />
        <Link rel="preconnect" href="https://fonts.gstatic.com" />
        <Link
            rel="stylesheet"
            href="https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&family=JetBrains+Mono:wght@400;600&family=Orbitron:wght@600;700;800&display=swap"
        />
        // Without this the browser asks for /favicon.ico on every page
        // and gets a 404 each time — harmless, but it fills the console
        // and hides the errors that matter.
        <Link rel="icon" type_="image/svg+xml" href="/favicon.svg" />
        <Title text="GameCloud OS" />
        <Meta name="description"
              content="GameCloud OS — l'OS gamifié de l'association game-dev d'Epitech Bénin." />

        <Router>
            <HudShell>
                <Routes fallback=|| view! { <p class="gc-empty">"404"</p> }>
                    <Route path=StaticSegment("") view=HomePage />
                    <Route path=StaticSegment("profile") view=ProfilePage />
                    <Route path=StaticSegment("projects") view=ProjectsPage />
                    // Declared *before* the `:id` route below. The
                    // router takes the first match, so with the
                    // parameter first `/projects/new` resolved to the
                    // detail page for a project called "new" and every
                    // member who clicked "Nouveau projet" landed on
                    // "Projet introuvable".
                    <Route path=(StaticSegment("projects"), StaticSegment("new"))
                           view=NewProjectPage />
                    <Route path=(StaticSegment("projects"), ParamSegment("id"))
                           view=ProjectDetailPage />
                    <Route path=StaticSegment("leaderboard") view=LeaderboardPage />
                    <Route path=StaticSegment("quests") view=QuestsPage />
                    <Route path=StaticSegment("resources") view=ResourcesPage />
                    <Route path=StaticSegment("reviews") view=ReviewsPage />
                    <Route path=StaticSegment("admin") view=AdminPage />
                    <Route path=StaticSegment("scan") view=ScanPage />
                    <Route path=StaticSegment("calendar") view=CalendarPage />
                    // The picker lives under /onboarding; this is the
                    // board for a track you already belong to, which is
                    // a different page for a different moment.
                    <Route path=StaticSegment("tracks") view=TrackPickerPage />
                    <Route path=(StaticSegment("tracks"), ParamSegment("id"))
                           view=TrackDetailPage />
                    <Route path=(StaticSegment("onboarding"), StaticSegment("email"))
                           view=OnboardingEmailPage />
                    <Route path=(StaticSegment("onboarding"), StaticSegment("verify"))
                           view=OnboardingVerifyPage />
                    <Route path=(StaticSegment("onboarding"), StaticSegment("tracks"))
                           view=TrackPickerPage />
                </Routes>
            </HudShell>
        </Router>
    }
}

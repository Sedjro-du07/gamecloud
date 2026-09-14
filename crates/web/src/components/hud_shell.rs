//! HUD shell — chrome around every page.
//!
//! Renders the top status bar (brand, navigation, auth pill) and a
//! content slot. Children are placed inside the main content area via
//! the `children` prop.

use leptos::prelude::*;
use leptos_router::components::A;

use crate::pages::profile::get_current_user;

/// HUD shell.
#[component]
pub fn HudShell(children: Children) -> impl IntoView {
    let me = Resource::new(|| (), |()| async { get_current_user().await });

    view! {
        <div class="gc-hud">
            <header class="gc-hud__top">
                <div class="gc-hud__brand">
                    <A href="/">"GameCloud OS"</A>
                </div>
                <nav class="gc-hud__nav">
                    <A href="/profile">"Profile"</A>
                    <A href="/projects">"Projects"</A>
                    <A href="/leaderboard">"Leaderboard"</A>
                </nav>
                <div class="gc-hud__auth">
                    <Suspense fallback=move || view! { <span class="gc-pill">"…"</span> }>
                        {move || match me.get() {
                            Some(Ok(Some(user))) => {
                                let name = user.display_name.clone();
                                view! {
                                    <span class="gc-pill">{name}</span>
                                    <a class="gc-link" href="/api/auth/logout"
                                       rel="external">"Déconnexion"</a>
                                }
                                .into_any()
                            }
                            _ => view! {
                                <a class="gc-btn" href="/api/auth/login" rel="external">
                                    "Connexion Discord"
                                </a>
                            }
                            .into_any(),
                        }}
                    </Suspense>
                </div>
            </header>
            <main class="gc-hud__content">
                {children()}
            </main>
            <footer class="gc-hud__bottom">
                <span>"GameCloud — Epitech Bénin"</span>
            </footer>
        </div>
    }
}

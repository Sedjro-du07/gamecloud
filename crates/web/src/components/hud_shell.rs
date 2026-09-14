//! HUD shell — chrome around every page.
//!
//! Renders the top status bar (brand, navigation, member pill) and a
//! content slot. The pill shows level and streak as well as the name,
//! because those are the two numbers a member checks most often and
//! they should never require a page visit.

use leptos::prelude::*;
use leptos_router::components::A;

use crate::server_fns::get_me;

/// HUD shell.
#[component]
pub fn HudShell(children: Children) -> impl IntoView {
    let me = Resource::new(|| (), |()| async { get_me().await });

    view! {
        <div class="gc-hud">
            <header class="gc-hud__top">
                <div class="gc-hud__brand">
                    <A href="/">"GameCloud OS"</A>
                </div>
                // The navigation shows what the member can actually use.
                // This is courtesy, not a control: every page behind these
                // links re-checks the right server-side, so typing the URL
                // gains nothing. Hiding a dead link is just kinder than
                // letting somebody click into a refusal.
                <nav class="gc-hud__nav">
                    <A href="/profile">"Profil"</A>
                    <A href="/projects">"Projets"</A>
                    <A href="/quests">"Quêtes"</A>
                    <A href="/resources">"Ressources"</A>
                    <A href="/leaderboard">"Classement"</A>
                    <Suspense fallback=|| ()>
                        {move || {
                            let me = me.get().and_then(Result::ok).flatten();
                            me.map(|user| {
                                view! {
                                    <Show when=move || user.can_review>
                                        <A href="/reviews">"À relire"</A>
                                    </Show>
                                    <Show when=move || user.can_generate_qr>
                                        <A href="/scan">"Scanner"</A>
                                    </Show>
                                    <Show when=move || user.can_access_admin>
                                        <A href="/admin">"Bureau"</A>
                                    </Show>
                                }
                            })
                        }}
                    </Suspense>
                </nav>
                <div class="gc-hud__auth">
                    <Suspense fallback=move || view! { <span class="gc-pill">"…"</span> }>
                        {move || match me.get() {
                            Some(Ok(Some(user))) => {
                                let streak = (user.streak_days > 0)
                                    .then(|| {
                                        view! {
                                            <span class="gc-pill__streak">
                                                {format!("🔥{}", user.streak_days)}
                                            </span>
                                        }
                                    });
                                view! {
                                    <span
                                        class="gc-pill"
                                        style=format!("--gc-ring: {};", user.rank_color)
                                        title=user.rank_title.clone()
                                    >
                                        <span class="gc-pill__name">{user.display_name}</span>
                                        <span class="gc-pill__level">
                                            {format!("Niv. {}", user.level)}
                                        </span>
                                        {streak}
                                    </span>
                                    <a class="gc-link" href="/api/auth/logout" rel="external">
                                        "Déconnexion"
                                    </a>
                                }
                                    .into_any()
                            }
                            _ => {
                                view! {
                                    <a class="gc-btn" href="/api/auth/login" rel="external">
                                        "Connexion Discord"
                                    </a>
                                }
                                    .into_any()
                            }
                        }}
                    </Suspense>
                </div>
            </header>

            // A member who has verified their email but never picked a
            // track is stuck at `Visitor` and earns no track XP. Nagging
            // once, globally, is cheaper than letting them wonder why
            // nothing is happening.
            <Suspense fallback=|| ()>
                {move || {
                    me.get()
                        .and_then(Result::ok)
                        .flatten()
                        .filter(|user| user.needs_onboarding)
                        .map(|_| {
                            view! {
                                <div class="gc-hud__nag">
                                    "Choisis ta track pour commencer à gagner de l'XP. "
                                    <a href="/onboarding/tracks" rel="external">"C'est par ici"</a>
                                </div>
                            }
                        })
                }}
            </Suspense>

            <main class="gc-hud__content">{children()}</main>

            <footer class="gc-hud__bottom">
                <span>"GameCloud — Epitech Bénin"</span>
            </footer>
        </div>
    }
}

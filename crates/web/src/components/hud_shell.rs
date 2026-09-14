//! HUD shell — chrome around every page.
//!
//! Renders the brand bar, the navigation rail and a content slot. The
//! member pill shows level and streak as well as the name, because those
//! are the two numbers a member checks most often and they should never
//! require a page visit.
//!
//! ## Why a rail rather than a row of tabs
//!
//! The navigation grew past what a single horizontal row can hold: ten
//! destinations in one line either wrap into an untidy second row or
//! squeeze the labels until they stop being readable, and on a phone it
//! becomes a horizontal scroll nobody discovers the end of.
//!
//! A vertical rail scales with the list instead of fighting it, and it
//! lets the destinations be **grouped** — what concerns me, what
//! concerns the association, what I run — which is the part a flat row
//! could never express. On narrow screens the same markup becomes a
//! bottom bar, where a thumb can reach it.
//!
//! The rail shows what the member can actually use. That is courtesy,
//! not a control: every page behind these links re-checks the right
//! server-side, so typing the URL directly gains nothing. Hiding a dead
//! link is simply kinder than letting somebody click into a refusal.

use leptos::prelude::*;
use leptos_router::components::A;

use crate::server_fns::get_me;

/// One heading and the links under it.
///
/// The grouping is the point of the rail: ten flat destinations is a
/// list to be scanned, four groups of two or three is a shape to be
/// recognised. On the narrow layout the headings are hidden and the
/// groups flow into one bar — the grouping still orders the icons, it
/// just stops announcing itself where there is no room.
#[component]
fn NavGroup(
    /// Heading shown above the links.
    label: &'static str,
    /// The links.
    children: Children,
) -> impl IntoView {
    view! {
        <div class="gc-rail__group">
            <span class="gc-rail__label">{label}</span>
            {children()}
        </div>
    }
}

/// One destination in the rail.
///
/// `A` rather than `a` so the client-side router handles the click, and
/// so Leptos marks the current route `aria-current="page"` — which is
/// what the active styling hangs off, rather than a class the component
/// would have to compute itself.
#[component]
fn NavLink(
    /// Destination path.
    href: &'static str,
    /// Emoji shown before the label, and alone on the narrow layout.
    icon: &'static str,
    /// Human-readable destination.
    label: &'static str,
) -> impl IntoView {
    view! {
        <A href=href attr:class="gc-rail__link">
            <span class="gc-rail__icon" aria-hidden="true">
                {icon}
            </span>
            <span class="gc-rail__text">{label}</span>
        </A>
    }
}

/// The navigation rail itself.
///
/// Split out of [`HudShell`] so the shell reads as "bar, rail, page,
/// footer" instead of opening with sixty lines of links.
#[component]
fn NavRail(
    /// The signed-in member, if any. Drives which responsibilities show.
    me: Resource<Result<Option<crate::api::MeView>, ServerFnError>>,
) -> impl IntoView {
    view! {
                    <nav class="gc-hud__rail" aria-label="Navigation principale">
                        <NavGroup label="Moi">
                            <NavLink href="/profile" icon="👤" label="Profil" />
                            <NavLink href="/quests" icon="🎯" label="Quêtes" />
                            <NavLink href="/tracks" icon="🧭" label="Tracks" />
                        </NavGroup>

                        <NavGroup label="Association">
                            <NavLink href="/calendar" icon="📅" label="Calendrier" />
                            <NavLink href="/projects" icon="🎮" label="Projets" />
                            <NavLink href="/leaderboard" icon="🏅" label="Classement" />
                            <NavLink href="/resources" icon="📚" label="Ressources" />
                        </NavGroup>

                        <Suspense fallback=|| ()>
                            {move || {
                                let user = me.get().and_then(Result::ok).flatten()?;
                                let (review, qr, admin) = (
                                    user.can_review,
                                    user.can_generate_qr,
                                    user.can_access_admin,
                                );
                                // The whole group disappears when a member
                                // holds none of these, rather than leaving an
                                // empty heading behind.
                                (review || qr || admin)
                                    .then(|| {
                                        view! {
                                            <NavGroup label="Responsabilités">
                                                <Show when=move || review>
                                                    <NavLink
                                                        href="/reviews"
                                                        icon="🔍"
                                                        label="À relire"
                                                    />
                                                </Show>
                                                <Show when=move || qr>
                                                    <NavLink
                                                        href="/scan"
                                                        icon="✅"
                                                        label="Présences"
                                                    />
                                                </Show>
                                                <Show when=move || admin>
                                                    <NavLink href="/admin" icon="🏛️" label="Bureau" />
                                                </Show>
                                            </NavGroup>
                                        }
                                    })
                            }}
                        </Suspense>
                    </nav>
    }
}

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
                                // The next title, always in sight.
                                let xp_pct = if user.rank_xp_needed == 0 {
                                    100.0
                                } else {
                                    crate::api::level_percent(user.rank_xp_into, user.rank_xp_needed)
                                };
                                view! {
                                    <span
                                        class="gc-pill"
                                        style=format!("--gc-ring: {};", user.rank_color)
                                        title=user
                                            .bureau_title
                                            .clone()
                                            .map_or_else(
                                                || user.rank_title.clone(),
                                                |b| format!("{b} · {}", user.rank_title),
                                            )
                                    >
                                        <span class="gc-pill__name">{user.display_name}</span>
                                        // The most important title; all of them on hover.
                                        <span class="gc-pill__level">
                                            {user.bureau_title.clone().unwrap_or_else(|| user.rank_title.clone())}
                                        </span>
                                        {streak}
                                        <span class="gc-pill__xp" aria-hidden="true">
                                            <span
                                                class="gc-pill__xp-fill"
                                                style=format!("width: {xp_pct:.0}%;")
                                            ></span>
                                        </span>
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

            <div class="gc-hud__body">
                <NavRail me=me />

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
        </div>
    }
}

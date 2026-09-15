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
                            // Signed-in people only, like the page.
                            <Suspense fallback=|| ()>
                                {move || {
                                    me.get()
                                        .and_then(Result::ok)
                                        .flatten()
                                        .map(|_| {
                                            view! {
                                                <NavLink href="/calendar" icon="📅" label="Calendrier" />
                                            }
                                        })
                                }}
                            </Suspense>
                            <NavLink href="/projects" icon="🎮" label="Projets" />
                            <NavLink href="/shares" icon="📦" label="Partages" />
                            // Members only, like the page itself.
                            <Suspense fallback=|| ()>
                                {move || {
                                    me.get()
                                        .and_then(Result::ok)
                                        .flatten()
                                        .filter(|u| u.email_verified)
                                        .map(|_| {
                                            view! {
                                                <NavLink href="/leaderboard" icon="🏅" label="Classement" />
                                            }
                                        })
                                }}
                            </Suspense>
                            <NavLink href="/resources" icon="📚" label="Ressources" />
                            // The Bureau and candidates; ordinary members never.
                            <Suspense fallback=|| ()>
                                {move || {
                                    me.get()
                                        .and_then(Result::ok)
                                        .flatten()
                                        .filter(|u| u.can_see_tests)
                                        .map(|_| {
                                            view! {
                                                <NavLink href="/tests" icon="🎓" label="Tests d'entrée" />
                                            }
                                        })
                                }}
                            </Suspense>
                        </NavGroup>

                        // A tab of its own, open to everyone, signed in or not.
                        <NavGroup label="Contact">
                            <NavLink href="/kumo" icon="💬" label="Contacter Kumo" />
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
                                    // The Discord logo says "sign in with Discord"
                                    // without the words; the label keeps it for
                                    // screen readers and the tooltip for everyone.
                                    <a
                                        class="gc-btn gc-btn--discord"
                                        href="/api/auth/login"
                                        rel="external"
                                        aria-label="Se connecter avec Discord"
                                        title="Se connecter avec Discord"
                                    >
                                        <svg class="gc-discord-icon" viewBox="0 0 24 24" width="22" height="22" fill="currentColor" aria-hidden="true"><path d="M20.317 4.3698a19.7913 19.7913 0 00-4.8851-1.5152.0741.0741 0 00-.0785.0371c-.211.3753-.4447.8648-.6083 1.2495-1.8447-.2762-3.68-.2762-5.4868 0-.1636-.3933-.4058-.8742-.6177-1.2495a.077.077 0 00-.0785-.037 19.7363 19.7363 0 00-4.8852 1.515.0699.0699 0 00-.0321.0277C.5334 9.0458-.319 13.5799.0992 18.0578a.0824.0824 0 00.0312.0561c2.0528 1.5076 4.0413 2.4228 5.9929 3.0294a.0777.0777 0 00.0842-.0276c.4616-.6304.8731-1.2952 1.226-1.9942a.076.076 0 00-.0416-.1057c-.6528-.2476-1.2743-.5495-1.8722-.8923a.077.077 0 01-.0076-.1277c.1258-.0943.2517-.1923.3718-.2914a.0743.0743 0 01.0776-.0105c3.9278 1.7933 8.18 1.7933 12.0614 0a.0739.0739 0 01.0785.0095c.1202.099.246.1981.3728.2924a.077.077 0 01-.0066.1276 12.2986 12.2986 0 01-1.873.8914.0766.0766 0 00-.0407.1067c.3604.698.7719 1.3628 1.225 1.9932a.076.076 0 00.0842.0286c1.961-.6067 3.9495-1.5219 6.0023-3.0294a.077.077 0 00.0313-.0552c.5004-5.177-.8382-9.6739-3.5485-13.6604a.061.061 0 00-.0312-.0286zM8.02 15.3312c-1.1825 0-2.1569-1.0857-2.1569-2.419 0-1.3332.9555-2.4189 2.157-2.4189 1.2108 0 2.1757 1.0952 2.1568 2.419 0 1.3332-.9555 2.4189-2.1569 2.4189zm7.9748 0c-1.1825 0-2.1569-1.0857-2.1569-2.419 0-1.3332.9554-2.4189 2.1569-2.4189 1.2108 0 2.1757 1.0952 2.1568 2.419 0 1.3332-.946 2.4189-2.1568 2.4189Z"/></svg>
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

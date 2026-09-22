//! The shell around every page.
//!
//! A top bar with the wordmark and the account, the menu on the left, and
//! on phones a bottom bar with the four destinations that matter most plus
//! "Menu", which opens every section. The menu shows what the viewer can
//! use — a courtesy: every page re-checks rights on the server.

use leptos::prelude::*;
use leptos_router::{components::A, hooks::use_location};

use crate::{
    api::MeView,
    components::ui::{
        play, vocab::plain_title, ButtonKind, ButtonLink, Icon, IconName, IconSize, Sound,
        SoundToggle, Tag, TagKind,
    },
    server_fns::get_me,
};

/// One destination in the menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Destination {
    href: &'static str,
    icon: IconName,
    label: &'static str,
}

const fn to(href: &'static str, icon: IconName, label: &'static str) -> Destination {
    Destination { href, icon, label }
}

const HOME: Destination = to("/", IconName::House, "Accueil");
const PROFILE: Destination = to("/profile", IconName::User, "Profil");
const QUESTS: Destination = to("/quests", IconName::Target, "Quêtes");
const TRACKS: Destination = to("/tracks", IconName::Compass, "Tracks");
const CALENDAR: Destination = to("/calendar", IconName::CalendarBlank, "Calendrier");
const PROJECTS: Destination = to("/projects", IconName::Cube, "Projets");
const SHARES: Destination = to("/shares", IconName::Package, "Partages");
const LEADERBOARD: Destination = to("/leaderboard", IconName::Trophy, "Classement");
const RESOURCES: Destination = to("/resources", IconName::Books, "Ressources");
const TESTS: Destination = to("/tests", IconName::GraduationCap, "Tests d'entrée");
const KUMO: Destination = to("/kumo", IconName::ChatCircle, "Contacter Kumo");
const REVIEWS: Destination = to("/reviews", IconName::Eye, "À relire");
const SCAN: Destination = to("/scan", IconName::QrCode, "Présences");
const BUREAU: Destination = to("/admin", IconName::Bank, "Bureau");

/// A menu section.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Section {
    label: &'static str,
    items: Vec<Destination>,
}

/// The sections this viewer sees.
///
/// "Moi" only for somebody signed in: for a visitor its pages are sign-in
/// prompts. The calendar needs to be signed in, the leaderboard a verified
/// member, entrance tests are for visitors, candidates and the Bureau, and
/// "Responsabilités" appears only with a role in it.
fn sections(me: Option<&MeView>) -> Vec<Section> {
    let mut out = Vec::new();
    if me.is_some() {
        out.push(Section {
            label: "Moi",
            items: vec![PROFILE, QUESTS, TRACKS],
        });
    }

    let mut association = Vec::new();
    if me.is_some() {
        association.push(CALENDAR);
    }
    // Projects, shares, the leaderboard and the library are the
    // association's own: a visitor is not shown the door to them.
    if me.is_some_and(|u| u.is_member) {
        association.extend([PROJECTS, SHARES, LEADERBOARD, RESOURCES]);
    }
    if me.map_or(true, |u| u.can_see_tests) {
        association.push(TESTS);
    }
    out.push(Section {
        label: "Association",
        items: association,
    });

    out.push(Section {
        label: "Contact",
        items: vec![KUMO],
    });

    if let Some(user) = me {
        let mut roles = Vec::new();
        if user.can_review {
            roles.push(REVIEWS);
        }
        if user.can_generate_qr {
            roles.push(SCAN);
        }
        if user.can_access_admin {
            roles.push(BUREAU);
        }
        if !roles.is_empty() {
            out.push(Section {
                label: "Responsabilités",
                items: roles,
            });
        }
    }
    out
}

/// The four destinations on a phone's bottom bar, before "Menu".
fn tab_bar(me: Option<&MeView>) -> [Destination; 4] {
    if me.is_some() {
        [HOME, QUESTS, CALENDAR, PROFILE]
    } else {
        [HOME, TESTS, PROJECTS, KUMO]
    }
}

/// One destination, as a menu row or a bottom-bar item.
#[component]
fn NavItem(
    /// Where it goes.
    item: Destination,
    /// Bottom-bar item rather than menu row.
    #[prop(default = false)]
    tab: bool,
) -> impl IntoView {
    let class = if tab { "ui-tabbar__item" } else { "ui-nav__link" };
    let size = if tab { IconSize::Medium } else { IconSize::Small };
    view! {
        <A href=item.href exact=item.href == "/" attr:class=class on:click=move |_| play(Sound::Nav)>
            <Icon name=item.icon size />
            <span class="ui-nav__text">{item.label}</span>
        </A>
    }
}

/// The sectioned menu.
#[component]
fn Menu(
    /// Sections to show.
    sections: Vec<Section>,
    /// Called when a destination is chosen (closes the phone menu).
    #[prop(default = None)]
    on_pick: Option<Callback<()>>,
) -> impl IntoView {
    view! {
        <nav class="ui-nav" aria-label="Navigation principale">
            {sections
                .into_iter()
                .map(|section| {
                    view! {
                        <div class="ui-nav__group">
                            <span class="ui-label ui-nav__label">{section.label}</span>
                            <ul class="ui-nav__list" role="list">
                                {section
                                    .items
                                    .into_iter()
                                    .map(|item| {
                                        view! {
                                            <li on:click=move |_| {
                                                if let Some(pick) = on_pick {
                                                    pick.run(());
                                                }
                                            }>
                                                <NavItem item />
                                            </li>
                                        }
                                    })
                                    .collect_view()}
                            </ul>
                        </div>
                    }
                })
                .collect_view()}
        </nav>
    }
}

/// The menu's shape while the viewer loads.
#[component]
fn MenuSkeleton() -> impl IntoView {
    view! {
        <div class="ui-nav" aria-hidden="true">
            <div class="ui-nav__group">
                <span class="ui-skeleton ui-skeleton--short"></span>
                <span class="ui-skeleton ui-skeleton--line"></span>
                <span class="ui-skeleton ui-skeleton--line"></span>
                <span class="ui-skeleton ui-skeleton--line"></span>
            </div>
        </div>
    }
}

/// Name, current title, sign-out.
#[component]
#[allow(clippy::needless_pass_by_value)] // Leptos prop convention
fn AccountBar(
    /// The signed-in member.
    user: MeView,
) -> impl IntoView {
    let title = plain_title(&user.rank_title).to_string();
    view! {
        <span class="ui-topbar__name">{user.display_name}</span>
        <Tag kind=TagKind::Accent>{title}</Tag>
        <ButtonLink
            kind=ButtonKind::Ghost
            href="/api/auth/logout"
            external=true
            icon=IconName::SignOut
            hide_label=true
        >
            "Se déconnecter"
        </ButtonLink>
    }
}

/// A verified member who has not picked a track earns no track XP.
#[component]
fn OnboardingNotice() -> impl IntoView {
    view! {
        <div class="ui-notice" role="status">
            <Icon name=IconName::Info size=IconSize::Medium />
            <p class="ui-notice__text">"Choisis ta track pour commencer à gagner de l'XP."</p>
            <ButtonLink
                kind=ButtonKind::Primary
                href="/onboarding/tracks"
                external=true
                trailing_icon=IconName::ArrowRight
            >
                "Choisir mes tracks"
            </ButtonLink>
        </div>
    }
}

/// The viewer, as every part of the shell reads it.
type Viewer = Resource<Result<Option<MeView>, ServerFnError>>;

/// Wordmark on the left, the account on the right.
#[component]
fn TopBar(
    /// The viewer.
    me: Viewer,
) -> impl IntoView {
    view! {
        <header class="ui-topbar">
            // A plain link, not a menu entry: it should never read as the
            // current page. The router still handles the click.
            <a href="/" class="ui-topbar__brand ui-wordmark">"GameCloud OS"</a>
            <div class="ui-topbar__account">
                <SoundToggle />
                <Suspense fallback=|| view! { <span class="ui-skeleton ui-topbar__loading"></span> }>
                    {move || {
                        me.get()
                            .map(|result| match result.ok().flatten() {
                                Some(user) => view! { <AccountBar user /> }.into_any(),
                                None => {
                                    view! {
                                        <ButtonLink
                                            href="/api/auth/login"
                                            external=true
                                            icon=IconName::DiscordLogo
                                            hide_label=true
                                        >
                                            "Se connecter avec Discord"
                                        </ButtonLink>
                                    }
                                        .into_any()
                                }
                            })
                    }}
                </Suspense>
            </div>
        </header>
    }
}

/// Phones: four destinations and "Menu".
#[component]
fn TabBar(
    /// The viewer.
    me: Viewer,
    /// Whether the full menu is open.
    menu_open: ReadSignal<bool>,
    /// Opens it.
    set_menu_open: WriteSignal<bool>,
) -> impl IntoView {
    view! {
        <nav class="ui-tabbar" aria-label="Navigation rapide">
            <Suspense fallback=|| ()>
                {move || {
                    me.get()
                        .map(|result| {
                            let user = result.ok().flatten();
                            tab_bar(user.as_ref())
                                .into_iter()
                                .map(|item| view! { <NavItem item tab=true /> })
                                .collect_view()
                        })
                }}
            </Suspense>
            <button
                class="ui-tabbar__item"
                type="button"
                aria-haspopup="dialog"
                aria-expanded=move || menu_open.get().to_string()
                on:click=move |_| {
                    play(Sound::Open);
                    set_menu_open.set(true);
                }
            >
                <Icon name=IconName::List size=IconSize::Medium />
                <span class="ui-nav__text">"Menu"</span>
            </button>
        </nav>
    }
}

/// Phones: every section, over a darkened page.
#[component]
fn MenuSheet(
    /// The viewer.
    me: Viewer,
    /// Whether it is open.
    menu_open: ReadSignal<bool>,
    /// Closes it.
    set_menu_open: WriteSignal<bool>,
) -> impl IntoView {
    let close = Callback::new(move |()| set_menu_open.set(false));
    let close_button = NodeRef::<leptos::html::Button>::new();

    // Opening it moves focus inside, so the keyboard is where the eyes are.
    Effect::new(move |_| {
        if menu_open.get() {
            if let Some(button) = close_button.get() {
                let _ = button.focus();
            }
        }
    });

    view! {
        <Show when=move || menu_open.get()>
            <div
                class="ui-sheet"
                on:keydown=move |ev| {
                    if ev.key() == "Escape" {
                        set_menu_open.set(false);
                    }
                }
            >
                <div class="ui-sheet__backdrop" on:click=move |_| set_menu_open.set(false)></div>
                <div class="ui-sheet__panel" role="dialog" aria-modal="true" aria-label="Menu">
                    <div class="ui-sheet__head">
                        <span class="ui-label">"Menu"</span>
                        <button
                            node_ref=close_button
                            class="ui-btn ui-btn--ghost"
                            type="button"
                            on:click=move |_| set_menu_open.set(false)
                        >
                            <Icon name=IconName::X />
                            <span class="ui-sr-only">"Fermer le menu"</span>
                        </button>
                    </div>
                    <Suspense fallback=|| view! { <MenuSkeleton /> }>
                        {move || {
                            me.get()
                                .map(|result| {
                                    let user = result.ok().flatten();
                                    view! { <Menu sections=sections(user.as_ref()) on_pick=Some(close) /> }
                                })
                        }}
                    </Suspense>
                </div>
            </div>
        </Show>
    }
}

/// The shell.
#[component]
pub fn AppShell(
    /// The page.
    children: Children,
) -> impl IntoView {
    let me = Resource::new(|| (), |()| async { get_me().await });
    let (menu_open, set_menu_open) = signal(false);

    // A route change closes the phone menu.
    let location = use_location();
    Effect::new(move |_| {
        location.pathname.track();
        set_menu_open.set(false);
    });

    view! {
        <div class="ui-shell ui-ground">
            <a class="ui-sr-only ui-skip" href="#contenu">"Aller au contenu"</a>
            <TopBar me />

            <aside class="ui-shell__rail">
                <Suspense fallback=|| view! { <MenuSkeleton /> }>
                    {move || {
                        me.get()
                            .map(|result| {
                                let user = result.ok().flatten();
                                view! { <Menu sections=sections(user.as_ref()) /> }
                            })
                    }}
                </Suspense>
            </aside>

            <main class="ui-shell__main" id="contenu">
                <Suspense fallback=|| ()>
                    {move || {
                        me.get()
                            .and_then(Result::ok)
                            .flatten()
                            .filter(|user| user.needs_onboarding)
                            .map(|_| view! { <OnboardingNotice /> })
                    }}
                </Suspense>
                {children()}
                <footer class="ui-shell__footer">
                    <p class="ui-meta">"GameCloud OS · association game-dev d'Epitech Bénin"</p>
                </footer>
            </main>

            <TabBar me menu_open set_menu_open />
            <MenuSheet me menu_open set_menu_open />
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn member() -> MeView {
        MeView {
            is_member: true,
            ..MeView::default()
        }
    }

    fn labels(sections: &[Section]) -> Vec<&'static str> {
        sections.iter().map(|s| s.label).collect()
    }

    fn hrefs(sections: &[Section]) -> Vec<&'static str> {
        sections.iter().flat_map(|s| s.items.iter().map(|i| i.href)).collect()
    }

    #[test]
    fn a_visitor_sees_the_tests_and_kumo_and_nothing_internal() {
        let menu = sections(None);
        assert_eq!(labels(&menu), ["Association", "Contact"]);
        let links = hrefs(&menu);
        assert!(links.contains(&"/tests"));
        assert!(links.contains(&"/kumo"));
        for internal in ["/calendar", "/leaderboard", "/projects", "/shares", "/resources"] {
            assert!(!links.contains(&internal), "{internal} shown to a visitor");
        }
    }

    #[test]
    fn an_account_without_a_verified_address_sees_nothing_internal() {
        // Signed in but not a member yet: a candidate waiting on a test.
        let candidate = MeView {
            can_see_tests: true,
            ..MeView::default()
        };
        let links = hrefs(&sections(Some(&candidate)));
        for internal in ["/projects", "/shares", "/resources", "/leaderboard"] {
            assert!(!links.contains(&internal), "{internal} shown to a candidate");
        }
        assert!(links.contains(&"/tests"));
    }

    #[test]
    fn a_member_sees_their_pages_but_not_the_tests() {
        let menu = sections(Some(&member()));
        assert_eq!(labels(&menu), ["Moi", "Association", "Contact"]);
        let links = hrefs(&menu);
        assert!(links.contains(&"/calendar"));
        assert!(links.contains(&"/leaderboard"));
        assert!(links.contains(&"/projects"));
        assert!(links.contains(&"/resources"));
        assert!(!links.contains(&"/tests"));
    }

    #[test]
    fn roles_add_their_section_and_nothing_else() {
        let bureau = MeView {
            can_access_admin: true,
            can_see_tests: true,
            ..member()
        };
        let menu = sections(Some(&bureau));
        assert_eq!(labels(&menu), ["Moi", "Association", "Contact", "Responsabilités"]);
        let links = hrefs(&menu);
        assert!(links.contains(&"/admin"));
        assert!(links.contains(&"/tests"));
        assert!(!links.contains(&"/scan"));
    }

    #[test]
    fn an_unverified_account_has_no_leaderboard() {
        let unverified = MeView {
            can_see_tests: true,
            ..MeView::default()
        };
        let links = hrefs(&sections(Some(&unverified)));
        assert!(!links.contains(&"/leaderboard"));
        assert!(links.contains(&"/tests"));
        assert!(links.contains(&"/kumo"));
    }

    #[test]
    fn the_bottom_bar_fits_four_destinations_and_the_menu() {
        assert_eq!(tab_bar(None).map(|d| d.href), ["/", "/tests", "/projects", "/kumo"]);
        assert_eq!(tab_bar(Some(&member())).map(|d| d.href), ["/", "/quests", "/calendar", "/profile"]);
    }
}

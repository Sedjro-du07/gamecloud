//! Landing page.
//!
//! Two faces. Signed out, it is the pitch: what the association is, how
//! progression works, and the whole ladder of titles to climb. Signed in,
//! the pitch is replaced by a **mission panel** — your title, how far the
//! next one is, your streak and your place — because a member opening the
//! site wants to know where they stand, not to be sold the club again.

use gamecloud_shared::roles::Track;
use leptos::prelude::*;

use crate::{
    api::{format_xp, level_percent, MeView},
    components::title_ladder::TitleLadder,
    server_fns::{get_me, get_upcoming},
};

/// Home page.
#[component]
pub fn HomePage() -> impl IntoView {
    let me = Resource::new(|| (), |()| async { get_me().await });
    let signed_in = move || me.get().and_then(Result::ok).flatten();

    view! {
        <section class="gc-home">
            <Suspense fallback=|| view! { <Hero /> }>
                {move || match signed_in() {
                    Some(user) => view! { <MissionPanel me=user /> }.into_any(),
                    None => view! { <Hero /> }.into_any(),
                }}
            </Suspense>

            <HowToProgress />

            <section class="gc-home__section">
                <h2 class="gc-section-title">"L'échelle des titres"</h2>
                <p class="gc-lead">
                    "Ton XP globale te fait monter de titre en titre. Le dernier ne se
                     dévoile qu'à celui ou celle qui l'atteint."
                </p>
                <Suspense fallback=|| view! { <TitleLadder current=None /> }>
                    {move || view! { <TitleLadder current=signed_in().map(|u| u.global_rank) /> }}
                </Suspense>
            </section>

            <TrackStrip />
            <UpcomingStrip />
        </section>
    }
}

/// The pitch, for somebody who is not signed in.
#[component]
fn Hero() -> impl IntoView {
    view! {
        <header class="gc-hero">
            <p class="gc-kicker">"Epitech Bénin · association game-dev"</p>
            <h1 class="gc-hero__title">"GameCloud OS"</h1>
            <p class="gc-hero__tagline">
                "Code, crée, viens aux séances : chaque action te rapporte de l'XP,
                 et l'XP te fait monter en titre."
            </p>
            <div class="gc-hero__cta">
                // `rel="external"` tells the Leptos client-side router
                // not to intercept the click; the browser performs a
                // real navigation to the Axum endpoint, which redirects
                // to Discord. Without it, the first click is swallowed
                // by the router and only a refresh actually navigates.
                <a class="gc-btn gc-btn--primary gc-btn--lg" href="/api/auth/login" rel="external">
                    "🎮 Rejoindre la partie"
                </a>
                <a class="gc-btn gc-btn--ghost gc-btn--lg" href="/leaderboard">
                    "Voir le classement"
                </a>
            </div>
            <ul class="gc-hero__stats">
                <li>
                    <strong>"8"</strong>
                    <span>"tracks"</span>
                </li>
                <li>
                    <strong>"10"</strong>
                    <span>"titres à conquérir"</span>
                </li>
                <li>
                    <strong>"∞"</strong>
                    <span>"quêtes"</span>
                </li>
            </ul>
        </header>
    }
}

/// Where a signed-in member stands, and what to do next.
#[component]
#[allow(clippy::needless_pass_by_value)] // Leptos prop convention
fn MissionPanel(me: MeView) -> impl IntoView {
    let pct = if me.rank_xp_needed == 0 {
        100.0
    } else {
        level_percent(me.rank_xp_into, me.rank_xp_needed)
    };
    let next = me.next_rank_title.clone().map_or_else(
        || "Titre le plus haut atteint. Respect.".to_string(),
        |title| {
            let left = (me.rank_xp_needed - me.rank_xp_into).max(0);
            format!("Encore {} XP avant {title}", format_xp(left))
        },
    );
    // Every title held, most important first.
    let titles = me
        .bureau_title
        .clone()
        .map_or_else(|| me.rank_title.clone(), |b| format!("{b} · {}", me.rank_title));
    let position = me
        .leaderboard_position
        .map_or_else(|| "—".to_string(), |p| format!("#{p}"));

    view! {
        <header class="gc-mission" style=format!("--gc-ring: {};", me.rank_color)>
            <p class="gc-kicker">"Bon retour dans la partie"</p>
            <h1 class="gc-mission__name">{me.display_name.clone()}</h1>
            <p class="gc-mission__titles">{titles}</p>

            <div
                class="gc-mission__bar"
                role="progressbar"
                aria-valuemin="0"
                aria-valuemax="100"
                aria-valuenow=format!("{pct:.0}")
            >
                <div class="gc-mission__fill" style=format!("width: {pct:.1}%;")></div>
            </div>
            <p class="gc-mission__next">{next}</p>

            <ul class="gc-mission__stats">
                <li>
                    <strong>{format_xp(me.xp_total)}</strong>
                    <span>"XP"</span>
                </li>
                <li>
                    <strong>{format!("🔥 {}", me.streak_days)}</strong>
                    <span>"jours de série"</span>
                </li>
                <li>
                    <strong>{position}</strong>
                    <span>"au classement"</span>
                </li>
            </ul>

            <div class="gc-hero__cta">
                {if me.needs_onboarding {
                    view! {
                        <a class="gc-btn gc-btn--primary" href="/onboarding/tracks" rel="external">
                            "🧭 Choisir mes tracks"
                        </a>
                    }
                        .into_any()
                } else {
                    view! {
                        <a class="gc-btn gc-btn--primary" href="/quests">
                            "🎯 Mes quêtes"
                        </a>
                    }
                        .into_any()
                }}
                <a class="gc-btn gc-btn--ghost" href="/profile">
                    "Ma fiche"
                </a>
            </div>
        </header>
    }
}

/// The three moves of the game.
#[component]
fn HowToProgress() -> impl IntoView {
    view! {
        <section class="gc-home__section">
            <h2 class="gc-section-title">"Comment on progresse"</h2>
            <ol class="gc-steps">
                <li class="gc-step">
                    <span class="gc-step__icon" aria-hidden="true">"🧭"</span>
                    <h3>"Choisis tes tracks"</h3>
                    <p>
                        "Engineering, Game Design, Narrative… une ou plusieurs.
                         À partir de deux, toute ton XP est multipliée."
                    </p>
                </li>
                <li class="gc-step">
                    <span class="gc-step__icon" aria-hidden="true">"⚡"</span>
                    <h3>"Gagne de l'XP"</h3>
                    <p>
                        "Pousse du code, scanne le QR aux séances, relis les projets
                         des autres, accomplis les quêtes du Bureau."
                    </p>
                </li>
                <li class="gc-step">
                    <span class="gc-step__icon" aria-hidden="true">"👑"</span>
                    <h3>"Monte en titre"</h3>
                    <p>
                        "De l'Initié au titre secret pour l'XP globale, et
                         d'Observateur à Mentor dans chaque track."
                    </p>
                </li>
            </ol>
        </section>
    }
}

/// The eight disciplines, each linking to its board.
#[component]
fn TrackStrip() -> impl IntoView {
    view! {
        <section class="gc-home__section">
            <h2 class="gc-section-title">"Les tracks"</h2>
            <ul class="gc-trackstrip">
                {Track::ALL
                    .iter()
                    .map(|track| {
                        view! {
                            <li style=format!("--gc-track: {};", track.color_hex())>
                                <a class="gc-trackstrip__item" href=format!("/tracks/{}", track.as_str())>
                                    <span class="gc-trackstrip__emoji" aria-hidden="true">
                                        {track.emoji()}
                                    </span>
                                    <span class="gc-trackstrip__name">{track.as_str()}</span>
                                </a>
                            </li>
                        }
                    })
                    .collect_view()}
            </ul>
        </section>
    }
}

/// The next few things on the calendar.
///
/// On the landing page rather than behind a login, because "when is the
/// next session" is the question a prospective member asks before they
/// have an account, and making them sign in to find out is a strange
/// way to run an association.
#[component]
fn UpcomingStrip() -> impl IntoView {
    let events = Resource::new(|| (), |()| async { get_upcoming().await });

    view! {
        <Suspense fallback=|| ()>
            {move || {
                let list = events.get().and_then(Result::ok).unwrap_or_default();
                (!list.is_empty())
                    .then(|| {
                        view! {
                            <section class="gc-home__next">
                                <h2>"📅 À venir"</h2>
                                <ul>
                                    {list
                                        .into_iter()
                                        .map(|e| {
                                            view! {
                                                <li>
                                                    <span class="gc-home__next-when">
                                                        {e.when_label}
                                                    </span>
                                                    <span class="gc-home__next-title">
                                                        {e.track_emoji.unwrap_or_default()} " "
                                                        {e.title}
                                                    </span>
                                                    <span class="gc-chip">{e.kind_label}</span>
                                                </li>
                                            }
                                        })
                                        .collect_view()}
                                </ul>
                                <a class="gc-link" href="/calendar">"Tout le calendrier →"</a>
                            </section>
                        }
                    })
            }}
        </Suspense>
    }
}

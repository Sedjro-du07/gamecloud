//! Profile page — the character sheet.
//!
//! Everything the platform knows about a member, in one place: rank and
//! level, the streak that multiplies their XP, their tracks, their
//! badges, and the ledger of what they actually did. This is the page
//! that makes the record worth something outside the club.

use leptos::prelude::*;

use crate::{
    api::{format_xp, level_percent, MeView, SheetView, XpEntry},
    components::{
        badge_grid::BadgeGrid, character_card::CharacterCard, track_list::TrackList,
    },
    server_fns::get_sheet,
};

/// Re-exported for the HUD, which needs the same lookup.
pub use crate::server_fns::get_me as get_current_user;

/// Level progress bar.
#[component]
#[allow(clippy::needless_pass_by_value)] // Leptos prop convention
fn LevelBar(me: MeView) -> impl IntoView {
    let pct = level_percent(me.level_xp_into, me.level_xp_needed);
    view! {
        <div class="gc-level">
            <div class="gc-level__head">
                <span class="gc-level__label">{format!("Niveau {}", me.level)}</span>
                <span class="gc-level__count">
                    {format!("{} / {} XP", me.level_xp_into, me.level_xp_needed)}
                </span>
            </div>
            <div
                class="gc-level__track"
                role="progressbar"
                aria-valuemin="0"
                aria-valuemax="100"
                aria-valuenow=format!("{pct:.0}")
            >
                <div
                    class="gc-level__fill"
                    style=format!("width: {pct:.1}%; background: {};", me.rank_color)
                ></div>
            </div>
        </div>
    }
}

/// The three headline stats.
#[component]
#[allow(clippy::needless_pass_by_value)] // Leptos prop convention
fn StatStrip(me: MeView) -> impl IntoView {
    let streak_note = if me.streak_days >= 7 {
        format!("×{:.2} sur tout ton XP", me.streak_multiplier)
    } else if me.streak_days > 0 {
        format!("{} j avant le bonus ×1.25", 7 - me.streak_days)
    } else {
        "Gagne de l'XP aujourd'hui pour lancer ta série".to_string()
    };

    view! {
        <ul class="gc-stats">
            <li class="gc-stat">
                <span class="gc-stat__value">{format_xp(me.xp_total)}</span>
                <span class="gc-stat__label">"XP total"</span>
            </li>
            <li class="gc-stat">
                <span class="gc-stat__value">
                    {if me.streak_days > 0 {
                        format!("🔥 {}", me.streak_days)
                    } else {
                        "—".to_string()
                    }}
                </span>
                <span class="gc-stat__label">"Série"</span>
                <span class="gc-stat__note">{streak_note}</span>
            </li>
            <li class="gc-stat">
                <span class="gc-stat__value">
                    {me.leaderboard_position
                        .map_or_else(|| "—".to_string(), |p| format!("#{p}"))}
                </span>
                <span class="gc-stat__label">"Classement"</span>
            </li>
        </ul>
    }
}

/// The profile body, once the sheet has loaded.
///
/// Split out of [`ProfilePage`] so that component stays a thin
/// load-state switch and this one owns the layout.
#[component]
#[allow(clippy::needless_pass_by_value)] // Leptos prop convention
fn SheetBody(sheet: SheetView) -> impl IntoView {
    let me = sheet.me;
    let email_banner = (!me.email_verified).then(|| {
        view! {
            <div class="gc-banner gc-banner--warning">
                "Email Epitech pas encore validé. "
                <a href="/onboarding/email" rel="external">"Compléter"</a>
                "."
            </div>
        }
    });
    let onboarding_banner = me.needs_onboarding.then(|| {
        view! {
            <div class="gc-banner">
                "Dernière étape : choisis ta track pour rejoindre l'échelle d'XP. "
                <a href="/onboarding/tracks" rel="external">"Choisir maintenant"</a>
            </div>
        }
    });
    let bureau = me
        .bureau_title
        .clone()
        .map(|t| view! { <p class="gc-profile__bureau">{t}</p> });

    view! {
        <>
            {email_banner}
            {onboarding_banner}
            <CharacterCard
                name=me.display_name.clone()
                avatar_url=me.avatar_url.clone()
                xp_total=me.xp_total
                global_rank=me.global_rank.clone()
                title=Some(me.rank_title.clone())
            />
            {bureau}
            <LevelBar me=me.clone() />
            <StatStrip me=me />
            <TrackList tracks=sheet.tracks />
            <BadgeGrid badges=sheet.badges />
            <XpHistory entries=sheet.recent_xp />
        </>
    }
}

/// Recent ledger lines.
#[component]
#[allow(clippy::needless_pass_by_value)] // Leptos prop convention
fn XpHistory(entries: Vec<XpEntry>) -> impl IntoView {
    view! {
        <section class="gc-history">
            <h2>"Activité récente"</h2>
            {if entries.is_empty() {
                view! {
                    <p class="gc-empty">
                        "Rien pour l'instant. Pousse du code, viens à une session,
                         ou relis le projet de quelqu'un."
                    </p>
                }
                    .into_any()
            } else {
                view! {
                    <ul class="gc-history__list">
                        {entries
                            .into_iter()
                            .map(|entry| {
                                view! {
                                    <li class="gc-history__item">
                                        <span class="gc-history__when">{entry.when}</span>
                                        <span class="gc-history__amount">
                                            {format!("{:+}", entry.amount)} " XP"
                                        </span>
                                        <span class="gc-history__source">{entry.source}</span>
                                        <span class="gc-history__desc">
                                            {entry.description.unwrap_or_default()}
                                        </span>
                                    </li>
                                }
                            })
                            .collect_view()}
                    </ul>
                }
                    .into_any()
            }}
        </section>
    }
}

/// Profile page.
#[component]
pub fn ProfilePage() -> impl IntoView {
    let sheet = Resource::new(|| (), |()| async { get_sheet().await });

    view! {
        <section class="gc-profile">
            <Suspense fallback=move || view! { <p class="gc-empty">"Chargement…"</p> }>
                {move || match sheet.get() {
                    None => view! { <p class="gc-empty">"Chargement…"</p> }.into_any(),
                    Some(Err(_)) => {
                        view! { <p class="gc-empty">"Erreur lors du chargement du profil."</p> }
                            .into_any()
                    }
                    Some(Ok(None)) => {
                        view! {
                            <p class="gc-empty">
                                "Pas encore connecté. "
                                <a href="/api/auth/login" rel="external">
                                    "Se connecter avec Discord"
                                </a>
                                "."
                            </p>
                        }
                            .into_any()
                    }
                    Some(Ok(Some(sheet))) => view! { <SheetBody sheet /> }.into_any(),
                }}
            </Suspense>
        </section>
    }
}

/// Kept so existing imports of `CurrentUserView` keep resolving; the
/// canonical shape now lives in [`crate::api::MeView`].
pub type CurrentUserView = MeView;

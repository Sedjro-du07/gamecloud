//! Leaderboard page.
//!
//! Three scopes share one table. **Season is the default**, on purpose:
//! an all-time board freezes, because the members who founded the club
//! sit on top of it permanently and a first-year joining in September
//! can see they will never catch up. The season board gives everybody
//! something they can actually win, while the all-time tab keeps the
//! permanent record for those who earned it.

use leptos::prelude::*;

use crate::{components::leaderboard_table::LeaderboardTable, server_fns::get_leaderboard};

/// Leaderboard page.
#[component]
pub fn LeaderboardPage() -> impl IntoView {
    let (scope, set_scope) = signal("season".to_string());
    let (track, set_track) = signal(String::new());

    let board = Resource::new(
        move || (scope.get(), track.get()),
        |(scope, track)| async move {
            let track = if track.is_empty() { None } else { Some(track) };
            get_leaderboard(scope, track).await
        },
    );

    // The 8 tracks, for the track-scope selector. Static, so no fetch.
    let track_options = gamecloud_shared::roles::Track::ALL
        .iter()
        .map(|t| (t.as_str().to_string(), format!("{} {}", t.emoji(), t.as_str())))
        .collect::<Vec<_>>();

    view! {
        <section class="gc-leaderboard">
            <h1>"Classement"</h1>

            <nav class="gc-tabs" aria-label="Portée du classement">
                <button
                    class=move || tab_class(&scope.get(), "season")
                    on:click=move |_| set_scope.set("season".to_string())
                >
                    "Saison"
                </button>
                <button
                    class=move || tab_class(&scope.get(), "all")
                    on:click=move |_| set_scope.set("all".to_string())
                >
                    "Depuis toujours"
                </button>
                <button
                    class=move || tab_class(&scope.get(), "track")
                    on:click=move |_| {
                        set_scope.set("track".to_string());
                        if track.get_untracked().is_empty() {
                            set_track.set("Engineering".to_string());
                        }
                    }
                >
                    "Par track"
                </button>
            </nav>

            <Show when=move || scope.get() == "track">
                <label class="gc-field gc-field--inline">
                    <span>"Track"</span>
                    <select
                        on:change=move |ev| set_track.set(event_target_value(&ev))
                        prop:value=move || track.get()
                    >
                        {track_options
                            .clone()
                            .into_iter()
                            .map(|(id, label)| view! { <option value=id>{label}</option> })
                            .collect_view()}
                    </select>
                </label>
            </Show>

            <Suspense fallback=move || {
                view! { <p class="gc-empty">"Chargement du classement…"</p> }
            }>
                {move || match board.get() {
                    None => view! { <p class="gc-empty">"Chargement du classement…"</p> }.into_any(),
                    Some(Err(_)) => {
                        view! {
                            <p class="gc-empty">"Impossible de charger le classement."</p>
                        }
                            .into_any()
                    }
                    Some(Ok(view_model)) if view_model.restricted => {
                        view! {
                            <div class="gc-banner gc-banner--warning">
                                "Le classement est réservé aux membres inscrits. "
                                <a href="/api/auth/login" rel="external">"Connecte-toi avec Discord"</a>
                                " et vérifie ton adresse Epitech pour le voir."
                            </div>
                        }
                            .into_any()
                    }
                    Some(Ok(view_model)) => {
                        let caption = match (view_model.scope.as_str(), view_model.label.clone()) {
                            ("season", Some(name)) => format!("Saison en cours — {name}"),
                            ("track", Some(name)) => format!("Track {name}"),
                            _ => "Classement général, toutes saisons confondues".to_string(),
                        };
                        view! {
                            <>
                                <p class="gc-leaderboard__caption">{caption}</p>
                                <LeaderboardTable entries=view_model.entries />
                            </>
                        }
                            .into_any()
                    }
                }}
            </Suspense>
        </section>
    }
}

/// Active-tab styling helper.
fn tab_class(current: &str, this: &str) -> &'static str {
    if current == this {
        "gc-tab gc-tab--active"
    } else {
        "gc-tab"
    }
}

#[cfg(all(test, feature = "ssr"))]
mod tests {
    use super::tab_class;

    #[test]
    fn the_selected_tab_is_marked_active() {
        assert!(tab_class("season", "season").contains("--active"));
        assert!(!tab_class("season", "all").contains("--active"));
    }
}

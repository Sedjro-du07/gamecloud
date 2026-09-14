//! Projects index — the Hall of Fame.
//!
//! Lists released work, filterable by track. Pre-release projects are
//! deliberately absent: the public index is the club's record, and a
//! half-finished build is the team's business until it ships.

use leptos::prelude::*;

use crate::{components::project_card::ProjectCard, server_fns::get_projects};

/// Projects index page.
#[component]
pub fn ProjectsPage() -> impl IntoView {
    let (track, set_track) = signal(String::new());
    let projects = Resource::new(
        move || track.get(),
        |track| async move {
            let filter = if track.is_empty() { None } else { Some(track) };
            get_projects(filter).await
        },
    );

    let track_options = gamecloud_shared::roles::Track::ALL
        .iter()
        .map(|t| (t.as_str().to_string(), format!("{} {}", t.emoji(), t.as_str())))
        .collect::<Vec<_>>();

    view! {
        <section class="gc-projects">
            <header class="gc-projects__head">
                <h1>"Hall of Fame"</h1>
                <label class="gc-field gc-field--inline">
                    <span>"Track"</span>
                    <select
                        on:change=move |ev| set_track.set(event_target_value(&ev))
                        prop:value=move || track.get()
                    >
                        <option value="">"Toutes"</option>
                        {track_options
                            .clone()
                            .into_iter()
                            .map(|(id, label)| view! { <option value=id>{label}</option> })
                            .collect_view()}
                    </select>
                </label>
            </header>

            <Suspense fallback=move || {
                view! { <p class="gc-empty">"Chargement des projets…"</p> }
            }>
                {move || match projects.get() {
                    None => view! { <p class="gc-empty">"Chargement des projets…"</p> }.into_any(),
                    Some(Err(_)) => {
                        view! { <p class="gc-empty">"Impossible de charger les projets."</p> }
                            .into_any()
                    }
                    Some(Ok(list)) if list.is_empty() => {
                        view! {
                            <p class="gc-empty">
                                "Aucun projet publié pour l'instant. Le premier qui
                                 passe la revue de track atterrit ici."
                            </p>
                        }
                            .into_any()
                    }
                    Some(Ok(list)) => {
                        view! {
                            <div class="gc-projects__grid">
                                {list
                                    .into_iter()
                                    .map(|project| view! { <ProjectCard project /> })
                                    .collect_view()}
                            </div>
                        }
                            .into_any()
                    }
                }}
            </Suspense>
        </section>
    }
}

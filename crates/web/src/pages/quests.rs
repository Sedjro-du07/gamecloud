//! Quests page.
//!
//! Open quests with the member's progress. Completed ones sink to the
//! bottom so the actionable list stays at the top.

use leptos::prelude::*;

use crate::{components::quest_card::QuestCard, server_fns::{get_me, get_quests}};

/// Quests page.
#[component]
pub fn QuestsPage() -> impl IntoView {
    let quests = Resource::new(|| (), |()| async { get_quests().await });
    let me = Resource::new(|| (), |()| async { get_me().await });

    view! {
        <section class="gc-quests">
            <h1>"Quêtes"</h1>
            <p class="gc-quests__intro">
                "Les quêtes sont proposées par le Bureau. Elles se valident
                 toutes seules : continue à pousser du code, à venir aux
                 sessions et à relire les projets des autres."
            </p>

            // Opening a quest lives in the Bureau panel, next to the
            // other things only the Bureau does. Saying so here saves a
            // member of the Bureau hunting for a button on the page
            // where quests are actually read.
            <Suspense fallback=|| ()>
                {move || {
                    me.get()
                        .and_then(Result::ok)
                        .flatten()
                        .filter(|u| u.can_access_admin)
                        .map(|_| {
                            view! {
                                <p class="gc-quests__bureau">
                                    "Vous êtes du Bureau : "
                                    <a class="gc-link" href="/admin">"ouvrez une quête depuis le panneau Bureau"</a>
                                    "."
                                </p>
                            }
                        })
                }}
            </Suspense>

            <Suspense fallback=move || {
                view! { <p class="gc-empty">"Chargement des quêtes…"</p> }
            }>
                {move || match quests.get() {
                    None => view! { <p class="gc-empty">"Chargement des quêtes…"</p> }.into_any(),
                    Some(Err(_)) => {
                        view! { <p class="gc-empty">"Impossible de charger les quêtes."</p> }
                            .into_any()
                    }
                    Some(Ok(list)) if list.is_empty() => {
                        view! {
                            <p class="gc-empty">
                                "Aucune quête en cours. Reviens lundi — le Bureau en
                                 publie une nouvelle chaque semaine."
                            </p>
                        }
                            .into_any()
                    }
                    Some(Ok(mut list)) => {
                        // Unfinished first, then by how close to done.
                        list.sort_by_key(|q| {
                            let remaining = (q.target - q.progress).max(0);
                            (q.completed, remaining)
                        });
                        view! {
                            <div class="gc-quests__grid">
                                {list
                                    .into_iter()
                                    .map(|quest| view! { <QuestCard quest /> })
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

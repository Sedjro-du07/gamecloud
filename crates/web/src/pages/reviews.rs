//! Review queue — the work waiting for you.
//!
//! A reviewer only sees projects in tracks where they hold `Reviewer` or
//! above, and never their own work. That filtering happens in SQL, not
//! here: a queue showing things you have no standing to judge is noise,
//! and hiding them in the browser would not stop anyone acting on them.

use leptos::prelude::*;

use crate::server_fns::get_review_queue;

/// Review queue page.
#[component]
pub fn ReviewsPage() -> impl IntoView {
    let queue = Resource::new(|| (), |()| async { get_review_queue().await });

    view! {
        <section class="gc-reviews">
            <h1>"À relire"</h1>
            <p class="gc-reviews__intro">
                "Les projets qui attendent l'avis d'une track où vous êtes relecteur
                 ou au-dessus. Un refus doit expliquer ce qui doit changer — c'est ce
                 retour qui fait progresser l'équipe, pas la note."
            </p>

            <Suspense fallback=move || view! { <p class="gc-empty">"Chargement…"</p> }>
                {move || match queue.get() {
                    None => view! { <p class="gc-empty">"Chargement…"</p> }.into_any(),
                    Some(Err(_)) => {
                        view! { <p class="gc-empty">"Impossible de charger la file."</p> }
                            .into_any()
                    }
                    Some(Ok(items)) if items.is_empty() => {
                        view! {
                            <p class="gc-empty">
                                "Rien à relire pour l'instant. Soit tout est à jour,
                                 soit vous n'êtes relecteur sur aucune track — dans ce
                                 cas, gagnez 300 XP dans une track pour le devenir."
                            </p>
                        }
                            .into_any()
                    }
                    Some(Ok(items)) => {
                        view! {
                            <ul class="gc-reviews__list">
                                {items
                                    .into_iter()
                                    .map(|item| {
                                        let href = format!("/projects/{}", item.project_id);
                                        view! {
                                            <li class="gc-reviews__item">
                                                <div class="gc-reviews__body">
                                                    <a class="gc-reviews__name" href=href>
                                                        {item.name}
                                                    </a>
                                                    {item
                                                        .short_description
                                                        .map(|d| {
                                                            view! { <p class="gc-reviews__desc">{d}</p> }
                                                        })}
                                                    <p class="gc-reviews__meta">
                                                        "Soumis par " {item.author_name}
                                                    </p>
                                                </div>
                                                <span class="gc-chip">{item.track}</span>
                                            </li>
                                        }
                                    })
                                    .collect_view()}
                            </ul>
                        }
                            .into_any()
                    }
                }}
            </Suspense>
        </section>
    }
}

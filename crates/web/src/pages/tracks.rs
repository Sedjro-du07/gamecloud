//! Track selection — the third and final onboarding step.
//!
//! This page is what unblocks the whole horizontal axis of the design.
//! Before it existed there was no path into `track_memberships`, so
//! track XP never accrued, track roles never progressed, the per-track
//! leaderboard was permanently empty, and the multi-track bonus always
//! evaluated to 1.0 because nobody was ever in a track. Joining a track
//! is also what promotes a `Visitor` to `Initiate` and puts them on the
//! XP ladder.

use leptos::prelude::*;

use crate::{
    api::TrackOption,
    server_fns::{get_track_options, join_track},
};

/// Outcome of a join attempt, as shown in the banner.
type Feedback = Result<String, String>;

/// One selectable track, with its specialization picker.
#[component]
#[allow(clippy::needless_pass_by_value)] // Leptos prop convention
fn TrackOptionCard(
    /// The track on offer.
    option: TrackOption,
    /// Which track is currently expanded.
    selected: ReadSignal<Option<String>>,
    /// Expand a track.
    set_selected: WriteSignal<Option<String>>,
    /// Fire the join.
    on_join: Callback<(String, Option<String>)>,
    /// Whether a join is in flight.
    pending: Memo<bool>,
) -> impl IntoView {
    let id = option.id.clone();
    let specs = option.specializations.clone();
    let (specialization, set_specialization) = signal(String::new());

    let is_selected = {
        let id = id.clone();
        move || selected.get().as_deref() == Some(id.as_str())
    };

    let pick = {
        let id = id.clone();
        move |_| {
            set_selected.set(Some(id.clone()));
            set_specialization.set(String::new());
        }
    };

    let confirm = {
        let id = id.clone();
        move |_| {
            let spec = specialization.get();
            let spec = (!spec.trim().is_empty()).then_some(spec);
            on_join.run((id.clone(), spec));
        }
    };

    view! {
        <li
            class=move || {
                if is_selected() {
                    "gc-track-option gc-track-option--selected"
                } else {
                    "gc-track-option"
                }
            }
            style=format!("--gc-track: {};", option.color)
        >
            <button class="gc-track-option__button" disabled=option.joined on:click=pick>
                <span class="gc-track-option__emoji">{option.emoji}</span>
                <span class="gc-track-option__name">{id}</span>
                {option
                    .joined
                    .then(|| view! { <span class="gc-chip">"Déjà rejointe"</span> })}
            </button>

            <Show when=is_selected.clone()>
                <div class="gc-track-option__detail">
                    <label class="gc-field">
                        <span>"Spécialisation (optionnel)"</span>
                        <select on:change=move |ev| {
                            set_specialization.set(event_target_value(&ev));
                        }>
                            <option value="">"— Je verrai plus tard —"</option>
                            {specs
                                .clone()
                                .into_iter()
                                .map(|s| {
                                    let value = s.clone();
                                    view! { <option value=value>{s}</option> }
                                })
                                .collect_view()}
                        </select>
                    </label>
                    <button
                        class="gc-btn gc-btn--primary"
                        disabled=move || pending.get()
                        on:click=confirm.clone()
                    >
                        {move || {
                            if pending.get() { "Inscription…" } else { "Rejoindre cette track" }
                        }}
                    </button>
                </div>
            </Show>
        </li>
    }
}

/// The grid of track options, once loaded.
#[component]
#[allow(clippy::needless_pass_by_value)] // Leptos prop convention
fn TrackGrid(
    /// Every track, flagged with whether the member already joined it.
    options: Vec<TrackOption>,
    /// Which track is expanded.
    selected: ReadSignal<Option<String>>,
    /// Expand a track.
    set_selected: WriteSignal<Option<String>>,
    /// Fire the join.
    on_join: Callback<(String, Option<String>)>,
    /// Whether a join is in flight.
    pending: Memo<bool>,
) -> impl IntoView {
    view! {
        <ul class="gc-tracks-picker__grid">
            {options
                .into_iter()
                .map(|option| {
                    view! {
                        <TrackOptionCard option selected set_selected on_join pending />
                    }
                })
                .collect_view()}
        </ul>
    }
}

/// Track selection page.
#[component]
pub fn TrackPickerPage() -> impl IntoView {
    let options = Resource::new(|| (), |()| async { get_track_options().await });
    let (selected, set_selected) = signal(Option::<String>::None);
    let (feedback, set_feedback) = signal(Option::<Feedback>::None);

    let join = Action::new(move |(track, spec): &(String, Option<String>)| {
        let track = track.clone();
        let spec = spec.clone();
        async move {
            match join_track(track.clone(), spec).await {
                Ok(()) => Ok(format!("Bienvenue dans la track {track} !")),
                Err(e) => Err(e.to_string()),
            }
        }
    });

    let pending = Memo::new(move |_| join.pending().get());
    let on_join = Callback::new(move |args: (String, Option<String>)| {
        join.dispatch(args);
    });

    // Reflect the action's result into the banner, and refresh the list
    // so a joined track immediately shows as joined.
    Effect::new(move |_| {
        if let Some(result) = join.value().get() {
            set_feedback.set(Some(result));
            options.refetch();
            set_selected.set(None);
        }
    });

    view! {
        <section class="gc-onboard gc-tracks-picker">
            <h1>"Choisis ta track"</h1>
            <p>
                "Une track, c'est ta discipline dans l'association. Tu peux en
                 rejoindre plusieurs — et c'est même encouragé : à partir de deux
                 tracks actives, tout ton XP est multiplié."
            </p>

            {move || {
                feedback
                    .get()
                    .map(|result| match result {
                        Ok(message) => {
                            view! { <div class="gc-banner gc-banner--ok">{message}</div> }
                                .into_any()
                        }
                        Err(message) => {
                            view! { <div class="gc-banner gc-banner--warning">{message}</div> }
                                .into_any()
                        }
                    })
            }}

            <Suspense fallback=move || view! { <p class="gc-empty">"Chargement…"</p> }>
                {move || match options.get() {
                    None | Some(Err(_)) => {
                        view! { <p class="gc-empty">"Chargement des tracks…"</p> }.into_any()
                    }
                    Some(Ok(list)) => {
                        view! {
                            <TrackGrid
                                options=list
                                selected
                                set_selected
                                on_join
                                pending
                            />
                        }
                            .into_any()
                    }
                }}
            </Suspense>

            <p class="gc-onboard__hint">
                <a href="/profile" rel="external">"Aller à mon profil"</a>
            </p>
        </section>
    }
}

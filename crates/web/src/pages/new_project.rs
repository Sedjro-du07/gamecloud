//! Project creation.
//!
//! Creating a project also creates its GitHub repository — private, with
//! the author as collaborator and the push webhook already installed.
//! That last part is why the form says so: a member who has not linked
//! their GitHub login gets a repository they cannot push to, and the
//! only moment they will read that warning is here.

use leptos::prelude::*;
use leptos_router::{hooks::use_navigate, NavigateOptions};

use crate::server_fns::create_project;

/// A labelled text input.
///
/// Four near-identical fields in one `view!` is what pushed this page
/// over the length limit; one component says the same thing once.
#[component]
fn TextField(
    /// Field label.
    label: &'static str,
    /// Placeholder shown when empty.
    placeholder: &'static str,
    /// Whether the field must be filled.
    required: bool,
    /// Current value.
    value: ReadSignal<String>,
    /// Setter.
    set_value: WriteSignal<String>,
) -> impl IntoView {
    view! {
        <label class="gc-field">
            <span>{label}</span>
            <input
                type="text"
                required=required
                placeholder=placeholder
                prop:value=move || value.get()
                on:input=move |ev| set_value.set(event_target_value(&ev))
            />
        </label>
    }
}

/// The two dropdowns: primary track and Epitech level.
#[component]
#[allow(clippy::needless_pass_by_value)] // Leptos prop convention
fn ProjectSelects(
    /// Current track.
    track: ReadSignal<String>,
    /// Track setter.
    set_track: WriteSignal<String>,
    /// Level setter.
    set_level: WriteSignal<String>,
) -> impl IntoView {
    let tracks = gamecloud_shared::roles::Track::ALL
        .iter()
        .map(|t| (t.as_str().to_string(), format!("{} {}", t.emoji(), t.as_str())))
        .collect::<Vec<_>>();

    view! {
        <label class="gc-field">
            <span>"Track principale"</span>
            <select
                on:change=move |ev| set_track.set(event_target_value(&ev))
                prop:value=move || track.get()
            >
                {tracks
                    .into_iter()
                    .map(|(id, label)| view! { <option value=id>{label}</option> })
                    .collect_view()}
            </select>
        </label>

        <label class="gc-field">
            <span>"Niveau Epitech (optionnel)"</span>
            <select on:change=move |ev| set_level.set(event_target_value(&ev))>
                <option value="">"— Aucun —"</option>
                <option value="Tek1">"Tek1"</option>
                <option value="Tek2">"Tek2"</option>
                <option value="Tek3">"Tek3"</option>
                <option value="Master">"Master"</option>
            </select>
        </label>
    }
}

/// Project creation page.
#[component]
pub fn NewProjectPage() -> impl IntoView {
    let (name, set_name) = signal(String::new());
    let (summary, set_summary) = signal(String::new());
    let (track, set_track) = signal("Engineering".to_string());
    let (level, set_level) = signal(String::new());
    let (error, set_error) = signal(Option::<String>::None);
    let navigate = use_navigate();

    let create = Action::new(
        move |(n, s, t, l): &(String, String, String, String)| {
            let (n, s, t, l) = (n.clone(), s.clone(), t.clone(), l.clone());
            async move { create_project(n, s, t, l).await }
        },
    );

    Effect::new(move |_| {
        if let Some(result) = create.value().get() {
            match result {
                Ok(id) => navigate(&format!("/projects/{id}"), NavigateOptions::default()),
                Err(e) => set_error.set(Some(e.to_string())),
            }
        }
    });

    view! {
        <section class="gc-onboard gc-new-project">
            <h1>"Nouveau projet"</h1>
            <p>
                "Le projet démarre en brouillon. Un dépôt GitHub privé est créé
                 automatiquement dans l'organisation de l'association, et il
                 passera public le jour où le projet est publié."
            </p>

            {move || {
                error
                    .get()
                    .map(|e| view! { <div class="gc-banner gc-banner--warning">{e}</div> })
            }}

            <form
                class="gc-form"
                on:submit=move |ev| {
                    ev.prevent_default();
                    if name.get().trim().is_empty() {
                        set_error.set(Some("Le nom est obligatoire.".into()));
                        return;
                    }
                    create.dispatch((name.get(), summary.get(), track.get(), level.get()));
                }
            >
                <TextField
                    label="Nom du projet"
                    placeholder="Aevaryn"
                    required=true
                    value=name
                    set_value=set_name
                />
                <TextField
                    label="En une phrase"
                    placeholder="Un RPG narratif en pixel art"
                    required=false
                    value=summary
                    set_value=set_summary
                />

                <ProjectSelects track set_track set_level />

                <button
                    class="gc-btn gc-btn--primary"
                    type="submit"
                    disabled=move || create.pending().get()
                >
                    {move || {
                        if create.pending().get() { "Création…" } else { "Créer le projet" }
                    }}
                </button>
            </form>

            <p class="gc-onboard__hint">
                "Pensez à lier votre compte GitHub sur votre profil : sans ça, vous
                 ne serez pas ajouté au dépôt, et vos commits ne rapporteront pas d'XP."
            </p>
        </section>
    }
}

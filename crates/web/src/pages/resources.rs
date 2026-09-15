//! The resource library.
//!
//! Members propose links, a validator signs off, and the submitter earns
//! XP for it. Validation is what separates a library from a dumping
//! ground, so unvalidated entries are visible only to the people who can
//! act on them — everyone else browses what the club has vouched for.

use leptos::prelude::*;

use crate::{
    api::ResourceItem,
    components::sign_in_prompt::SignInPrompt,
    server_fns::{act_on_resource, get_me, get_resources, submit_resource},
};

/// One entry, with whatever actions the viewer is entitled to.
#[component]
#[allow(clippy::needless_pass_by_value)] // Leptos prop convention
fn ResourceRow(
    /// The entry.
    item: ResourceItem,
    /// Called after an action, to refresh the list.
    on_changed: Callback<()>,
) -> impl IntoView {
    let act = Action::new(move |(id, what): &(String, String)| {
        let (id, what) = (id.clone(), what.clone());
        async move { act_on_resource(id, what).await }
    });
    let (error, set_error) = signal(Option::<String>::None);

    Effect::new(move |_| {
        if let Some(result) = act.value().get() {
            match result {
                Ok(()) => {
                    set_error.set(None);
                    on_changed.run(());
                }
                Err(e) => set_error.set(Some(e.to_string())),
            }
        }
    });

    let id = item.id.clone();
    let vote_id = id.clone();
    let validate_id = id;
    let voted = item.has_voted;

    view! {
        <li class=if item.validated { "gc-resource" } else { "gc-resource gc-resource--pending" }>
            <div class="gc-resource__body">
                <a class="gc-resource__title" href=item.url.clone() rel="external noopener">
                    {item.title}
                </a>
                <p class="gc-resource__meta">
                    {item.kind.unwrap_or_else(|| "Lien".into())}
                    {item.level.map(|l| format!(" · {l}"))}
                    " · proposé par " {item.submitted_by}
                    {(!item.tracks.is_empty()).then(|| format!(" · {}", item.tracks.join(", ")))}
                </p>
                {move || {
                    error.get().map(|e| view! { <p class="gc-resource__error">{e}</p> })
                }}
            </div>

            <div class="gc-resource__actions">
                <button
                    class=if voted { "gc-btn gc-btn--on" } else { "gc-btn" }
                    disabled=move || act.pending().get()
                    on:click={
                        let id = vote_id.clone();
                        move |_| {
                            act.dispatch((
                                id.clone(),
                                if voted { "unvote".into() } else { "vote".into() },
                            ));
                        }
                    }
                >
                    {format!("▲ {}", item.votes)}
                </button>

                <Show when=move || item.may_validate && !item.validated>
                    <button
                        class="gc-btn gc-btn--primary"
                        disabled=move || act.pending().get()
                        on:click={
                            let id = validate_id.clone();
                            move |_| { act.dispatch((id.clone(), "validate".into())); }
                        }
                    >
                        "Valider"
                    </button>
                </Show>

                <Show when=move || !item.validated && !item.may_validate>
                    <span class="gc-chip">"En attente"</span>
                </Show>
            </div>
        </li>
    }
}

/// The submission form.
#[component]
fn SubmitForm(
    /// Called after a successful submission.
    on_changed: Callback<()>,
) -> impl IntoView {
    let (title, set_title) = signal(String::new());
    let (url, set_url) = signal(String::new());
    let (kind, set_kind) = signal("Tutorial".to_string());
    let (track, set_track) = signal(String::new());
    let (error, set_error) = signal(Option::<String>::None);

    let send = Action::new(move |(t, u, k, tr): &(String, String, String, String)| {
        let (t, u, k, tr) = (t.clone(), u.clone(), k.clone(), tr.clone());
        async move { submit_resource(t, u, k, tr).await }
    });

    Effect::new(move |_| {
        if let Some(result) = send.value().get() {
            match result {
                Ok(()) => {
                    set_title.set(String::new());
                    set_url.set(String::new());
                    set_error.set(None);
                    on_changed.run(());
                }
                Err(e) => set_error.set(Some(e.to_string())),
            }
        }
    });

    let tracks = gamecloud_shared::roles::Track::ALL
        .iter()
        .map(|t| (t.as_str().to_string(), format!("{} {}", t.emoji(), t.as_str())))
        .collect::<Vec<_>>();

    view! {
        <form
            class="gc-form gc-resources__form"
            on:submit=move |ev| {
                ev.prevent_default();
                send.dispatch((title.get(), url.get(), kind.get(), track.get()));
            }
        >
            {move || {
                error.get().map(|e| view! { <div class="gc-banner gc-banner--warning">{e}</div> })
            }}

            <label class="gc-field">
                <span>"Titre"</span>
                <input
                    type="text"
                    required=true
                    prop:value=move || title.get()
                    on:input=move |ev| set_title.set(event_target_value(&ev))
                />
            </label>
            <label class="gc-field">
                <span>"Lien"</span>
                <input
                    type="url"
                    required=true
                    placeholder="https://…"
                    prop:value=move || url.get()
                    on:input=move |ev| set_url.set(event_target_value(&ev))
                />
            </label>
            <label class="gc-field">
                <span>"Type"</span>
                <select on:change=move |ev| set_kind.set(event_target_value(&ev))>
                    <option value="Tutorial">"Tutoriel"</option>
                    <option value="Tool">"Outil"</option>
                    <option value="Asset">"Asset"</option>
                    <option value="Doc">"Documentation"</option>
                    <option value="Video">"Vidéo"</option>
                </select>
            </label>
            <label class="gc-field">
                <span>"Track concernée (optionnel)"</span>
                <select on:change=move |ev| set_track.set(event_target_value(&ev))>
                    <option value="">"— Aucune —"</option>
                    {tracks
                        .into_iter()
                        .map(|(id, label)| view! { <option value=id>{label}</option> })
                        .collect_view()}
                </select>
            </label>
            <button
                class="gc-btn gc-btn--primary"
                type="submit"
                disabled=move || send.pending().get()
            >
                {move || if send.pending().get() { "Envoi…" } else { "Proposer" }}
            </button>
        </form>
    }
}

/// Resource library page.
#[component]
pub fn ResourcesPage() -> impl IntoView {
    let items = Resource::new(|| (), |()| async { get_resources().await });
    let on_changed = Callback::new(move |()| items.refetch());
    let me = Resource::new(|| (), |()| async { get_me().await });

    view! {
        <section class="gc-resources">
            <h1>"Ressources"</h1>
            <p class="gc-resources__intro">
                "Les liens que l'association recommande. Une ressource validée
                 rapporte de l'XP à qui l'a proposée."
            </p>

            // Proposing a resource is for members: a visitor is asked to
            // sign in, an unverified account to verify its address.
            <Suspense fallback=|| ()>
                {move || {
                    me.get()
                        .map(|result| match result.ok().flatten() {
                            Some(user) if user.email_verified => {
                                view! { <SubmitForm on_changed /> }.into_any()
                            }
                            Some(_) => {
                                view! {
                                    <div class="gc-banner gc-banner--warning">
                                        "Vérifie ton adresse Epitech pour proposer une ressource."
                                    </div>
                                }
                                    .into_any()
                            }
                            None => {
                                view! { <SignInPrompt what="proposer une ressource" /> }.into_any()
                            }
                        })
                }}
            </Suspense>

            <Suspense fallback=move || view! { <p class="gc-empty">"Chargement…"</p> }>
                {move || match items.get() {
                    None => view! { <p class="gc-empty">"Chargement…"</p> }.into_any(),
                    Some(Err(_)) => {
                        view! { <p class="gc-empty">"Impossible de charger la bibliothèque."</p> }
                            .into_any()
                    }
                    Some(Ok(list)) if list.is_empty() => {
                        view! {
                            <p class="gc-empty">
                                "La bibliothèque est vide. Proposez le premier lien."
                            </p>
                        }
                            .into_any()
                    }
                    Some(Ok(list)) => {
                        view! {
                            <ul class="gc-resources__list">
                                {list
                                    .into_iter()
                                    .map(|item| view! { <ResourceRow item on_changed /> })
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

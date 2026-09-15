//! Member shares.
//!
//! An open shelf: anyone signed in drops a zip or a link — a script,
//! some lore, a game, a pack of assets — and every member can download
//! it. Each download is counted and shown, which is the only feedback
//! an author gets, so it is on every card.
//!
//! The upload is a plain multipart form posting straight to the API, as
//! for project builds: it keeps a 500 MB file out of the WASM boundary.
//! The API redirects back here with `?ok` or `?erreur=…`.

use leptos::prelude::*;
use leptos_router::hooks::use_query_map;

use crate::{
    api::ShareItem,
    server_fns::{delete_share, get_shares},
};

/// Kinds offered: stored value, label, icon.
const KINDS: [(&str, &str, &str); 5] = [
    ("Script", "Script", "📜"),
    ("Lore", "Lore", "📖"),
    ("Game", "Jeu", "🎮"),
    ("Asset", "Assets", "🎨"),
    ("Other", "Autre", "📦"),
];

fn kind_icon(kind: &str) -> &'static str {
    KINDS
        .iter()
        .find(|(value, _, _)| *value == kind)
        .map_or("📦", |(_, _, icon)| icon)
}

/// One share.
#[component]
#[allow(clippy::needless_pass_by_value)] // Leptos prop convention
fn ShareRow(
    /// The share.
    item: ShareItem,
    /// Whether the viewer is signed in.
    signed_in: bool,
    /// Whether the viewer may download (verified address).
    can_download: bool,
    /// Called after removal, to refresh the list.
    on_changed: Callback<()>,
) -> impl IntoView {
    let (downloads, set_downloads) = signal(item.downloads);
    let (confirming, set_confirming) = signal(false);
    let (error, set_error) = signal(Option::<String>::None);

    let remove = Action::new(move |id: &String| {
        let id = id.clone();
        async move { delete_share(id).await }
    });
    Effect::new(move |_| {
        if let Some(result) = remove.value().get() {
            match result {
                Ok(()) => on_changed.run(()),
                Err(e) => set_error.set(Some(e.to_string())),
            }
        }
    });

    let href = format!("/api/shares/{}/download", item.id);
    // Stored rather than cloned: the remove button sits two `Show`s deep,
    // and each layer has to be able to rebuild its children.
    let id = StoredValue::new(item.id.clone());
    let is_link = item.is_link;
    let may_delete = item.may_delete;
    let detail = if is_link {
        item.host.clone().map(|h| format!("🔗 {h}"))
    } else {
        item.filename
            .clone()
            .map(|f| format!("{f} · {}", item.size.clone().unwrap_or_default()))
    };

    view! {
        <li class="gc-share">
            <span class="gc-share__icon" aria-hidden="true">{kind_icon(&item.kind)}</span>
            <div class="gc-share__body">
                <div class="gc-share__head">
                    <h3 class="gc-share__title">{item.title}</h3>
                    <span class="gc-chip">{item.kind_label}</span>
                </div>
                {item.description.map(|d| view! { <p class="gc-share__desc">{d}</p> })}
                <p class="gc-share__meta">
                    "par " {item.author} " · " {item.when}
                    {detail.map(|d| format!(" · {d}"))}
                </p>
                {move || error.get().map(|e| view! { <p class="gc-share__error">{e}</p> })}
            </div>

            <div class="gc-share__actions">
                <span class="gc-share__count" title="Téléchargements">
                    <strong>{move || downloads.get()}</strong>
                    {move || if downloads.get() == 1 { " téléchargement" } else { " téléchargements" }}
                </span>
                {if can_download {
                    view! {
                        <a
                            class="gc-btn gc-btn--primary"
                            href=href
                            rel=if is_link { "external noopener" } else { "external" }
                            target=if is_link { "_blank" } else { "_self" }
                            on:click=move |_| set_downloads.update(|n| *n += 1)
                        >
                            {if is_link { "Ouvrir le lien" } else { "Télécharger" }}
                        </a>
                    }
                        .into_any()
                } else if signed_in {
                    view! {
                        <a class="gc-btn" href="/onboarding/email">
                            "Vérifie ton adresse pour télécharger"
                        </a>
                    }
                        .into_any()
                } else {
                    view! {
                        <a class="gc-btn" href="/api/auth/login" rel="external">
                            "Se connecter pour télécharger"
                        </a>
                    }
                        .into_any()
                }}
                <Show when=move || may_delete>
                    <Show
                        when=move || confirming.get()
                        fallback=move || {
                            view! {
                                <button
                                    class="gc-btn gc-btn--ghost"
                                    on:click=move |_| set_confirming.set(true)
                                >
                                    "Retirer"
                                </button>
                            }
                        }
                    >
                        <button
                            class="gc-btn gc-btn--ghost"
                            disabled=move || remove.pending().get()
                            on:click=move |_| {
                                remove.dispatch(id.get_value());
                            }
                        >
                            "Confirmer le retrait"
                        </button>
                    </Show>
                </Show>
            </div>
        </li>
    }
}

/// The posting form.
#[component]
fn UploadForm() -> impl IntoView {
    let (link, set_link) = signal(false);
    let (sending, set_sending) = signal(false);

    view! {
        <form
            class="gc-form gc-shares__form"
            method="post"
            action="/api/shares"
            enctype="multipart/form-data"
            on:submit=move |_| set_sending.set(true)
        >
            <h2>"Déposer quelque chose"</h2>
            <div class="gc-shares__row">
                <label class="gc-field">
                    <span>"Titre"</span>
                    <input
                        type="text"
                        name="title"
                        required=true
                        maxlength="120"
                        placeholder="Pack de sprites forêt"
                    />
                </label>
                <label class="gc-field">
                    <span>"Type"</span>
                    <select name="kind">
                        {KINDS
                            .iter()
                            .map(|(value, label, icon)| {
                                view! { <option value=*value>{format!("{icon} {label}")}</option> }
                            })
                            .collect_view()}
                    </select>
                </label>
            </div>
            <label class="gc-field">
                <span>"Description (optionnel)"</span>
                <textarea
                    name="description"
                    rows="3"
                    maxlength="2000"
                    placeholder="De quoi il s'agit, comment l'utiliser, le moteur…"
                ></textarea>
            </label>

            <div class="gc-tabs gc-shares__mode" role="group" aria-label="Fichier ou lien">
                <button
                    type="button"
                    class=move || if link.get() { "gc-tab" } else { "gc-tab gc-tab--active" }
                    aria-pressed=move || (!link.get()).to_string()
                    on:click=move |_| set_link.set(false)
                >
                    "📁 Un fichier"
                </button>
                <button
                    type="button"
                    class=move || if link.get() { "gc-tab gc-tab--active" } else { "gc-tab" }
                    aria-pressed=move || link.get().to_string()
                    on:click=move |_| set_link.set(true)
                >
                    "🔗 Un lien"
                </button>
            </div>

            <Show
                when=move || link.get()
                fallback=|| {
                    view! {
                        <label class="gc-field">
                            <span>"Fichier — zip, script, pdf, build… (500 Mo maximum)"</span>
                            <input type="file" name="file" required=true />
                        </label>
                    }
                }
            >
                <label class="gc-field">
                    <span>"Lien (itch.io, GitHub, Drive…)"</span>
                    <input type="url" name="url" required=true placeholder="https://…" />
                </label>
            </Show>

            <button class="gc-btn gc-btn--primary" type="submit" disabled=move || sending.get()>
                {move || if sending.get() { "Envoi en cours…" } else { "Partager" }}
            </button>
        </form>
    }
}

/// The kind filter above the list. An empty value means every kind.
#[component]
fn KindTabs(
    /// Kind currently shown.
    filter: ReadSignal<String>,
    /// Change it.
    set_filter: WriteSignal<String>,
) -> impl IntoView {
    let tabs = std::iter::once(("", "Tout", "✨"))
        .chain(KINDS.iter().copied())
        .map(|(value, label, icon)| {
            let current = value.to_string();
            let target = current.clone();
            view! {
                <button
                    type="button"
                    class=move || {
                        if filter.get() == current { "gc-tab gc-tab--active" } else { "gc-tab" }
                    }
                    on:click=move |_| set_filter.set(target.clone())
                >
                    {format!("{icon} {label}")}
                </button>
            }
        })
        .collect_view();

    view! { <div class="gc-tabs" role="group" aria-label="Filtrer par type">{tabs}</div> }
}

/// Member shares page.
#[component]
pub fn SharesPage() -> impl IntoView {
    let shares = Resource::new(|| (), |()| async { get_shares().await });
    let on_changed = Callback::new(move |()| shares.refetch());
    let query = use_query_map();
    let (filter, set_filter) = signal(String::new());

    view! {
        <section class="gc-shares">
            <h1>"Partages"</h1>
            <p class="gc-shares__intro">
                "Scripts, lore, jeux, assets : dépose un zip ou un lien, et tous les
                 membres peuvent le télécharger."
            </p>

            {move || {
                query
                    .read()
                    .get("ok")
                    .map(|_| {
                        view! {
                            <div class="gc-banner gc-banner--ok">"C'est en ligne, merci du partage !"</div>
                        }
                    })
            }}
            {move || {
                query
                    .read()
                    .get("erreur")
                    .map(|e| view! { <div class="gc-banner gc-banner--warning">{e}</div> })
            }}

            <Suspense fallback=|| ()>
                {move || {
                    shares
                        .get()
                        .and_then(Result::ok)
                        .map(|view| {
                            if view.can_upload {
                                view! { <UploadForm /> }.into_any()
                            } else if view.signed_in {
                                view! {
                                    <div class="gc-banner gc-banner--warning">
                                        "Vérifie ton adresse Epitech pour pouvoir partager. "
                                        <a href="/onboarding/email">"C'est par ici"</a>
                                    </div>
                                }
                                    .into_any()
                            } else {
                                view! {
                                    <div class="gc-banner">
                                        <a href="/api/auth/login" rel="external">"Connecte-toi"</a>
                                        " pour partager et télécharger."
                                    </div>
                                }
                                    .into_any()
                            }
                        })
                }}
            </Suspense>

            <KindTabs filter set_filter />

            <Suspense fallback=move || view! { <p class="gc-empty">"Chargement…"</p> }>
                {move || {
                    shares
                        .get()
                        .map(|result| match result {
                            Err(_) => {
                                view! { <p class="gc-empty">"Impossible de charger les partages."</p> }
                                    .into_any()
                            }
                            Ok(view) => {
                                let wanted = filter.get();
                                let signed_in = view.signed_in;
                                let can_download = view.can_upload;
                                let items = view
                                    .items
                                    .into_iter()
                                    .filter(|i| wanted.is_empty() || i.kind == wanted)
                                    .collect::<Vec<_>>();
                                if items.is_empty() {
                                    view! {
                                        <p class="gc-empty">"Rien ici pour l'instant. Lance-toi !"</p>
                                    }
                                        .into_any()
                                } else {
                                    view! {
                                        <ul class="gc-shares__list">
                                            {items
                                                .into_iter()
                                                .map(|item| {
                                                    view! { <ShareRow item signed_in can_download on_changed /> }
                                                })
                                                .collect_view()}
                                        </ul>
                                    }
                                        .into_any()
                                }
                            }
                        })
                }}
            </Suspense>
        </section>
    }
}

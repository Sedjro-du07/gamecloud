//! Member shares — a grid of cards.
//!
//! An open shelf: anyone verified drops a zip or a link — a script, some
//! lore, a game, a pack of assets — and every member can download it. Each
//! download is counted and shown, which is the only feedback an author
//! gets, so it is on every card.
//!
//! The upload is a plain multipart form posting straight to the API, as for
//! project builds: it keeps a 500 MB file out of the WASM boundary. The API
//! redirects back here with `?ok` or `?erreur=…`.

use leptos::prelude::*;
use leptos_router::hooks::use_query_map;

use crate::{
    api::ShareItem,
    components::ui::{
        segment, segment_with_icon,
        vocab::{share_kind_icon, SHARE_KINDS},
        Button, ButtonKind, ButtonLink, Card, CardGrid, EmptyState, ErrorState, ErrorText, Field,
        FilterBar, Form, FormActions, FormRow, GridSkeleton, IconName, Notice, NoticeKind, Page,
        PageHeader, Panel, Pattern, Segment, SegmentedControl,
    },
    server_fns::{delete_share, get_shares},
};

/// "1 téléchargement", "12 téléchargements".
fn downloads_label(n: i64) -> String {
    if n == 1 {
        "1 téléchargement".to_string()
    } else {
        format!("{n} téléchargements")
    }
}

/// How the viewer gets at a share: download, or what stands in the way.
fn access_button(href: String, is_link: bool, can_download: bool, signed_in: bool, on_open: Callback<()>) -> AnyView {
    if can_download {
        view! {
            <ButtonLink
                kind=ButtonKind::Primary
                href
                external=!is_link
                new_tab=is_link
                icon=if is_link { IconName::ArrowSquareOut } else { IconName::DownloadSimple }
                on:click=move |_| on_open.run(())
            >
                {if is_link { "Ouvrir le lien" } else { "Télécharger" }}
            </ButtonLink>
        }
        .into_any()
    } else if signed_in {
        view! {
            <ButtonLink href="/onboarding/email" icon=IconName::Envelope>"Vérifier mon adresse pour télécharger"</ButtonLink>
        }
        .into_any()
    } else {
        view! {
            <ButtonLink href="/api/auth/login" external=true icon=IconName::DiscordLogo hide_label=true>
                "Se connecter avec Discord pour télécharger"
            </ButtonLink>
        }
        .into_any()
    }
}

/// One share.
#[component]
#[allow(clippy::needless_pass_by_value)] // Leptos prop convention
fn ShareCard(
    /// The share.
    item: ShareItem,
    /// Whether the viewer is signed in.
    signed_in: bool,
    /// Whether the viewer may download (verified address).
    can_download: bool,
    /// Called after removal.
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

    let id = StoredValue::new(item.id.clone());
    let detail = if item.is_link {
        item.host.clone()
    } else {
        item.filename.clone().map(|f| format!("{f} · {}", item.size.clone().unwrap_or_default()))
    };
    let meta = format!(
        "par {} · {}{}",
        item.author,
        item.when,
        detail.map(|d| format!(" · {d}")).unwrap_or_default()
    );
    let may_delete = item.may_delete;
    let access = access_button(
        format!("/api/shares/{}/download", item.id),
        item.is_link,
        can_download,
        signed_in,
        Callback::new(move |()| set_downloads.update(|n| *n += 1)),
    );
    let actions = view! {
        {access}
        <span class="ui-meta">{move || downloads_label(downloads.get())}</span>
        {may_delete.then(|| view! {
            <Show
                when=move || confirming.get()
                fallback=move || view! {
                    <Button kind=ButtonKind::Ghost icon=IconName::Trash on:click=move |_| set_confirming.set(true)>"Retirer"</Button>
                }
            >
                <Button kind=ButtonKind::Ghost icon=IconName::Trash
                    disabled=Signal::derive(move || remove.pending().get())
                    on:click=move |_| { remove.dispatch(id.get_value()); }>
                    "Confirmer le retrait"
                </Button>
            </Show>
        })}
    }
    .into_any();

    view! {
        <Card kicker=item.kind_label icon=share_kind_icon(&item.kind) title=item.title meta actions>
            {item.description}
            {move || error.get().map(|message| view! { <ErrorText message /> })}
        </Card>
    }
}

/// The posting form.
#[component]
fn UploadForm(
    /// Closes it.
    on_cancel: Callback<()>,
) -> impl IntoView {
    let (mode, set_mode) = signal("file".to_string());
    let (sending, set_sending) = signal(false);
    view! {
        <Form
            attr:method="post"
            attr:action="/api/shares"
            attr:enctype="multipart/form-data"
            on:submit=move |_| set_sending.set(true)
        >
            <Field id="partage-titre" label="Titre">
                <input id="partage-titre" class="ui-control" type="text" name="title" required=true maxlength="120" placeholder="Pack de sprites forêt" />
            </Field>
            <Field id="partage-type" label="Type">
                <select id="partage-type" class="ui-control" name="kind">
                    {SHARE_KINDS.iter().map(|(value, label, _)| view! { <option value=*value>{*label}</option> }).collect_view()}
                </select>
            </Field>
            <Field id="partage-description" label="Description (optionnel)" wide=true>
                <textarea id="partage-description" class="ui-control" name="description" rows="3" maxlength="2000"
                    placeholder="De quoi il s'agit, comment l'utiliser, le moteur…"></textarea>
            </Field>
            <FormRow>
                <SegmentedControl
                    label="Fichier ou lien"
                    options=vec![
                        segment_with_icon("file", "Un fichier", IconName::UploadSimple),
                        segment_with_icon("link", "Un lien", IconName::LinkSimple),
                    ]
                    value=mode
                    on_change=Callback::new(move |v| set_mode.set(v))
                />
            </FormRow>
            <Show
                when=move || mode.get() == "link"
                fallback=|| view! {
                    <Field id="partage-fichier" label="Fichier : zip, script, pdf, build… (500 Mo maximum)" wide=true>
                        <input id="partage-fichier" class="ui-control" type="file" name="file" required=true />
                    </Field>
                }
            >
                <Field id="partage-lien" label="Lien (itch.io, GitHub, Drive…)" wide=true>
                    <input id="partage-lien" class="ui-control" type="url" name="url" required=true placeholder="https://…" />
                </Field>
            </Show>
            <FormActions>
                <Button kind=ButtonKind::Primary button_type="submit" icon=IconName::UploadSimple disabled=Signal::derive(move || sending.get())>
                    {move || if sending.get() { "Envoi en cours…" } else { "Partager" }}
                </Button>
                <Button kind=ButtonKind::Ghost on:click=move |_| on_cancel.run(())>"Annuler"</Button>
            </FormActions>
        </Form>
    }
}

/// The kind filter: everything, then each kind.
fn kind_segments() -> Vec<Segment> {
    std::iter::once(segment("", "Tout"))
        .chain(SHARE_KINDS.iter().map(|(value, label, icon)| segment_with_icon(value, label, *icon)))
        .collect()
}

/// Member shares page.
#[component]
pub fn SharesPage() -> impl IntoView {
    let shares = Resource::new(|| (), |()| async { get_shares().await });
    let on_changed = Callback::new(move |()| shares.refetch());
    let query = use_query_map();
    let (filter, set_filter) = signal(String::new());
    let (uploading, set_uploading) = signal(false);

    let filters = view! {
        <FilterBar label="Filtrer les partages">
            <SegmentedControl label="Type" options=kind_segments() value=filter on_change=Callback::new(move |v| set_filter.set(v)) />
        </FilterBar>
    }
    .into_any();

    view! {
        <Page pattern=Pattern::Grid>
            <PageHeader
                title="Partages"
                lead="Scripts, lore, jeux, assets : dépose un zip ou un lien, et tous les membres peuvent le télécharger."
                filters
            >
                <Suspense fallback=|| ()>
                    {move || shares.get().and_then(Result::ok).map(|view| {
                        if view.can_upload {
                            view! {
                                <Button kind=ButtonKind::Primary icon=IconName::UploadSimple on:click=move |_| set_uploading.update(|o| *o = !*o)>
                                    "Déposer"
                                </Button>
                            }
                            .into_any()
                        } else if view.signed_in {
                            view! { <ButtonLink href="/onboarding/email" icon=IconName::Envelope>"Vérifier mon adresse pour partager"</ButtonLink> }.into_any()
                        } else {
                            view! {
                                <ButtonLink href="/api/auth/login" external=true icon=IconName::DiscordLogo hide_label=true>
                                    "Se connecter avec Discord pour partager"
                                </ButtonLink>
                            }
                            .into_any()
                        }
                    })}
                </Suspense>
            </PageHeader>

            {move || query.read().get("ok").map(|_| view! { <Notice kind=NoticeKind::Success>"C'est en ligne, merci du partage."</Notice> })}
            {move || query.read().get("erreur").map(|e| view! { <Notice kind=NoticeKind::Error>{e}</Notice> })}

            <Show when=move || uploading.get()>
                <Panel>
                    <h2 class="ui-h3">"Déposer quelque chose"</h2>
                    <UploadForm on_cancel=Callback::new(move |()| set_uploading.set(false)) />
                </Panel>
            </Show>

            <Transition fallback=|| view! { <GridSkeleton cards=6 /> }>
                {move || {
                    shares.get().map(|result| match result {
                        Err(_) => view! {
                            <ErrorState message="Impossible de charger les partages." on_retry=Callback::new(move |()| shares.refetch()) />
                        }
                        .into_any(),
                        Ok(view) => {
                            let wanted = filter.get();
                            let (signed_in, can_download) = (view.signed_in, view.can_upload);
                            let items: Vec<ShareItem> = view.items.into_iter().filter(|i| wanted.is_empty() || i.kind == wanted).collect();
                            if items.is_empty() {
                                view! {
                                    <EmptyState icon=IconName::Package title="Rien ici pour l'instant" body="Le premier dépôt ouvre l'étagère." />
                                }
                                .into_any()
                            } else {
                                view! {
                                    <CardGrid>
                                        {items.into_iter().map(|item| view! { <ShareCard item signed_in can_download on_changed /> }).collect_view()}
                                    </CardGrid>
                                }
                                .into_any()
                            }
                        }
                    })
                }}
            </Transition>
        </Page>
    }
}

#[cfg(test)]
mod tests {
    use super::downloads_label;

    #[test]
    fn downloads_are_counted_in_words() {
        assert_eq!(downloads_label(0), "0 téléchargements");
        assert_eq!(downloads_label(1), "1 téléchargement");
        assert_eq!(downloads_label(12), "12 téléchargements");
    }
}

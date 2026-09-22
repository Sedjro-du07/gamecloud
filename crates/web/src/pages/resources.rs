//! The resource library — a dense list.
//!
//! Members propose links, a validator signs off, and the submitter earns
//! XP for it. Validation is what separates a library from a dumping
//! ground, so unvalidated entries are visible only to the people who can
//! act on them — everyone else browses what the club has vouched for.

use leptos::prelude::*;

use crate::{
    api::ResourceItem,
    components::ui::{
        segment, segment_with_icon,
        vocab::{resource_kind, track_choices, RESOURCE_KINDS},
        Button, ButtonKind, ButtonLink, Cluster, DenseList, EmptyState, ErrorState, ErrorText,
        Field, FilterBar, Form, FormActions, FormRow, Icon, IconName, ListRow, MembersOnlyState,
        Page, PageHeader, Panel, Pattern, RowsSkeleton, Segment, SegmentedControl, Tag, TrackTag,
    },
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

    let (kind_label, kind_icon) = resource_kind(item.kind.as_deref());
    let mut meta = kind_label.to_string();
    if let Some(level) = &item.level {
        meta.push_str(" · ");
        meta.push_str(level);
    }
    meta.push_str(" · proposée par ");
    meta.push_str(&item.submitted_by);

    let id = StoredValue::new(item.id.clone());
    let voted = item.has_voted;
    let busy = Signal::derive(move || act.pending().get());
    let can_validate = item.may_validate && !item.validated;
    let waiting = !item.validated;

    let lead = view! { <Icon name=kind_icon /> }.into_any();
    let end = view! {
        <Button
            kind=if voted { ButtonKind::Primary } else { ButtonKind::Secondary }
            icon=IconName::ArrowFatUp
            disabled=busy
            attr:aria-pressed=voted.to_string()
            attr:title=if voted { "Retirer ton vote" } else { "Voter pour cette ressource" }
            on:click=move |_| {
                let what = if voted { "unvote" } else { "vote" };
                act.dispatch((id.get_value(), what.to_string()));
            }
        >
            {item.votes.to_string()}
            <span class="ui-sr-only">" votes"</span>
        </Button>
        {can_validate
            .then(|| {
                view! {
                    <Button
                        kind=ButtonKind::Primary
                        icon=IconName::SealCheck
                        disabled=busy
                        on:click=move |_| {
                            act.dispatch((id.get_value(), "validate".to_string()));
                        }
                    >
                        "Valider"
                    </Button>
                }
            })}
    }
    .into_any();

    let tracks = item.tracks.clone();
    view! {
        <ListRow lead title=item.title href=item.url external=true meta end>
            {(!tracks.is_empty() || waiting)
                .then(|| {
                    view! {
                        <Cluster>
                            {tracks.into_iter().map(|track| view! { <TrackTag track /> }).collect_view()}
                            {waiting.then(|| view! { <Tag icon=IconName::Clock>"En attente de validation"</Tag> })}
                        </Cluster>
                    }
                })}
            {move || error.get().map(|message| view! { <ErrorText message /> })}
        </ListRow>
    }
}

/// The proposal form.
#[component]
fn SubmitForm(
    /// Called after a successful proposal.
    on_sent: Callback<()>,
    /// Called when the member gives up.
    on_cancel: Callback<()>,
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
                    on_sent.run(());
                }
                Err(e) => set_error.set(Some(e.to_string())),
            }
        }
    });

    view! {
        <Form on:submit=move |ev| {
            ev.prevent_default();
            send.dispatch((title.get(), url.get(), kind.get(), track.get()));
        }>
            <Field id="ressource-titre" label="Titre">
                <input
                    id="ressource-titre"
                    class="ui-control"
                    type="text"
                    required=true
                    prop:value=move || title.get()
                    on:input=move |ev| set_title.set(event_target_value(&ev))
                />
            </Field>
            <Field id="ressource-lien" label="Lien">
                <input
                    id="ressource-lien"
                    class="ui-control"
                    type="url"
                    required=true
                    placeholder="https://…"
                    prop:value=move || url.get()
                    on:input=move |ev| set_url.set(event_target_value(&ev))
                />
            </Field>
            <Field id="ressource-type" label="Type">
                <select id="ressource-type" class="ui-control" on:change=move |ev| set_kind.set(event_target_value(&ev))>
                    {RESOURCE_KINDS
                        .iter()
                        .map(|(value, label, _)| view! { <option value=*value>{*label}</option> })
                        .collect_view()}
                </select>
            </Field>
            <Field id="ressource-track" label="Track concernée (optionnel)">
                <select id="ressource-track" class="ui-control" on:change=move |ev| set_track.set(event_target_value(&ev))>
                    <option value="">"Aucune"</option>
                    {track_choices()
                        .into_iter()
                        .map(|(id, label)| view! { <option value=id>{label}</option> })
                        .collect_view()}
                </select>
            </Field>
            {move || error.get().map(|message| view! { <FormRow><ErrorText message /></FormRow> })}
            <FormActions>
                <Button
                    kind=ButtonKind::Primary
                    button_type="submit"
                    icon=IconName::Plus
                    disabled=Signal::derive(move || send.pending().get())
                >
                    {move || if send.pending().get() { "Envoi…" } else { "Proposer" }}
                </Button>
                <Button kind=ButtonKind::Ghost on:click=move |_| on_cancel.run(())>"Annuler"</Button>
            </FormActions>
        </Form>
    }
}

/// The kind filter: every kind, then each one.
fn kind_segments() -> Vec<Segment> {
    std::iter::once(segment("", "Tout"))
        .chain(RESOURCE_KINDS.iter().map(|(value, label, icon)| segment_with_icon(value, label, *icon)))
        .collect()
}

/// What the viewer may do in the header: propose, verify, or sign in.
#[component]
fn HeaderAction(
    /// Opens the proposal form.
    set_proposing: WriteSignal<bool>,
) -> impl IntoView {
    let me = Resource::new(|| (), |()| async { get_me().await });
    view! {
        <Suspense fallback=|| ()>
            {move || {
                me.get()
                    .map(|result| match result.ok().flatten() {
                        Some(user) if user.is_member => {
                            view! {
                                <Button
                                    kind=ButtonKind::Primary
                                    icon=IconName::Plus
                                    on:click=move |_| set_proposing.update(|open| *open = !*open)
                                >
                                    "Proposer une ressource"
                                </Button>
                            }
                                .into_any()
                        }
                        Some(_) => {
                            view! {
                                <ButtonLink href="/tests" icon=IconName::GraduationCap>
                                    "Passer le test d'entrée pour proposer"
                                </ButtonLink>
                            }
                                .into_any()
                        }
                        None => {
                            view! {
                                <ButtonLink href="/api/auth/login" external=true icon=IconName::DiscordLogo hide_label=true>
                                    "Se connecter avec Discord pour proposer une ressource"
                                </ButtonLink>
                            }
                                .into_any()
                        }
                    })
            }}
        </Suspense>
    }
}

/// Resource library page.
#[component]
pub fn ResourcesPage() -> impl IntoView {
    let me = Resource::new(|| (), |()| async { get_me().await });
    let items = Resource::new(|| (), |()| async { get_resources().await });
    let on_changed = Callback::new(move |()| items.refetch());
    let (kind, set_kind) = signal(String::new());
    let (proposing, set_proposing) = signal(false);
    let on_sent = Callback::new(move |()| {
        set_proposing.set(false);
        items.refetch();
    });
    let on_cancel = Callback::new(move |()| set_proposing.set(false));

    let filters = view! {
        <FilterBar label="Filtrer les ressources">
            <SegmentedControl
                label="Type"
                options=kind_segments()
                value=kind
                on_change=Callback::new(move |v| set_kind.set(v))
            />
        </FilterBar>
    }
    .into_any();

    view! {
        <Page pattern=Pattern::List>
            <PageHeader
                title="Ressources"
                lead="Les liens que l'association recommande. Une ressource validée rapporte de l'XP à qui l'a proposée."
                filters
            >
                <HeaderAction set_proposing />
            </PageHeader>

            <Show when=move || proposing.get()>
                <Panel>
                    <h2 class="ui-h3">"Proposer une ressource"</h2>
                    <SubmitForm on_sent on_cancel />
                </Panel>
            </Show>

            <Suspense fallback=|| view! { <RowsSkeleton rows=5 /> }>
                {move || {
                    me.get()
                        .map(|result| match result.ok().flatten() {
                            Some(user) if user.is_member => view! { <ResourceList items kind on_changed /> }.into_any(),
                            _ => view! { <MembersOnlyState what="voir la bibliothèque de l'association" /> }.into_any(),
                        })
                }}
            </Suspense>
        </Page>
    }
}

/// The library itself, once the viewer is known to be a member.
#[component]
fn ResourceList(
    /// Everything the library holds.
    items: Resource<Result<Vec<ResourceItem>, ServerFnError>>,
    /// Kind currently shown.
    kind: ReadSignal<String>,
    /// Refresh after an action.
    on_changed: Callback<()>,
) -> impl IntoView {
    view! {
            <Transition fallback=|| view! { <RowsSkeleton rows=5 /> }>
                {move || {
                    items
                        .get()
                        .map(|result| match result {
                            Err(_) => {
                                view! {
                                    <ErrorState
                                        message="Impossible de charger la bibliothèque."
                                        on_retry=Callback::new(move |()| items.refetch())
                                    />
                                }
                                    .into_any()
                            }
                            Ok(list) => {
                                let wanted = kind.get();
                                let shown: Vec<ResourceItem> = list
                                    .into_iter()
                                    .filter(|i| wanted.is_empty() || i.kind.as_deref() == Some(wanted.as_str()))
                                    .collect();
                                if shown.is_empty() {
                                    view! {
                                        <EmptyState
                                            icon=IconName::Books
                                            title="Aucune ressource ici pour l'instant"
                                            body="Propose le premier lien : il rapporte de l'XP une fois validé."
                                        />
                                    }
                                        .into_any()
                                } else {
                                    view! {
                                        <DenseList label="Ressources">
                                            {shown
                                                .into_iter()
                                                .map(|item| view! { <ResourceRow item on_changed /> })
                                                .collect_view()}
                                        </DenseList>
                                    }
                                        .into_any()
                                }
                            }
                        })
                }}
            </Transition>
    }
}

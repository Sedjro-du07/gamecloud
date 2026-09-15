//! Tracks — a grid of cards, one per track.
//!
//! Joining a track is what puts a member on the XP ladder and makes track
//! XP, track titles and the multi-track bonus exist at all. A card says the
//! track's name with its icon, its specialisations, and whether the member
//! is in it; joining happens on the card.

use leptos::prelude::*;

use crate::{
    api::TrackOption,
    components::ui::{
        vocab::{track_icon_by_id, track_name},
        Button, ButtonKind, ButtonLink, Card, CardGrid, Cluster, ErrorState, Field, Gap,
        GridSkeleton, IconName, Notice, NoticeKind, Page, PageHeader, Pattern, SignInState, Stack,
    },
    server_fns::{get_me, get_track_options, join_track},
};

/// A join to fire: track and optional specialisation.
type Join = (String, Option<String>);

/// The joining controls of a card not joined yet.
#[component]
fn JoinControls(
    /// Track identifier.
    id: String,
    /// Its specialisations.
    specializations: Vec<String>,
    /// Which card is open.
    selected: ReadSignal<Option<String>>,
    /// Opens a card.
    set_selected: WriteSignal<Option<String>>,
    /// Fires the join.
    on_join: Callback<Join>,
    /// A join is in flight.
    pending: Memo<bool>,
) -> impl IntoView {
    let id = StoredValue::new(id);
    let specs = StoredValue::new(specializations);
    let (spec, set_spec) = signal(String::new());
    let is_open = Memo::new(move |_| selected.get().as_deref() == Some(id.get_value().as_str()));
    let field_id = StoredValue::new(format!("specialisation-{}", id.get_value().to_lowercase()));
    view! {
        <Show
            when=move || is_open.get()
            fallback=move || view! {
                <Button kind=ButtonKind::Primary icon=IconName::Plus
                    on:click=move |_| { set_spec.set(String::new()); set_selected.set(Some(id.get_value())); }>
                    "Rejoindre"
                </Button>
                <ButtonLink kind=ButtonKind::Ghost href=format!("/tracks/{}", id.get_value())>"Voir le tableau"</ButtonLink>
            }
        >
            <Stack gap=Gap::Tight>
                <Field id=field_id.get_value() label="Spécialisation (optionnel)">
                    <select id=field_id.get_value() class="ui-control" on:change=move |ev| set_spec.set(event_target_value(&ev))>
                        <option value="">"Je verrai plus tard"</option>
                        {specs.get_value().into_iter().map(|s| { let value = s.clone(); view! { <option value=value>{s}</option> } }).collect_view()}
                    </select>
                </Field>
                <Cluster>
                    <Button kind=ButtonKind::Primary icon=IconName::Check disabled=pending
                        on:click=move |_| {
                            let chosen = spec.get();
                            on_join.run((id.get_value(), (!chosen.trim().is_empty()).then_some(chosen)));
                        }>
                        {move || if pending.get() { "Inscription…" } else { "Confirmer" }}
                    </Button>
                    <Button kind=ButtonKind::Ghost on:click=move |_| set_selected.set(None)>"Annuler"</Button>
                </Cluster>
            </Stack>
        </Show>
    }
}

/// One track.
#[component]
#[allow(clippy::needless_pass_by_value)] // Leptos prop convention
fn TrackCard(
    /// The track on offer.
    option: TrackOption,
    /// Which card is open.
    selected: ReadSignal<Option<String>>,
    /// Opens a card.
    set_selected: WriteSignal<Option<String>>,
    /// Fires the join.
    on_join: Callback<Join>,
    /// A join is in flight.
    pending: Memo<bool>,
) -> impl IntoView {
    let name = track_name(&option.id);
    let icon = track_icon_by_id(&option.id).unwrap_or(IconName::Compass);
    let body = option.specializations.join(" · ");
    let actions = if option.joined {
        view! {
            <ButtonLink href=format!("/tracks/{}", option.id) trailing_icon=IconName::ArrowRight>"Voir le tableau"</ButtonLink>
        }
        .into_any()
    } else {
        view! {
            <JoinControls id=option.id.clone() specializations=option.specializations.clone() selected set_selected on_join pending />
        }
        .into_any()
    };
    if option.joined {
        // No accent edge: a member in every track would get eight, and the
        // accent is kept for their title.
        view! { <Card title=name icon kicker="Rejointe" actions>{body}</Card> }.into_any()
    } else {
        view! { <Card title=name icon actions>{body}</Card> }.into_any()
    }
}

/// The grid, for a signed-in member.
#[component]
fn TrackGrid() -> impl IntoView {
    let options = Resource::new(|| (), |()| async { get_track_options().await });
    let (selected, set_selected) = signal(Option::<String>::None);
    let (feedback, set_feedback) = signal(Option::<Result<String, String>>::None);
    let join = Action::new(move |(track, spec): &Join| {
        let (track, spec) = (track.clone(), spec.clone());
        async move {
            match join_track(track.clone(), spec).await {
                Ok(()) => Ok(format!("Bienvenue dans la track {}.", track_name(&track))),
                Err(e) => Err(e.to_string()),
            }
        }
    });
    let pending = Memo::new(move |_| join.pending().get());
    let on_join = Callback::new(move |args: Join| {
        join.dispatch(args);
    });
    Effect::new(move |_| {
        if let Some(result) = join.value().get() {
            set_feedback.set(Some(result));
            options.refetch();
            set_selected.set(None);
        }
    });

    view! {
        {move || feedback.get().map(|result| match result {
            Ok(m) => view! { <Notice kind=NoticeKind::Success>{m}</Notice> }.into_any(),
            Err(m) => view! { <Notice kind=NoticeKind::Error>{m}</Notice> }.into_any(),
        })}
        <Suspense fallback=|| view! { <GridSkeleton cards=8 /> }>
            {move || options.get().map(|result| match result {
                Err(_) => view! {
                    <ErrorState message="Impossible de charger les tracks." on_retry=Callback::new(move |()| options.refetch()) />
                }
                .into_any(),
                Ok(list) => view! {
                    <CardGrid>
                        {list.into_iter().map(|option| view! { <TrackCard option selected set_selected on_join pending /> }).collect_view()}
                    </CardGrid>
                }
                .into_any(),
            })}
        </Suspense>
    }
}

/// Track selection page.
#[component]
pub fn TrackPickerPage() -> impl IntoView {
    let me = Resource::new(|| (), |()| async { get_me().await });
    view! {
        <Page pattern=Pattern::Grid>
            <PageHeader
                title="Tracks"
                lead="Une track, c'est ta discipline dans l'association. Tu peux en rejoindre plusieurs : à partir de deux tracks actives, toute ton XP est multipliée."
            >
                <ButtonLink kind=ButtonKind::Ghost href="/profile" trailing_icon=IconName::ArrowRight>"Mon profil"</ButtonLink>
            </PageHeader>
            <Suspense fallback=|| view! { <GridSkeleton cards=8 /> }>
                {move || me.get().map(|result| match result {
                    Ok(Some(_)) => view! { <TrackGrid /> }.into_any(),
                    Ok(None) => view! { <SignInState what="choisir tes tracks" /> }.into_any(),
                    Err(_) => view! {
                        <ErrorState message="Impossible de charger les tracks." on_retry=Callback::new(move |()| me.refetch()) />
                    }
                    .into_any(),
                })}
            </Suspense>
        </Page>
    }
}

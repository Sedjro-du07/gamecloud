//! Calendar — sessions, workshops, jams, deadlines, as an agenda.
//!
//! One month at a time, one row per event under its day. Whoever may manage
//! an event opens its tools in place: edit, cancel, delete, the attendance
//! QR code and who scanned it.
//!
//! Two decisions worth stating.
//!
//! **Days come from the server's `day` string, not from a date parsed in
//! the browser.** Every event arrives carrying `YYYY-MM-DD`, so grouping is
//! a string comparison. Nothing here has to know when the clocks change.
//!
//! **Management controls are driven by `can_manage`, which the server
//! computed.** The page never inspects the viewer's roles itself, and the
//! server re-checks anyway: the hidden button is a courtesy.

use std::fmt::Write as _;

use leptos::prelude::*;

use crate::{
    api::{CalendarEvent, EventDraft},
    components::ui::{
        vocab::track_choices, Button, ButtonKind, Cluster, DenseList, EmptyState, ErrorState,
        ErrorText, Field, FilterBar, Figure, Form, FormActions, FormRow, Gap, IconName, ListGroup,
        ListRow, Notice, Page, PageHeader, Panel, Pattern, RowText, RowValue, RowsSkeleton,
        SignInState, Stack, Stepper, Tag, TrackTag,
    },
    server_fns::{
        cancel_event, delete_event, generate_event_qr, get_calendar, get_calendar_rights,
        get_event_attendees, get_me, save_event,
    },
};

/// Month names, so the header reads in French without a locale crate in the
/// WASM bundle for twelve strings.
const MONTHS: [&str; 12] = [
    "janvier", "février", "mars", "avril", "mai", "juin", "juillet", "août", "septembre",
    "octobre", "novembre", "décembre",
];

/// Weekday names, Monday first.
const WEEKDAYS: [&str; 7] = ["lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi", "dimanche"];

/// Split a `YYYY-MM` into its parts, falling back rather than panicking.
fn split_month(month: &str) -> (i32, u32) {
    let mut parts = month.split('-');
    let year = parts.next().and_then(|y| y.parse().ok()).unwrap_or(2026);
    let m = parts.next().and_then(|m| m.parse().ok()).unwrap_or(1);
    (year, if (1..=12).contains(&m) { m } else { 1 })
}

/// Step a `YYYY-MM` forward or back, rolling the year.
fn shift_month(month: &str, delta: i32) -> String {
    let (year, m) = split_month(month);
    let m = i32::try_from(m).unwrap_or(1);
    let total = year * 12 + (m - 1) + delta;
    let (y, m) = (total.div_euclid(12), total.rem_euclid(12) + 1);
    format!("{y:04}-{m:02}")
}

/// "septembre 2026".
fn month_label(month: &str) -> String {
    let (y, m) = split_month(month);
    format!("{} {y}", MONTHS[(m as usize).saturating_sub(1).min(11)])
}

/// Day of the week the 1st of a month falls on, Monday = 0 (Sakamoto).
fn first_weekday(year: i32, month: u32) -> u32 {
    const T: [i32; 12] = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let y = if month < 3 { year - 1 } else { year };
    let m = month as usize - 1;
    let dow = (y + y / 4 - y / 100 + y / 400 + T[m] + 1).rem_euclid(7);
    u32::try_from((dow + 6).rem_euclid(7)).unwrap_or(0)
}

/// "mardi 16" for "2026-09-16".
fn day_label(day: &str) -> String {
    let (year, month) = split_month(day.get(..7).unwrap_or(day));
    let date: u32 = day.get(8..10).and_then(|d| d.parse().ok()).unwrap_or(1);
    let weekday = (first_weekday(year, month) + date.saturating_sub(1)) % 7;
    format!("{} {date}", WEEKDAYS[weekday as usize])
}

/// The scheduling form, which doubles as the edit form.
#[component]
fn EventForm(
    /// Event being edited, or `None` to schedule a new one.
    editing: ReadSignal<Option<CalendarEvent>>,
    /// Called once the save lands.
    on_saved: Callback<()>,
    /// Called when the organiser gives up.
    on_cancel: Callback<()>,
    /// Tracks the viewer may scope an event to.
    tracks: Vec<(String, String)>,
    /// Whether the viewer may file an association-wide event.
    association: bool,
    /// Whether the viewer may call a Bureau meeting.
    bureau: bool,
) -> impl IntoView {
    let track_options = StoredValue::new(tracks.clone());
    let (title, set_title) = signal(String::new());
    let (description, set_description) = signal(String::new());
    let (kind, set_kind) = signal("Session".to_string());
    let (track, set_track) = signal(tracks.first().map(|(id, _)| id.clone()).unwrap_or_default());
    // Start on a scope this member may actually file under.
    let (audience, set_audience) = signal(
        if association { "Association" } else if tracks.is_empty() { "Bureau" } else { "Track" }.to_string(),
    );
    let (starts, set_starts) = signal(String::new());
    let (ends, set_ends) = signal(String::new());
    let (location, set_location) = signal(String::new());
    let (xp, set_xp) = signal("20".to_string());
    let (error, set_error) = signal(Option::<String>::None);

    // Pre-fill from the event handed over, clear for a new one.
    Effect::new(move |_| {
        if let Some(e) = editing.get() {
            set_audience.set(e.audience.clone());
            set_track.set(e.track.clone().unwrap_or_default());
            set_title.set(e.title.clone());
            set_description.set(e.description.clone().unwrap_or_default());
            set_kind.set(e.kind.clone());
            set_location.set(e.location.clone().unwrap_or_default());
            set_xp.set(e.xp_reward.to_string());
        } else {
            set_title.set(String::new());
            set_description.set(String::new());
            set_location.set(String::new());
        }
    });

    let save = Action::new(move |draft: &EventDraft| {
        let draft = draft.clone();
        async move { save_event(draft).await }
    });
    Effect::new(move |_| {
        if let Some(result) = save.value().get() {
            match result {
                Ok(_) => {
                    set_error.set(None);
                    on_saved.run(());
                }
                Err(e) => set_error.set(Some(e.to_string())),
            }
        }
    });

    view! {
        <Form on:submit=move |ev| {
            ev.prevent_default();
            save.dispatch(EventDraft {
                id: editing.get().map(|e| e.id).unwrap_or_default(),
                title: title.get(),
                description: description.get(),
                kind: kind.get(),
                track: track.get(),
                audience: audience.get(),
                starts_at: starts.get(),
                ends_at: ends.get(),
                location: location.get(),
                xp_reward: xp.get().trim().parse().unwrap_or(0),
            });
        }>
            <Field id="evenement-titre" label="Intitulé" wide=true>
                <input id="evenement-titre" class="ui-control" type="text" required=true placeholder="Séance du mardi"
                    prop:value=move || title.get() on:input=move |ev| set_title.set(event_target_value(&ev)) />
            </Field>
            <Field id="evenement-type" label="Type">
                <select id="evenement-type" class="ui-control" prop:value=move || kind.get()
                    on:change=move |ev| set_kind.set(event_target_value(&ev))>
                    <option value="Session">"Séance"</option>
                    <option value="Workshop">"Atelier"</option>
                    <option value="Jam">"Game jam"</option>
                    <option value="Meeting">"Réunion"</option>
                    <option value="Deadline">"Échéance"</option>
                    <option value="Showcase">"Présentation"</option>
                </select>
            </Field>
            <Field id="evenement-public" label="Pour qui">
                <select id="evenement-public" class="ui-control" prop:value=move || audience.get()
                    on:change=move |ev| set_audience.set(event_target_value(&ev))>
                    {association.then(|| view! { <option value="Association">"Toute l'association"</option> })}
                    {(!tracks.is_empty()).then(|| view! { <option value="Track">"Une track"</option> })}
                    {bureau.then(|| view! { <option value="Bureau">"Le Bureau seulement"</option> })}
                </select>
            </Field>
            // Only asked for when it means something: a Bureau meeting
            // carrying a track would be refused by the database.
            <Show when=move || audience.get() == "Track">
                <Field id="evenement-track" label="Track concernée">
                    <select id="evenement-track" class="ui-control" prop:value=move || track.get()
                        on:change=move |ev| set_track.set(event_target_value(&ev))>
                        {track_options
                            .get_value()
                            .into_iter()
                            .map(|(id, label)| view! { <option value=id>{label}</option> })
                            .collect_view()}
                    </select>
                </Field>
            </Show>
            <Field id="evenement-debut" label="Début">
                <input id="evenement-debut" class="ui-control" type="datetime-local" required=true
                    prop:value=move || starts.get() on:input=move |ev| set_starts.set(event_target_value(&ev)) />
            </Field>
            <Field id="evenement-fin" label="Fin">
                <input id="evenement-fin" class="ui-control" type="datetime-local" required=true
                    prop:value=move || ends.get() on:input=move |ev| set_ends.set(event_target_value(&ev)) />
            </Field>
            <Field id="evenement-lieu" label="Lieu">
                <input id="evenement-lieu" class="ui-control" type="text" placeholder="Salle 204, ou un lien"
                    prop:value=move || location.get() on:input=move |ev| set_location.set(event_target_value(&ev)) />
            </Field>
            <Field id="evenement-xp" label="XP de présence">
                <input id="evenement-xp" class="ui-control" type="number" min="0" max="500"
                    prop:value=move || xp.get() on:input=move |ev| set_xp.set(event_target_value(&ev)) />
            </Field>
            <Field id="evenement-description" label="Description" wide=true>
                <textarea id="evenement-description" class="ui-control" rows="2"
                    placeholder="Ce qui sera fait, ce qu'il faut apporter."
                    prop:value=move || description.get()
                    on:input=move |ev| set_description.set(event_target_value(&ev))></textarea>
            </Field>
            <Show when=move || audience.get() == "Bureau">
                <FormRow>
                    <Notice>
                        "Visible du Bureau uniquement, et annoncée dans le salon du Bureau en taguant ses membres."
                    </Notice>
                </FormRow>
            </Show>
            {move || error.get().map(|message| view! { <FormRow><ErrorText message /></FormRow> })}
            <FormActions>
                <Button kind=ButtonKind::Primary button_type="submit" icon=IconName::CalendarBlank
                    disabled=Signal::derive(move || save.pending().get())>
                    {move || if save.pending().get() { "Envoi…" } else { "Enregistrer" }}
                </Button>
                <Button kind=ButtonKind::Ghost on:click=move |_| on_cancel.run(())>"Annuler"</Button>
            </FormActions>
        </Form>
    }
}

/// Who scanned in.
#[component]
fn Attendees(
    /// Event.
    event_id: String,
    /// How many, as the list said.
    count: i64,
) -> impl IntoView {
    let rows = Resource::new(move || event_id.clone(), |id| async move { get_event_attendees(id).await });
    view! {
        <Stack gap=Gap::Tight>
            <h3 class="ui-h3">{format!("Présences ({count})")}</h3>
            <Suspense fallback=|| view! { <RowsSkeleton rows=2 /> }>
                {move || {
                    rows.get()
                        .map(|result| match result {
                            Err(e) => view! { <ErrorText message=format!("Feuille de présence indisponible : {e}") /> }.into_any(),
                            Ok(list) if list.is_empty() => {
                                view! { <EmptyState icon=IconName::Users title="Personne n'a encore scanné" /> }.into_any()
                            }
                            Ok(list) => {
                                view! {
                                    <DenseList label="Présences">
                                        {list
                                            .into_iter()
                                            .map(|a| {
                                                let end = view! { <RowValue text=format!("+{} XP", a.xp_rewarded) /> }.into_any();
                                                view! { <ListRow title=a.display_name meta=a.when end /> }
                                            })
                                            .collect_view()}
                                    </DenseList>
                                }
                                    .into_any()
                            }
                        })
                }}
            </Suspense>
        </Stack>
    }
}

/// The attendance QR code.
#[component]
fn QrPanel(
    /// Event.
    event_id: String,
    /// A cancelled event takes no attendance.
    cancelled: bool,
) -> impl IntoView {
    let id = StoredValue::new(event_id);
    let (minutes, set_minutes) = signal("120".to_string());
    let (error, set_error) = signal(Option::<String>::None);
    let mint = Action::new(move |(id, mins): &(String, i32)| {
        let (id, mins) = (id.clone(), *mins);
        async move { generate_event_qr(id, mins, None).await }
    });
    Effect::new(move |_| {
        if let Some(Err(e)) = mint.value().get() {
            set_error.set(Some(e.to_string()));
        }
    });
    view! {
        <Stack gap=Gap::Tight>
            <h3 class="ui-h3">"Code de présence"</h3>
            <p class="ui-meta">"Le code encode un lien : n'importe quel appareil photo l'ouvre, sans application."</p>
            <Cluster>
                <Field id=format!("qr-minutes-{}", id.get_value()) label="Valable (minutes)" inline=true>
                    <input id=format!("qr-minutes-{}", id.get_value()) class="ui-control" type="number" min="1" max="1440"
                        prop:value=move || minutes.get() on:input=move |ev| set_minutes.set(event_target_value(&ev)) />
                </Field>
                <Button kind=ButtonKind::Primary icon=IconName::QrCode
                    disabled=Signal::derive(move || mint.pending().get() || cancelled)
                    on:click=move |_| {
                        set_error.set(None);
                        mint.dispatch((id.get_value(), minutes.get().trim().parse().unwrap_or(120)));
                    }>
                    {move || if mint.pending().get() { "Génération…" } else { "Générer le code" }}
                </Button>
            </Cluster>
            {move || error.get().map(|message| view! { <ErrorText message /> })}
            {move || {
                mint.value()
                    .get()
                    .and_then(Result::ok)
                    .map(|t| view! { <Figure svg=t.svg caption=format!("Expire le {} · {}", t.expires_label, t.scan_url) /> })
            }}
        </Stack>
    }
}

/// The tools for one event.
#[component]
fn EventPanel(
    /// The event.
    event: CalendarEvent,
    /// Loads it into the form.
    on_edit: Callback<CalendarEvent>,
    /// Asks the page to reload.
    on_changed: Callback<()>,
) -> impl IntoView {
    let (error, set_error) = signal(Option::<String>::None);
    let cancel = Action::new(move |(id, to): &(String, bool)| {
        let (id, to) = (id.clone(), *to);
        async move { cancel_event(id, to).await }
    });
    // Deleting is narrower than cancelling: the server refuses once anybody
    // scanned in, because that attendance is somebody's XP.
    let remove = Action::new(move |id: &String| {
        let id = id.clone();
        async move { delete_event(id).await }
    });
    Effect::new(move |_| {
        if let Some(result) = cancel.value().get() {
            match result {
                Ok(()) => on_changed.run(()),
                Err(e) => set_error.set(Some(e.to_string())),
            }
        }
    });
    Effect::new(move |_| {
        if let Some(result) = remove.value().get() {
            match result {
                Ok(()) => on_changed.run(()),
                Err(e) => set_error.set(Some(e.to_string())),
            }
        }
    });

    let id = StoredValue::new(event.id.clone());
    let cancelled = event.cancelled;
    let has_attendance = event.attendee_count > 0;
    let for_edit = StoredValue::new(event.clone());

    view! {
        <Panel>
            <Cluster>
                <Button icon=IconName::PencilSimple on:click=move |_| on_edit.run(for_edit.get_value())>"Modifier"</Button>
                <Button kind=ButtonKind::Ghost icon=IconName::Prohibit
                    disabled=Signal::derive(move || cancel.pending().get())
                    on:click=move |_| { cancel.dispatch((id.get_value(), !cancelled)); }>
                    {if cancelled { "Réactiver" } else { "Annuler l'événement" }}
                </Button>
                <Button kind=ButtonKind::Ghost icon=IconName::Trash
                    disabled=Signal::derive(move || remove.pending().get() || has_attendance)
                    on:click=move |_| { remove.dispatch(id.get_value()); }>
                    "Supprimer"
                </Button>
            </Cluster>
            {has_attendance.then(|| view! {
                <p class="ui-meta">"Des présences sont enregistrées : annule l'événement plutôt que de le supprimer."</p>
            })}
            {move || error.get().map(|message| view! { <ErrorText message /> })}
            <QrPanel event_id=event.id.clone() cancelled />
            <Attendees event_id=event.id count=event.attendee_count />
        </Panel>
    }
}

/// One event's row.
#[component]
fn EventRow(
    /// The event.
    event: CalendarEvent,
    /// Which event's tools are open.
    open: ReadSignal<Option<String>>,
    /// Opens or closes them.
    on_toggle: Callback<String>,
    /// Loads an event into the form.
    on_edit: Callback<CalendarEvent>,
    /// Asks the page to reload.
    on_changed: Callback<()>,
) -> impl IntoView {
    let id = StoredValue::new(event.id.clone());
    let is_open = Memo::new(move |_| open.get().as_deref() == Some(id.get_value().as_str()));

    let mut meta = event.kind_label.clone();
    if let Some(place) = &event.location {
        meta.push_str(" · ");
        meta.push_str(place);
    }
    if event.xp_reward > 0 {
        let _ = write!(meta, " · +{} XP de présence", event.xp_reward);
    }
    match event.attendee_count {
        0 => {}
        1 => meta.push_str(" · 1 présent"),
        n => {
            let _ = write!(meta, " · {n} présents");
        }
    }

    let lead = view! { <span>{event.time_label.clone()}</span> }.into_any();
    let end = if event.can_manage {
        view! {
            <Button kind=ButtonKind::Ghost icon=IconName::PencilSimple
                attr:aria-expanded=move || is_open.get().to_string()
                on:click=move |_| on_toggle.run(id.get_value())>
                {move || if is_open.get() { "Fermer" } else { "Gérer" }}
            </Button>
        }
        .into_any()
    } else {
        ().into_any()
    };

    let track = event.track.clone();
    let bureau = event.audience == "Bureau";
    let cancelled = event.cancelled;
    let panel_event = StoredValue::new(event.clone());

    view! {
        <ListRow lead title=event.title.clone() meta end dimmed=event.past || cancelled>
            {(track.is_some() || bureau || cancelled)
                .then(|| view! {
                    <Cluster>
                        {track.map(|track| view! { <TrackTag track /> })}
                        {bureau.then(|| view! { <Tag icon=IconName::LockSimple>"Bureau seulement"</Tag> })}
                        {cancelled.then(|| view! { <Tag icon=IconName::Prohibit>"Annulé"</Tag> })}
                    </Cluster>
                })}
            {event.description.clone().map(|text| view! { <RowText text /> })}
            <Show when=move || is_open.get()>
                <EventPanel event=panel_event.get_value() on_edit on_changed />
            </Show>
        </ListRow>
    }
}

/// The month, day by day.
#[component]
fn Agenda(
    /// Everything in the month.
    events: Vec<CalendarEvent>,
    /// Which event's tools are open.
    open: ReadSignal<Option<String>>,
    /// Opens or closes them.
    on_toggle: Callback<String>,
    /// Loads an event into the form.
    on_edit: Callback<CalendarEvent>,
    /// Asks the page to reload.
    on_changed: Callback<()>,
) -> impl IntoView {
    let mut events = events;
    events.sort_by(|a, b| (a.day.as_str(), a.time_label.as_str()).cmp(&(b.day.as_str(), b.time_label.as_str())));
    let mut rows = Vec::with_capacity(events.len() * 2);
    let mut last_day = String::new();
    for event in events {
        if event.day != last_day {
            last_day.clone_from(&event.day);
            rows.push(view! { <ListGroup label=day_label(&event.day) /> }.into_any());
        }
        rows.push(view! { <EventRow event open on_toggle on_edit on_changed /> }.into_any());
    }
    view! { <DenseList label="Événements du mois">{rows}</DenseList> }
}

/// Calendar page. Signed-in people only.
#[component]
pub fn CalendarPage() -> impl IntoView {
    let me = Resource::new(|| (), |()| async { get_me().await });
    view! {
        <Suspense fallback=|| view! {
            <Page pattern=Pattern::List>
                <PageHeader title="Calendrier" />
                <RowsSkeleton rows=4 />
            </Page>
        }>
            {move || {
                me.get()
                    .map(|result| match result {
                        Ok(Some(_)) => view! { <CalendarBoard /> }.into_any(),
                        Ok(None) => view! {
                            <Page pattern=Pattern::List>
                                <PageHeader title="Calendrier" lead="Les séances, ateliers, jams et échéances de l'association." />
                                <SignInState what="voir le calendrier" />
                            </Page>
                        }
                        .into_any(),
                        Err(_) => view! {
                            <Page pattern=Pattern::List>
                                <PageHeader title="Calendrier" />
                                <ErrorState message="Impossible de charger le calendrier." on_retry=Callback::new(move |()| me.refetch()) />
                            </Page>
                        }
                        .into_any(),
                    })
            }}
        </Suspense>
    }
}

/// The calendar itself.
#[component]
fn CalendarBoard() -> impl IntoView {
    // The current month, in UTC on both the server and the browser.
    let (month, set_month) = signal(chrono::Utc::now().format("%Y-%m").to_string());
    let (filter, set_filter) = signal(String::new());
    let (open, set_open) = signal(Option::<String>::None);
    let (editing, set_editing) = signal(Option::<CalendarEvent>::None);
    let (form_open, set_form_open) = signal(false);
    let (reload, set_reload) = signal(0_u32);

    let events = Resource::new(
        move || (month.get(), filter.get(), reload.get()),
        |(m, f, _)| async move { get_calendar(m, (!f.is_empty()).then_some(f)).await },
    );
    // Asked of the server, not inferred from the events on screen, so the
    // first event of an empty month can be created.
    let rights = Resource::new(|| (), |()| async { get_calendar_rights().await });

    let on_toggle = Callback::new(move |id: String| {
        set_open.update(|cur| *cur = if cur.as_deref() == Some(id.as_str()) { None } else { Some(id) });
    });
    let on_edit = Callback::new(move |e: CalendarEvent| {
        set_editing.set(Some(e));
        set_form_open.set(true);
    });
    let on_saved = Callback::new(move |()| {
        set_editing.set(None);
        set_form_open.set(false);
        set_reload.update(|n| *n += 1);
    });
    let on_cancel = Callback::new(move |()| {
        set_editing.set(None);
        set_form_open.set(false);
    });
    let on_changed = Callback::new(move |()| set_reload.update(|n| *n += 1));

    let filters = view! {
        <FilterBar label="Mois et track">
            <Stepper
                label=Signal::derive(move || month_label(&month.get()))
                previous="Mois précédent"
                next="Mois suivant"
                on_step=Callback::new(move |delta| set_month.update(|m| *m = shift_month(m, delta)))
            />
            <Field id="calendrier-track" label="Track" inline=true>
                <select id="calendrier-track" class="ui-control" on:change=move |ev| set_filter.set(event_target_value(&ev))>
                    <option value="">"Toutes"</option>
                    {track_choices()
                        .into_iter()
                        .map(|(id, label)| view! { <option value=id>{label}</option> })
                        .collect_view()}
                </select>
            </Field>
        </FilterBar>
    }
    .into_any();

    view! {
        <Page pattern=Pattern::List>
            <PageHeader
                title="Calendrier"
                lead="Les événements de l'association restent visibles quelle que soit la track choisie : le filtre ajoute les séances d'une track, il n'enlève pas l'assemblée générale."
                filters
            >
                <Suspense fallback=|| ()>
                    {move || {
                        rights.get().and_then(Result::ok).filter(|r| r.any).map(|_| view! {
                            <Button kind=ButtonKind::Primary icon=IconName::Plus
                                on:click=move |_| {
                                    set_editing.set(None);
                                    set_form_open.update(|o| *o = !*o);
                                }>
                                "Programmer"
                            </Button>
                        })
                    }}
                </Suspense>
            </PageHeader>

            <Suspense fallback=|| ()>
                {move || {
                    let r = rights.get().and_then(Result::ok).unwrap_or_default();
                    (r.any && form_open.get()).then(|| {
                        // Only the scopes this member may file under.
                        let allowed: Vec<(String, String)> = track_choices()
                            .into_iter()
                            .filter(|(id, _)| r.tracks.iter().any(|t| t == id))
                            .map(|(id, label)| (id.to_string(), label.to_string()))
                            .collect();
                        let heading = move || editing.get().map_or_else(
                            || "Programmer un événement".to_string(),
                            |e| format!("Modifier : {}", e.title),
                        );
                        view! {
                            <Panel>
                                <h2 class="ui-h3">{heading}</h2>
                                <EventForm editing on_saved on_cancel tracks=allowed association=r.association bureau=r.bureau />
                            </Panel>
                        }
                    })
                }}
            </Suspense>

            <Transition fallback=|| view! { <RowsSkeleton rows=4 /> }>
                {move || {
                    events
                        .get()
                        .map(|result| match result {
                            Err(_) => view! {
                                <ErrorState message="Impossible de charger le calendrier." on_retry=Callback::new(move |()| events.refetch()) />
                            }
                            .into_any(),
                            Ok(list) if list.is_empty() => view! {
                                <EmptyState
                                    icon=IconName::CalendarBlank
                                    title=format!("Rien de prévu en {}", month_label(&month.get_untracked()))
                                    body="Les séances programmées apparaîtront ici."
                                />
                            }
                            .into_any(),
                            Ok(list) => view! { <Agenda events=list open on_toggle on_edit on_changed /> }.into_any(),
                        })
                }}
            </Transition>
        </Page>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn month_shifts_roll_the_year() {
        assert_eq!(shift_month("2026-01", -1), "2025-12");
        assert_eq!(shift_month("2026-12", 1), "2027-01");
        assert_eq!(shift_month("2026-09", 1), "2026-10");
        assert_eq!(shift_month("2026-09", -1), "2026-08");
    }

    #[test]
    fn month_shifts_survive_a_long_jump() {
        assert_eq!(shift_month("2026-01", -13), "2024-12");
        assert_eq!(shift_month("2026-01", 25), "2028-02");
    }

    #[test]
    fn split_month_falls_back_rather_than_panicking() {
        assert_eq!(split_month("2026-09"), (2026, 9));
        assert_eq!(split_month("nonsense"), (2026, 1));
        assert_eq!(split_month("2026-13"), (2026, 1));
    }

    #[test]
    fn the_week_starts_on_monday() {
        // 1 September 2026 is a Tuesday; 1 March 2027 a Monday.
        assert_eq!(first_weekday(2026, 9), 1);
        assert_eq!(first_weekday(2026, 1), 3);
        assert_eq!(first_weekday(2027, 3), 0);
    }

    #[test]
    fn days_read_with_their_weekday() {
        assert_eq!(day_label("2026-09-15"), "mardi 15");
        assert_eq!(day_label("2026-09-20"), "dimanche 20");
        assert_eq!(month_label("2026-09"), "septembre 2026");
    }
}

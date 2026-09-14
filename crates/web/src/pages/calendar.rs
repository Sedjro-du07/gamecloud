//! Calendar — sessions, workshops, jams, deadlines.
//!
//! A month grid with the events laid on it, and, for whoever may manage
//! them, a form to schedule one and a panel to put its QR code on a
//! projector.
//!
//! Two decisions worth stating.
//!
//! **The month grid is built on the server's `day` string, not on a date
//! parsed in the browser.** Every event arrives carrying `YYYY-MM-DD`,
//! so bucketing is a string comparison. Nothing here has to know how a
//! French week starts or when the clocks change.
//!
//! **Management controls are driven by `can_manage`, which the server
//! computed.** The page never inspects the viewer's roles itself. That
//! keeps one answer to "may I edit this?" instead of two that can drift
//! apart — and the server re-checks anyway, so the hidden button is a
//! courtesy rather than the control.

use leptos::prelude::*;

use crate::{
    api::{CalendarEvent, EventDraft},
    server_fns::{
        cancel_event, delete_event, generate_event_qr, get_calendar, get_calendar_rights,
        get_event_attendees, save_event,
    },
};

/// Month names, so the header reads in French without pulling a locale
/// crate into the WASM bundle for twelve strings.
const MONTHS: [&str; 12] = [
    "janvier",
    "février",
    "mars",
    "avril",
    "mai",
    "juin",
    "juillet",
    "août",
    "septembre",
    "octobre",
    "novembre",
    "décembre",
];

/// Split a `YYYY-MM` into its parts.
///
/// Returns `(2026, 9)` for `"2026-09"`, and falls back to something
/// sane rather than panicking — the value comes from our own state, but
/// a slip should still render a calendar.
fn split_month(month: &str) -> (i32, u32) {
    let mut parts = month.split('-');
    let year = parts.next().and_then(|y| y.parse().ok()).unwrap_or(2026);
    let m = parts.next().and_then(|m| m.parse().ok()).unwrap_or(1);
    (year, if (1..=12).contains(&m) { m } else { 1 })
}

/// Step a `YYYY-MM` forward or back, rolling the year.
fn shift_month(month: &str, delta: i32) -> String {
    let (year, m) = split_month(month);
    // Work in months-since-year-zero so the rollover is arithmetic
    // rather than a pair of special cases. `split_month` has already
    // clamped the month to 1..=12, so the conversion cannot fail; the
    // fallback keeps January rather than panicking if that ever changes.
    let m = i32::try_from(m).unwrap_or(1);
    let total = year * 12 + (m - 1) + delta;
    let (y, m) = (total.div_euclid(12), total.rem_euclid(12) + 1);
    format!("{y:04}-{m:02}")
}

/// Day of the week the 1st of a month falls on, Monday = 0.
///
/// Sakamoto's method: a table lookup and a division, correct for any
/// Gregorian date. A calendar grid needs exactly this one fact and
/// nothing else about dates, so pulling a date library into the browser
/// bundle for it would be a poor trade.
fn first_weekday(year: i32, month: u32) -> u32 {
    const T: [i32; 12] = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let y = if month < 3 { year - 1 } else { year };
    let m = month as usize - 1;
    let dow = (y + y / 4 - y / 100 + y / 400 + T[m] + 1).rem_euclid(7);
    // Sakamoto yields Sunday = 0; the grid starts on Monday.
    u32::try_from((dow + 6).rem_euclid(7)).unwrap_or(0)
}

/// How many days a month has, leap years included.
fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ => {
            if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 {
                29
            } else {
                28
            }
        }
    }
}

/// The scheduling form, shown only to whoever may use it.
///
/// Doubles as the edit form: passing an existing event pre-fills it and
/// the save becomes an update. One form rather than two, because the
/// fields are identical and a second copy would drift.
#[component]
fn EventForm(
    /// Event being edited, or `None` to schedule a new one.
    editing: ReadSignal<Option<CalendarEvent>>,
    /// Clears the edit target once the save lands.
    on_saved: Callback<()>,
    /// Tracks the viewer may scope an event to.
    tracks: Vec<(String, String)>,
    /// Whether the viewer may file an association-wide event.
    association: bool,
    /// Whether the viewer may call a Bureau meeting.
    bureau: bool,
) -> impl IntoView {
    let track_options = tracks.clone();
    let (title, set_title) = signal(String::new());
    let (description, set_description) = signal(String::new());
    let (kind, set_kind) = signal("Session".to_string());
    let (track, set_track) = signal(
        tracks
            .first()
            .map(|(id, _)| id.clone())
            .unwrap_or_default(),
    );
    // Start on a scope this member may actually file under, so the form
    // cannot open on something the server would refuse.
    let (audience, set_audience) = signal(
        if association {
            "Association"
        } else if !tracks.is_empty() {
            "Track"
        } else {
            "Bureau"
        }
        .to_string(),
    );
    let (starts, set_starts) = signal(String::new());
    let (ends, set_ends) = signal(String::new());
    let (location, set_location) = signal(String::new());
    let (xp, set_xp) = signal("20".to_string());
    let (notice, set_notice) = signal(Option::<Result<String, String>>::None);

    // Pre-fill when the page hands us something to edit, and clear when
    // it hands us `None`. Done in an effect rather than at construction
    // because the same form instance serves every event in turn.
    Effect::new(move |_| {
        if let Some(e) = editing.get() {
            set_audience.set(e.audience.clone());
            if let Some(t) = e.track.clone() {
                set_track.set(t);
            }
            set_title.set(e.title.clone());
            set_description.set(e.description.clone().unwrap_or_default());
            set_kind.set(e.kind.clone());
            set_track.set(e.track.clone().unwrap_or_default());
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
            set_notice.set(Some(match result {
                Ok(_) => {
                    set_title.set(String::new());
                    set_description.set(String::new());
                    on_saved.run(());
                    Ok("Événement enregistré.".to_string())
                }
                Err(e) => Err(e.to_string()),
            }));
        }
    });

    let editing_label = move || {
        editing
            .get()
            .map_or("Programmer un événement".to_string(), |e| {
                format!("Modifier — {}", e.title)
            })
    };

    view! {
        <section class="gc-cal__form-card">
            <h2>{editing_label}</h2>

            {move || {
                notice
                    .get()
                    .map(|r| match r {
                        Ok(m) => view! { <div class="gc-banner gc-banner--ok">{m}</div> }.into_any(),
                        Err(m) => {
                            view! { <div class="gc-banner gc-banner--warning">{m}</div> }.into_any()
                        }
                    })
            }}

            <form
                class="gc-form gc-form--grid"
                on:submit=move |ev| {
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
                }
            >
                <label class="gc-field gc-field--wide">
                    <span>"Intitulé"</span>
                    <input
                        type="text"
                        required=true
                        placeholder="Séance du mardi"
                        prop:value=move || title.get()
                        on:input=move |ev| set_title.set(event_target_value(&ev))
                    />
                </label>

                <label class="gc-field">
                    <span>"Type"</span>
                    <select
                        prop:value=move || kind.get()
                        on:change=move |ev| set_kind.set(event_target_value(&ev))
                    >
                        <option value="Session">"Séance"</option>
                        <option value="Workshop">"Atelier"</option>
                        <option value="Jam">"Game jam"</option>
                        <option value="Meeting">"Réunion"</option>
                        <option value="Deadline">"Échéance"</option>
                        <option value="Showcase">"Présentation"</option>
                    </select>
                </label>

                <label class="gc-field">
                    <span>"Pour qui"</span>
                    <select
                        prop:value=move || audience.get()
                        on:change=move |ev| set_audience.set(event_target_value(&ev))
                    >
                        {association
                            .then(|| {
                                view! { <option value="Association">"Toute l'association"</option> }
                            })}
                        {(!tracks.is_empty())
                            .then(|| view! { <option value="Track">"Une track"</option> })}
                        {bureau
                            .then(|| view! { <option value="Bureau">"Le Bureau seulement"</option> })}
                    </select>
                </label>

                // Only asked for when it means something. A Bureau
                // meeting carrying a track would be refused by the
                // database, and offering the field invites the mistake.
                <Show when=move || audience.get() == "Track">
                    <label class="gc-field">
                        <span>"Track concernée"</span>
                        <select
                            prop:value=move || track.get()
                            on:change=move |ev| set_track.set(event_target_value(&ev))
                        >
                            {track_options
                                .iter()
                                .map(|(id, label)| {
                                    view! { <option value=id.clone()>{label.clone()}</option> }
                                })
                                .collect_view()}
                        </select>
                    </label>
                </Show>

                <Show when=move || audience.get() == "Bureau">
                    <p class="gc-cal__private">
                        "🔒 Visible du Bureau uniquement, et annoncé dans le salon du Bureau
                         en taguant ses membres. Personne d'autre ne verra cette réunion."
                    </p>
                </Show>

                <label class="gc-field">
                    <span>"Début"</span>
                    <input
                        type="datetime-local"
                        required=true
                        prop:value=move || starts.get()
                        on:input=move |ev| set_starts.set(event_target_value(&ev))
                    />
                </label>

                <label class="gc-field">
                    <span>"Fin"</span>
                    <input
                        type="datetime-local"
                        required=true
                        prop:value=move || ends.get()
                        on:input=move |ev| set_ends.set(event_target_value(&ev))
                    />
                </label>

                <label class="gc-field">
                    <span>"Lieu"</span>
                    <input
                        type="text"
                        placeholder="Salle 204, ou un lien"
                        prop:value=move || location.get()
                        on:input=move |ev| set_location.set(event_target_value(&ev))
                    />
                </label>

                <label class="gc-field">
                    <span>"XP de présence"</span>
                    <input
                        type="number"
                        min="0"
                        max="500"
                        prop:value=move || xp.get()
                        on:input=move |ev| set_xp.set(event_target_value(&ev))
                    />
                </label>

                <label class="gc-field gc-field--wide">
                    <span>"Description"</span>
                    <textarea
                        rows="2"
                        placeholder="Ce qui sera fait, ce qu'il faut apporter."
                        prop:value=move || description.get()
                        on:input=move |ev| set_description.set(event_target_value(&ev))
                    ></textarea>
                </label>

                <div class="gc-form__actions">
                    <button
                        class="gc-btn gc-btn--primary"
                        type="submit"
                        disabled=move || save.pending().get()
                    >
                        {move || if save.pending().get() { "Envoi…" } else { "Enregistrer" }}
                    </button>
                    <Show when=move || editing.get().is_some()>
                        <button
                            class="gc-btn"
                            type="button"
                            on:click=move |_| on_saved.run(())
                        >
                            "Annuler la modification"
                        </button>
                    </Show>
                </div>
            </form>
        </section>
    }
}

/// The QR panel and attendance sheet for one event.
#[component]
fn EventPanel(
    /// The event being looked at.
    event: CalendarEvent,
    /// Asks the page to load this event into the edit form.
    on_edit: Callback<CalendarEvent>,
    /// Asks the page to reload after a cancellation.
    on_changed: Callback<()>,
) -> impl IntoView {
    let id = event.id.clone();
    let (minutes, set_minutes) = signal("120".to_string());
    let (ticket, set_ticket) = signal(Option::<crate::api::QrTicket>::None);
    let (error, set_error) = signal(Option::<String>::None);

    let mint = Action::new(move |(id, mins): &(String, i32)| {
        let (id, mins) = (id.clone(), *mins);
        async move { generate_event_qr(id, mins, None).await }
    });

    Effect::new(move |_| {
        if let Some(result) = mint.value().get() {
            match result {
                Ok(t) => {
                    set_error.set(None);
                    set_ticket.set(Some(t));
                }
                Err(e) => set_error.set(Some(e.to_string())),
            }
        }
    });

    let cancel = Action::new(move |(id, to): &(String, bool)| {
        let (id, to) = (id.clone(), *to);
        async move { cancel_event(id, to).await }
    });

    Effect::new(move |_| {
        if cancel.value().get().is_some() {
            on_changed.run(());
        }
    });

    // Deleting is separate from cancelling and deliberately narrower:
    // the server refuses it once anybody has scanned in, because that
    // attendance is somebody's XP and somebody's record of being there.
    let remove = Action::new(move |id: &String| {
        let id = id.clone();
        async move { delete_event(id).await }
    });

    Effect::new(move |_| {
        if let Some(result) = remove.value().get() {
            match result {
                Ok(()) => on_changed.run(()),
                Err(e) => set_error.set(Some(e.to_string())),
            }
        }
    });

    let attendees = Resource::new(
        {
            let id = event.id.clone();
            move || id.clone()
        },
        |id| async move { get_event_attendees(id).await },
    );

    let cancelled = event.cancelled;
    let is_cancelled = event.cancelled;
    let for_edit = event.clone();
    let cancel_id = event.id.clone();
    let delete_id = event.id.clone();
    let attendee_count = event.attendee_count;
    let has_attendance = attendee_count > 0;

    view! {
        <div class="gc-cal__panel">
            <div class="gc-cal__panel-actions">
                <button
                    class="gc-btn"
                    type="button"
                    on:click=move |_| on_edit.run(for_edit.clone())
                >
                    "Modifier"
                </button>
                <button
                    class="gc-btn gc-btn--ghost"
                    type="button"
                    disabled=move || cancel.pending().get()
                    on:click=move |_| {
                        cancel.dispatch((cancel_id.clone(), !is_cancelled));
                    }
                >
                    {if cancelled { "Réactiver" } else { "Annuler l'événement" }}
                </button>
                <button
                    class="gc-btn gc-btn--ghost"
                    type="button"
                    disabled=move || remove.pending().get() || has_attendance
                    title=if has_attendance {
                        "Des présences sont enregistrées : annulez plutôt que de supprimer."
                    } else {
                        "Retirer définitivement cet événement du calendrier"
                    }
                    on:click=move |_| {
                        remove.dispatch(delete_id.clone());
                    }
                >
                    "Supprimer"
                </button>
            </div>

            {move || {
                error.get().map(|e| view! { <div class="gc-banner gc-banner--warning">{e}</div> })
            }}

            <div class="gc-cal__qr">
                <h4>"Code de présence"</h4>
                <p class="gc-admin__note">
                    "Le code encode un lien : n'importe quel appareil photo l'ouvre.
                     Personne n'a besoin d'une application."
                </p>
                <div class="gc-cal__qr-controls">
                    <label class="gc-field">
                        <span>"Valable (minutes)"</span>
                        <input
                            type="number"
                            min="1"
                            max="1440"
                            prop:value=move || minutes.get()
                            on:input=move |ev| set_minutes.set(event_target_value(&ev))
                        />
                    </label>
                    <button
                        class="gc-btn gc-btn--primary"
                        type="button"
                        disabled=move || mint.pending().get() || cancelled
                        on:click=move |_| {
                            mint.dispatch((
                                id.clone(),
                                minutes.get().trim().parse().unwrap_or(120),
                            ));
                        }
                    >
                        {move || if mint.pending().get() { "Génération…" } else { "Générer le code" }}
                    </button>
                </div>

                {move || {
                    error.get().map(|e| view! { <div class="gc-banner gc-banner--warning">{e}</div> })
                }}

                {move || {
                    ticket
                        .get()
                        .map(|t| {
                            view! {
                                <figure class="gc-cal__qr-out">
                                    <div class="gc-cal__qr-svg" inner_html=t.svg></div>
                                    <figcaption>
                                        "Expire le " {t.expires_label} <br />
                                        <code class="gc-cal__qr-url">{t.scan_url}</code>
                                    </figcaption>
                                </figure>
                            }
                        })
                }}
            </div>

            <div class="gc-cal__sheet">
                <h4>{format!("Présences ({attendee_count})")}</h4>
                <Suspense fallback=move || view! { <p class="gc-empty">"Chargement…"</p> }>
                    {move || match attendees.get() {
                        Some(Ok(rows)) if rows.is_empty() => {
                            view! { <p class="gc-empty">"Personne n'a encore scanné."</p> }.into_any()
                        }
                        Some(Ok(rows)) => {
                            view! {
                                <ul class="gc-cal__attendees">
                                    {rows
                                        .into_iter()
                                        .map(|a| {
                                            view! {
                                                <li>
                                                    <span class="gc-cal__who">{a.display_name}</span>
                                                    <span class="gc-cal__when">{a.when}</span>
                                                    <span class="gc-cal__xp">
                                                        {format!("+{} XP", a.xp_rewarded)}
                                                    </span>
                                                </li>
                                            }
                                        })
                                        .collect_view()}
                                </ul>
                            }
                                .into_any()
                        }
                        Some(Err(e)) => {
                            view! { <p class="gc-empty">{format!("Feuille indisponible : {e}")}</p> }
                                .into_any()
                        }
                        None => view! { <p class="gc-empty">"Chargement…"</p> }.into_any(),
                    }}
                </Suspense>
            </div>
        </div>
    }
}

/// One event's card in the day cell or the agenda list.
#[component]
fn EventCard(
    /// The event.
    event: CalendarEvent,
    /// Whether this card is expanded.
    ///
    /// A `Signal`, not a `bool`: a plain bool is read once when the card
    /// is built and never again, so the card could be drawn collapsed
    /// and stay that way for ever — which is exactly what it did, taking
    /// the edit, cancel and QR controls with it.
    open: Signal<bool>,
    /// Toggles expansion.
    on_toggle: Callback<String>,
    /// Loads this event into the edit form.
    on_edit: Callback<CalendarEvent>,
    /// Asks the page to reload.
    on_changed: Callback<()>,
) -> impl IntoView {
    let id = event.id.clone();
    let classes = {
        let mut c = format!("gc-cal__event gc-cal__event--{}", event.kind.to_lowercase());
        // A private meeting has to be unmistakable on a calendar that is
        // otherwise entirely public.
        if event.audience == "Bureau" {
            c.push_str(" gc-cal__event--bureau");
        }
        if event.cancelled {
            c.push_str(" gc-cal__event--off");
        }
        if event.past {
            c.push_str(" gc-cal__event--past");
        }
        c
    };
    let can_manage = event.can_manage;
    // `Show` re-runs its children on every toggle, so the panel's copy
    // has to survive being read more than once. `StoredValue` is `Copy`
    // and hands out a fresh clone each time; a plain captured `String`
    // would make the closure `FnOnce`.
    let panel_event = StoredValue::new(event.clone());

    view! {
        <article class=classes>
            <button
                class="gc-cal__event-head"
                type="button"
                on:click=move |_| on_toggle.run(id.clone())
            >
                <span class="gc-cal__time">{event.time_label.clone()}</span>
                <span class="gc-cal__title">
                    {event.track_emoji.clone().unwrap_or_default()} " " {event.title.clone()}
                </span>
                <span class="gc-cal__kind">
                    {if event.audience == "Bureau" {
                        format!("🔒 {}", event.kind_label)
                    } else {
                        event.kind_label.clone()
                    }}
                </span>
            </button>

            <Show when=move || open.get()>
                <div class="gc-cal__body">
                    {event
                        .description
                        .clone()
                        .map(|d| view! { <p class="gc-cal__desc">{d}</p> })}
                    <ul class="gc-cal__meta">
                        {event
                            .location
                            .clone()
                            .map(|l| view! { <li>"📍 " {l}</li> })}
                        <li>{format!("⭐ {} XP de présence", event.xp_reward)}</li>
                        <li>{format!("👥 {} présent(s)", event.attendee_count)}</li>
                        <li>{format!("🎯 {}", event.audience_label)}</li>
                        {event.cancelled.then(|| view! { <li class="gc-cal__off">"Annulé"</li> })}
                    </ul>

                    <Show when=move || can_manage>
                        <EventPanel
                            event=panel_event.get_value()
                            on_edit=on_edit
                            on_changed=on_changed
                        />
                    </Show>
                </div>
            </Show>
        </article>
    }
}

/// The month grid.
#[component]
fn MonthGrid(
    /// `YYYY-MM` being shown.
    month: Memo<String>,
    /// Everything in that month.
    events: Vec<CalendarEvent>,
    /// Which event is expanded.
    open: ReadSignal<Option<String>>,
    /// Toggles expansion.
    on_toggle: Callback<String>,
    /// Loads an event into the edit form.
    on_edit: Callback<CalendarEvent>,
    /// Asks the page to reload.
    on_changed: Callback<()>,
) -> impl IntoView {
    let (year, m) = split_month(&month.get());
    let lead = first_weekday(year, m);
    let days = days_in_month(year, m);

    // The leading blanks keep the 1st under the right weekday. They are
    // rendered rather than skipped so the grid has a cell in every
    // position and CSS Grid does not reflow the row.
    let blanks = (0..lead)
        .map(|i| view! { <div class="gc-cal__cell gc-cal__cell--blank" data-i=i></div> })
        .collect_view();

    let cells = (1..=days)
        .map(|d| {
            let key = format!("{year:04}-{m:02}-{d:02}");
            let today = events
                .iter()
                .filter(|e| e.day == key)
                .cloned()
                .collect::<Vec<_>>();
            let count = today.len();
            view! {
                <div
                    class=if count == 0 {
                        "gc-cal__cell"
                    } else {
                        "gc-cal__cell gc-cal__cell--busy"
                    }
                >
                    <span class="gc-cal__daynum">{d}</span>
                    {today
                        .into_iter()
                        .map(|e| {
                            let id = e.id.clone();
                            let is_open = Memo::new({
                                let id = id.clone();
                                move |_| open.get().as_deref() == Some(id.as_str())
                            });
                            view! {
                                <EventCard
                                    event=e
                                    open=is_open.into()
                                    on_toggle=on_toggle
                                    on_edit=on_edit
                                    on_changed=on_changed
                                />
                            }
                        })
                        .collect_view()}
                </div>
            }
        })
        .collect_view();

    view! {
        <div class="gc-cal__grid">
            {["lun", "mar", "mer", "jeu", "ven", "sam", "dim"]
                .into_iter()
                .map(|d| view! { <div class="gc-cal__dow">{d}</div> })
                .collect_view()}
            {blanks}
            {cells}
        </div>
    }
}

/// The month stepper and the track filter.
///
/// Extracted so [`CalendarPage`] reads as "load the month, draw it,
/// offer the form" rather than opening with thirty lines of chrome.
#[component]
fn CalendarHead(
    /// `YYYY-MM` currently shown.
    month: ReadSignal<String>,
    /// Steps the month.
    set_month: WriteSignal<String>,
    /// Writes the track filter.
    set_filter: WriteSignal<String>,
    /// Tracks that can be filtered on.
    tracks: Vec<(String, String)>,
) -> impl IntoView {
    let heading = move || {
        let (y, m) = split_month(&month.get());
        format!("{} {y}", MONTHS[(m as usize).saturating_sub(1).min(11)])
    };

    view! {
        <header class="gc-cal__head">
            <h1>"Calendrier"</h1>
            <div class="gc-cal__nav">
                <button
                    class="gc-btn gc-btn--ghost"
                    type="button"
                    aria-label="Mois précédent"
                    on:click=move |_| set_month.update(|m| *m = shift_month(m, -1))
                >
                    "‹"
                </button>
                <span class="gc-cal__month">{heading}</span>
                <button
                    class="gc-btn gc-btn--ghost"
                    type="button"
                    aria-label="Mois suivant"
                    on:click=move |_| set_month.update(|m| *m = shift_month(m, 1))
                >
                    "›"
                </button>
            </div>
            <label class="gc-field gc-cal__filter">
                <span>"Track"</span>
                <select on:change=move |ev| set_filter.set(event_target_value(&ev))>
                    <option value="">"Tout"</option>
                    {tracks
                        .into_iter()
                        .map(|(id, label)| view! { <option value=id>{label}</option> })
                        .collect_view()}
                </select>
            </label>
        </header>
    }
}

/// Calendar page.
#[component]
pub fn CalendarPage() -> impl IntoView {
    // Start on the current month. Computed once, from the browser on
    // hydration and from the server on the first render — both agree,
    // because both ask for "now" in UTC.
    let current = {
        let now = chrono::Utc::now();
        format!("{}", now.format("%Y-%m"))
    };
    let (month, set_month) = signal(current);
    let (filter, set_filter) = signal(String::new());
    let (open, set_open) = signal(Option::<String>::None);
    let (editing, set_editing) = signal(Option::<CalendarEvent>::None);
    let (reload, set_reload) = signal(0_u32);

    let month_memo = Memo::new(move |_| month.get());

    let events = Resource::new(
        move || (month.get(), filter.get(), reload.get()),
        |(m, f, _)| async move {
            get_calendar(m, (!f.is_empty()).then_some(f)).await
        },
    );

    let tracks: Vec<(String, String)> = gamecloud_shared::roles::Track::ALL
        .iter()
        .map(|t| {
            (
                t.as_str().to_string(),
                format!("{} {}", t.emoji(), t.as_str()),
            )
        })
        .collect();
    let filter_tracks = tracks.clone();

    let on_toggle = Callback::new(move |id: String| {
        set_open.update(|cur| {
            *cur = if cur.as_deref() == Some(id.as_str()) {
                None
            } else {
                Some(id)
            };
        });
    });
    let on_edit = Callback::new(move |e: CalendarEvent| set_editing.set(Some(e)));
    let on_saved = Callback::new(move |()| {
        set_editing.set(None);
        set_reload.update(|n| *n += 1);
    });
    let on_changed = Callback::new(move |()| set_reload.update(|n| *n += 1));

    // Asked of the server, not inferred from what is on screen. The
    // page used to look for an event the viewer could manage, which
    // meant the first event of a month could never be created: the form
    // only appeared once an event was already there.
    let rights = Resource::new(|| (), |()| async { get_calendar_rights().await });

    view! {
        <section class="gc-cal">
            <CalendarHead
                month=month
                set_month=set_month
                set_filter=set_filter
                tracks=filter_tracks
            />

            <p class="gc-cal__hint">
                "Les événements d'association apparaissent quelle que soit la track choisie :
                 un filtre sur une track ajoute ses séances, il n'enlève pas l'assemblée générale."
            </p>

            <Suspense fallback=move || view! { <p class="gc-empty">"Chargement du calendrier…"</p> }>
                {move || match events.get() {
                    None => view! { <p class="gc-empty">"Chargement du calendrier…"</p> }.into_any(),
                    Some(Err(e)) => {
                        view! { <p class="gc-empty">{format!("Calendrier indisponible : {e}")}</p> }
                            .into_any()
                    }
                    Some(Ok(list)) => {
                        view! {
                            <MonthGrid
                                month=month_memo
                                events=list
                                open=open
                                on_toggle=on_toggle
                                on_edit=on_edit
                                on_changed=on_changed
                            />
                        }
                            .into_any()
                    }
                }}
            </Suspense>

            <Suspense fallback=|| ()>
                {move || {
                    let r = rights.get().and_then(Result::ok).unwrap_or_default();
                    r.any
                        .then(|| {
                            // Only offer the scopes this member may
                            // actually file an event under, so the form
                            // cannot produce a refusal.
                            let allowed: Vec<(String, String)> = tracks
                                .iter()
                                .filter(|(id, _)| r.tracks.iter().any(|t| t == id))
                                .cloned()
                                .collect();
                            view! {
                                <EventForm
                                    editing=editing
                                    on_saved=on_saved
                                    tracks=allowed
                                    association=r.association
                                    bureau=r.bureau
                                />
                            }
                        })
                }}
            </Suspense>
        </section>
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
    fn february_knows_about_leap_years() {
        assert_eq!(days_in_month(2024, 2), 29);
        assert_eq!(days_in_month(2025, 2), 28);
        assert_eq!(days_in_month(2000, 2), 29);
        assert_eq!(days_in_month(1900, 2), 28);
    }

    #[test]
    fn month_lengths_are_right() {
        assert_eq!(days_in_month(2026, 1), 31);
        assert_eq!(days_in_month(2026, 4), 30);
        assert_eq!(days_in_month(2026, 9), 30);
        assert_eq!(days_in_month(2026, 12), 31);
    }

    #[test]
    fn the_grid_starts_the_week_on_monday() {
        // 1 September 2026 is a Tuesday, so it sits in column 1.
        assert_eq!(first_weekday(2026, 9), 1);
        // 1 January 2026 is a Thursday.
        assert_eq!(first_weekday(2026, 1), 3);
        // 1 March 2027 is a Monday — column 0.
        assert_eq!(first_weekday(2027, 3), 0);
    }

    #[test]
    fn every_weekday_is_in_range() {
        for year in 2020..2030 {
            for month in 1..=12 {
                assert!(first_weekday(year, month) < 7, "{year}-{month}");
            }
        }
    }
}

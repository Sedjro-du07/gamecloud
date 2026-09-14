//! Verdict form — the notation itself.
//!
//! One form per track the viewer may judge. The important rule is
//! enforced here as well as in the API: **a rejection must say what has
//! to change**. A reviewer who refuses a build without explaining why
//! has not reviewed it, and the submit button stays disabled until the
//! feedback box has content.
//!
//! `NotApplicable` is the third option and it matters: a text adventure
//! has no Audio deliverable, and forcing the Audio lead to either
//! approve or reject it would corrupt the record either way.

use leptos::prelude::*;

use crate::server_fns::review_project;

/// The three-way verdict picker.
///
/// Split out so [`ReviewForm`] stays a readable form rather than a wall
/// of markup, and so the chosen option can borrow the colour of the
/// badge it will become — the decision then reads the same before and
/// after it is recorded.
#[component]
fn VerdictPicker(
    /// Currently selected verdict.
    chosen: ReadSignal<String>,
    /// Called with the new verdict.
    on_pick: WriteSignal<String>,
) -> impl IntoView {
    view! {
        <div class="gc-review__verdicts" role="radiogroup" aria-label="Verdict">
            {[
                ("Approved", "✅ Approuver", "gc-verdict--ok"),
                ("Rejected", "❌ Refuser", "gc-verdict--no"),
                ("NotApplicable", "— Non applicable", "gc-verdict--na"),
            ]
                .into_iter()
                .map(|(value, label, modifier)| {
                    let v = value.to_string();
                    // `Memo` is `Copy`, so the class, the aria state and
                    // the click handler can each read it independently.
                    let is_on = {
                        let v = v.clone();
                        Memo::new(move |_| chosen.get() == v)
                    };
                    let pick = v.clone();
                    view! {
                        <button
                            type="button"
                            class=move || {
                                if is_on.get() {
                                    format!("gc-review__choice gc-review__choice--on {modifier}")
                                } else {
                                    "gc-review__choice".to_string()
                                }
                            }
                            aria-pressed=move || is_on.get().to_string()
                            on:click=move |_| on_pick.set(pick.clone())
                        >
                            {label}
                        </button>
                    }
                })
                .collect_view()}
        </div>
    }
}

/// Verdict form for one track.
#[component]
#[allow(clippy::needless_pass_by_value)] // Leptos prop convention
pub fn ReviewForm(
    /// Project being judged.
    project_id: String,
    /// Track this form renders a verdict for.
    track: String,
    /// Called with the project's new status once the verdict lands.
    on_done: Callback<String>,
) -> impl IntoView {
    let (verdict, set_verdict) = signal("Approved".to_string());
    let (feedback, set_feedback) = signal(String::new());
    let (error, set_error) = signal(Option::<String>::None);

    let submit = Action::new(
        move |(id, track, verdict, feedback): &(String, String, String, Option<String>)| {
            let (id, track, verdict, feedback) =
                (id.clone(), track.clone(), verdict.clone(), feedback.clone());
            async move { review_project(id, track, verdict, feedback).await }
        },
    );

    Effect::new(move |_| {
        if let Some(result) = submit.value().get() {
            match result {
                Ok(status) => {
                    set_error.set(None);
                    set_feedback.set(String::new());
                    on_done.run(status);
                }
                Err(e) => set_error.set(Some(e.to_string())),
            }
        }
    });

    // A rejection without feedback is refused by the API; refusing it
    // here too means the reviewer finds out before losing what they typed.
    let feedback_required = move || verdict.get() == "Rejected";
    let blocked = move || {
        submit.pending().get() || (feedback_required() && feedback.get().trim().is_empty())
    };

    let track_label = track.clone();

    view! {
        <form
            class="gc-review"
            on:submit=move |ev| {
                ev.prevent_default();
                if blocked() {
                    return;
                }
                let text = feedback.get();
                submit
                    .dispatch((
                        project_id.clone(),
                        track.clone(),
                        verdict.get(),
                        (!text.trim().is_empty()).then_some(text),
                    ));
            }
        >
            <h3 class="gc-review__title">"Votre verdict — " {track_label}</h3>

            {move || {
                error
                    .get()
                    .map(|e| view! { <div class="gc-banner gc-banner--warning">{e}</div> })
            }}

            <VerdictPicker chosen=verdict on_pick=set_verdict />

            <label class="gc-field">
                <span>
                    {move || {
                        if feedback_required() {
                            "Ce qui doit changer (obligatoire)"
                        } else {
                            "Retour à l'équipe (optionnel)"
                        }
                    }}
                </span>
                <textarea
                    rows="4"
                    placeholder="Soyez précis : c'est ce que l'équipe lira pour corriger."
                    prop:value=move || feedback.get()
                    on:input=move |ev| set_feedback.set(event_target_value(&ev))
                ></textarea>
            </label>

            <button class="gc-btn gc-btn--primary" type="submit" disabled=blocked>
                {move || {
                    if submit.pending().get() {
                        "Envoi…"
                    } else {
                        "Rendre le verdict"
                    }
                }}
            </button>

            <Show when=move || feedback_required() && feedback.get().trim().is_empty()>
                <p class="gc-review__hint">
                    "Un refus doit expliquer ce qui ne va pas — sinon l'équipe ne peut rien corriger."
                </p>
            </Show>
        </form>
    }
}

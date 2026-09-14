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
//!
//! A **mark out of 100** rides alongside the verdict. It is optional on
//! purpose: approving is a gate, grading is a judgement, and a reviewer
//! who only wants to open the gate should not be forced to invent a
//! number. When they do give one, it is the track's mark for the track's
//! share of the work — not a mark for the whole project.

use leptos::prelude::*;

use crate::server_fns::review_project;

/// What one submitted verdict carries.
///
/// Named because the tuple is five fields wide and appears in both the
/// action's type and its dispatch; an anonymous tuple that long is a
/// transposition waiting to happen.
type Submission = (String, String, String, Option<String>, Option<i32>);

/// A mark the reviewer has typed, once it has been checked.
///
/// `Ok(None)` is an empty box, which is allowed — grading is optional.
/// `Err(())` is something that is not a mark at all, which blocks the
/// submit rather than travelling to the server and coming back after
/// the form has been cleared.
type ParsedScore = Result<Option<i32>, ()>;

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

/// The box the reviewer writes in.
///
/// Its label changes with the verdict, because "optional feedback" and
/// "what has to change before this can pass" are different requests and
/// a reviewer should be able to tell which one they are answering.
#[component]
fn FeedbackField(
    /// What the reviewer has written.
    feedback: ReadSignal<String>,
    /// Writes it back.
    set_feedback: WriteSignal<String>,
    /// Whether the chosen verdict makes it mandatory.
    required: Signal<bool>,
) -> impl IntoView {
    view! {
        <label class="gc-field">
            <span>
                {move || {
                    if required.get() {
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
    }
}

/// The optional mark out of 100.
///
/// Split out so [`ReviewForm`] stays a form rather than a wall of
/// markup, and because the field carries its own rule — empty is fine,
/// anything unparseable is not — which reads better next to the input
/// than buried among the verdict controls.
#[component]
fn ScoreField(
    /// Raw text the reviewer typed.
    score: ReadSignal<String>,
    /// Writes that text back.
    set_score: WriteSignal<String>,
    /// The checked mark, or `Err` when the text is not one.
    parsed: Memo<ParsedScore>,
    /// Whether a mark means anything for the chosen verdict.
    applies: Signal<bool>,
) -> impl IntoView {
    view! {
        <Show when=move || applies.get()>
            <label class="gc-field gc-review__score">
                <span>"Note sur 100 " <em>"(facultative)"</em></span>
                <input
                    type="number"
                    min="0"
                    max="100"
                    inputmode="numeric"
                    placeholder="—"
                    prop:value=move || score.get()
                    on:input=move |ev| set_score.set(event_target_value(&ev))
                />
                <small class="gc-review__hint">
                    "Elle porte sur la part de cette track, pas sur le projet entier.
                     Laissez vide pour simplement valider sans noter."
                </small>
            </label>
        </Show>

        <Show when=move || parsed.get().is_err()>
            <p class="gc-review__hint gc-review__hint--bad">
                "La note doit être un entier compris entre 0 et 100."
            </p>
        </Show>
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
    let (score, set_score) = signal(String::new());
    let (error, set_error) = signal(Option::<String>::None);

    let submit = Action::new(move |args: &Submission| {
        let (id, track, verdict, feedback, score) = args.clone();
        async move { review_project(id, track, verdict, feedback, score).await }
    });

    Effect::new(move |_| {
        if let Some(result) = submit.value().get() {
            match result {
                Ok(status) => {
                    set_error.set(None);
                    set_feedback.set(String::new());
                    set_score.set(String::new());
                    on_done.run(status);
                }
                Err(e) => set_error.set(Some(e.to_string())),
            }
        }
    });

    // A rejection without feedback is refused by the API; refusing it
    // here too means the reviewer finds out before losing what they typed.
    let feedback_required = move || verdict.get() == "Rejected";

    // An empty box means "no mark", which is allowed. Anything else has
    // to be a number on the scale — typing "9/10" or "95%" should stop
    // the reviewer here rather than travel to the server and come back
    // as a validation error with the form already cleared.
    let parsed_score = Memo::new(move |_| {
        let raw = score.get();
        let raw = raw.trim();
        if raw.is_empty() {
            return Ok(None);
        }
        match raw.parse::<i32>() {
            Ok(n) if (0..=100).contains(&n) => Ok(Some(n)),
            _ => Err(()),
        }
    });

    // A mark on something the track does not deliver is meaningless, so
    // the field disappears rather than being quietly ignored.
    let score_applies = move || verdict.get() != "NotApplicable";

    let blocked = move || {
        submit.pending().get()
            || (feedback_required() && feedback.get().trim().is_empty())
            || parsed_score.get().is_err()
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
                let mark = parsed_score.get().unwrap_or(None).filter(|_| score_applies());
                submit
                    .dispatch((
                        project_id.clone(),
                        track.clone(),
                        verdict.get(),
                        (!text.trim().is_empty()).then_some(text),
                        mark,
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

            <FeedbackField
                feedback=feedback
                set_feedback=set_feedback
                required=Signal::derive(feedback_required)
            />

            <ScoreField
                score=score
                set_score=set_score
                parsed=parsed_score
                applies=Signal::derive(score_applies)
            />

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

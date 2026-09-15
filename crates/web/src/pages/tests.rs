//! Entrance tests.
//!
//! The Bureau opens a session with a PDF subject and a duration. Until it
//! closes, candidates — anybody signed in who is not a verified member
//! yet — download the subject and hand in their work. The Bureau
//! downloads the work here and admits or turns down each candidate;
//! admitting somebody who is not on the Discord server creates their
//! invitation, shown to them on this same page.
//!
//! Members never see this page: once verified, it is no longer for them,
//! unless they hold a Bureau office. Uploads are plain multipart forms
//! posting to `/api/tests`, which redirect back here with `?ok` or
//! `?erreur=…`.

use leptos::prelude::*;
use leptos_router::hooks::use_query_map;

use crate::{
    api::{SubmissionItem, TestItem, TestsView},
    components::sign_in_prompt::SignInPrompt,
    server_fns::{close_test, delete_test, get_test_submissions, get_tests, judge_submission},
};

/// Entrance tests page.
#[component]
pub fn TestsPage() -> impl IntoView {
    let tests = Resource::new(|| (), |()| async { get_tests().await });
    let on_changed = Callback::new(move |()| tests.refetch());
    let query = use_query_map();

    view! {
        <section class="gc-tests">
            <h1>"🎓 Tests d'entrée"</h1>

            {move || {
                query
                    .read()
                    .get("ok")
                    .map(|what| {
                        let text = if what == "session" {
                            "Test ouvert : les candidats peuvent télécharger le sujet."
                        } else {
                            "Rendu reçu. Tu peux le remplacer jusqu'à la fin du test."
                        };
                        view! { <div class="gc-banner gc-banner--ok">{text}</div> }
                    })
            }}
            {move || {
                query
                    .read()
                    .get("erreur")
                    .map(|e| view! { <div class="gc-banner gc-banner--warning">{e}</div> })
            }}

            <Suspense fallback=|| view! { <p class="gc-empty">"Chargement…"</p> }>
                {move || {
                    tests
                        .get()
                        .map(|result| match result {
                            Err(_) => {
                                view! { <p class="gc-empty">"Impossible de charger les tests."</p> }
                                    .into_any()
                            }
                            Ok(view) => match view.access.as_str() {
                                "bureau" => view! { <BureauView view on_changed /> }.into_any(),
                                "candidate" => view! { <CandidateView view /> }.into_any(),
                                "signin" => view! { <VisitorView view /> }.into_any(),
                                _ => {
                                    view! {
                                        <div class="gc-banner">
                                            "Cette partie est réservée au Bureau et aux candidats
                                             qui passent le test d'entrée."
                                        </div>
                                    }
                                        .into_any()
                                }
                            },
                        })
                }}
            </Suspense>
        </section>
    }
}

// ---------------------------------------------------------------------------
// Visitors
// ---------------------------------------------------------------------------

/// What somebody who is not signed in sees: how joining works, and the
/// tests open right now. The subject and handing in need an account.
#[component]
#[allow(clippy::needless_pass_by_value)] // Leptos prop convention
fn VisitorView(
    /// The page.
    view: TestsView,
) -> impl IntoView {
    let list = if view.tests.is_empty() {
        view! { <p class="gc-empty">"Aucun test ouvert pour l'instant. Reviens bientôt."</p> }
            .into_any()
    } else {
        view! {
            <ul class="gc-tests__list">
                {view
                    .tests
                    .into_iter()
                    .map(|test| {
                        view! {
                            <li class="gc-test gc-test--open">
                                <div class="gc-test__head">
                                    <h3>{test.title}</h3>
                                    <span class="gc-chip">{test.time_left}</span>
                                </div>
                                {test.description.map(|d| view! { <p class="gc-test__desc">{d}</p> })}
                                <p class="gc-test__meta">"Fin : " {test.closes}</p>
                            </li>
                        }
                    })
                    .collect_view()}
            </ul>
        }
            .into_any()
    };

    view! {
        <div class="gc-tests__how">
            <h2>"Rejoindre l'association"</h2>
            <ol>
                <li>
                    "Déjà sur le serveur Discord de l'association ? Connecte-toi avec Discord :
                     l'inscription est directe."
                </li>
                <li>
                    "Sinon, connecte-toi avec Discord, télécharge le sujet d'un test ouvert et rends
                     ton travail avant la fin."
                </li>
                <li>
                    "Le Bureau corrige. Une fois admis, ton invitation au serveur s'affiche ici et tu
                     finis ton inscription."
                </li>
            </ol>
        </div>
        <SignInPrompt what="passer le test d'entrée" />
        <h2 class="gc-tests__heading">"Tests ouverts"</h2>
        {list}
    }
}

// ---------------------------------------------------------------------------
// Candidates
// ---------------------------------------------------------------------------

/// What a candidate sees.
#[component]
#[allow(clippy::needless_pass_by_value)] // Leptos prop convention
fn CandidateView(
    /// The page.
    view: TestsView,
) -> impl IntoView {
    let banner = if let Some(url) = view.invite_url.clone() {
        view! {
            <div class="gc-banner gc-banner--ok">
                "🎉 Tu es admis ! "
                <a href=url rel="external noopener" target="_blank">"Rejoins le serveur Discord"</a>
                " (lien valable une semaine), puis "
                <a href="/onboarding/email">"vérifie ton adresse Epitech"</a>
                " pour finir ton inscription."
            </div>
        }
            .into_any()
    } else if view.admitted {
        view! {
            <div class="gc-banner gc-banner--ok">
                "🎉 Tu es admis ! "
                <a href="/onboarding/email">"Vérifie ton adresse Epitech"</a>
                " pour finir ton inscription."
            </div>
        }
            .into_any()
    } else if view.is_candidate {
        view! {
            <div class="gc-banner">
                "Tu n'es pas encore sur le serveur Discord de l'association : l'inscription
                 s'ouvre quand tu as réussi un test d'entrée. Une fois admis, ton invitation
                 au serveur apparaît ici."
            </div>
        }
            .into_any()
    } else {
        view! {
            <p class="gc-tests__intro">
                "Télécharge le sujet et rends ton travail avant la fin du test. Le Bureau
                 corrige et te répond ici."
            </p>
        }
            .into_any()
    };

    let list = if view.tests.is_empty() {
        view! { <p class="gc-empty">"Aucun test ouvert pour l'instant. Reviens bientôt."</p> }
            .into_any()
    } else {
        view! {
            <ul class="gc-tests__list">
                {view
                    .tests
                    .into_iter()
                    .map(|test| view! { <CandidateTest test /> })
                    .collect_view()}
            </ul>
        }
            .into_any()
    };

    view! {
        {banner}
        {list}
    }
}

/// One session, for a candidate.
#[component]
#[allow(clippy::needless_pass_by_value)] // Leptos prop convention
fn CandidateTest(
    /// The session.
    test: TestItem,
) -> impl IntoView {
    let (sending, set_sending) = signal(false);
    let subject = format!("/api/tests/{}/subject", test.id);
    let action = format!("/api/tests/{}/submit", test.id);
    let open = test.open;
    let graded = test.mine.as_ref().is_some_and(|m| m.verdict.is_some());

    let mine = test.mine.clone().map(|m| {
        let verdict = match m.verdict.as_deref() {
            Some("Admitted") => "✅ Admis",
            Some("Rejected") => "❌ Non retenu",
            _ => "⏳ En attente de correction",
        };
        view! {
            <p class="gc-test__mine">
                "Ton rendu : " <strong>{m.filename}</strong> " · " {m.size} " · " {m.when}
                " — " {verdict}
            </p>
        }
    });

    view! {
        <li class=if open { "gc-test gc-test--open" } else { "gc-test" }>
            <div class="gc-test__head">
                <h3>{test.title}</h3>
                <span class="gc-chip">
                    {if open { test.time_left.clone() } else { "Terminé".to_string() }}
                </span>
            </div>
            {test.description.map(|d| view! { <p class="gc-test__desc">{d}</p> })}
            <p class="gc-test__meta">"Fin : " {test.closes}</p>
            {open
                .then(|| {
                    view! {
                        <a class="gc-btn" href=subject rel="external">
                            "📄 Télécharger le sujet (" {test.subject_size} ")"
                        </a>
                    }
                })}
            {mine}
            {(open && !graded)
                .then(|| {
                    view! {
                        <form
                            class="gc-form gc-test__form"
                            method="post"
                            action=action
                            enctype="multipart/form-data"
                            on:submit=move |_| set_sending.set(true)
                        >
                            <label class="gc-field">
                                <span>"Ton travail — zip, pdf… (500 Mo maximum)"</span>
                                <input type="file" name="file" required=true />
                            </label>
                            <label class="gc-field">
                                <span>"Un mot pour le Bureau (optionnel)"</span>
                                <textarea name="comment" rows="2" maxlength="1000"></textarea>
                            </label>
                            <button
                                class="gc-btn gc-btn--primary"
                                type="submit"
                                disabled=move || sending.get()
                            >
                                {move || {
                                    if sending.get() { "Envoi en cours…" } else { "Rendre mon travail" }
                                }}
                            </button>
                        </form>
                    }
                })}
        </li>
    }
}

// ---------------------------------------------------------------------------
// Bureau
// ---------------------------------------------------------------------------

/// What the Bureau sees.
#[component]
#[allow(clippy::needless_pass_by_value)] // Leptos prop convention
fn BureauView(
    /// The page.
    view: TestsView,
    /// Refresh after closing or deleting.
    on_changed: Callback<()>,
) -> impl IntoView {
    let list = if view.tests.is_empty() {
        view! { <p class="gc-empty">"Aucun test pour l'instant."</p> }.into_any()
    } else {
        view! {
            <ul class="gc-tests__list">
                {view
                    .tests
                    .into_iter()
                    .map(|test| view! { <BureauTest test on_changed /> })
                    .collect_view()}
            </ul>
        }
            .into_any()
    };

    view! {
        <OpenTestForm />
        <h2 class="gc-tests__heading">"Tests"</h2>
        {list}
    }
}

/// Opening a session.
#[component]
fn OpenTestForm() -> impl IntoView {
    let (sending, set_sending) = signal(false);
    view! {
        <form
            class="gc-form gc-tests__form"
            method="post"
            action="/api/tests"
            enctype="multipart/form-data"
            on:submit=move |_| set_sending.set(true)
        >
            <h2>"Ouvrir un test"</h2>
            <label class="gc-field">
                <span>"Titre"</span>
                <input
                    type="text"
                    name="title"
                    required=true
                    maxlength="120"
                    placeholder="Test d'entrée — Engineering"
                />
            </label>
            <label class="gc-field">
                <span>"Consignes (optionnel)"</span>
                <textarea
                    name="description"
                    rows="3"
                    maxlength="2000"
                    placeholder="Ce qu'il faut rendre, sous quelle forme…"
                ></textarea>
            </label>
            <label class="gc-field">
                <span>"Durée, en heures (les rendus ferment ensuite)"</span>
                <input type="number" name="hours" min="1" max="720" value="72" required=true />
            </label>
            <label class="gc-field">
                <span>"Sujet (PDF)"</span>
                <input type="file" name="subject" accept=".pdf,application/pdf" required=true />
            </label>
            <button class="gc-btn gc-btn--primary" type="submit" disabled=move || sending.get()>
                {move || if sending.get() { "Ouverture…" } else { "Ouvrir le test" }}
            </button>
        </form>
    }
}

/// One session, for the Bureau.
#[component]
#[allow(clippy::needless_pass_by_value)] // Leptos prop convention
fn BureauTest(
    /// The session.
    test: TestItem,
    /// Refresh after closing or deleting.
    on_changed: Callback<()>,
) -> impl IntoView {
    let (show, set_show) = signal(false);
    let (confirming, set_confirming) = signal(false);
    let (error, set_error) = signal(Option::<String>::None);
    let id = StoredValue::new(test.id.clone());

    let close = Action::new(move |id: &String| {
        let id = id.clone();
        async move { close_test(id).await }
    });
    let remove = Action::new(move |id: &String| {
        let id = id.clone();
        async move { delete_test(id).await }
    });
    Effect::new(move |_| {
        if let Some(result) = close.value().get() {
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

    let subject = format!("/api/tests/{}/subject", test.id);
    let open = test.open;
    let count = match test.submissions {
        0 => "aucun rendu".to_string(),
        1 => "1 rendu".to_string(),
        n => format!("{n} rendus"),
    };

    view! {
        <li class=if open { "gc-test gc-test--open" } else { "gc-test" }>
            <div class="gc-test__head">
                <h3>{test.title}</h3>
                <span class="gc-chip">
                    {if open { test.time_left.clone() } else { "Terminé".to_string() }}
                </span>
                <span class="gc-chip">{count}</span>
            </div>
            {test.description.map(|d| view! { <p class="gc-test__desc">{d}</p> })}
            <p class="gc-test__meta">"Fin : " {test.closes}</p>

            <div class="gc-test__actions">
                <a class="gc-btn" href=subject rel="external">
                    "📄 Sujet (" {test.subject_size} ")"
                </a>
                <button class="gc-btn" on:click=move |_| set_show.update(|s| *s = !*s)>
                    {move || if show.get() { "Masquer les rendus" } else { "Voir les rendus" }}
                </button>
                <Show when=move || open>
                    <button
                        class="gc-btn gc-btn--ghost"
                        disabled=move || close.pending().get()
                        on:click=move |_| {
                            close.dispatch(id.get_value());
                        }
                    >
                        "Clore maintenant"
                    </button>
                </Show>
                <Show
                    when=move || confirming.get()
                    fallback=move || {
                        view! {
                            <button
                                class="gc-btn gc-btn--ghost"
                                on:click=move |_| set_confirming.set(true)
                            >
                                "Supprimer"
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
                        "Confirmer : supprimer le sujet et tous les rendus"
                    </button>
                </Show>
            </div>
            {move || error.get().map(|e| view! { <p class="gc-test__error">{e}</p> })}

            <Show when=move || show.get()>
                <Submissions test_id=id.get_value() />
            </Show>
        </li>
    }
}

/// The work handed in for one session.
#[component]
fn Submissions(
    /// Session.
    test_id: String,
) -> impl IntoView {
    let rows = Resource::new(
        move || test_id.clone(),
        |id| async move { get_test_submissions(id).await },
    );
    view! {
        <Suspense fallback=|| view! { <p class="gc-empty">"Chargement des rendus…"</p> }>
            {move || {
                rows.get()
                    .map(|result| match result {
                        Err(e) => view! { <p class="gc-test__error">{e.to_string()}</p> }.into_any(),
                        Ok(list) if list.is_empty() => {
                            view! { <p class="gc-empty">"Aucun rendu pour l'instant."</p> }.into_any()
                        }
                        Ok(list) => {
                            view! {
                                <ul class="gc-subs">
                                    {list
                                        .into_iter()
                                        .map(|sub| view! { <SubmissionRow sub /> })
                                        .collect_view()}
                                </ul>
                            }
                                .into_any()
                        }
                    })
            }}
        </Suspense>
    }
}

/// One candidate's work, with the verdict buttons.
#[component]
#[allow(clippy::needless_pass_by_value)] // Leptos prop convention
fn SubmissionRow(
    /// The work.
    sub: SubmissionItem,
) -> impl IntoView {
    let (verdict, set_verdict) = signal(sub.verdict.clone());
    let (chosen, set_chosen) = signal(String::new());
    let (notice, set_notice) = signal(Option::<Result<String, String>>::None);
    let id = StoredValue::new(sub.id.clone());

    let judge = Action::new(move |(id, verdict): &(String, String)| {
        let (id, verdict) = (id.clone(), verdict.clone());
        async move { judge_submission(id, verdict).await }
    });
    Effect::new(move |_| {
        if let Some(result) = judge.value().get() {
            match result {
                Ok(message) => {
                    set_verdict.set(Some(chosen.get_untracked()));
                    set_notice.set(Some(Ok(message)));
                }
                Err(e) => set_notice.set(Some(Err(e.to_string()))),
            }
        }
    });
    let decide = move |what: &'static str| {
        set_chosen.set(what.to_string());
        judge.dispatch((id.get_value(), what.to_string()));
    };

    let href = format!("/api/tests/submissions/{}/download", sub.id);

    view! {
        <li class="gc-sub">
            <div class="gc-sub__body">
                <strong>{sub.candidate}</strong>
                {sub.handle.map(|h| format!(" · @{h}"))}
                <span class="gc-sub__meta">
                    {sub.filename} " · " {sub.size} " · " {sub.when}
                </span>
                {sub.comment.map(|c| view! { <p class="gc-sub__comment">{c}</p> })}
                {move || {
                    notice
                        .get()
                        .map(|n| match n {
                            Ok(m) => view! { <p class="gc-sub__ok">{m}</p> }.into_any(),
                            Err(m) => view! { <p class="gc-test__error">{m}</p> }.into_any(),
                        })
                }}
            </div>
            <div class="gc-sub__actions">
                <a class="gc-btn" href=href rel="external">"⬇ Télécharger"</a>
                {move || match verdict.get().as_deref() {
                    Some("Admitted") => view! { <span class="gc-chip gc-chip--ok">"✅ Admis"</span> }.into_any(),
                    Some(_) => view! { <span class="gc-chip">"❌ Refusé"</span> }.into_any(),
                    None => {
                        view! {
                            <button
                                class="gc-btn gc-btn--primary"
                                disabled=move || judge.pending().get()
                                on:click=move |_| decide("Admitted")
                            >
                                "Admettre"
                            </button>
                            <button
                                class="gc-btn gc-btn--ghost"
                                disabled=move || judge.pending().get()
                                on:click=move |_| decide("Rejected")
                            >
                                "Refuser"
                            </button>
                        }
                            .into_any()
                    }
                }}
            </div>
        </li>
    }
}

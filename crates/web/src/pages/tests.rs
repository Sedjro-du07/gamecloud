//! Entrance tests — a detail page.
//!
//! The Bureau opens a session with a PDF subject and a duration. Until it
//! closes, candidates — anybody signed in who is not a verified member yet —
//! download the subject and hand in their work. The Bureau downloads the
//! work here and admits or turns down each candidate; admitting somebody
//! who is not on the Discord server creates their invitation, shown to them
//! on this same page.
//!
//! Members never see this page unless they hold a Bureau office. Uploads are
//! plain multipart forms posting to `/api/tests`, which redirect back here
//! with `?ok` or `?erreur=…`.

use std::fmt::Write as _;

use leptos::prelude::*;
use leptos_router::hooks::use_query_map;

use crate::{
    api::{SubmissionItem, TestItem, TestsView},
    components::ui::{
        Button, ButtonKind, ButtonLink, Cluster, DenseList, EmptyState, ErrorState, ErrorText,
        Field, Form, FormActions, IconName, ListRow, Notice, NoticeKind, Page, PageHeader, Panel,
        Pattern, RowText, RowsSkeleton, Section, SignInState, Step, Steps, Tag,
    },
    server_fns::{close_test, delete_test, get_test_submissions, get_tests, judge_submission},
};

/// A session's time left, or that it is over.
fn time_tag(test: &TestItem) -> AnyView {
    if test.open {
        let left = test.time_left.clone();
        view! { <Tag icon=IconName::Clock>{left}</Tag> }.into_any()
    } else {
        view! { <Tag icon=IconName::Prohibit>"Terminé"</Tag> }.into_any()
    }
}

/// A candidate's verdict as a tag.
fn verdict_tag(verdict: Option<&str>) -> AnyView {
    match verdict {
        Some("Admitted") => view! { <Tag icon=IconName::CheckCircle>"Admis"</Tag> }.into_any(),
        Some("Rejected") => view! { <Tag icon=IconName::XCircle>"Non retenu"</Tag> }.into_any(),
        _ => view! { <Tag icon=IconName::Clock>"En attente de correction"</Tag> }.into_any(),
    }
}

/// Entrance tests page.
#[component]
pub fn TestsPage() -> impl IntoView {
    let tests = Resource::new(|| (), |()| async { get_tests().await });
    let on_changed = Callback::new(move |()| tests.refetch());
    let query = use_query_map();

    view! {
        <Page pattern=Pattern::Detail>
            <PageHeader
                title="Tests d'entrée"
                lead="Le Bureau ouvre un test avec un sujet et une durée ; les candidats rendent leur travail avant la fin et reçoivent la réponse ici."
            />
            {move || query.read().get("ok").map(|what| {
                let text = if what == "session" {
                    "Test ouvert : les candidats peuvent télécharger le sujet."
                } else {
                    "Rendu reçu. Tu peux le remplacer jusqu'à la fin du test."
                };
                view! { <Notice kind=NoticeKind::Success>{text}</Notice> }
            })}
            {move || query.read().get("erreur").map(|e| view! { <Notice kind=NoticeKind::Error>{e}</Notice> })}
            <Suspense fallback=|| view! { <RowsSkeleton rows=3 /> }>
                {move || tests.get().map(|result| match result {
                    Err(_) => view! {
                        <ErrorState message="Impossible de charger les tests." on_retry=Callback::new(move |()| tests.refetch()) />
                    }
                    .into_any(),
                    Ok(view) => match view.access.as_str() {
                        "bureau" => view! { <BureauView view on_changed /> }.into_any(),
                        "candidate" => view! { <CandidateView view /> }.into_any(),
                        "signin" => view! { <VisitorView view /> }.into_any(),
                        _ => view! {
                            <EmptyState icon=IconName::LockSimple title="Réservé au Bureau et aux candidats"
                                body="Une fois membre, les tests d'entrée ne te concernent plus." />
                        }
                        .into_any(),
                    },
                })}
            </Suspense>
        </Page>
    }
}

/// The open sessions, as a visitor or candidate reads them.
fn open_tests_list(tests: Vec<TestItem>) -> AnyView {
    if tests.is_empty() {
        return view! {
            <EmptyState icon=IconName::Hourglass title="Aucun test ouvert pour l'instant" body="Reviens bientôt." />
        }
        .into_any();
    }
    view! {
        <DenseList label="Tests ouverts">
            {tests
                .into_iter()
                .map(|test| {
                    let end = time_tag(&test);
                    view! {
                        <ListRow title=test.title meta=format!("Fin : {}", test.closes) end>
                            {test.description.map(|text| view! { <RowText text /> })}
                        </ListRow>
                    }
                })
                .collect_view()}
        </DenseList>
    }
    .into_any()
}

/// Somebody signed out: how joining works, and the sessions open now.
#[component]
#[allow(clippy::needless_pass_by_value)] // Leptos prop convention
fn VisitorView(
    /// The page.
    view: TestsView,
) -> impl IntoView {
    view! {
        <Section title="Rejoindre l'association">
            <Steps>
                <Step icon=IconName::DiscordLogo title="Déjà sur le serveur ?">
                    "Connecte-toi avec Discord : l'inscription est directe."
                </Step>
                <Step icon=IconName::GraduationCap title="Sinon, passe un test">
                    "Connecte-toi avec Discord, télécharge le sujet d'un test ouvert et rends ton travail avant la fin."
                </Step>
                <Step icon=IconName::CheckCircle title="Le Bureau corrige">
                    "Une fois admis, ton invitation au serveur s'affiche ici et tu finis ton inscription."
                </Step>
            </Steps>
            <SignInState what="passer le test d'entrée" />
        </Section>
        <Section title="Tests ouverts">{open_tests_list(view.tests)}</Section>
    }
}

/// A candidate: where they stand, and the sessions.
#[component]
#[allow(clippy::needless_pass_by_value)] // Leptos prop convention
fn CandidateView(
    /// The page.
    view: TestsView,
) -> impl IntoView {
    let status = if let Some(url) = view.invite_url.clone() {
        Some(view! {
            <Notice kind=NoticeKind::Success>
                <span>"Tu es admis. Rejoins le serveur Discord (lien valable une semaine), puis vérifie ton adresse Epitech."</span>
                <ButtonLink kind=ButtonKind::Primary href=url new_tab=true icon=IconName::DiscordLogo>"Rejoindre le serveur"</ButtonLink>
                <ButtonLink href="/onboarding/email" icon=IconName::Envelope>"Vérifier mon adresse"</ButtonLink>
            </Notice>
        }.into_any())
    } else if view.admitted {
        Some(view! {
            <Notice kind=NoticeKind::Success>
                <span>"Tu es admis. Vérifie ton adresse Epitech pour finir ton inscription."</span>
                <ButtonLink kind=ButtonKind::Primary href="/onboarding/email" icon=IconName::Envelope>"Vérifier mon adresse"</ButtonLink>
            </Notice>
        }.into_any())
    } else if view.is_candidate {
        Some(view! {
            <Notice>
                "Tu n'es pas encore sur le serveur Discord de l'association : l'inscription s'ouvre quand tu as réussi un test. Une fois admis, ton invitation apparaît ici."
            </Notice>
        }.into_any())
    } else {
        None
    };
    let list = if view.tests.is_empty() {
        open_tests_list(Vec::new())
    } else {
        view! {
            <DenseList label="Tests">
                {view.tests.into_iter().map(|test| view! { <CandidateTest test /> }).collect_view()}
            </DenseList>
        }
        .into_any()
    };
    view! {
        {status}
        <Section title="Tests">{list}</Section>
    }
}

/// One session, for a candidate: the subject, their work, handing in.
#[component]
#[allow(clippy::needless_pass_by_value)] // Leptos prop convention
fn CandidateTest(
    /// The session.
    test: TestItem,
) -> impl IntoView {
    let (sending, set_sending) = signal(false);
    let open = test.open;
    let graded = test.mine.as_ref().is_some_and(|m| m.verdict.is_some());
    let end = time_tag(&test);
    let subject = format!("/api/tests/{}/subject", test.id);
    let subject_label = format!("Télécharger le sujet ({})", test.subject_size);
    let submit_to = format!("/api/tests/{}/submit", test.id);
    let (file_id, note_id) = (format!("rendu-{}", test.id), format!("mot-{}", test.id));
    let mine = test.mine.clone().map(|m| view! {
        <Cluster>
            {verdict_tag(m.verdict.as_deref())}
            <span class="ui-meta">{format!("Ton rendu : {} · {} · {}", m.filename, m.size, m.when)}</span>
        </Cluster>
    });
    view! {
        <ListRow title=test.title meta=format!("Fin : {}", test.closes) end>
            {test.description.map(|text| view! { <RowText text /> })}
            {open.then(|| view! {
                <Cluster>
                    <ButtonLink href=subject external=true icon=IconName::FileText>{subject_label}</ButtonLink>
                </Cluster>
            })}
            {mine}
            {(open && !graded).then(|| view! {
                <Panel>
                    <Form attr:method="post" attr:action=submit_to
                        attr:enctype="multipart/form-data" on:submit=move |_| set_sending.set(true)>
                        <Field id=file_id.clone() label="Ton travail : zip, pdf… (500 Mo maximum)" wide=true>
                            <input id=file_id class="ui-control" type="file" name="file" required=true />
                        </Field>
                        <Field id=note_id.clone() label="Un mot pour le Bureau (optionnel)" wide=true>
                            <textarea id=note_id class="ui-control" name="comment" rows="2" maxlength="1000"></textarea>
                        </Field>
                        <FormActions>
                            <Button kind=ButtonKind::Primary button_type="submit" icon=IconName::UploadSimple disabled=Signal::derive(move || sending.get())>
                                {move || if sending.get() { "Envoi en cours…" } else { "Rendre mon travail" }}
                            </Button>
                        </FormActions>
                    </Form>
                </Panel>
            })}
        </ListRow>
    }
}

/// The Bureau: open a session, follow each one.
#[component]
#[allow(clippy::needless_pass_by_value)] // Leptos prop convention
fn BureauView(
    /// The page.
    view: TestsView,
    /// Refresh after closing or deleting.
    on_changed: Callback<()>,
) -> impl IntoView {
    let (opening, set_opening) = signal(false);
    let action = view! {
        <Button kind=ButtonKind::Primary icon=IconName::Plus on:click=move |_| set_opening.update(|o| *o = !*o)>
            "Ouvrir un test"
        </Button>
    }
    .into_any();
    let list = if view.tests.is_empty() {
        view! { <EmptyState icon=IconName::GraduationCap title="Aucun test pour l'instant" body="Ouvre le premier avec un sujet en PDF." /> }.into_any()
    } else {
        view! {
            <DenseList label="Tests">
                {view.tests.into_iter().map(|test| view! { <BureauTest test on_changed /> }).collect_view()}
            </DenseList>
        }
        .into_any()
    };
    view! {
        <Section title="Tests" action>
            <Show when=move || opening.get()>
                <Panel>
                    <h3 class="ui-h3">"Ouvrir un test"</h3>
                    <OpenTestForm />
                </Panel>
            </Show>
            {list}
        </Section>
    }
}

/// Opening a session.
#[component]
fn OpenTestForm() -> impl IntoView {
    let (sending, set_sending) = signal(false);
    view! {
        <Form attr:method="post" attr:action="/api/tests" attr:enctype="multipart/form-data" on:submit=move |_| set_sending.set(true)>
            <Field id="test-titre" label="Titre" wide=true>
                <input id="test-titre" class="ui-control" type="text" name="title" required=true maxlength="120" placeholder="Test d'entrée — Engineering" />
            </Field>
            <Field id="test-consignes" label="Consignes (optionnel)" wide=true>
                <textarea id="test-consignes" class="ui-control" name="description" rows="3" maxlength="2000"
                    placeholder="Ce qu'il faut rendre, sous quelle forme…"></textarea>
            </Field>
            <Field id="test-duree" label="Durée, en heures" hint="Les rendus ferment ensuite.">
                <input id="test-duree" class="ui-control" type="number" name="hours" min="1" max="720" value="72" required=true aria-describedby="test-duree-hint" />
            </Field>
            <Field id="test-sujet" label="Sujet (PDF)">
                <input id="test-sujet" class="ui-control" type="file" name="subject" accept=".pdf,application/pdf" required=true />
            </Field>
            <FormActions>
                <Button kind=ButtonKind::Primary button_type="submit" icon=IconName::GraduationCap disabled=Signal::derive(move || sending.get())>
                    {move || if sending.get() { "Ouverture…" } else { "Ouvrir le test" }}
                </Button>
            </FormActions>
        </Form>
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
    for action in [close, remove] {
        Effect::new(move |_| {
            if let Some(result) = action.value().get() {
                match result {
                    Ok(()) => on_changed.run(()),
                    Err(e) => set_error.set(Some(e.to_string())),
                }
            }
        });
    }
    let open = test.open;
    let count = match test.submissions {
        0 => "aucun rendu".to_string(),
        1 => "1 rendu".to_string(),
        n => format!("{n} rendus"),
    };
    let end = time_tag(&test);
    view! {
        <ListRow title=test.title meta=format!("Fin : {} · {count}", test.closes) end>
            {test.description.map(|text| view! { <RowText text /> })}
            <Cluster>
                <ButtonLink href=format!("/api/tests/{}/subject", test.id) external=true icon=IconName::FileText>
                    {format!("Sujet ({})", test.subject_size)}
                </ButtonLink>
                <Button icon=IconName::Eye attr:aria-expanded=move || show.get().to_string() on:click=move |_| set_show.update(|s| *s = !*s)>
                    {move || if show.get() { "Masquer les rendus" } else { "Voir les rendus" }}
                </Button>
                {open.then(|| view! {
                    <Button kind=ButtonKind::Ghost icon=IconName::Prohibit disabled=Signal::derive(move || close.pending().get())
                        on:click=move |_| { close.dispatch(id.get_value()); }>
                        "Clore maintenant"
                    </Button>
                })}
                <Show
                    when=move || confirming.get()
                    fallback=move || view! {
                        <Button kind=ButtonKind::Ghost icon=IconName::Trash on:click=move |_| set_confirming.set(true)>"Supprimer"</Button>
                    }
                >
                    <Button kind=ButtonKind::Ghost icon=IconName::Trash disabled=Signal::derive(move || remove.pending().get())
                        on:click=move |_| { remove.dispatch(id.get_value()); }>
                        "Confirmer : supprimer le sujet et tous les rendus"
                    </Button>
                </Show>
            </Cluster>
            {move || error.get().map(|message| view! { <ErrorText message /> })}
            <Show when=move || show.get()>
                <Panel><Submissions test_id=id.get_value() /></Panel>
            </Show>
        </ListRow>
    }
}

/// The work handed in for one session.
#[component]
fn Submissions(
    /// Session.
    test_id: String,
) -> impl IntoView {
    let rows = Resource::new(move || test_id.clone(), |id| async move { get_test_submissions(id).await });
    view! {
        <Suspense fallback=|| view! { <RowsSkeleton rows=2 /> }>
            {move || rows.get().map(|result| match result {
                Err(_) => view! {
                    <ErrorState message="Impossible de charger les rendus." on_retry=Callback::new(move |()| rows.refetch()) />
                }
                .into_any(),
                Ok(list) if list.is_empty() => view! { <EmptyState icon=IconName::Clock title="Aucun rendu pour l'instant" /> }.into_any(),
                Ok(list) => view! {
                    <DenseList label="Rendus">
                        {list.into_iter().map(|sub| view! { <SubmissionRow sub /> }).collect_view()}
                    </DenseList>
                }
                .into_any(),
            })}
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
    let busy = Signal::derive(move || judge.pending().get());
    let mut meta = String::new();
    let _ = write!(meta, "{} · {} · {}", sub.filename, sub.size, sub.when);
    let end = view! {
        <ButtonLink href=format!("/api/tests/submissions/{}/download", sub.id) external=true icon=IconName::DownloadSimple>
            "Télécharger"
        </ButtonLink>
        {move || match verdict.get().as_deref() {
            Some(v) => verdict_tag(Some(v)),
            None => view! {
                <Button kind=ButtonKind::Primary icon=IconName::CheckCircle disabled=busy on:click=move |_| decide("Admitted")>"Admettre"</Button>
                <Button kind=ButtonKind::Ghost icon=IconName::XCircle disabled=busy on:click=move |_| decide("Rejected")>"Refuser"</Button>
            }
            .into_any(),
        }}
    }
    .into_any();
    view! {
        <ListRow title=sub.candidate meta end>
            {sub.comment.map(|text| view! { <RowText text /> })}
            {move || notice.get().map(|n| match n {
                Ok(m) => view! { <p class="ui-meta">{m}</p> }.into_any(),
                Err(message) => view! { <ErrorText message /> }.into_any(),
            })}
        </ListRow>
    }
}

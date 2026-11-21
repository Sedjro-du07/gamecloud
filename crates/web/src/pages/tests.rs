//! Entrance tests — detail pages.
//!
//! The Bureau opens a session with a PDF subject and a duration. Anybody
//! who is not on the Discord server pre-registers for an open session on
//! `/tests/:id/preinscription` — name, e-mail, GitHub, Discord username,
//! no account — and follows their application on `/candidature/:token`,
//! a private link that is their only key. Two requests go to the Bureau:
//! access to the subject, then the verdict on the work. Admission creates
//! an invitation to the server, and the applicant's page sends them there.
//!
//! Members never see the tests page unless they hold a Bureau office.
//! Forms are plain HTML posts to `/api/tests/…`, which redirect back with
//! `?ok` or `?erreur=…`.

use std::fmt::Write as _;

use leptos::prelude::*;
use leptos_router::hooks::{use_params_map, use_query_map};

use crate::{
    api::{ApplicationItem, ApplicationView, SubmissionItem, TestItem, TestsView},
    components::ui::{
        Button, ButtonKind, ButtonLink, Cluster, DenseList, EmptyState, ErrorState, ErrorText,
        Field, Form, FormActions, IconName, Journey, ListRow, Notice, NoticeKind, Page, PageHeader, Panel,
        Pattern, Quote, RowText, RowsSkeleton, Section, Step, StepState, Steps, Tag,
    },
    server_fns::{
        close_test, decide_application_access, delete_test, get_application, get_open_test,
        get_test_applications, get_test_submissions, get_tests, judge_application,
        judge_submission, reissue_application_link,
    },
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

/// A verdict on work handed in through the former Discord sign-in.
fn verdict_tag(verdict: Option<&str>) -> AnyView {
    match verdict {
        Some("Admitted") => view! { <Tag icon=IconName::CheckCircle>"Admis"</Tag> }.into_any(),
        Some("Rejected") => view! { <Tag icon=IconName::XCircle>"Non retenu"</Tag> }.into_any(),
        _ => view! { <Tag icon=IconName::Clock>"En attente de correction"</Tag> }.into_any(),
    }
}

/// Where a pre-registration stands, as a tag.
fn status_tag(status: &str) -> AnyView {
    let (icon, label) = match status {
        "Requested" => (IconName::Hourglass, "Demande d'accès au sujet"),
        "Granted" => (IconName::Key, "Sujet autorisé"),
        "Refused" => (IconName::Prohibit, "Préinscription refusée"),
        "HandedIn" => (IconName::Clock, "Rendu à corriger"),
        "Admitted" => (IconName::CheckCircle, "Admis"),
        _ => (IconName::XCircle, "Non retenu"),
    };
    view! { <Tag icon>{label}</Tag> }.into_any()
}

/// Notices for `?ok=…` and `?erreur=…`.
fn query_notices(ok_text: fn(&str) -> &'static str) -> impl IntoView {
    let query = use_query_map();
    view! {
        {move || query.read().get("ok").map(|what| {
            view! { <Notice kind=NoticeKind::Success>{ok_text(&what)}</Notice> }
        })}
        {move || query.read().get("erreur").map(|e| view! { <Notice kind=NoticeKind::Error>{e}</Notice> })}
    }
}

/// Entrance tests page.
#[component]
pub fn TestsPage() -> impl IntoView {
    let tests = Resource::new(|| (), |()| async { get_tests().await });
    let on_changed = Callback::new(move |()| tests.refetch());

    view! {
        <Page pattern=Pattern::Detail>
            <PageHeader
                title="Tests d'entrée"
                lead="Pas encore sur le serveur Discord de l'association ? Préinscris-toi à un test : le Bureau t'ouvre le sujet, corrige ton travail, et t'invite sur le serveur si tu es admis."
            />
            {query_notices(|_| "Test ouvert : les candidats peuvent s'y préinscrire.")}
            <Suspense fallback=|| view! { <RowsSkeleton rows=3 /> }>
                {move || tests.get().map(|result| match result {
                    Err(_) => view! {
                        <ErrorState message="Impossible de charger les tests." on_retry=Callback::new(move |()| tests.refetch()) />
                    }
                    .into_any(),
                    Ok(view) => match view.access.as_str() {
                        "bureau" => view! { <BureauView view on_changed /> }.into_any(),
                        "candidate" | "signin" => view! { <VisitorView view /> }.into_any(),
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

/// The open sessions, each with its pre-registration button.
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
                    let apply = format!("/tests/{}/preinscription", test.id);
                    view! {
                        <ListRow title=test.title meta=format!("Fin : {}", test.closes) end>
                            {test.description.map(|text| view! { <RowText text /> })}
                            <Cluster>
                                <ButtonLink kind=ButtonKind::Primary href=apply icon=IconName::PencilSimple>"Me préinscrire"</ButtonLink>
                            </Cluster>
                        </ListRow>
                    }
                })
                .collect_view()}
        </DenseList>
    }
    .into_any()
}

/// Everybody who is not a member: how joining works, and the open sessions.
#[component]
#[allow(clippy::needless_pass_by_value)] // Leptos prop convention
fn VisitorView(
    /// The page.
    view: TestsView,
) -> impl IntoView {
    // Somebody admitted through the former Discord sign-in path.
    let legacy = view.invite_url.clone().map(|url| view! {
        <Notice kind=NoticeKind::Success>
            <span>"Tu es admis. Rejoins le serveur Discord (lien valable une semaine), puis connecte-toi ici avec Discord."</span>
            <ButtonLink kind=ButtonKind::Primary href=url new_tab=true icon=IconName::DiscordLogo>"Rejoindre le serveur"</ButtonLink>
        </Notice>
    });
    let admitted = (view.admitted && view.invite_url.is_none()).then(|| view! {
        <Notice kind=NoticeKind::Success>"Tu es admis : la plateforme t'est ouverte."</Notice>
    });
    view! {
        {legacy}
        {admitted}
        <Section title="Rejoindre l'association">
            <Steps>
                <Step icon=IconName::DiscordLogo title="Déjà sur le serveur ?">
                    "Connecte-toi avec Discord : l'inscription est directe, sans test."
                </Step>
                <Step icon=IconName::PencilSimple title="Sinon, préinscris-toi">
                    "Choisis un test ouvert ci-dessous et donne ton nom, ton e-mail, ton GitHub et ton nom d'utilisateur Discord. Aucun compte n'est nécessaire."
                </Step>
                <Step icon=IconName::Key title="Le Bureau t'ouvre le sujet">
                    "Ta page de suivi te montre sa réponse ; une fois autorisé, tu y télécharges le sujet et tu y rends ton travail avant la fin."
                </Step>
                <Step icon=IconName::CheckCircle title="Le Bureau corrige">
                    "Si tu es admis, ta page t'envoie directement sur le serveur Discord de l'association."
                </Step>
            </Steps>
            <Cluster>
                <ButtonLink href="/api/auth/login" external=true icon=IconName::DiscordLogo>"Je suis déjà sur le serveur"</ButtonLink>
            </Cluster>
        </Section>
        <Section title="Tests ouverts">{open_tests_list(view.tests)}</Section>
        <Quote salt=4 />
    }
}

// ---------------------------------------------------------------------------
// Pre-registration
// ---------------------------------------------------------------------------

/// `/tests/:id/preinscription`: the pre-registration form. Public.
#[component]
pub fn PreRegistrationPage() -> impl IntoView {
    let params = use_params_map();
    let test = Resource::new(
        move || params.read().get("id").unwrap_or_default(),
        |id| async move { get_open_test(id).await },
    );
    view! {
        <Page pattern=Pattern::Detail>
            <PageHeader
                title="Préinscription à un test d'entrée"
                lead="Aucun compte n'est nécessaire. Le Bureau reçoit ta demande et t'ouvre le sujet sur ta page de suivi."
            />
            {query_notices(|_| "")}
            <Suspense fallback=|| view! { <RowsSkeleton rows=3 /> }>
                {move || test.get().map(|result| match result {
                    Err(_) => view! {
                        <ErrorState message="Impossible de charger ce test." on_retry=Callback::new(move |()| test.refetch()) />
                    }
                    .into_any(),
                    Ok(None) => view! {
                        <EmptyState icon=IconName::Prohibit title="Ce test n'est pas ouvert"
                            body="Il est terminé ou n'existe pas. Les tests ouverts sont listés sur la page Tests d'entrée." />
                        <Cluster>
                            <ButtonLink href="/tests" icon=IconName::GraduationCap>"Voir les tests ouverts"</ButtonLink>
                        </Cluster>
                    }
                    .into_any(),
                    Ok(Some(test)) => view! { <PreRegistrationForm test /> }.into_any(),
                })}
            </Suspense>
        </Page>
    }
}

/// The form itself.
#[component]
#[allow(clippy::needless_pass_by_value)] // Leptos prop convention
fn PreRegistrationForm(
    /// The session.
    test: TestItem,
) -> impl IntoView {
    let (sending, set_sending) = signal(false);
    let action = format!("/api/tests/{}/apply", test.id);
    let end = time_tag(&test);
    view! {
        <Section title="Le test">
            <DenseList label="Test">
                <ListRow title=test.title.clone() meta=format!("Fin : {}", test.closes) end>
                    {test.description.clone().map(|text| view! { <RowText text /> })}
                </ListRow>
            </DenseList>
        </Section>
        <Section title="Tes informations">
            <Panel>
                <Form attr:method="post" attr:action=action on:submit=move |_| set_sending.set(true)>
                    <Field id="pre-nom" label="Nom et prénom(s)" wide=true>
                        <input id="pre-nom" class="ui-control" type="text" name="full_name" required=true maxlength="100" autocomplete="name" />
                    </Field>
                    <Field id="pre-mail" label="Adresse e-mail">
                        <input id="pre-mail" class="ui-control" type="email" name="email" required=true maxlength="254" autocomplete="email" />
                    </Field>
                    <Field id="pre-github" label="Nom d'utilisateur GitHub" hint="Ou le lien de ton profil GitHub.">
                        <input id="pre-github" class="ui-control" type="text" name="github" required=true maxlength="80"
                            placeholder="mon-pseudo" aria-describedby="pre-github-hint" />
                    </Field>
                    <Field id="pre-discord" label="Nom d'utilisateur Discord"
                        hint="Ton identifiant unique (Paramètres → Mon compte → Nom d'utilisateur), pas ton nom d'affichage.">
                        <input id="pre-discord" class="ui-control" type="text" name="discord" required=true maxlength="33"
                            placeholder="mon_pseudo" aria-describedby="pre-discord-hint" />
                    </Field>
                    <Field id="pre-motivation" label="Pourquoi veux-tu rejoindre l'association ? (optionnel)" wide=true>
                        <textarea id="pre-motivation" class="ui-control" name="motivation" rows="3" maxlength="1000"></textarea>
                    </Field>
                    <FormActions>
                        <Button kind=ButtonKind::Primary button_type="submit" icon=IconName::PaperPlaneRight disabled=Signal::derive(move || sending.get())>
                            {move || if sending.get() { "Envoi…" } else { "Envoyer ma préinscription" }}
                        </Button>
                    </FormActions>
                </Form>
            </Panel>
        </Section>
    }
}

// ---------------------------------------------------------------------------
// The applicant's private page
// ---------------------------------------------------------------------------

/// `/candidature/:token`: where an applicant follows their application.
#[component]
pub fn ApplicationPage() -> impl IntoView {
    let params = use_params_map();
    let token = move || params.read().get("token").unwrap_or_default();
    let application = Resource::new(token, |token| async move { get_application(token).await });
    view! {
        <Page pattern=Pattern::Detail>
            <PageHeader
                title="Ma candidature"
                lead="Ta page de suivi : la réponse du Bureau, le sujet et ton rendu sont ici."
            />
            {query_notices(|what| match what {
                "nouveau" => "Préinscription envoyée au Bureau. Garde le lien de cette page : c'est ta seule clé pour suivre ta candidature.",
                _ => "Rendu reçu. Tu peux le remplacer jusqu'à la fin du test.",
            })}
            <Suspense fallback=|| view! { <RowsSkeleton rows=3 /> }>
                {move || application.get().map(|result| match result {
                    Err(_) => view! {
                        <ErrorState message="Impossible de charger ta candidature." on_retry=Callback::new(move |()| application.refetch()) />
                    }
                    .into_any(),
                    Ok(None) => view! {
                        <EmptyState icon=IconName::LinkSimple title="Lien introuvable"
                            body="Ce lien n'existe pas ou a été remplacé. Demande au Bureau de t'en envoyer un nouveau." />
                    }
                    .into_any(),
                    Ok(Some(app)) => view! { <ApplicationDetail app token=token() /> }.into_any(),
                })}
            </Suspense>
        </Page>
    }
}

/// The four steps of an application, and where this one stands.
fn journey_for(status: &str, open: bool) -> Vec<(&'static str, StepState)> {
    use StepState::{Current, Done, Failed, Todo};
    let stage = match status {
        "Requested" => [Done, Current, Todo, Todo],
        "Refused" => [Done, Failed, Todo, Todo],
        "Granted" if !open => [Done, Done, Failed, Todo],
        "Granted" => [Done, Done, Current, Todo],
        "HandedIn" => [Done, Done, Done, Current],
        "Admitted" => [Done, Done, Done, Done],
        _ => [Done, Done, Done, Failed],
    };
    ["Préinscription", "Accès au sujet", "Rendu du travail", "Décision du Bureau"]
        .into_iter()
        .zip(stage)
        .collect()
}

/// An application, from the applicant's side.
#[component]
#[allow(clippy::needless_pass_by_value)] // Leptos prop convention
fn ApplicationDetail(
    /// The application.
    app: ApplicationView,
    /// Its token, for the links and the form.
    token: String,
) -> impl IntoView {
    let (sending, set_sending) = signal(false);
    let base = format!("/api/tests/candidature/{token}");
    let (discord_href, subject_href, submit_href) =
        (format!("{base}/discord"), format!("{base}/subject"), format!("{base}/submit"));
    let status = app.status.clone();
    let may_hand_in = app.open && matches!(status.as_str(), "Granted" | "HandedIn");
    let sees_subject = matches!(status.as_str(), "Granted" | "HandedIn" | "Admitted" | "Rejected");

    let state = match status.as_str() {
        "Requested" => view! {
            <Notice>"Ta demande d'accès au sujet est entre les mains du Bureau. Reviens sur cette page pour voir sa réponse."</Notice>
        }
        .into_any(),
        "Refused" => view! {
            <Notice kind=NoticeKind::Error>"Le Bureau n'a pas retenu ta préinscription pour ce test."</Notice>
        }
        .into_any(),
        "Granted" if !app.open => view! {
            <Notice kind=NoticeKind::Error>"Le test est terminé sans que tu aies rendu de travail."</Notice>
        }
        .into_any(),
        "Granted" => view! {
            <Notice kind=NoticeKind::Success>"Le Bureau t'a ouvert le sujet. Télécharge-le et rends ton travail avant la fin du test."</Notice>
        }
        .into_any(),
        "HandedIn" => view! {
            <Notice>"Ton travail est rendu : le Bureau va le corriger. Reviens sur cette page pour voir sa décision."</Notice>
        }
        .into_any(),
        "Admitted" => view! {
            <Notice kind=NoticeKind::Success>
                <span>"Bravo, tu es admis ! Rejoins le serveur Discord de l'association, puis connecte-toi sur la plateforme avec Discord pour finir ton inscription."</span>
                <ButtonLink kind=ButtonKind::Primary href=discord_href external=true icon=IconName::DiscordLogo>
                    "Rejoindre le serveur Discord"
                </ButtonLink>
            </Notice>
        }
        .into_any(),
        _ => view! {
            <Notice kind=NoticeKind::Error>"Le Bureau n'a pas retenu ta candidature cette fois-ci."</Notice>
        }
        .into_any(),
    };

    let work = app.work.clone().map(|w| view! {
        <p class="ui-meta">{format!("Ton rendu : {} · {} · {}", w.filename, w.size, w.when)}</p>
    });
    let journey = journey_for(&status, app.open);
    let end = status_tag(&status);
    view! {
        <Journey label="Où en est ma candidature" steps=journey />
        {state}
        <Section title="Le test">
            <DenseList label="Test">
                <ListRow title=app.test_title.clone() meta=format!("Fin : {} · {}", app.closes, app.time_left) end>
                    {app.test_description.clone().map(|text| view! { <RowText text /> })}
                    {sees_subject.then(|| view! {
                        <Cluster>
                            <ButtonLink href=subject_href external=true icon=IconName::FileText>
                                {format!("Télécharger le sujet ({})", app.subject_size)}
                            </ButtonLink>
                        </Cluster>
                    })}
                    {work}
                </ListRow>
            </DenseList>
        </Section>
        {may_hand_in.then(|| view! {
            <Section title="Rendre mon travail">
                <Panel>
                    <Form attr:method="post" attr:action=submit_href
                        attr:enctype="multipart/form-data" on:submit=move |_| set_sending.set(true)>
                        <Field id="rendu-fichier" label="Ton travail : pdf, zip… (500 Mo maximum)" wide=true>
                            <input id="rendu-fichier" class="ui-control" type="file" name="file" required=true />
                        </Field>
                        <Field id="rendu-mot" label="Un mot pour le Bureau (optionnel)" wide=true>
                            <textarea id="rendu-mot" class="ui-control" name="comment" rows="2" maxlength="1000"></textarea>
                        </Field>
                        <FormActions>
                            <Button kind=ButtonKind::Primary button_type="submit" icon=IconName::UploadSimple disabled=Signal::derive(move || sending.get())>
                                {move || if sending.get() { "Envoi en cours…" } else { "Rendre mon travail" }}
                            </Button>
                        </FormActions>
                    </Form>
                </Panel>
            </Section>
        })}
        <Quote salt=5 shuffle=true />
        <Section title="Mon lien de suivi" lead="Garde-le (favori, note, message à toi-même) : sans compte, c'est la seule façon de revenir ici.">
            <Field id="lien-suivi" label="Lien" wide=true>
                <input id="lien-suivi" class="ui-control" type="text" readonly=true value=app.link.clone() />
            </Field>
            <p class="ui-meta">{format!("Candidature de {} · Discord : {}", app.full_name, app.discord)}</p>
        </Section>
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
        view! { <EmptyState icon=IconName::GraduationCap title="Aucun test pour l'instant" body="Ouvre le premier avec un sujet en PDF : les candidats pourront s'y préinscrire." /> }.into_any()
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
    let (show, set_show) = signal(test.waiting > 0);
    let (show_legacy, set_show_legacy) = signal(false);
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
    let legacy = test.submissions > 0;
    let mut count = match test.applications {
        0 => "aucune préinscription".to_string(),
        1 => "1 préinscription".to_string(),
        n => format!("{n} préinscriptions"),
    };
    if test.waiting > 0 {
        let _ = write!(count, " dont {} en attente du Bureau", test.waiting);
    }
    let end = time_tag(&test);
    view! {
        <ListRow title=test.title meta=format!("Fin : {} · {count}", test.closes) end>
            {test.description.map(|text| view! { <RowText text /> })}
            <Cluster>
                <ButtonLink href=format!("/api/tests/{}/subject", test.id) external=true icon=IconName::FileText>
                    {format!("Sujet ({})", test.subject_size)}
                </ButtonLink>
                <Button icon=IconName::Eye attr:aria-expanded=move || show.get().to_string() on:click=move |_| set_show.update(|s| *s = !*s)>
                    {move || if show.get() { "Masquer les préinscriptions" } else { "Voir les préinscriptions" }}
                </Button>
                {legacy.then(|| view! {
                    <Button kind=ButtonKind::Ghost icon=IconName::DiscordLogo attr:aria-expanded=move || show_legacy.get().to_string()
                        on:click=move |_| set_show_legacy.update(|s| *s = !*s)>
                        {move || if show_legacy.get() { "Masquer les anciens rendus" } else { "Anciens rendus (connexion Discord)" }}
                    </Button>
                })}
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
                        "Confirmer : supprimer le sujet, les préinscriptions et tous les rendus"
                    </Button>
                </Show>
            </Cluster>
            {move || error.get().map(|message| view! { <ErrorText message /> })}
            <Show when=move || show.get()>
                <Panel><Applications test_id=id.get_value() on_changed /></Panel>
            </Show>
            <Show when=move || show_legacy.get()>
                <Panel><Submissions test_id=id.get_value() /></Panel>
            </Show>
        </ListRow>
    }
}

/// The pre-registrations for one session.
#[component]
fn Applications(
    /// Session.
    test_id: String,
    /// Refresh the session's counts after a decision.
    on_changed: Callback<()>,
) -> impl IntoView {
    let rows = Resource::new(move || test_id.clone(), |id| async move { get_test_applications(id).await });
    let refresh = Callback::new(move |()| {
        rows.refetch();
        on_changed.run(());
    });
    view! {
        <Suspense fallback=|| view! { <RowsSkeleton rows=2 /> }>
            {move || rows.get().map(|result| match result {
                Err(_) => view! {
                    <ErrorState message="Impossible de charger les préinscriptions." on_retry=Callback::new(move |()| rows.refetch()) />
                }
                .into_any(),
                Ok(list) if list.is_empty() => view! {
                    <EmptyState icon=IconName::Clock title="Aucune préinscription pour l'instant" />
                }
                .into_any(),
                Ok(list) => view! {
                    <DenseList label="Préinscriptions">
                        {list.into_iter().map(|app| view! { <ApplicationRow app on_changed=refresh /> }).collect_view()}
                    </DenseList>
                }
                .into_any(),
            })}
        </Suspense>
    }
}

/// One pre-registration, with the Bureau's two decisions.
#[component]
#[allow(clippy::needless_pass_by_value)] // Leptos prop convention
fn ApplicationRow(
    /// The application.
    app: ApplicationItem,
    /// Refresh after a decision.
    on_changed: Callback<()>,
) -> impl IntoView {
    let (notice, set_notice) = signal(Option::<Result<String, String>>::None);
    let (link, set_link) = signal(Option::<String>::None);
    let id = StoredValue::new(app.id.clone());

    let access = Action::new(move |grant: &bool| {
        let grant = *grant;
        async move { decide_application_access(id.get_value(), grant).await.map(|()| String::new()) }
    });
    let judge = Action::new(move |verdict: &&'static str| {
        let verdict = (*verdict).to_string();
        async move { judge_application(id.get_value(), verdict).await }
    });
    let report = move |result: Result<String, ServerFnError>| match result {
        Ok(message) => {
            if !message.is_empty() {
                set_notice.set(Some(Ok(message)));
            }
            on_changed.run(());
        }
        Err(e) => set_notice.set(Some(Err(e.to_string()))),
    };
    Effect::new(move |_| {
        if let Some(result) = access.value().get() {
            report(result);
        }
    });
    Effect::new(move |_| {
        if let Some(result) = judge.value().get() {
            report(result);
        }
    });
    let reissue = Action::new(move |(): &()| async move { reissue_application_link(id.get_value()).await });
    Effect::new(move |_| {
        if let Some(result) = reissue.value().get() {
            match result {
                Ok(url) => set_link.set(Some(url)),
                Err(e) => set_notice.set(Some(Err(e.to_string()))),
            }
        }
    });
    let busy = Signal::derive(move || access.pending().get() || judge.pending().get());

    let mut meta = format!("@{} · GitHub : {} · {}", app.discord, app.github, app.email);
    let _ = write!(meta, " · préinscrit le {}", app.registered);
    let work = app.work.clone().map(|w| {
        let href = format!("/api/tests/applications/{}/download", app.id);
        view! {
            <Cluster>
                <ButtonLink href external=true icon=IconName::DownloadSimple>"Télécharger le rendu"</ButtonLink>
                <span class="ui-meta">{format!("{} · {} · {}", w.filename, w.size, w.when)}</span>
            </Cluster>
        }
    });
    let actions = match app.status.as_str() {
        "Requested" => Some(view! {
            <Button kind=ButtonKind::Primary icon=IconName::Key disabled=busy on:click=move |_| { access.dispatch(true); }>"Autoriser le sujet"</Button>
            <Button kind=ButtonKind::Ghost icon=IconName::XCircle disabled=busy on:click=move |_| { access.dispatch(false); }>"Refuser"</Button>
        }.into_any()),
        "HandedIn" => Some(view! {
            <Button kind=ButtonKind::Primary icon=IconName::CheckCircle disabled=busy on:click=move |_| { judge.dispatch("Admitted"); }>"Admettre"</Button>
            <Button kind=ButtonKind::Ghost icon=IconName::XCircle disabled=busy on:click=move |_| { judge.dispatch("Rejected"); }>"Refuser"</Button>
        }.into_any()),
        _ => None,
    };
    let end = view! {
        {status_tag(&app.status)}
        {actions}
    }
    .into_any();
    view! {
        <ListRow title=app.full_name meta end>
            {app.motivation.map(|text| view! { <RowText text /> })}
            {work}
            {app.comment.map(|text| view! { <RowText text=format!("Son mot : {text}") /> })}
            {app.invite_url.map(|url| view! { <p class="ui-meta">{format!("Invitation au serveur : {url}")}</p> })}
            <Cluster>
                <Button kind=ButtonKind::Ghost icon=IconName::LinkSimple disabled=Signal::derive(move || reissue.pending().get())
                    on:click=move |_| { reissue.dispatch(()); }>
                    "Lui envoyer un nouveau lien de suivi"
                </Button>
            </Cluster>
            {move || link.get().map(|url| view! {
                <p class="ui-meta">{format!("Nouveau lien (l'ancien ne marche plus), à lui envoyer : {url}")}</p>
            })}
            {move || notice.get().map(|n| match n {
                Ok(m) => view! { <p class="ui-meta">{m}</p> }.into_any(),
                Err(message) => view! { <ErrorText message /> }.into_any(),
            })}
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

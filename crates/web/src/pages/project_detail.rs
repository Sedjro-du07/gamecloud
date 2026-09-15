//! Project detail — a detail page.
//!
//! The interesting half of this page is the verdicts. A project is judged
//! once per concerned track, and a rejection must carry written feedback —
//! so this is where a member finds out what to fix, from the specialist who
//! would know.

use leptos::prelude::*;
use leptos_router::hooks::use_params_map;

use crate::{
    api::{ContributorItem, ProjectDetailView, ProjectRights, VerdictItem},
    components::ui::{
        segment_with_icon,
        vocab::{rarity_label, status_label, track_choices, track_icon_by_id, track_name, verdict},
        Avatar, Button, ButtonKind, ButtonLink, Cluster, DenseList, Disclosure, EmptyState,
        ErrorState, ErrorText, Fact, Facts, Field, Form, FormActions, FormRow, Gap, Icon,
        IconName, ListRow, Notice, NoticeKind, Page, PageHeader, PageSkeleton, Panel, Pattern,
        RowText, RowsSkeleton, Section, SegmentedControl, Stack, Visual,
    },
    server_fns::{add_contributor, get_project, get_project_files, review_project, submit_project},
};

/// What one verdict carries: project, track, verdict, feedback, mark.
type Submission = (String, String, String, Option<String>, Option<i32>);

/// A typed mark, once checked: empty is fine, anything else must be 0–100.
fn parse_score(raw: &str) -> Result<Option<i32>, ()> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Ok(None);
    }
    match raw.parse::<i32>() {
        Ok(n) if (0..=100).contains(&n) => Ok(Some(n)),
        _ => Err(()),
    }
}

/// The per-track verdicts.
#[component]
#[allow(clippy::needless_pass_by_value)] // Leptos prop convention
fn Verdicts(
    /// One per concerned track.
    validations: Vec<VerdictItem>,
) -> impl IntoView {
    if validations.is_empty() {
        return view! {
            <EmptyState
                icon=IconName::Eye
                title="Pas encore soumis à la revue"
                body="Chaque track concernée rendra son verdict ici."
            />
        }
        .into_any();
    }
    view! {
        <DenseList label="Verdicts par track">
            {validations
                .into_iter()
                .map(|v| {
                    let (label, icon) = verdict(&v.status);
                    let lead = view! { <Icon name=icon /> }.into_any();
                    let meta = v.reviewer_name.map_or_else(|| label.to_string(), |r| format!("{label} · relu par {r}"));
                    view! {
                        <ListRow lead title=track_name(&v.track) meta>
                            {v.feedback.map(|text| view! { <RowText text /> })}
                        </ListRow>
                    }
                })
                .collect_view()}
        </DenseList>
    }
    .into_any()
}

/// The verdict form for one track.
///
/// **A rejection must say what has to change**: the submit stays disabled
/// until it does. "Non applicable" matters too: a text adventure has no
/// Audio deliverable, and forcing a verdict would corrupt the record. The
/// mark out of 100 is optional: approving is a gate, grading a judgement.
#[component]
fn ReviewForm(
    /// Project being judged.
    project_id: String,
    /// Track this verdict is for.
    track: String,
    /// Called with the project's new status.
    on_done: Callback<String>,
) -> impl IntoView {
    let (choice, set_choice) = signal("Approved".to_string());
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

    let required = move || choice.get() == "Rejected";
    let parsed = Memo::new(move |_| parse_score(&score.get()));
    let applies = move || choice.get() != "NotApplicable";
    let blocked = Signal::derive(move || {
        submit.pending().get() || (required() && feedback.get().trim().is_empty()) || parsed.get().is_err()
    });
    let slug = track.to_lowercase();
    let feedback_id = StoredValue::new(format!("retour-{slug}"));
    let score_id = StoredValue::new(format!("note-{slug}"));
    let heading = format!("Ton verdict : {}", track_name(&track));
    let ids = StoredValue::new((project_id, track));

    view! {
        <Panel>
            <h3 class="ui-h3">{heading}</h3>
            <Form on:submit=move |ev| {
                ev.prevent_default();
                if blocked.get() {
                    return;
                }
                let text = feedback.get();
                let mark = parsed.get().unwrap_or(None).filter(|_| applies());
                let (id, track) = ids.get_value();
                submit.dispatch((id, track, choice.get(), (!text.trim().is_empty()).then_some(text), mark));
            }>
                <FormRow>
                    <SegmentedControl
                        label="Verdict"
                        options=vec![
                            segment_with_icon("Approved", "Approuver", IconName::CheckCircle),
                            segment_with_icon("Rejected", "Refuser", IconName::XCircle),
                            segment_with_icon("NotApplicable", "Non applicable", IconName::MinusCircle),
                        ]
                        value=choice
                        on_change=Callback::new(move |v| set_choice.set(v))
                    />
                </FormRow>
                // Rebuilt when the verdict changes: "what has to change" and
                // "optional feedback" are different requests.
                {move || {
                    let label = if required() { "Ce qui doit changer (obligatoire)" } else { "Retour à l'équipe (optionnel)" };
                    let id = feedback_id.get_value();
                    view! {
                        <Field id=id.clone() label hint="Sois précis : c'est ce que l'équipe lira pour corriger." wide=true>
                            <textarea id=id.clone() class="ui-control" rows="4" aria-describedby=format!("{id}-hint")
                                prop:value=move || feedback.get() on:input=move |ev| set_feedback.set(event_target_value(&ev))></textarea>
                        </Field>
                    }
                }}
                <Show when=applies>
                    <Field id=score_id.get_value() label="Note sur 100 (facultative)"
                        hint="Elle porte sur la part de cette track, pas sur le projet entier. Laisse vide pour valider sans noter.">
                        <input id=score_id.get_value() class="ui-control" type="number" min="0" max="100" inputmode="numeric"
                            aria-invalid=move || parsed.get().is_err().to_string()
                            prop:value=move || score.get() on:input=move |ev| set_score.set(event_target_value(&ev)) />
                    </Field>
                </Show>
                <Show when=move || parsed.get().is_err()>
                    <FormRow><ErrorText message="La note doit être un entier compris entre 0 et 100." /></FormRow>
                </Show>
                {move || error.get().map(|message| view! { <FormRow><ErrorText message /></FormRow> })}
                <FormActions>
                    <Button kind=ButtonKind::Primary button_type="submit" icon=IconName::SealCheck disabled=blocked>
                        {move || if submit.pending().get() { "Envoi…" } else { "Rendre le verdict" }}
                    </Button>
                    <Show when=move || required() && feedback.get().trim().is_empty()>
                        <span class="ui-meta">"Un refus explique ce qui ne va pas, sinon l'équipe ne peut rien corriger."</span>
                    </Show>
                </FormActions>
            </Form>
        </Panel>
    }
}

/// Credit a teammate, which also decides which tracks are asked for a verdict.
#[component]
fn ContributorForm(
    /// Project being credited.
    project_id: String,
    /// Refetch hook.
    on_changed: Callback<String>,
) -> impl IntoView {
    let (member, set_member) = signal(String::new());
    let (track, set_track) = signal("Engineering".to_string());
    let (role, set_role) = signal(String::new());
    let (error, set_error) = signal(Option::<String>::None);
    let add = Action::new(move |(p, m, t, r): &(String, String, String, String)| {
        let (p, m, t, r) = (p.clone(), m.clone(), t.clone(), r.clone());
        async move { add_contributor(p, m, t, r).await }
    });
    Effect::new(move |_| {
        if let Some(result) = add.value().get() {
            match result {
                Ok(()) => {
                    set_member.set(String::new());
                    set_role.set(String::new());
                    set_error.set(None);
                    on_changed.run("contributor".to_string());
                }
                Err(e) => set_error.set(Some(e.to_string())),
            }
        }
    });
    view! {
        <Form on:submit=move |ev| {
            ev.prevent_default();
            add.dispatch((project_id.clone(), member.get(), track.get(), role.get()));
        }>
            <Field id="credit-membre" label="Nom d'utilisateur Discord">
                <input id="credit-membre" class="ui-control" type="text" required=true placeholder="fred04"
                    prop:value=move || member.get() on:input=move |ev| set_member.set(event_target_value(&ev)) />
            </Field>
            <Field id="credit-track" label="Sur quelle track">
                <select id="credit-track" class="ui-control" on:change=move |ev| set_track.set(event_target_value(&ev))>
                    {track_choices().into_iter().map(|(id, label)| view! { <option value=id>{label}</option> }).collect_view()}
                </select>
            </Field>
            <Field id="credit-role" label="Rôle sur le projet (optionnel)">
                <input id="credit-role" class="ui-control" type="text" placeholder="Lead Artist"
                    prop:value=move || role.get() on:input=move |ev| set_role.set(event_target_value(&ev)) />
            </Field>
            {move || error.get().map(|message| view! { <FormRow><ErrorText message /></FormRow> })}
            <FormActions>
                <Button button_type="submit" icon=IconName::Plus disabled=Signal::derive(move || add.pending().get())>
                    {move || if add.pending().get() { "Ajout…" } else { "Créditer" }}
                </Button>
            </FormActions>
        </Form>
    }
}

/// The team, and crediting while the project can still change.
#[component]
#[allow(clippy::needless_pass_by_value)] // Leptos prop convention
fn Team(
    /// Who is credited.
    contributors: Vec<ContributorItem>,
    /// Whether crediting is still open.
    can_credit: bool,
    /// Project.
    project_id: String,
    /// Refetch hook.
    on_changed: Callback<String>,
) -> impl IntoView {
    let list = if contributors.is_empty() {
        view! { <EmptyState icon=IconName::Users title="Personne n'est encore crédité" /> }.into_any()
    } else {
        view! {
            <DenseList label="Équipe">
                {contributors
                    .into_iter()
                    .map(|c| {
                        let lead = view! { <Avatar name=c.display_name.clone() src=c.avatar_url.clone() /> }.into_any();
                        let meta = c.role.map_or_else(|| track_name(&c.track), |r| format!("{} · {r}", track_name(&c.track)));
                        view! { <ListRow lead title=c.display_name meta /> }
                    })
                    .collect_view()}
            </DenseList>
        }
        .into_any()
    };
    view! {
        <Section title="Équipe">
            {list}
            {can_credit.then(|| view! {
                <Disclosure summary="Créditer un coéquipier">
                    <ContributorForm project_id on_changed />
                </Disclosure>
            })}
        </Section>
    }
}

/// Builds attached to the project, and publishing one.
///
/// The upload is a plain multipart form posting straight to the API: it
/// works without JavaScript and keeps a 500 MB build out of the WASM
/// boundary. Downloads go through the platform, where the permission model
/// lives, because a private repository's asset is not fetchable by link.
#[component]
fn Builds(
    /// Project.
    project_id: String,
    /// Whether the viewer may attach a build.
    can_upload: bool,
) -> impl IntoView {
    let action = format!("/api/projects/{project_id}/files");
    let files = Resource::new(move || project_id.clone(), |id| async move { get_project_files(id).await });
    view! {
        <Section title="Builds">
            <Suspense fallback=|| view! { <RowsSkeleton rows=2 /> }>
                {move || {
                    files.get().map(|result| match result {
                        Err(_) => view! {
                            <ErrorState message="Impossible de charger les builds." on_retry=Callback::new(move |()| files.refetch()) />
                        }
                        .into_any(),
                        Ok(list) if list.is_empty() => view! {
                            <EmptyState icon=IconName::Package title="Aucun build publié pour l'instant" />
                        }
                        .into_any(),
                        Ok(list) => view! {
                            <DenseList label="Builds">
                                {list
                                    .into_iter()
                                    .map(|f| {
                                        let end = view! {
                                            <ButtonLink href=format!("/api/projects/files/{}/download", f.id) external=true icon=IconName::DownloadSimple>
                                                "Télécharger"
                                            </ButtonLink>
                                        }
                                        .into_any();
                                        let meta = format!("{} · {} · {} · {}", f.version, f.kind, f.size, f.when);
                                        view! {
                                            <ListRow title=f.filename meta end>
                                                {f.changelog.map(|text| view! { <RowText text /> })}
                                            </ListRow>
                                        }
                                    })
                                    .collect_view()}
                            </DenseList>
                        }
                        .into_any(),
                    })
                }}
            </Suspense>
            {can_upload.then(|| view! {
                <Disclosure summary="Publier un build">
                    <Form attr:method="post" attr:action=action attr:enctype="multipart/form-data">
                        <Field id="build-fichier" label="Fichier (500 Mo maximum)" wide=true>
                            <input id="build-fichier" class="ui-control" type="file" name="file" required=true />
                        </Field>
                        <Field id="build-version" label="Version">
                            <input id="build-version" class="ui-control" type="text" name="version" value="v1.0" required=true />
                        </Field>
                        <Field id="build-changelog" label="Ce qui change (optionnel)">
                            <input id="build-changelog" class="ui-control" type="text" name="changelog" placeholder="Correction du boss final" />
                        </Field>
                        <FormActions>
                            <Button kind=ButtonKind::Primary button_type="submit" icon=IconName::UploadSimple>"Envoyer"</Button>
                            <span class="ui-meta">"Le fichier part dans une release du dépôt ; tout le monde peut le télécharger à la publication."</span>
                        </FormActions>
                    </Form>
                </Disclosure>
            })}
        </Section>
    }
}

/// What the viewer may do: submit for review, publish, render verdicts.
#[component]
#[allow(clippy::needless_pass_by_value)] // Leptos prop convention
fn Actions(
    /// Project.
    project_id: String,
    /// The viewer's rights, from the server; each action re-checks them.
    rights: ProjectRights,
    /// Refetch hook.
    on_changed: Callback<String>,
) -> impl IntoView {
    let submit = Action::new(move |id: &String| {
        let id = id.clone();
        async move { submit_project(id).await }
    });
    let (notice, set_notice) = signal(Option::<Result<String, String>>::None);
    Effect::new(move |_| {
        if let Some(result) = submit.value().get() {
            match result {
                Ok(tracks) => {
                    let names: Vec<String> = tracks.iter().map(|t| track_name(t)).collect();
                    set_notice.set(Some(Ok(format!("Revue ouverte auprès de : {}.", names.join(", ")))));
                    on_changed.run("InReview".to_string());
                }
                Err(e) => set_notice.set(Some(Err(e.to_string()))),
            }
        }
    });

    if rights.reviewable_tracks.is_empty() && !rights.can_submit && !rights.can_release {
        return ().into_any();
    }
    let id = StoredValue::new(project_id.clone());
    let can_submit = rights.can_submit;
    let can_release = rights.can_release;
    view! {
        <Section title="Tes actions">
            {move || notice.get().map(|n| match n {
                Ok(m) => view! { <Notice kind=NoticeKind::Success>{m}</Notice> }.into_any(),
                Err(m) => view! { <Notice kind=NoticeKind::Error>{m}</Notice> }.into_any(),
            })}
            {can_submit.then(|| view! {
                <Cluster>
                    <Button kind=ButtonKind::Primary icon=IconName::PaperPlaneRight
                        disabled=Signal::derive(move || submit.pending().get())
                        on:click=move |_| { submit.dispatch(id.get_value()); }>
                        {move || if submit.pending().get() { "Soumission…" } else { "Soumettre en revue" }}
                    </Button>
                    <span class="ui-meta">"Chaque track concernée est sollicitée dans son propre salon Discord."</span>
                </Cluster>
            })}
            {can_release.then(|| view! {
                <Notice>
                    "Ce projet est approuvé. Responsable de la track principale, tu peux le publier : l'équipe est récompensée et le dépôt devient public."
                </Notice>
            })}
            <Stack gap=Gap::Normal>
                {rights
                    .reviewable_tracks
                    .into_iter()
                    .map(|track| view! { <ReviewForm project_id=project_id.clone() track on_done=on_changed /> })
                    .collect_view()}
            </Stack>
        </Section>
    }
    .into_any()
}

/// The project, once loaded.
#[component]
#[allow(clippy::needless_pass_by_value)] // Leptos prop convention
fn ProjectBody(
    /// The loaded project.
    detail: ProjectDetailView,
    /// Refetch hook.
    on_changed: Callback<String>,
) -> impl IntoView {
    let card = detail.card.clone();
    let shot = detail.screenshots.first().cloned().or_else(|| card.thumbnail_url.clone());
    let icon = track_icon_by_id(&card.track).unwrap_or(IconName::Cube);
    let team = match card.contributor_count {
        0 => "Personne".to_string(),
        1 => "1 membre".to_string(),
        n => format!("{n} membres"),
    };
    // Crediting is open while the project can still be edited.
    let can_credit = detail.rights.can_submit;
    view! {
        <Page pattern=Pattern::Detail>
            <PageHeader title=card.name.clone() kicker=track_name(&card.track) lead=card.short_description.clone().unwrap_or_default()>
                {detail.github_repo_url.map(|url| view! {
                    <ButtonLink href=url new_tab=true icon=IconName::GithubLogo>"Dépôt GitHub"</ButtonLink>
                })}
                {detail.itch_url.map(|url| view! {
                    <ButtonLink href=url new_tab=true icon=IconName::ArrowSquareOut>"Page itch.io"</ButtonLink>
                })}
            </PageHeader>
            <Visual src=shot alt=format!("Capture de {}", card.name) icon />
            <Facts>
                <Fact label="Statut" value=status_label(&card.status) />
                <Fact label="Rareté" value=rarity_label(&card.rarity) />
                <Fact label="Équipe" value=team />
                {card.released_on.clone().map(|day| view! { <Fact label="Publié le" value=day /> })}
            </Facts>
            {detail.long_description.map(|text| view! {
                <Section title="Le projet"><p class="ui-body">{text}</p></Section>
            })}
            <Team contributors=detail.contributors can_credit project_id=card.id.clone() on_changed />
            <Section title="Revue par track">
                <Verdicts validations=detail.validations />
            </Section>
            <Builds project_id=card.id.clone() can_upload=detail.rights.can_upload />
            <Actions project_id=card.id rights=detail.rights on_changed />
        </Page>
    }
}

/// Project detail page.
#[component]
pub fn ProjectDetailPage() -> impl IntoView {
    let params = use_params_map();
    let project = Resource::new(
        move || params.read().get("id").unwrap_or_default(),
        |id| async move { get_project(id).await },
    );
    let on_changed = Callback::new(move |_status: String| project.refetch());

    view! {
        <Suspense fallback=|| view! { <Page pattern=Pattern::Detail><PageSkeleton /></Page> }>
            {move || {
                project.get().map(|result| match result {
                    Err(_) => view! {
                        <Page pattern=Pattern::Detail>
                            <PageHeader title="Projet" />
                            <ErrorState message="Impossible de charger ce projet." on_retry=Callback::new(move |()| project.refetch()) />
                        </Page>
                    }
                    .into_any(),
                    Ok(None) => view! {
                        <Page pattern=Pattern::Detail>
                            <PageHeader title="Projet introuvable" />
                            <EmptyState icon=IconName::Cube title="Ce projet n'existe pas, ou n'est pas encore publié">
                                <ButtonLink href="/projects" icon=IconName::CaretLeft>"Tous les projets"</ButtonLink>
                            </EmptyState>
                        </Page>
                    }
                    .into_any(),
                    Ok(Some(detail)) => view! { <ProjectBody detail on_changed /> }.into_any(),
                })
            }}
        </Suspense>
    }
}

#[cfg(test)]
mod tests {
    use super::parse_score;

    #[test]
    fn a_mark_is_optional_but_must_be_on_the_scale() {
        assert_eq!(parse_score(""), Ok(None));
        assert_eq!(parse_score(" 85 "), Ok(Some(85)));
        assert_eq!(parse_score("0"), Ok(Some(0)));
        assert!(parse_score("101").is_err());
        assert!(parse_score("9/10").is_err());
    }
}

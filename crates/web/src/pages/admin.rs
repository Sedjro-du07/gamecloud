//! Bureau panel — a detail page.
//!
//! What only the Bureau can do, plus the trail it leaves. The page is
//! reachable by anyone who types the URL — hiding it in the menu is a
//! courtesy, not a control — so every action is re-authorised server-side
//! and the page simply reports the refusal.

use leptos::prelude::*;

use crate::{
    components::ui::{
        vocab::track_choices, Button, ButtonKind, DenseList, EmptyState, ErrorState, Field, Form,
        FormActions, IconName, ListRow, Notice, NoticeKind, Page, PageHeader, Pattern, RowText,
        RowsSkeleton, Section,
    },
    api::DirectoryEntry,
    server_fns::{
        appoint_bureau, appoint_track_role, get_audit, get_directory, get_me, grant_xp, open_quest,
    },
};

/// Everybody on the server, loaded once for the whole page.
type Directory = Resource<Result<Vec<DirectoryEntry>, ServerFnError>>;

/// The id of the `<datalist>` both appointment forms suggest names from.
const MEMBERS_LIST: &str = "membres-du-serveur";

/// Name suggestions for the appointment forms: every human on the
/// Discord server, whether or not they have opened the platform.
///
/// A `<datalist>` rather than a `<select>` because thirty-odd names in a
/// drop-down is a scroll, while typing three letters narrows it — and
/// the field still accepts a name that is not in the list, which the
/// server resolves against Discord itself.
#[component]
fn MemberSuggestions(directory: Directory) -> impl IntoView {
    view! {
        <datalist id=MEMBERS_LIST>
            <Suspense fallback=|| ()>
                {move || directory.get().and_then(Result::ok).map(|people| {
                    people.into_iter().map(|p| {
                        let label = if p.display_name == p.username {
                            p.username.clone()
                        } else {
                            format!("{} ({})", p.display_name, p.username)
                        };
                        view! { <option value=p.username>{label}</option> }
                    }).collect_view()
                })}
            </Suspense>
        </datalist>
    }
}

/// Every office, gamified title first, with the plain name beside it.
fn office_choices() -> Vec<(&'static str, String)> {
    gamecloud_shared::roles::BureauRole::ALL
        .iter()
        .map(|b| (b.as_str(), format!("{} — {}", b.title(), b.plain())))
        .collect()
}

/// Who holds which office, and the form to change it.
///
/// Appointing here is the whole procedure: the member is told privately,
/// their Discord role follows through the bot, and the rights the office
/// opens appear on their profile at their next page load.
#[component]
fn OfficesPanel(
    /// Everybody on the server.
    directory: Directory,
    /// Whether the viewer may appoint; everybody else on the Bureau sees
    /// who holds what, without the form.
    can_appoint: Signal<bool>,
) -> impl IntoView {
    let (member, set_member) = signal(String::new());
    let (office, set_office) = signal("Treasurer".to_string());
    let (notice, set_notice) = signal(Outcome::None);
    // `true` adds the office on top of any others, `false` removes just
    // this one: a member may hold several, so neither is a replacement.
    let appoint = Action::new(move |(m, o, add): &(String, String, bool)| {
        let (m, o, add) = (m.clone(), o.clone(), *add);
        async move { appoint_bureau(m, o, add).await }
    });
    Effect::new(move |_| {
        if let Some(result) = appoint.value().get() {
            set_notice.set(Some(match result {
                Ok(done) => {
                    set_member.set(String::new());
                    directory.refetch();
                    Ok(done)
                }
                Err(e) => Err(e.to_string()),
            }));
        }
    });

    view! {
        <Section title="Offices du Bureau"
            lead="Qui occupe quel office. Seuls le ou la président·e et le ou la vice-président·e nomment ; une personne peut cumuler plusieurs offices, elle est prévenue en privé et ses rôles Discord suivent.">
            {move || outcome_notice(notice.get())}
            <Show when=move || can_appoint.get()>
            <Form on:submit=move |ev| {
                ev.prevent_default();
                appoint.dispatch((member.get(), office.get(), true));
            }>
                <Field id="office-membre" label="Membre (pseudo Discord)" wide=true
                    hint="N'importe qui sur le serveur, même sans s'être encore connecté à la plateforme.">
                    <input id="office-membre" class="ui-control" type="text" required=true
                        list=MEMBERS_LIST autocomplete="off" placeholder="Commence à taper un nom…"
                        prop:value=move || member.get() on:input=move |ev| set_member.set(event_target_value(&ev)) />
                </Field>
                <Field id="office-role" label="Office" wide=true>
                    <select id="office-role" class="ui-control"
                        prop:value=move || office.get()
                        on:change=move |ev| set_office.set(event_target_value(&ev))>
                        {office_choices().into_iter().map(|(id, label)| view! { <option value=id>{label}</option> }).collect_view()}
                    </select>
                </Field>
                <FormActions>
                    <Button kind=ButtonKind::Primary button_type="submit" icon=IconName::Crown
                        disabled=Signal::derive(move || appoint.pending().get())>
                        {move || if appoint.pending().get() { "Envoi…" } else { "Ajouter cet office" }}
                    </Button>
                    <Button kind=ButtonKind::Ghost button_type="button"
                        disabled=Signal::derive(move || appoint.pending().get() || member.get().trim().is_empty())
                        on:click=move |_| { appoint.dispatch((member.get(), office.get(), false)); }>
                        "Retirer cet office"
                    </Button>
                </FormActions>
            </Form>
            </Show>

            <Suspense fallback=|| view! { <RowsSkeleton rows=5 /> }>
                {move || directory.get().map(|result| match result {
                    Err(e) => view! {
                        <ErrorState message=format!("Annuaire indisponible : {e}") on_retry=Callback::new(move |()| directory.refetch()) />
                    }.into_any(),
                    Ok(people) => {
                        let holders: Vec<DirectoryEntry> = people
                            .into_iter()
                            .filter(|p| p.bureau_label.is_some() || !p.track_roles.is_empty())
                            .collect();
                        if holders.is_empty() {
                            return view! { <EmptyState icon=IconName::Crown title="Personne n'a encore d'office" /> }.into_any();
                        }
                        view! {
                            <DenseList label="Offices et responsabilités">
                                {holders.into_iter().map(|p| {
                                    let mut meta = vec![format!("@{}", p.username)];
                                    if !p.signed_in {
                                        meta.push("pas encore connecté·e à la plateforme".into());
                                    }
                                    let mut detail: Vec<String> = p.bureau_label.clone().into_iter().collect();
                                    detail.extend(p.track_roles.iter().cloned());
                                    view! {
                                        <ListRow title=p.display_name meta=meta.join(" · ")>
                                            <RowText text=detail.join(" · ") />
                                        </ListRow>
                                    }
                                }).collect_view()}
                            </DenseList>
                        }.into_any()
                    }
                })}
            </Suspense>
        </Section>
    }
}

/// The outcome of an action, shown above its form.
type Outcome = Option<Result<String, String>>;

/// A success or error notice for an outcome.
fn outcome_notice(outcome: Outcome) -> Option<AnyView> {
    outcome.map(|r| match r {
        Ok(m) => view! { <Notice kind=NoticeKind::Success>{m}</Notice> }.into_any(),
        Err(m) => view! { <Notice kind=NoticeKind::Error>{m}</Notice> }.into_any(),
    })
}

/// Manual XP grant.
#[component]
fn GrantPanel() -> impl IntoView {
    let (member, set_member) = signal(String::new());
    let (amount, set_amount) = signal("50".to_string());
    let (reason, set_reason) = signal(String::new());
    let (notice, set_notice) = signal(Outcome::None);
    let grant = Action::new(move |(m, a, r): &(String, i32, String)| {
        let (m, a, r) = (m.clone(), *a, r.clone());
        async move { grant_xp(m, a, r).await }
    });
    Effect::new(move |_| {
        if let Some(result) = grant.value().get() {
            set_notice.set(Some(match result {
                Ok(awarded) => {
                    set_member.set(String::new());
                    set_reason.set(String::new());
                    Ok(format!("{awarded} XP attribués."))
                }
                Err(e) => Err(e.to_string()),
            }));
        }
    });
    view! {
        <Section title="Attribuer de l'XP"
            lead="Les multiplicateurs ne s'appliquent pas : le montant saisi est le montant crédité. Le motif finit dans le journal.">
            {move || outcome_notice(notice.get())}
            <Form on:submit=move |ev| {
                ev.prevent_default();
                let parsed = amount.get().trim().parse::<i32>().unwrap_or(0);
                if parsed == 0 {
                    set_notice.set(Some(Err("Le montant doit être différent de zéro.".into())));
                    return;
                }
                grant.dispatch((member.get(), parsed, reason.get()));
            }>
                <Field id="xp-membre" label="Nom d'utilisateur Discord">
                    <input id="xp-membre" class="ui-control" type="text" required=true placeholder="fred04"
                        prop:value=move || member.get() on:input=move |ev| set_member.set(event_target_value(&ev)) />
                </Field>
                <Field id="xp-montant" label="Montant (négatif pour retirer)">
                    <input id="xp-montant" class="ui-control" type="number"
                        prop:value=move || amount.get() on:input=move |ev| set_amount.set(event_target_value(&ev)) />
                </Field>
                <Field id="xp-motif" label="Motif" wide=true>
                    <input id="xp-motif" class="ui-control" type="text" required=true placeholder="Aide au montage du stand"
                        prop:value=move || reason.get() on:input=move |ev| set_reason.set(event_target_value(&ev)) />
                </Field>
                <FormActions>
                    <Button kind=ButtonKind::Primary button_type="submit" icon=IconName::Lightning
                        disabled=Signal::derive(move || grant.pending().get())>
                        {move || if grant.pending().get() { "Envoi…" } else { "Attribuer" }}
                    </Button>
                </FormActions>
            </Form>
        </Section>
    }
}

/// Open a quest: the one mechanism that directs effort rather than measuring it.
#[component]
fn QuestPanel() -> impl IntoView {
    let (title, set_title) = signal(String::new());
    let (condition, set_condition) = signal("Review".to_string());
    let (target, set_target) = signal("3".to_string());
    let (reward, set_reward) = signal("40".to_string());
    let (days, set_days) = signal("7".to_string());
    let (notice, set_notice) = signal(Outcome::None);
    let open = Action::new(move |(t, c, n, x, d): &(String, String, i32, i32, i32)| {
        let (t, c, n, x, d) = (t.clone(), c.clone(), *n, *x, *d);
        async move { open_quest(t, c, n, x, d).await }
    });
    Effect::new(move |_| {
        if let Some(result) = open.value().get() {
            set_notice.set(Some(match result {
                Ok(_) => {
                    set_title.set(String::new());
                    Ok("Quête ouverte et annoncée sur Discord.".to_string())
                }
                Err(e) => Err(e.to_string()),
            }));
        }
    });
    view! {
        <Section title="Ouvrir une quête" lead="La progression se calcule toute seule à partir de l'XP qui arrive : rien à cocher à la main.">
            {move || outcome_notice(notice.get())}
            <Form on:submit=move |ev| {
                ev.prevent_default();
                open.dispatch((
                    title.get(),
                    condition.get(),
                    target.get().trim().parse().unwrap_or(1),
                    reward.get().trim().parse().unwrap_or(10),
                    days.get().trim().parse().unwrap_or(7),
                ));
            }>
                <Field id="quete-titre" label="Intitulé" wide=true>
                    <input id="quete-titre" class="ui-control" type="text" required=true placeholder="Trois revues cette semaine"
                        prop:value=move || title.get() on:input=move |ev| set_title.set(event_target_value(&ev)) />
                </Field>
                <Field id="quete-condition" label="Ce qui est demandé">
                    <select id="quete-condition" class="ui-control" on:change=move |ev| set_condition.set(event_target_value(&ev))>
                        <option value="Review">"Rendre des revues"</option>
                        <option value="Push">"Pousser du code"</option>
                        <option value="Attend">"Être présent à des événements"</option>
                        <option value="Submit">"Soumettre des projets"</option>
                    </select>
                </Field>
                <Field id="quete-cible" label="Combien de fois">
                    <input id="quete-cible" class="ui-control" type="number"
                        prop:value=move || target.get() on:input=move |ev| set_target.set(event_target_value(&ev)) />
                </Field>
                <Field id="quete-xp" label="Récompense (XP)">
                    <input id="quete-xp" class="ui-control" type="number"
                        prop:value=move || reward.get() on:input=move |ev| set_reward.set(event_target_value(&ev)) />
                </Field>
                <Field id="quete-jours" label="Durée (jours)">
                    <input id="quete-jours" class="ui-control" type="number"
                        prop:value=move || days.get() on:input=move |ev| set_days.set(event_target_value(&ev)) />
                </Field>
                <FormActions>
                    <Button kind=ButtonKind::Primary button_type="submit" icon=IconName::Target
                        disabled=Signal::derive(move || open.pending().get())>
                        {move || if open.pending().get() { "Ouverture…" } else { "Ouvrir la quête" }}
                    </Button>
                </FormActions>
            </Form>
        </Section>
    }
}

/// Appoint a track lead or co-lead: the two track titles XP never confers.
#[component]
fn AppointPanel(directory: Directory) -> impl IntoView {
    let (member, set_member) = signal(String::new());
    let (track, set_track) = signal("Engineering".to_string());
    let (role, set_role) = signal("Lead".to_string());
    let (notice, set_notice) = signal(Outcome::None);
    let appoint = Action::new(move |(m, t, r): &(String, String, String)| {
        let (m, t, r) = (m.clone(), t.clone(), r.clone());
        async move { appoint_track_role(m, t, r).await }
    });
    Effect::new(move |_| {
        if let Some(result) = appoint.value().get() {
            set_notice.set(Some(match result {
                Ok(()) => {
                    set_member.set(String::new());
                    directory.refetch();
                    Ok("Nomination enregistrée. La personne est prévenue et rejoint la track si elle n'y était pas.".to_string())
                }
                Err(e) => Err(e.to_string()),
            }));
        }
    });
    view! {
        <Section title="Nommer un responsable de track"
            lead="Responsable et co-responsable sont les deux seuls titres de track qui ne s'obtiennent pas à l'XP.">
            {move || outcome_notice(notice.get())}
            <Form on:submit=move |ev| {
                ev.prevent_default();
                appoint.dispatch((member.get(), track.get(), role.get()));
            }>
                <Field id="nomination-membre" label="Membre (pseudo Discord)" wide=true>
                    <input id="nomination-membre" class="ui-control" type="text" required=true
                        list=MEMBERS_LIST autocomplete="off" placeholder="Commence à taper un nom…"
                        prop:value=move || member.get() on:input=move |ev| set_member.set(event_target_value(&ev)) />
                </Field>
                <Field id="nomination-track" label="Track">
                    <select id="nomination-track" class="ui-control" on:change=move |ev| set_track.set(event_target_value(&ev))>
                        {track_choices().into_iter().map(|(id, label)| view! { <option value=id>{label}</option> }).collect_view()}
                    </select>
                </Field>
                <Field id="nomination-role" label="Titre">
                    <select id="nomination-role" class="ui-control" on:change=move |ev| set_role.set(event_target_value(&ev))>
                        <option value="Lead">"Responsable"</option>
                        <option value="CoLead">"Co-responsable"</option>
                        <option value="Observer">"Retirer (remettre Observateur)"</option>
                    </select>
                </Field>
                <FormActions>
                    <Button kind=ButtonKind::Primary button_type="submit" icon=IconName::Crown
                        disabled=Signal::derive(move || appoint.pending().get())>
                        {move || if appoint.pending().get() { "Envoi…" } else { "Nommer" }}
                    </Button>
                </FormActions>
            </Form>
        </Section>
    }
}

/// The audit trail.
#[component]
fn AuditPanel() -> impl IntoView {
    let audit = Resource::new(|| (), |()| async { get_audit().await });
    view! {
        <Section title="Journal" lead="Tout ce qui change la situation de quelqu'un d'autre laisse une trace ici. Le journal ne se modifie pas.">
            <Suspense fallback=|| view! { <RowsSkeleton rows=4 /> }>
                {move || audit.get().map(|result| match result {
                    Err(e) => view! {
                        <ErrorState message=format!("Journal indisponible : {e}") on_retry=Callback::new(move |()| audit.refetch()) />
                    }
                    .into_any(),
                    Ok(lines) if lines.is_empty() => view! { <EmptyState icon=IconName::Bank title="Aucune entrée pour l'instant" /> }.into_any(),
                    Ok(lines) => view! {
                        <DenseList label="Journal">
                            {lines.into_iter().map(|l| view! {
                                <ListRow title=l.action meta=format!("{} · {}", l.when, l.actor)>
                                    <RowText text=l.detail />
                                </ListRow>
                            }).collect_view()}
                        </DenseList>
                    }
                    .into_any(),
                })}
            </Suspense>
        </Section>
    }
}

/// Bureau panel page.
#[component]
pub fn AdminPage() -> impl IntoView {
    let directory: Directory = Resource::new(|| (), |()| async { get_directory().await });
    // Appointing is for the President and the Vice-President alone. The
    // server refuses anybody else regardless; hiding the forms spares the
    // rest of the Bureau a button that can only fail.
    let me = Resource::new(|| (), |()| async { get_me().await });
    let appoints = move || {
        me.get()
            .and_then(Result::ok)
            .flatten()
            .is_some_and(|m| m.can_appoint)
    };
    view! {
        <Page pattern=Pattern::Detail>
            <PageHeader title="Bureau" lead="Ce que seul le Bureau peut faire, et la trace que ça laisse. Chaque action est revérifiée par le serveur." />
            <Suspense fallback=|| ()>
                <Show when=appoints>
                    <MemberSuggestions directory />
                </Show>
                <OfficesPanel directory can_appoint=Signal::derive(appoints) />
                <Show when=appoints>
                    <AppointPanel directory />
                </Show>
            </Suspense>
            <GrantPanel />
            <QuestPanel />
            <AuditPanel />
        </Page>
    }
}

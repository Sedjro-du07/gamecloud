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
    server_fns::{appoint_track_role, get_audit, grant_xp, open_quest},
};

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
fn AppointPanel() -> impl IntoView {
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
                    Ok("Nomination enregistrée.".to_string())
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
                <Field id="nomination-membre" label="Nom d'utilisateur Discord" wide=true>
                    <input id="nomination-membre" class="ui-control" type="text" required=true placeholder="fred04"
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
    view! {
        <Page pattern=Pattern::Detail>
            <PageHeader title="Bureau" lead="Ce que seul le Bureau peut faire, et la trace que ça laisse. Chaque action est revérifiée par le serveur." />
            <GrantPanel />
            <AppointPanel />
            <QuestPanel />
            <AuditPanel />
        </Page>
    }
}

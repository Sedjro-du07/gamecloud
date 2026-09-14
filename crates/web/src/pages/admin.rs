//! Bureau panel.
//!
//! Two things only the Bureau can do, plus the trail they leave. The
//! page is reachable by anyone who types the URL — hiding it in the
//! navigation is a courtesy, not a control — so every action here is
//! re-authorised server-side and the page simply reports the refusal.

use leptos::prelude::*;

use crate::server_fns::{get_audit, grant_xp, open_quest};

/// Manual XP grant.
#[component]
fn GrantPanel() -> impl IntoView {
    let (member, set_member) = signal(String::new());
    let (amount, set_amount) = signal("50".to_string());
    let (reason, set_reason) = signal(String::new());
    let (notice, set_notice) = signal(Option::<Result<String, String>>::None);

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
        <section class="gc-admin__panel">
            <h2>"Attribuer de l'XP"</h2>
            <p class="gc-admin__note">
                "Les multiplicateurs ne s'appliquent pas : le montant saisi est le
                 montant crédité. Un motif est obligatoire — il finit dans le journal."
            </p>

            {move || {
                notice
                    .get()
                    .map(|r| match r {
                        Ok(m) => {
                            view! { <div class="gc-banner gc-banner--ok">{m}</div> }.into_any()
                        }
                        Err(m) => {
                            view! { <div class="gc-banner gc-banner--warning">{m}</div> }
                                .into_any()
                        }
                    })
            }}

            <form
                class="gc-form"
                on:submit=move |ev| {
                    ev.prevent_default();
                    let parsed = amount.get().trim().parse::<i32>().unwrap_or(0);
                    if parsed == 0 {
                        set_notice.set(Some(Err("Le montant doit être non nul.".into())));
                        return;
                    }
                    grant.dispatch((member.get(), parsed, reason.get()));
                }
            >
                <label class="gc-field">
                    <span>"Membre (identifiant plateforme ou Discord)"</span>
                    <input
                        type="text"
                        required=true
                        placeholder="865973472223428608"
                        prop:value=move || member.get()
                        on:input=move |ev| set_member.set(event_target_value(&ev))
                    />
                </label>
                <label class="gc-field">
                    <span>"Montant (négatif pour retirer)"</span>
                    <input
                        type="number"
                        prop:value=move || amount.get()
                        on:input=move |ev| set_amount.set(event_target_value(&ev))
                    />
                </label>
                <label class="gc-field">
                    <span>"Motif"</span>
                    <input
                        type="text"
                        required=true
                        placeholder="Aide au montage du stand"
                        prop:value=move || reason.get()
                        on:input=move |ev| set_reason.set(event_target_value(&ev))
                    />
                </label>
                <button
                    class="gc-btn gc-btn--primary"
                    type="submit"
                    disabled=move || grant.pending().get()
                >
                    {move || if grant.pending().get() { "Envoi…" } else { "Attribuer" }}
                </button>
            </form>
        </section>
    }
}

/// Open a weekly quest.
///
/// Quests are the one mechanism that *directs* effort rather than
/// measuring it, so the form leads with the action asked and the reward,
/// not with metadata.
#[component]
fn QuestPanel() -> impl IntoView {
    let (title, set_title) = signal(String::new());
    let (condition, set_condition) = signal("Review".to_string());
    let (target, set_target) = signal("3".to_string());
    let (reward, set_reward) = signal("40".to_string());
    let (days, set_days) = signal("7".to_string());
    let (notice, set_notice) = signal(Option::<Result<String, String>>::None);

    let open = Action::new(
        move |(t, c, n, x, d): &(String, String, i32, i32, i32)| {
            let (t, c, n, x, d) = (t.clone(), c.clone(), *n, *x, *d);
            async move { open_quest(t, c, n, x, d).await }
        },
    );

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
        <section class="gc-admin__panel">
            <h2>"Ouvrir une quête"</h2>
            <p class="gc-admin__note">
                "La progression se calcule toute seule à partir de l'XP qui arrive.
                 Rien à cocher à la main."
            </p>

            {move || {
                notice
                    .get()
                    .map(|r| match r {
                        Ok(m) => {
                            view! { <div class="gc-banner gc-banner--ok">{m}</div> }.into_any()
                        }
                        Err(m) => {
                            view! { <div class="gc-banner gc-banner--warning">{m}</div> }
                                .into_any()
                        }
                    })
            }}

            <form
                class="gc-form"
                on:submit=move |ev| {
                    ev.prevent_default();
                    open.dispatch((
                        title.get(),
                        condition.get(),
                        target.get().trim().parse().unwrap_or(1),
                        reward.get().trim().parse().unwrap_or(10),
                        days.get().trim().parse().unwrap_or(7),
                    ));
                }
            >
                <label class="gc-field">
                    <span>"Intitulé"</span>
                    <input
                        type="text"
                        required=true
                        placeholder="Trois revues cette semaine"
                        prop:value=move || title.get()
                        on:input=move |ev| set_title.set(event_target_value(&ev))
                    />
                </label>
                <label class="gc-field">
                    <span>"Ce qui est demandé"</span>
                    <select on:change=move |ev| set_condition.set(event_target_value(&ev))>
                        <option value="Review">"Rendre des revues"</option>
                        <option value="Push">"Pousser du code"</option>
                        <option value="Attend">"Être présent à des événements"</option>
                        <option value="Submit">"Soumettre des projets"</option>
                    </select>
                </label>
                <label class="gc-field">
                    <span>"Combien de fois"</span>
                    <input
                        type="number"
                        prop:value=move || target.get()
                        on:input=move |ev| set_target.set(event_target_value(&ev))
                    />
                </label>
                <label class="gc-field">
                    <span>"Récompense (XP)"</span>
                    <input
                        type="number"
                        prop:value=move || reward.get()
                        on:input=move |ev| set_reward.set(event_target_value(&ev))
                    />
                </label>
                <label class="gc-field">
                    <span>"Durée (jours)"</span>
                    <input
                        type="number"
                        prop:value=move || days.get()
                        on:input=move |ev| set_days.set(event_target_value(&ev))
                    />
                </label>
                <button
                    class="gc-btn gc-btn--primary"
                    type="submit"
                    disabled=move || open.pending().get()
                >
                    {move || if open.pending().get() { "Ouverture…" } else { "Ouvrir la quête" }}
                </button>
            </form>
        </section>
    }
}

/// The audit trail.
#[component]
fn AuditPanel() -> impl IntoView {
    let audit = Resource::new(|| (), |()| async { get_audit().await });

    view! {
        <section class="gc-admin__panel">
            <h2>"Journal"</h2>
            <p class="gc-admin__note">
                "Tout ce qui change la situation de quelqu'un d'autre laisse une trace
                 ici. Le journal ne se modifie pas."
            </p>

            <Suspense fallback=move || view! { <p class="gc-empty">"Chargement…"</p> }>
                {move || match audit.get() {
                    None => view! { <p class="gc-empty">"Chargement…"</p> }.into_any(),
                    Some(Err(e)) => {
                        view! {
                            <p class="gc-empty">{format!("Accès refusé : {e}")}</p>
                        }
                            .into_any()
                    }
                    Some(Ok(lines)) if lines.is_empty() => {
                        view! { <p class="gc-empty">"Aucune entrée."</p> }.into_any()
                    }
                    Some(Ok(lines)) => {
                        view! {
                            <ul class="gc-admin__audit">
                                {lines
                                    .into_iter()
                                    .map(|l| {
                                        view! {
                                            <li>
                                                <span class="gc-admin__when">{l.when}</span>
                                                <span class="gc-admin__actor">{l.actor}</span>
                                                <span class="gc-admin__action">{l.action}</span>
                                                <span class="gc-admin__detail">{l.detail}</span>
                                            </li>
                                        }
                                    })
                                    .collect_view()}
                            </ul>
                        }
                            .into_any()
                    }
                }}
            </Suspense>
        </section>
    }
}

/// Bureau panel page.
#[component]
pub fn AdminPage() -> impl IntoView {
    view! {
        <section class="gc-admin">
            <h1>"Bureau"</h1>
            <GrantPanel />
            <QuestPanel />
            <AuditPanel />
        </section>
    }
}

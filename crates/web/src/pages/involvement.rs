//! Involvement — the member's sheet, its print version, and the Bureau's board.
//!
//! `/fiche` is the link every member is sent: they fill in their name and
//! promotion once, see their involvement this month and over the year, and
//! export their sheet. `/fiche/imprimer` is that sheet as a plain document,
//! black on white, to save as PDF; the Bureau opens it for one member
//! (`?membre=pseudo`) or for everyone (`?tous=1`). The board, on the Bureau
//! page, lists every member; the President and the Vice-President force a
//! level there, with a reason, and write the remarks the sheet prints.

use leptos::prelude::*;
use leptos_router::hooks::use_query_map;

use crate::{
    api::{InvolvementView, InvolvementBoard},
    components::ui::{
        Button, ButtonKind, ButtonLink, Cluster, DenseList, Disclosure, EmptyState, ErrorState,
        ErrorText, Fact, Facts, Field, Form, FormActions, IconName, ListRow, Notice, NoticeKind,
        Page, PageHeader, Pattern, RowText, RowValue, RowsSkeleton, Section, SignInState, Tag,
        TagKind, XpProgress,
    },
    server_fns::{
        get_involvement_board, get_my_involvement, get_sheets, save_involvement_review,
        save_my_identity,
    },
};

/// Promotions offered in the form.
const PROMOTIONS: [(&str, &str); 7] = [
    ("Tek1", "Tek1 (1re année)"),
    ("Tek2", "Tek2 (2e année)"),
    ("Tek3", "Tek3 (3e année)"),
    ("Tek4", "Tek4 (4e année)"),
    ("Tek5", "Tek5 (5e année)"),
    ("MSc", "MSc"),
    ("Autre", "Autre"),
];

/// "Prénom Nom", or the Discord username while they are not filled in.
fn full_name(v: &InvolvementView) -> String {
    match (&v.first_name, &v.last_name) {
        (Some(first), Some(last)) => format!("{first} {last}"),
        _ => v.username.clone(),
    }
}

fn level_text(level: Option<u8>) -> String {
    level.map_or_else(|| "—".to_string(), |l| format!("{l} / 5"))
}

/// The level that counts, with its mention when it is maximal.
fn headline_level(v: &InvolvementView) -> String {
    if v.maximal {
        "5 / 5, implication maximale".to_string()
    } else {
        level_text(v.level)
    }
}

/// Where a member stands: this month, the year, and what to do next.
#[component]
#[allow(clippy::needless_pass_by_value)] // Leptos prop convention
pub fn InvolvementSummary(
    /// The member's involvement.
    view: InvolvementView,
) -> impl IntoView {
    let next = match (view.current_level, view.to_next) {
        _ if view.maximal => "Implication maximale : niveau 5 et 3 crédits recommandés, quoi qu'il arrive. Le détail ci-dessous reste ton activité réelle.".to_string(),
        (Some(5), _) | (_, None) => "Tu es au niveau maximal ce mois-ci.".to_string(),
        (_, Some(points)) => format!("Encore {points} points ce mois-ci pour le niveau suivant."),
    };
    let decision = view.forced_level.filter(|_| !view.maximal).map(|l| {
        let why = view.force_reason.clone().unwrap_or_default();
        view! {
            <Notice>{format!("Niveau de l'année fixé à {l} / 5 par le président ou le vice-président : {why}")}</Notice>
        }
    });
    let floor = (view.floor > 1).then(|| {
        view! {
            <p class="ui-meta">{format!(
                "Tes responsabilités te garantissent au moins le niveau {} chaque mois.", view.floor
            )}</p>
        }
    });
    let axes = view.axes.clone();
    let hints = view.hints.clone();
    let mut months = view.months.clone();
    months.reverse();
    view! {
        <Section title="Mon implication" lead=format!("Année scolaire {}. Recalculée chaque jour à partir de ce que tu fais sur la plateforme.", view.school_year)>
            {decision}
            <Facts>
                <Fact label="Ce mois-ci" value=level_text(view.current_level) />
                <Fact label="Sur l'année" value=headline_level(&view) />
                <Fact label="Crédits recommandés" value=format!("{} / 3", view.credits) />
            </Facts>
            <p class="ui-body">{next}</p>
            {floor}
            <DenseList label="Ce mois-ci, axe par axe">
                {axes.into_iter().map(|a| {
                    let (pct, caption) = match a.points {
                        Some(p) => (f64::from(p) * 100.0 / f64::from(a.max.max(1)), format!("{p} / {}", a.max)),
                        None => (0.0, "aucune séance ce mois-ci : ne compte pas".to_string()),
                    };
                    view! {
                        <ListRow title=a.label>
                            <XpProgress percent=pct caption />
                        </ListRow>
                    }
                }).collect_view()}
            </DenseList>
            {(!hints.is_empty()).then(move || view! {
                <DenseList label="Pour progresser">
                    {hints.into_iter().map(|h| view! {
                        <ListRow title="Pour progresser"><RowText text=h /></ListRow>
                    }).collect_view()}
                </DenseList>
            })}
            <DenseList label="Mois par mois">
                {months.into_iter().map(|m| {
                    let meta = if m.current { "30 derniers jours".to_string() } else { format!("score {} / 100", m.score) };
                    let end = view! { <RowValue text=format!("niveau {}", m.level) /> }.into_any();
                    view! { <ListRow title=m.label meta end /> }
                }).collect_view()}
            </DenseList>
        </Section>
    }
}

// ---------------------------------------------------------------------------
// /fiche
// ---------------------------------------------------------------------------

/// `/fiche`: fill in the sheet, see the involvement, export it.
#[component]
pub fn SheetPage() -> impl IntoView {
    let me = Resource::new(|| (), |()| async { get_my_involvement().await });
    view! {
        <Page pattern=Pattern::Detail>
            <PageHeader
                title="Ma fiche d'implication"
                lead="Ce que l'association remet à l'école en fin d'année. Remplis ton nom et ta promotion une fois ; le reste se calcule tout seul."
            >
                <ButtonLink kind=ButtonKind::Primary href="/fiche/imprimer" icon=IconName::Printer>"Exporter ma fiche (PDF)"</ButtonLink>
            </PageHeader>
            <Suspense fallback=|| view! { <RowsSkeleton rows=4 /> }>
                {move || me.get().map(|result| match result {
                    Err(_) => view! {
                        <ErrorState message="Impossible de charger ta fiche." on_retry=Callback::new(move |()| me.refetch()) />
                    }.into_any(),
                    Ok(None) => view! { <SignInState what="remplir ta fiche" /> }.into_any(),
                    Ok(Some(view)) => view! {
                        <IdentityForm view=view.clone() />
                        <InvolvementSummary view />
                    }.into_any(),
                })}
            </Suspense>
        </Page>
    }
}

/// Name and promotion, as they will be printed.
#[component]
#[allow(clippy::needless_pass_by_value)] // Leptos prop convention
fn IdentityForm(
    /// The member, as saved.
    view: InvolvementView,
) -> impl IntoView {
    let (first, set_first) = signal(view.first_name.clone().unwrap_or_default());
    let (last, set_last) = signal(view.last_name.clone().unwrap_or_default());
    let (promo, set_promo) = signal(view.promotion.clone().unwrap_or_else(|| "Tek1".into()));
    let (notice, set_notice) = signal(Option::<Result<(), String>>::None);
    let save = Action::new(move |(f, l, p): &(String, String, String)| {
        let (f, l, p) = (f.clone(), l.clone(), p.clone());
        async move { save_my_identity(f, l, p).await }
    });
    Effect::new(move |_| {
        if let Some(result) = save.value().get() {
            match result {
                Ok(()) => set_notice.set(Some(Ok(()))),
                Err(e) => set_notice.set(Some(Err(e.to_string()))),
            }
        }
    });
    let filled = view.first_name.is_some() && view.last_name.is_some() && view.promotion.is_some();
    view! {
        <Section title="Qui je suis" lead=if filled {
            "Tel que ce sera imprimé sur ta fiche. Ton pseudo Discord reste ton nom sur le reste de la plateforme."
        } else {
            "À remplir pour que ta fiche porte ton nom : seuls toi et le Bureau le voient."
        }>
            {move || notice.get().map(|n| match n {
                Ok(()) => view! { <Notice kind=NoticeKind::Success>"Fiche enregistrée."</Notice> }.into_any(),
                Err(message) => view! { <ErrorText message /> }.into_any(),
            })}
            <Form on:submit=move |ev| {
                ev.prevent_default();
                save.dispatch((first.get(), last.get(), promo.get()));
            }>
                <Field id="fiche-prenom" label="Prénom">
                    <input id="fiche-prenom" class="ui-control" type="text" required=true maxlength="80" autocomplete="given-name"
                        prop:value=move || first.get() on:input=move |ev| set_first.set(event_target_value(&ev)) />
                </Field>
                <Field id="fiche-nom" label="Nom">
                    <input id="fiche-nom" class="ui-control" type="text" required=true maxlength="80" autocomplete="family-name"
                        prop:value=move || last.get() on:input=move |ev| set_last.set(event_target_value(&ev)) />
                </Field>
                <Field id="fiche-promo" label="Promotion">
                    <select id="fiche-promo" class="ui-control" prop:value=move || promo.get()
                        on:change=move |ev| set_promo.set(event_target_value(&ev))>
                        {PROMOTIONS.iter().map(|(value, label)| view! { <option value=*value>{*label}</option> }).collect_view()}
                    </select>
                </Field>
                <FormActions>
                    <Button kind=ButtonKind::Primary button_type="submit" icon=IconName::Check
                        disabled=Signal::derive(move || save.pending().get())>
                        {move || if save.pending().get() { "Enregistrement…" } else { "Enregistrer ma fiche" }}
                    </Button>
                </FormActions>
            </Form>
        </Section>
    }
}

// ---------------------------------------------------------------------------
// /fiche/imprimer
// ---------------------------------------------------------------------------

/// `/fiche/imprimer`: the sheet as a plain document, to save as PDF.
#[component]
pub fn SheetPrintPage() -> impl IntoView {
    let query = use_query_map();
    let sheets = Resource::new(
        move || {
            let q = query.read();
            (q.get("membre"), q.get("tous").is_some())
        },
        |(member, all)| async move { get_sheets(member, all).await },
    );
    view! {
        <div class="ui-doc-page">
            <div class="ui-print-hide">
                <Cluster>
                    <Button kind=ButtonKind::Primary icon=IconName::Printer attr:onclick="window.print()">
                        "Enregistrer en PDF"
                    </Button>
                    <ButtonLink href="/fiche" icon=IconName::FileText>"Retour à ma fiche"</ButtonLink>
                </Cluster>
                <p class="ui-meta">"Dans la fenêtre d'impression, choisis « Enregistrer au format PDF »."</p>
            </div>
            <Suspense fallback=|| view! { <RowsSkeleton rows=6 /> }>
                {move || sheets.get().map(|result| match result {
                    Err(e) => view! { <ErrorState message=e.to_string() on_retry=Callback::new(move |()| sheets.refetch()) /> }.into_any(),
                    Ok(list) if list.is_empty() => view! { <EmptyState icon=IconName::FileText title="Aucune fiche" /> }.into_any(),
                    Ok(list) => list.into_iter().map(|v| view! { <SheetDocument view=v /> }).collect_view().into_any(),
                })}
            </Suspense>
        </div>
    }
}

/// One sheet: plain, black on white, one page.
#[component]
#[allow(clippy::needless_pass_by_value)] // Leptos prop convention
fn SheetDocument(
    /// The member.
    view: InvolvementView,
) -> impl IntoView {
    let name = full_name(&view);
    let responsibilities = if view.responsibilities.is_empty() {
        "Aucune".to_string()
    } else {
        view.responsibilities.join(", ")
    };
    let today = chrono::Utc::now().format("%d/%m/%Y").to_string();
    let decided = view.forced_level.filter(|_| !view.maximal).map(|l| {
        format!(
            "Niveau fixé à {l} / 5 par le président ou le vice-président (niveau calculé : {}). Motif : {}",
            level_text(view.computed_level),
            view.force_reason.clone().unwrap_or_default()
        )
    });
    view! {
        <article class="ui-doc">
            <header class="ui-doc__head">
                <p class="ui-doc__org">"GameCloud OS · association game-dev d'Epitech Bénin"</p>
                <h1 class="ui-doc__title">{format!("Fiche d'implication {}", view.school_year)}</h1>
            </header>
            <table class="ui-doc__table">
                <tbody>
                    <tr><th scope="row">"Nom et prénom"</th><td>{name}</td></tr>
                    <tr><th scope="row">"Promotion"</th><td>{view.promotion.clone().unwrap_or_else(|| "Non renseignée".into())}</td></tr>
                    <tr><th scope="row">"Pseudo Discord"</th><td>{view.username.clone()}</td></tr>
                    <tr><th scope="row">"Responsabilités"</th><td>{responsibilities}</td></tr>
                    <tr><th scope="row">"Niveau d'implication sur l'année"</th><td><strong>{headline_level(&view)}</strong></td></tr>
                    <tr><th scope="row">"Crédits recommandés"</th><td><strong>{format!("{} / 3", view.credits)}</strong></td></tr>
                </tbody>
            </table>
            {decided.map(|d| view! { <p class="ui-doc__note">{d}</p> })}

            <h2 class="ui-doc__h2">"Remarques du président"</h2>
            <p class="ui-doc__remarks">{view.remarks.clone().unwrap_or_else(|| "—".into())}</p>

            <footer class="ui-doc__foot">
                <p>{format!("Édité le {today}. Le niveau est calculé chaque mois à partir de l'activité enregistrée par la plateforme : production (35 points), présence aux séances (30), régularité (25) et entraide (10). Les crédits sont une recommandation de l'association ; l'administration en décide.")}</p>
            </footer>
        </article>
    }
}

// ---------------------------------------------------------------------------
// The Bureau's board
// ---------------------------------------------------------------------------

/// Every member's involvement, for the Bureau page.
#[component]
pub fn InvolvementBoardPanel() -> impl IntoView {
    let board = Resource::new(|| (), |()| async { get_involvement_board().await });
    let (saved, set_saved) = signal(Option::<String>::None);
    let refresh = Callback::new(move |who: String| {
        set_saved.set(Some(who));
        board.refetch();
    });
    view! {
        {move || saved.get().map(|who| view! {
            <Notice kind=NoticeKind::Success>{format!("Décision enregistrée pour @{who} : sa fiche est à jour.")}</Notice>
        })}
        <Suspense fallback=|| view! { <RowsSkeleton rows=4 /> }>
            {move || board.get().map(|result| match result {
                Err(e) => view! { <ErrorState message=e.to_string() on_retry=Callback::new(move |()| board.refetch()) /> }.into_any(),
                Ok(b) => view! { <BoardBody board=b on_changed=refresh /> }.into_any(),
            })}
        </Suspense>
    }
}

#[component]
#[allow(clippy::needless_pass_by_value)] // Leptos prop convention
fn BoardBody(board: InvolvementBoard, on_changed: Callback<String>) -> impl IntoView {
    let action = view! {
        <ButtonLink href="/fiche/imprimer?tous=1" icon=IconName::Printer>"Toutes les fiches (PDF)"</ButtonLink>
    }
    .into_any();
    let lead = if board.can_decide {
        "Calculé chaque jour. Tu peux forcer le niveau de l'année d'un membre (avec un motif) et écrire les remarques imprimées sur sa fiche."
    } else {
        "Calculé chaque jour. Seuls le président et le vice-président peuvent forcer un niveau ou écrire les remarques."
    };
    let can_decide = board.can_decide;
    view! {
        <Section title=format!("Implication {}", board.school_year) lead action>
            {if board.rows.is_empty() {
                view! { <EmptyState icon=IconName::Users title="Aucun membre pour l'instant" /> }.into_any()
            } else {
                view! {
                    <DenseList label="Implication des membres">
                        {board.rows.into_iter().map(|row| view! { <BoardRow row can_decide on_changed /> }).collect_view()}
                    </DenseList>
                }.into_any()
            }}
        </Section>
    }
}

#[component]
#[allow(clippy::needless_pass_by_value)] // Leptos prop convention
fn BoardRow(row: InvolvementView, can_decide: bool, on_changed: Callback<String>) -> impl IntoView {
    let promotion = row.promotion.as_deref().map(|p| format!(", {p}")).unwrap_or_default();
    let meta = format!("@{}{promotion}, ce mois-ci {}", row.username, level_text(row.current_level));
    let credits_tag = if row.credits > 0 {
        view! { <Tag kind=TagKind::Accent>{format!("{} crédit{}", row.credits, if row.credits > 1 { "s" } else { "" })}</Tag> }.into_any()
    } else {
        view! { <Tag>"0 crédit"</Tag> }.into_any()
    };
    let maximal_tag = row.maximal.then(|| view! { <Tag kind=TagKind::Accent icon=IconName::Crown>"Implication maximale"</Tag> });
    let end = view! {
        {maximal_tag}
        <RowValue text=format!("année {}", level_text(row.level)) />
        {credits_tag}
        <ButtonLink kind=ButtonKind::Ghost href=format!("/fiche/imprimer?membre={}", row.username) icon=IconName::FileText>"Fiche"</ButtonLink>
    }
    .into_any();
    let forced = row.forced_level.map(|l| {
        view! { <RowText text=format!("Niveau fixé à {l} / 5 : {}", row.force_reason.clone().unwrap_or_default()) /> }
    });
    let missing = (row.first_name.is_none() || row.promotion.is_none())
        .then(|| view! { <p class="ui-meta">"Fiche pas encore remplie (nom ou promotion manquant)."</p> });
    view! {
        <ListRow title=full_name(&row) meta end>
            {forced}
            {missing}
            {can_decide.then(|| view! { <DecideForm row=row.clone() on_changed /> })}
        </ListRow>
    }
}

/// Force the year's level and write the remarks. President and Vice-President.
#[component]
#[allow(clippy::needless_pass_by_value)] // Leptos prop convention
fn DecideForm(row: InvolvementView, on_changed: Callback<String>) -> impl IntoView {
    let username = StoredValue::new(row.username.clone());
    let (level, set_level) = signal(row.forced_level.map_or_else(|| "0".to_string(), |l| l.to_string()));
    let (reason, set_reason) = signal(row.force_reason.clone().unwrap_or_default());
    let (remarks, set_remarks) = signal(row.remarks.clone().unwrap_or_default());
    let (maximal, set_maximal) = signal(row.maximal_by_name);
    // The two offices hold it by right: nothing to grant or lift.
    let by_office = row.maximal && !row.maximal_by_name;
    let (notice, set_notice) = signal(Option::<Result<(), String>>::None);
    let save = Action::new(move |(l, r, m, x): &(u8, String, String, bool)| {
        let (l, r, m, x) = (*l, r.clone(), m.clone(), *x);
        async move { save_involvement_review(username.get_value(), l, r, m, x).await }
    });
    Effect::new(move |_| {
        if let Some(result) = save.value().get() {
            match result {
                Ok(()) => {
                    set_notice.set(Some(Ok(())));
                    on_changed.run(username.get_value());
                }
                Err(e) => set_notice.set(Some(Err(e.to_string()))),
            }
        }
    });
    let ids = (
        format!("niveau-{}", row.username),
        format!("motif-{}", row.username),
        format!("remarques-{}", row.username),
        format!("maximale-{}", row.username),
    );
    view! {
        <Disclosure summary="Décider pour l'année">
            {move || notice.get().map(|n| match n {
                Ok(()) => view! { <p class="ui-meta">"Enregistré."</p> }.into_any(),
                Err(message) => view! { <ErrorText message /> }.into_any(),
            })}
            <Form on:submit=move |ev| {
                ev.prevent_default();
                let l = level.get().parse::<u8>().unwrap_or(0);
                save.dispatch((l, reason.get(), remarks.get(), maximal.get()));
            }>
                <Field id=ids.3.clone() label="Implication maximale garantie" wide=true
                    hint=if by_office { "Acquise par l'office de président ou de vice-président." } else { "Niveau 5 et 3 crédits, quoi qu'il arrive ; l'analyse reste calculée normalement." }>
                    <input id=ids.3.clone() type="checkbox" disabled=by_office
                        prop:checked=move || by_office || maximal.get()
                        on:change=move |ev| set_maximal.set(event_target_checked(&ev)) />
                </Field>
                <Field id=ids.0.clone() label="Niveau de l'année">
                    <select id=ids.0.clone() class="ui-control" prop:value=move || level.get()
                        on:change=move |ev| set_level.set(event_target_value(&ev))>
                        <option value="0">"Calculé par la plateforme"</option>
                        {(1..=5).map(|l| view! { <option value=l.to_string()>{format!("Forcer au niveau {l}")}</option> }).collect_view()}
                    </select>
                </Field>
                <Field id=ids.1.clone() label="Motif (obligatoire pour forcer)" wide=true>
                    <input id=ids.1.clone() class="ui-control" type="text" maxlength="300"
                        prop:value=move || reason.get() on:input=move |ev| set_reason.set(event_target_value(&ev)) />
                </Field>
                <Field id=ids.2.clone() label="Remarques imprimées sur la fiche" wide=true>
                    <textarea id=ids.2.clone() class="ui-control" rows="3" maxlength="2000"
                        prop:value=move || remarks.get() on:input=move |ev| set_remarks.set(event_target_value(&ev))></textarea>
                </Field>
                <FormActions>
                    <Button kind=ButtonKind::Primary button_type="submit" icon=IconName::Check
                        disabled=Signal::derive(move || save.pending().get())>
                        "Enregistrer"
                    </Button>
                </FormActions>
            </Form>
        </Disclosure>
    }
}

/// The profile's "Implication" tab: the summary, and the way to the sheet.
#[component]
pub fn MyInvolvementPanel() -> impl IntoView {
    let me = Resource::new(|| (), |()| async { get_my_involvement().await });
    view! {
        <Suspense fallback=|| view! { <RowsSkeleton rows=3 /> }>
            {move || me.get().map(|result| match result {
                Err(_) => view! {
                    <ErrorState message="Impossible de calculer ton implication." on_retry=Callback::new(move |()| me.refetch()) />
                }.into_any(),
                Ok(None) => ().into_any(),
                Ok(Some(view)) => view! {
                    <InvolvementSummary view />
                    <Cluster>
                        <ButtonLink href="/fiche" icon=IconName::FileText>"Ma fiche d'implication"</ButtonLink>
                    </Cluster>
                }.into_any(),
            })}
        </Suspense>
    }
}

//! Game jams — the list, and one jam from registration to results.
//!
//! `/jams` lists every jam as a card; the Bureau opens one there. `/jams/:id`
//! follows a jam through its phases (registration, the jam, the vote, the
//! results): the theme once revealed, the reward categories, the viewer's
//! team and their game, every team, the vote, then the winners.

use leptos::prelude::*;
use leptos_router::hooks::{use_params_map, use_query_map};

use crate::{
    api::{JamCard, JamTeamItem, JamView},
    components::ui::{
        Button, ButtonKind, ButtonLink, Card, CardGrid, Cluster, DenseList, EmptyState, ErrorState,
        ErrorText, Fact, Facts, Field, Form, FormActions, IconName, Journey, ListRow, Notice,
        NoticeKind, Page, PageHeader, Panel, Pattern, RowText, RowValue, RowsSkeleton, Section,
        SignInState, StepState, Tag, TagKind,
    },
    server_fns::{
        add_jam_member, create_jam, create_jam_team, delete_jam, get_jam, get_jams, leave_jam_team,
        publish_jam, vote_jam,
    },
};

/// Suggested categories for a new jam: "Nom | récompense | XP", one per line.
const DEFAULT_CATEGORIES: &str = "Meilleur jeu | Goodies | 300\nGameplay | | 200\nDirection artistique | | 200\nSon et musique | | 150\nCoup de cœur | Priorité sur le matériel | 150";

fn phase_journey(phase: &str) -> Vec<(&'static str, StepState)> {
    use StepState::{Current, Done, Todo};
    let at = match phase {
        "Registration" => 0,
        "Running" => 1,
        "Voting" | "Deliberation" => 2,
        _ => 3,
    };
    ["Inscriptions", "Jam en cours", "Vote", "Résultats"]
        .into_iter()
        .enumerate()
        .map(|(i, name)| {
            let state = if i < at || (i == 3 && at == 3) {
                Done
            } else if i == at {
                Current
            } else {
                Todo
            };
            (name, state)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// /jams
// ---------------------------------------------------------------------------

/// `/jams`: every jam.
#[component]
pub fn JamsPage() -> impl IntoView {
    let jams = Resource::new(|| (), |()| async { get_jams().await });
    let (opening, set_opening) = signal(false);
    view! {
        <Page pattern=Pattern::Grid>
            <PageHeader title="Game jams" lead="Un thème, une date limite, des équipes : fais un jeu, rends-le, puis vote pour celui des autres.">
                <Suspense fallback=|| ()>
                    {move || jams.get().and_then(Result::ok).filter(|v| v.can_organise).map(|_| view! {
                        <Button kind=ButtonKind::Primary icon=IconName::Plus on:click=move |_| set_opening.update(|o| *o = !*o)>
                            "Organiser une jam"
                        </Button>
                    })}
                </Suspense>
            </PageHeader>
            <Show when=move || opening.get()>
                <Section title="Organiser une jam" lead="Le thème reste caché jusqu'au début. Les horaires s'entendent comme tu les saisis.">
                    <CreateJamForm on_created=Callback::new(move |()| { set_opening.set(false); jams.refetch(); }) />
                </Section>
            </Show>
            <Suspense fallback=|| view! { <RowsSkeleton rows=3 /> }>
                {move || jams.get().map(|result| match result {
                    Err(_) => view! { <SignInState what="voir les game jams" /> }.into_any(),
                    Ok(v) if !v.is_member && !v.can_organise => view! {
                        <EmptyState icon=IconName::LockSimple title="Réservé aux membres" body="Les game jams sont ouvertes aux membres de l'association." />
                    }.into_any(),
                    Ok(v) if v.jams.is_empty() => view! {
                        <EmptyState icon=IconName::GameController title="Aucune jam pour l'instant" body="La prochaine sera annoncée ici et sur Discord." />
                    }.into_any(),
                    Ok(v) => view! {
                        <CardGrid>
                            {v.jams.into_iter().map(|j| view! { <JamCardView jam=j /> }).collect_view()}
                        </CardGrid>
                    }.into_any(),
                })}
            </Suspense>
        </Page>
    }
}

#[component]
#[allow(clippy::needless_pass_by_value)] // Leptos prop convention
fn JamCardView(jam: JamCard) -> impl IntoView {
    let meta = format!("{} équipes, {} jeux rendus", jam.teams, jam.entries);
    let body = jam.theme.clone().map_or_else(
        || format!("Thème dévoilé le {}.", jam.starts),
        |t| format!("Thème : {t}"),
    );
    let kicker = match &jam.time_left {
        Some(left) => format!("{}, {left}", jam.phase_label),
        None => jam.phase_label.clone(),
    };
    view! {
        <Card kicker title=jam.title.clone() icon=IconName::GameController href=format!("/jams/{}", jam.id) meta>
            {body}
        </Card>
    }
}

#[component]
fn CreateJamForm(on_created: Callback<()>) -> impl IntoView {
    let fields: [(RwSignal<String>, &str); 10] = [
        (RwSignal::new(String::new()), "title"),
        (RwSignal::new(String::new()), "theme"),
        (RwSignal::new(String::new()), "description"),
        (RwSignal::new(String::new()), "rules"),
        (RwSignal::new(String::new()), "starts"),
        (RwSignal::new(String::new()), "ends"),
        (RwSignal::new(String::new()), "voting_ends"),
        (RwSignal::new("4".to_string()), "team"),
        (RwSignal::new("100".to_string()), "xp"),
        (RwSignal::new(DEFAULT_CATEGORIES.to_string()), "categories"),
    ];
    let [title, theme, description, rules, starts, ends, voting, team, xp, cats] = fields.map(|(s, _)| s);
    let (error, set_error) = signal(Option::<String>::None);
    let create = Action::new(move |(): &()| async move {
        create_jam(
            title.get_untracked(),
            description.get_untracked(),
            rules.get_untracked(),
            theme.get_untracked(),
            starts.get_untracked(),
            ends.get_untracked(),
            voting.get_untracked(),
            team.get_untracked().parse().unwrap_or(4),
            xp.get_untracked().parse().unwrap_or(100),
            cats.get_untracked(),
        )
        .await
    });
    Effect::new(move |_| {
        if let Some(result) = create.value().get() {
            match result {
                Ok(_) => {
                    set_error.set(None);
                    on_created.run(());
                }
                Err(e) => set_error.set(Some(e.to_string())),
            }
        }
    });
    let text = |id: &'static str, label: &'static str, s: RwSignal<String>, wide: bool, kind: &'static str| {
        view! {
            <Field id=id label=label wide=wide>
                <input id=id class="ui-control" type=kind required=true
                    prop:value=move || s.get() on:input=move |ev| s.set(event_target_value(&ev)) />
            </Field>
        }
    };
    view! {
        {move || error.get().map(|message| view! { <ErrorText message /> })}
        <Form on:submit=move |ev| { ev.prevent_default(); create.dispatch(()); }>
            {text("jam-titre", "Titre", title, true, "text")}
            {text("jam-theme", "Thème (caché jusqu'au début)", theme, true, "text")}
            <Field id="jam-description" label="Présentation (optionnel)" wide=true>
                <textarea id="jam-description" class="ui-control" rows="2" maxlength="2000"
                    prop:value=move || description.get() on:input=move |ev| description.set(event_target_value(&ev))></textarea>
            </Field>
            <Field id="jam-regles" label="Règles (optionnel)" wide=true>
                <textarea id="jam-regles" class="ui-control" rows="3" maxlength="4000"
                    prop:value=move || rules.get() on:input=move |ev| rules.set(event_target_value(&ev))></textarea>
            </Field>
            {text("jam-debut", "Début", starts, false, "datetime-local")}
            {text("jam-fin", "Fin des rendus", ends, false, "datetime-local")}
            {text("jam-vote", "Fin du vote", voting, false, "datetime-local")}
            {text("jam-equipe", "Taille maximale d'une équipe", team, false, "number")}
            {text("jam-xp", "XP de participation", xp, false, "number")}
            <Field id="jam-categories" label="Catégories de récompense" wide=true
                hint="Une par ligne : nom | récompense | XP pour chaque membre de l'équipe gagnante.">
                <textarea id="jam-categories" class="ui-control" rows="5" aria-describedby="jam-categories-hint"
                    prop:value=move || cats.get() on:input=move |ev| cats.set(event_target_value(&ev))></textarea>
            </Field>
            <FormActions>
                <Button kind=ButtonKind::Primary button_type="submit" icon=IconName::GameController
                    disabled=Signal::derive(move || create.pending().get())>
                    {move || if create.pending().get() { "Ouverture…" } else { "Ouvrir la jam" }}
                </Button>
            </FormActions>
        </Form>
    }
}

// ---------------------------------------------------------------------------
// /jams/:id
// ---------------------------------------------------------------------------

/// `/jams/:id`: one jam.
#[component]
pub fn JamPage() -> impl IntoView {
    let params = use_params_map();
    let query = use_query_map();
    let id = move || params.read().get("id").unwrap_or_default();
    let jam = Resource::new(id, |id| async move { get_jam(id).await });
    let refresh = Callback::new(move |()| jam.refetch());
    view! {
        <Page pattern=Pattern::Detail>
            {move || query.read().get("ok").map(|_| view! {
                <Notice kind=NoticeKind::Success>"Jeu rendu. Tu peux le remplacer jusqu'à la fin des rendus."</Notice>
            })}
            {move || query.read().get("erreur").map(|e| view! { <Notice kind=NoticeKind::Error>{e}</Notice> })}
            <Suspense fallback=|| view! { <RowsSkeleton rows=5 /> }>
                {move || jam.get().map(|result| match result {
                    Err(e) => view! {
                        <PageHeader title="Game jam" />
                        <ErrorState message=e.to_string() on_retry=Callback::new(move |()| jam.refetch()) />
                    }.into_any(),
                    Ok(v) => view! { <JamBody jam=v on_changed=refresh /> }.into_any(),
                })}
            </Suspense>
        </Page>
    }
}

#[component]
#[allow(clippy::needless_pass_by_value)] // Leptos prop convention
fn JamBody(jam: JamView, on_changed: Callback<()>) -> impl IntoView {
    let phase = jam.card.phase.clone();
    let id = StoredValue::new(jam.card.id.clone());
    let (error, set_error) = signal(Option::<String>::None);
    let publish = Action::new(move |(): &()| async move { publish_jam(id.get_value()).await });
    let remove = Action::new(move |(): &()| async move { delete_jam(id.get_value()).await });
    Effect::new(move |_| {
        if let Some(r) = publish.value().get() {
            match r {
                Ok(()) => on_changed.run(()),
                Err(e) => set_error.set(Some(e.to_string())),
            }
        }
    });
    Effect::new(move |_| {
        if let Some(r) = remove.value().get() {
            match r {
                Ok(()) => {
                    if let Some(w) = web_sys_window() {
                        let _ = w.location().set_href("/jams");
                    }
                }
                Err(e) => set_error.set(Some(e.to_string())),
            }
        }
    });

    let actions = jam.can_organise.then(|| match phase.as_str() {
        "Deliberation" => view! {
            <Button kind=ButtonKind::Primary icon=IconName::Trophy disabled=Signal::derive(move || publish.pending().get())
                on:click=move |_| { publish.dispatch(()); }>"Publier les résultats"</Button>
        }.into_any(),
        "Registration" => view! {
            <Button kind=ButtonKind::Ghost icon=IconName::Trash disabled=Signal::derive(move || remove.pending().get())
                on:click=move |_| { remove.dispatch(()); }>"Supprimer la jam"</Button>
        }.into_any(),
        _ => ().into_any(),
    });

    let theme = match jam.card.theme.clone() {
        Some(t) => view! {
            <section class="ui-hero-panel" aria-label="Thème">
                <span class="ui-label">"Thème"</span>
                <h2 class="ui-display">{t}</h2>
            </section>
        }.into_any(),
        None => view! {
            <Notice>{format!("Le thème sera dévoilé le {}, au début de la jam.", jam.card.starts)}</Notice>
        }.into_any(),
    };
    let lead = jam.description.clone().unwrap_or_else(|| "Une game jam de l'association.".into());
    let my_team = jam.teams.iter().find(|t| t.mine).cloned();
    let teams_open = matches!(phase.as_str(), "Registration" | "Running");
    let voting = phase == "Voting";

    let title = jam.card.title.clone();
    let (starts, ends, voting_ends) = (jam.card.starts.clone(), jam.card.ends.clone(), jam.card.voting_ends.clone());
    let results = (phase == "Results").then(|| view! { <Results jam=jam.clone() /> });
    let vote = voting.then(|| view! { <Vote jam=jam.clone() /> });
    let categories_lead = format!(
        "Chaque membre d'une équipe qui rend un jeu gagne {} XP et le badge Jammeur. Les gagnants reçoivent en plus l'XP de leur catégorie et le badge Vainqueur de Jam.",
        jam.participation_xp
    );
    let categories = jam
        .categories
        .iter()
        .map(|c| {
            let end = view! { <RowValue text=format!("+{} XP", c.xp_reward) /> }.into_any();
            view! { <ListRow title=c.name.clone() meta=c.reward.clone().unwrap_or_default() end /> }
        })
        .collect_view();
    let rules = jam.rules.clone().map(|r| view! { <Section title="Règles"><p class="ui-body ui-pre">{r}</p></Section> });
    let submissions_open = phase == "Running";
    let mine = (jam.is_member && (teams_open || my_team.is_some())).then(|| {
        view! {
            <MyTeam jam_id=jam.card.id.clone() team=my_team.clone() max=jam.max_team_size
                teams_open submissions_open on_changed />
        }
    });
    let team_count = format!("{} équipes", jam.teams.len());
    let teams = if jam.teams.is_empty() {
        view! { <EmptyState icon=IconName::Users title="Aucune équipe pour l'instant" body="Crée la tienne : seul ou à plusieurs." /> }.into_any()
    } else {
        let rows = jam.teams.iter().map(|t| view! { <TeamRow team=t.clone() /> }).collect_view();
        view! { <DenseList label="Équipes">{rows}</DenseList> }.into_any()
    };
    let journey = phase_journey(&phase);

    view! {
        <PageHeader title lead>{actions}</PageHeader>
        {move || error.get().map(|message| view! { <ErrorText message /> })}
        <Journey label="Où en est la jam" steps=journey />
        {theme}
        <Facts>
            <Fact label="Début" value=starts />
            <Fact label="Fin des rendus" value=ends />
            <Fact label="Fin du vote" value=voting_ends />
        </Facts>
        {results}
        <Section title="Catégories et récompenses" lead=categories_lead>
            <DenseList label="Catégories">{categories}</DenseList>
        </Section>
        {rules}
        {mine}
        {vote}
        <Section title="Équipes" meta=team_count>{teams}</Section>
    }
}

/// `window`, in the browser only.
fn web_sys_window() -> Option<leptos::web_sys::Window> {
    #[cfg(feature = "hydrate")]
    {
        leptos::web_sys::window()
    }
    #[cfg(not(feature = "hydrate"))]
    {
        None
    }
}

#[component]
#[allow(clippy::needless_pass_by_value)] // Leptos prop convention
fn TeamRow(team: JamTeamItem) -> impl IntoView {
    let meta = team.members.iter().map(|m| format!("@{m}")).collect::<Vec<_>>().join(", ");
    let end = match &team.entry {
        Some(_) => view! { <Tag kind=TagKind::Accent icon=IconName::CheckCircle>"Jeu rendu"</Tag> }.into_any(),
        None => view! { <Tag icon=IconName::Hourglass>"En création"</Tag> }.into_any(),
    };
    let entry = team.entry.clone().map(|e| {
        let id = team.id.clone();
        view! {
            <RowText text=format!("{} (rendu le {})", e.title, e.when) />
            {e.description.map(|d| view! { <RowText text=d /> })}
            <Cluster>
                {e.url.map(|u| view! { <ButtonLink href=u new_tab=true icon=IconName::ArrowSquareOut>"Jouer"</ButtonLink> })}
                {e.file.map(|f| view! {
                    <ButtonLink href=format!("/api/jams/entries/{id}/file") external=true icon=IconName::DownloadSimple>{format!("Télécharger : {f}")}</ButtonLink>
                })}
            </Cluster>
        }
    });
    view! { <ListRow title=team.name.clone() meta end>{entry}</ListRow> }
}

#[component]
#[allow(clippy::needless_pass_by_value)] // Leptos prop convention
fn MyTeam(
    jam_id: String,
    team: Option<JamTeamItem>,
    max: i16,
    teams_open: bool,
    submissions_open: bool,
    on_changed: Callback<()>,
) -> impl IntoView {
    let id = StoredValue::new(jam_id.clone());
    let (name, set_name) = signal(String::new());
    let (invite, set_invite) = signal(String::new());
    let (error, set_error) = signal(Option::<String>::None);
    let (sending, set_sending) = signal(false);
    let create = Action::new(move |n: &String| {
        let n = n.clone();
        async move { create_jam_team(id.get_value(), n).await }
    });
    let add = Action::new(move |u: &String| {
        let u = u.clone();
        async move { add_jam_member(id.get_value(), u).await }
    });
    let leave = Action::new(move |(): &()| async move { leave_jam_team(id.get_value()).await });
    for action in [create, add] {
        Effect::new(move |_| {
            if let Some(r) = action.value().get() {
                match r {
                    Ok(()) => {
                        set_error.set(None);
                        set_invite.set(String::new());
                        on_changed.run(());
                    }
                    Err(e) => set_error.set(Some(e.to_string())),
                }
            }
        });
    }
    Effect::new(move |_| {
        if let Some(r) = leave.value().get() {
            match r {
                Ok(()) => on_changed.run(()),
                Err(e) => set_error.set(Some(e.to_string())),
            }
        }
    });

    let body = match team {
        None => view! {
            <Form on:submit=move |ev| { ev.prevent_default(); create.dispatch(name.get()); }>
                <Field id="equipe-nom" label="Nom de ton équipe" wide=true hint="Tu pourras ajouter des coéquipiers ensuite, ou faire la jam seul.">
                    <input id="equipe-nom" class="ui-control" type="text" required=true maxlength="60" aria-describedby="equipe-nom-hint"
                        prop:value=move || name.get() on:input=move |ev| set_name.set(event_target_value(&ev)) />
                </Field>
                <FormActions>
                    <Button kind=ButtonKind::Primary button_type="submit" icon=IconName::Plus disabled=Signal::derive(move || create.pending().get())>
                        "Créer mon équipe"
                    </Button>
                </FormActions>
            </Form>
        }.into_any(),
        Some(team) => {
            let full = i16::try_from(team.members.len()).unwrap_or(i16::MAX) >= max;
            let members = team.members.iter().map(|m| format!("@{m}")).collect::<Vec<_>>().join(", ");
            let entry = team.entry.clone();
            let submit_to = format!("/api/jams/{jam_id}/entry");
            view! {
                <p class="ui-body">{format!("{} ({} / {max}) : {members}", team.name, team.members.len())}</p>
                {(teams_open && !full).then(|| view! {
                    <Form on:submit=move |ev| { ev.prevent_default(); add.dispatch(invite.get()); }>
                        <Field id="equipe-ajout" label="Ajouter un coéquipier (pseudo Discord)">
                            <input id="equipe-ajout" class="ui-control" type="text" required=true
                                prop:value=move || invite.get() on:input=move |ev| set_invite.set(event_target_value(&ev)) />
                        </Field>
                        <FormActions>
                            <Button button_type="submit" icon=IconName::Plus disabled=Signal::derive(move || add.pending().get())>"Ajouter"</Button>
                        </FormActions>
                    </Form>
                })}
                {entry.clone().map(|e| view! {
                    <p class="ui-meta">{format!("Jeu rendu : {} (le {}). Un nouvel envoi le remplace.", e.title, e.when)}</p>
                })}
                {submissions_open.then(|| view! {
                    <Panel>
                        <h3 class="ui-h3">{if entry.is_some() { "Remplacer notre jeu" } else { "Rendre notre jeu" }}</h3>
                        <Form attr:method="post" attr:action=submit_to attr:enctype="multipart/form-data"
                            on:submit=move |_| set_sending.set(true)>
                            <Field id="rendu-titre" label="Titre du jeu" wide=true>
                                <input id="rendu-titre" class="ui-control" type="text" name="title" required=true maxlength="120" />
                            </Field>
                            <Field id="rendu-texte" label="Présentation (optionnel)" wide=true>
                                <textarea id="rendu-texte" class="ui-control" name="description" rows="2" maxlength="2000"></textarea>
                            </Field>
                            <Field id="rendu-lien" label="Lien pour jouer (itch.io…)" hint="Un lien, un fichier, ou les deux.">
                                <input id="rendu-lien" class="ui-control" type="url" name="url" placeholder="https://" aria-describedby="rendu-lien-hint" />
                            </Field>
                            <Field id="rendu-fichier" label="Fichier du jeu (500 Mo maximum)">
                                <input id="rendu-fichier" class="ui-control" type="file" name="file" />
                            </Field>
                            <FormActions>
                                <Button kind=ButtonKind::Primary button_type="submit" icon=IconName::UploadSimple disabled=Signal::derive(move || sending.get())>
                                    {move || if sending.get() { "Envoi en cours…" } else { "Rendre le jeu" }}
                                </Button>
                            </FormActions>
                        </Form>
                    </Panel>
                })}
                {teams_open.then(|| view! {
                    <Cluster>
                        <Button kind=ButtonKind::Ghost icon=IconName::SignOut disabled=Signal::derive(move || leave.pending().get())
                            on:click=move |_| { leave.dispatch(()); }>"Quitter l'équipe"</Button>
                    </Cluster>
                })}
            }.into_any()
        }
    };
    view! {
        <Section title="Mon équipe">
            {move || error.get().map(|message| view! { <ErrorText message /> })}
            {body}
        </Section>
    }
}

#[component]
#[allow(clippy::needless_pass_by_value)] // Leptos prop convention
fn Vote(jam: JamView) -> impl IntoView {
    let jam_id = StoredValue::new(jam.card.id.clone());
    let games: Vec<JamTeamItem> = jam.teams.iter().filter(|t| t.entry.is_some() && !t.mine).cloned().collect();
    let lead = if jam.can_organise {
        "Tu votes comme membre du jury : le jury compte pour moitié dans chaque catégorie."
    } else {
        "Note chaque jeu de 1 à 5 dans chaque catégorie. Le vote des membres compte pour moitié, celui du jury du Bureau pour l'autre moitié."
    };
    if games.is_empty() {
        return view! {
            <Section title="Vote" lead>
                <EmptyState icon=IconName::GameController title="Aucun jeu à noter" body="Aucune autre équipe n'a rendu de jeu." />
            </Section>
        }.into_any();
    }
    let categories = jam.categories.clone();
    let scores = jam.my_scores.clone();
    view! {
        <Section title="Vote" lead>
            <DenseList label="Jeux à noter">
                {games.into_iter().map(|g| {
                    let team_id = g.id.clone();
                    let title = g.entry.as_ref().map_or_else(|| g.name.clone(), |e| format!("{} ({})", e.title, g.name));
                    let selects = categories.iter().map(|c| {
                        let current = scores.iter().find(|s| s.team_id == team_id && s.category_id == c.id).map(|s| s.score);
                        let (saved, set_saved) = signal(current);
                        let (error, set_error) = signal(Option::<String>::None);
                        let (tid, cid) = (team_id.clone(), c.id.clone());
                        let cast = Action::new(move |score: &u8| {
                            let (tid, cid, score) = (tid.clone(), cid.clone(), *score);
                            async move { vote_jam(jam_id.get_value(), tid, cid, score).await.map(|()| score) }
                        });
                        Effect::new(move |_| {
                            if let Some(r) = cast.value().get() {
                                match r {
                                    Ok(s) => { set_saved.set(Some(s)); set_error.set(None); }
                                    Err(e) => set_error.set(Some(e.to_string())),
                                }
                            }
                        });
                        let field_id = format!("note-{}-{}", team_id, c.id);
                        view! {
                            <Field id=field_id.clone() label=c.name.clone()>
                                <select id=field_id class="ui-control"
                                    prop:value=move || saved.get().map(|s| s.to_string()).unwrap_or_default()
                                    on:change=move |ev| {
                                        if let Ok(s) = event_target_value(&ev).parse::<u8>() { cast.dispatch(s); }
                                    }>
                                    <option value="">"Pas encore noté"</option>
                                    {(1..=5).map(|n| view! { <option value=n.to_string()>{format!("{n} / 5")}</option> }).collect_view()}
                                </select>
                            </Field>
                            {move || error.get().map(|message| view! { <ErrorText message /> })}
                        }
                    }).collect_view();
                    view! {
                        <ListRow title>
                            <div class="ui-form">{selects}</div>
                        </ListRow>
                    }
                }).collect_view()}
            </DenseList>
        </Section>
    }.into_any()
}

#[component]
#[allow(clippy::needless_pass_by_value)] // Leptos prop convention
fn Results(jam: JamView) -> impl IntoView {
    view! {
        <Section title="Résultats" lead="Moitié jury du Bureau, moitié vote des membres.">
            <DenseList label="Gagnants">
                {jam.results.into_iter().map(|r| {
                    let end = match &r.team {
                        Some(_) => view! { <Tag kind=TagKind::Accent icon=IconName::Trophy>{r.score.clone().unwrap_or_default()}</Tag> }.into_any(),
                        None => view! { <Tag>"Aucun vote"</Tag> }.into_any(),
                    };
                    let meta = r.team.clone().map_or_else(
                        || "Personne n'a voté dans cette catégorie.".to_string(),
                        |t| format!("{t} : {}", r.members.iter().map(|m| format!("@{m}")).collect::<Vec<_>>().join(", ")),
                    );
                    let reward = r.reward.clone().map(|x| format!("{x}, ")).unwrap_or_default();
                    view! {
                        <ListRow title=r.category.clone() meta end>
                            <RowText text=format!("Récompense : {reward}+{} XP", r.xp_reward) />
                        </ListRow>
                    }
                }).collect_view()}
            </DenseList>
        </Section>
    }
}

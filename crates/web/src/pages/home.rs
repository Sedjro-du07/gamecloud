//! Home.
//!
//! Signed in, a dashboard: the member's titles, XP and the bar to the next
//! title, their streak, their quests in progress and the next session.
//! Signed out, what the association is and the three ways in — join, take
//! the entrance test, write to Kumo — then how progression works.

use gamecloud_shared::roles::Track;
use leptos::prelude::*;

use crate::{
    api::{format_xp, MeView, QuestItem},
    components::ui::{
        vocab::{plain_title, track_icon, track_label},
        xp::{rank_percent, streak_caption, xp_caption},
        ButtonKind, ButtonLink, Card, CardGrid, CardSkeleton, Cluster, DenseList, Disclosure,
        EmptyState, ErrorState, Fact, Facts, Gap, IconName, RowsSkeleton, Section, Split, Stack,
        Step, Steps, StreakMarks, Tag, TagKind, TitleLadder, XpProgress,
    },
    pages::quests::QuestRow,
    server_fns::{get_me, get_quests, get_upcoming},
};

/// Quests shown on the dashboard; the rest are one click away.
const DASHBOARD_QUESTS: usize = 5;

/// Home page.
#[component]
pub fn HomePage() -> impl IntoView {
    let me = Resource::new(|| (), |()| async { get_me().await });
    view! {
        <Suspense fallback=|| view! { <DashboardSkeleton /> }>
            {move || {
                me.get()
                    .map(|result| match result {
                        Ok(Some(user)) => view! { <Dashboard me=user /> }.into_any(),
                        Ok(None) => view! { <Welcome /> }.into_any(),
                        Err(_) => {
                            view! {
                                <ErrorState
                                    message="Impossible de charger l'accueil."
                                    on_retry=Callback::new(move |()| me.refetch())
                                />
                            }
                                .into_any()
                        }
                    })
            }}
        </Suspense>
    }
}

// ---------------------------------------------------------------------------
// Dashboard
// ---------------------------------------------------------------------------

/// A "see all" link at the end of a section heading.
fn see_all(href: &'static str, label: &'static str) -> AnyView {
    view! {
        <ButtonLink kind=ButtonKind::Ghost href trailing_icon=IconName::ArrowRight>{label}</ButtonLink>
    }
    .into_any()
}

/// Where a signed-in member stands, and what is next.
#[component]
#[allow(clippy::needless_pass_by_value)] // Leptos prop convention
fn Dashboard(
    /// The signed-in member.
    me: MeView,
) -> impl IntoView {
    let next_title = me
        .next_rank_title
        .as_deref()
        .map_or_else(|| "—".to_string(), |t| plain_title(t).to_string());
    // Tag text is owned by the tag, so it is worked out here.
    let offices: Vec<String> = me.office_titles.iter().map(|o| plain_title(o).to_string()).collect();
    let title = plain_title(&me.rank_title).to_string();
    // The leaderboard is for members; its place only shows to them.
    let place = me
        .is_member
        .then_some(me.leaderboard_position)
        .flatten()
        .map(|p| format!("{p}ᵉ"));
    let (percent, caption, streak) = (rank_percent(&me), xp_caption(&me), streak_caption(&me));
    let name = me.display_name.clone();

    view! {
        <Stack gap=Gap::Loose>
            <Stack gap=Gap::Tight reading=true>
                <span class="ui-label">"Tableau de bord"</span>
                <h1 class="ui-display">{name}</h1>
                // Every title held, most important first: office, then member title.
                <Cluster>
                    {offices.into_iter().map(|o| view! { <Tag>{o}</Tag> }).collect_view()}
                    <Tag kind=TagKind::Accent>{title}</Tag>
                </Cluster>
            </Stack>

            <Stack reading=true>
                <XpProgress percent caption />
                <Cluster>
                    <StreakMarks days=me.streak_days />
                    <span class="ui-meta">{streak}</span>
                </Cluster>
                <Facts>
                    <Fact label="XP total" value=format_xp(me.xp_total) />
                    <Fact label="Titre suivant" value=next_title />
                    {place.map(|p| view! { <Fact label="Classement" value=p /> })}
                </Facts>
            </Stack>

            <Split>
                <QuestsInProgress />
                <NextSession />
            </Split>

            <Disclosure summary="Comment on progresse">
                <HowItWorks />
            </Disclosure>
        </Stack>
    }
}

/// The dashboard's shape while the member loads.
#[component]
fn DashboardSkeleton() -> impl IntoView {
    view! {
        <Stack gap=Gap::Loose>
            <Stack gap=Gap::Tight>
                <span class="ui-skeleton ui-skeleton--short"></span>
                <span class="ui-skeleton ui-skeleton--title"></span>
            </Stack>
            <RowsSkeleton rows=2 />
        </Stack>
    }
}

/// The member's quests that are not finished.
#[component]
fn QuestsInProgress() -> impl IntoView {
    let quests = Resource::new(|| (), |()| async { get_quests().await });
    view! {
        <Section title="Quêtes en cours" action=see_all("/quests", "Toutes les quêtes")>
            <Suspense fallback=|| view! { <RowsSkeleton rows=3 /> }>
                {move || {
                    quests
                        .get()
                        .map(|result| match result {
                            Err(_) => {
                                view! {
                                    <ErrorState
                                        message="Impossible de charger tes quêtes."
                                        on_retry=Callback::new(move |()| quests.refetch())
                                    />
                                }
                                    .into_any()
                            }
                            Ok(list) => {
                                let open: Vec<QuestItem> = list
                                    .into_iter()
                                    .filter(|q| !q.completed)
                                    .take(DASHBOARD_QUESTS)
                                    .collect();
                                if open.is_empty() {
                                    view! {
                                        <EmptyState
                                            icon=IconName::Target
                                            title="Aucune quête en cours"
                                            body="Le Bureau en publie chaque semaine."
                                        />
                                    }
                                        .into_any()
                                } else {
                                    view! {
                                        <DenseList label="Quêtes en cours">
                                            {open
                                                .into_iter()
                                                .map(|quest| view! { <QuestRow quest /> })
                                                .collect_view()}
                                        </DenseList>
                                    }
                                        .into_any()
                                }
                            }
                        })
                }}
            </Suspense>
        </Section>
    }
}

/// The next session that is not called off.
#[component]
fn NextSession() -> impl IntoView {
    let events = Resource::new(|| (), |()| async { get_upcoming().await });
    view! {
        <Section title="Prochaine séance" action=see_all("/calendar", "Calendrier")>
            <Suspense fallback=|| view! { <CardSkeleton /> }>
                {move || {
                    events
                        .get()
                        .map(|result| match result {
                            Err(_) => {
                                view! {
                                    <ErrorState
                                        message="Impossible de charger le calendrier."
                                        on_retry=Callback::new(move |()| events.refetch())
                                    />
                                }
                                    .into_any()
                            }
                            Ok(list) => match list.into_iter().find(|e| !e.cancelled) {
                                None => {
                                    view! {
                                        <EmptyState
                                            icon=IconName::CalendarBlank
                                            title="Aucune séance prévue"
                                            body="Les prochaines séances apparaîtront ici dès qu'elles seront au calendrier."
                                        />
                                    }
                                        .into_any()
                                }
                                Some(event) => {
                                    let meta = if event.xp_reward > 0 {
                                        format!("{} · +{} XP de présence", event.when_label, event.xp_reward)
                                    } else {
                                        event.when_label.clone()
                                    };
                                    match event.location {
                                        Some(place) => {
                                            view! {
                                                <Card kicker=event.kind_label title=event.title meta>
                                                    {format!("Lieu : {place}")}
                                                </Card>
                                            }
                                                .into_any()
                                        }
                                        None => {
                                            view! { <Card kicker=event.kind_label title=event.title meta /> }
                                                .into_any()
                                        }
                                    }
                                }
                            },
                        })
                }}
            </Suspense>
        </Section>
    }
}

// ---------------------------------------------------------------------------
// Signed out
// ---------------------------------------------------------------------------

/// What the association is, and the three ways in.
#[component]
fn Welcome() -> impl IntoView {
    view! {
        <Stack gap=Gap::Loose>
            <Stack reading=true>
                <span class="ui-label">"Epitech Bénin · association game-dev"</span>
                <h1 class="ui-display">"Code, crée, viens aux séances : chaque action te fait monter en titre."</h1>
                <p class="ui-body ui-muted">
                    "Les membres gagnent de l'XP en codant, en publiant des projets et en venant
                     aux séances, et l'XP les fait monter de titre en titre."
                </p>
                <Cluster>
                    <ButtonLink kind=ButtonKind::Primary href="/api/auth/login" external=true icon=IconName::DiscordLogo>
                        "Rejoindre la partie"
                    </ButtonLink>
                    <ButtonLink href="/tests" icon=IconName::GraduationCap>
                        "Passer le test d'entrée"
                    </ButtonLink>
                    <ButtonLink kind=ButtonKind::Ghost href="/kumo" icon=IconName::ChatCircle>
                        "Contacter Kumo"
                    </ButtonLink>
                </Cluster>
                <p class="ui-meta">
                    "Déjà sur le serveur Discord de l'association ? Connecte-toi, l'inscription est
                     directe. Sinon, le test d'entrée ouvre la porte."
                </p>
            </Stack>

            <Section title="Comment on progresse">
                <HowItWorks />
            </Section>

            <Section
                title="L'échelle des titres"
                lead="Ton XP globale te fait monter de titre en titre. Le dernier ne se dévoile qu'à celui ou celle qui l'atteint."
            >
                <TitleLadder />
            </Section>

            // Named, not linked: what happens inside a track — its members,
            // its XP, the projects it judges — is for members.
            <Section title="Les tracks" lead="Les huit disciplines de l'association. Ce qui s'y passe se découvre une fois membre.">
                <CardGrid>
                    {Track::ALL
                        .iter()
                        .map(|track| {
                            view! { <Card title=track_label(*track) icon=track_icon(*track) /> }
                        })
                        .collect_view()}
                </CardGrid>
            </Section>
        </Stack>
    }
}

/// The three moves of the game, in order.
#[component]
fn HowItWorks() -> impl IntoView {
    view! {
        <Steps>
            <Step icon=IconName::Compass title="Choisis tes tracks">
                "Engineering, Game Design, Narrative… une ou plusieurs. À partir de deux, toute
                 ton XP est multipliée."
            </Step>
            <Step icon=IconName::Lightning title="Gagne de l'XP">
                "Pousse du code, scanne le QR aux séances, relis les projets des autres,
                 accomplis les quêtes du Bureau."
            </Step>
            <Step icon=IconName::Crown title="Monte en titre">
                "De l'Initié au titre secret pour l'XP globale, et d'Observateur à Mentor dans
                 chaque track."
            </Step>
        </Steps>
    }
}

//! Quests — a dense list, one row per quest.
//!
//! A row says where the member stands (to do, in progress, validated), the
//! quest, its track and the XP it pays. The filter opens on the quests still
//! open, which is what a member comes here for; validated ones are a click
//! away.

use leptos::prelude::*;

use crate::{
    api::QuestItem,
    components::ui::{
        segment, ButtonKind, ButtonLink, Cluster, DenseList, EmptyState, ErrorState, FilterBar,
        Icon, IconName, ListRow, Page, PageHeader, Pattern, RowText, RowValue, RowsSkeleton,
        SegmentedControl, SignInState, Tag, TrackTag,
    },
    server_fns::{get_me, get_quests},
};

/// Whether a quest belongs in the list for a filter.
fn keep(filter: &str, quest: &QuestItem) -> bool {
    match filter {
        "done" => quest.completed,
        "all" => true,
        _ => !quest.completed,
    }
}

/// A quest's state, as an icon and the words a screen reader says.
fn quest_state(quest: &QuestItem) -> (IconName, &'static str) {
    if quest.completed {
        (IconName::CheckCircle, "Validée")
    } else if quest.progress > 0 {
        (IconName::CircleHalf, "En cours")
    } else {
        (IconName::Circle, "À faire")
    }
}

/// Weekly quests are the norm and carry no tag.
fn type_label(kind: &str) -> Option<&'static str> {
    match kind {
        "Special" => Some("Spéciale"),
        "Hidden" => Some("Secrète"),
        _ => None,
    }
}

/// One quest: state, name, track, XP.
#[component]
#[allow(clippy::needless_pass_by_value)] // Leptos prop convention
pub fn QuestRow(
    /// The quest.
    quest: QuestItem,
) -> impl IntoView {
    let (icon, state) = quest_state(&quest);
    let meta = if quest.completed {
        format!("{} · {} / {}", quest.condition_label, quest.target, quest.target)
    } else {
        format!(
            "{} · {} / {} · {}",
            quest.condition_label, quest.progress, quest.target, quest.time_left
        )
    };
    let lead = view! {
        <Icon name=icon />
        <span class="ui-sr-only">{state}</span>
    }
    .into_any();
    let end = view! { <RowValue text=format!("+{} XP", quest.xp_reward) /> }.into_any();
    let kind = type_label(&quest.quest_type);
    let track = quest.track.clone();
    let tags = (track.is_some() || kind.is_some()).then(|| {
        view! {
            <Cluster>
                {track.map(|track| view! { <TrackTag track /> })}
                {kind.map(|k| view! { <Tag>{k}</Tag> })}
            </Cluster>
        }
    });
    view! {
        <ListRow lead title=quest.title meta end>
            {tags}
            {quest.description.map(|text| view! { <RowText text /> })}
        </ListRow>
    }
}

/// What an empty list says, per filter.
fn empty_for(filter: &str) -> AnyView {
    match filter {
        "done" => view! {
            <EmptyState
                icon=IconName::CheckCircle
                title="Aucune quête validée pour l'instant"
                body="Elles se valident toutes seules quand l'XP qu'elles demandent arrive."
            />
        }
        .into_any(),
        _ => view! {
            <EmptyState
                icon=IconName::Target
                title="Aucune quête en cours"
                body="Le Bureau en publie chaque semaine."
            />
        }
        .into_any(),
    }
}

/// The list, once the member is known.
#[component]
fn QuestList(
    /// Which quests to show.
    filter: ReadSignal<String>,
) -> impl IntoView {
    let quests = Resource::new(|| (), |()| async { get_quests().await });
    view! {
        <Suspense fallback=|| view! { <RowsSkeleton rows=5 /> }>
            {move || {
                quests
                    .get()
                    .map(|result| match result {
                        Err(_) => {
                            view! {
                                <ErrorState
                                    message="Impossible de charger les quêtes."
                                    on_retry=Callback::new(move |()| quests.refetch())
                                />
                            }
                                .into_any()
                        }
                        Ok(list) => {
                            let wanted = filter.get();
                            let mut shown: Vec<QuestItem> = list
                                .into_iter()
                                .filter(|q| keep(&wanted, q))
                                .collect();
                            // Unfinished first, then by how close to done.
                            shown.sort_by_key(|q| (q.completed, (q.target - q.progress).max(0)));
                            if shown.is_empty() {
                                empty_for(&wanted)
                            } else {
                                view! {
                                    <DenseList label="Quêtes">
                                        {shown
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
    }
}

/// Quests page.
#[component]
pub fn QuestsPage() -> impl IntoView {
    let me = Resource::new(|| (), |()| async { get_me().await });
    let (filter, set_filter) = signal("open".to_string());
    let filters = view! {
        <FilterBar label="Filtrer les quêtes">
            <SegmentedControl
                label="État"
                options=vec![segment("open", "En cours"), segment("done", "Validées"), segment("all", "Toutes")]
                value=filter
                on_change=Callback::new(move |v| set_filter.set(v))
            />
        </FilterBar>
    }
    .into_any();

    view! {
        <Page pattern=Pattern::List>
            <PageHeader
                title="Quêtes"
                lead="Le Bureau les propose, elles se valident toutes seules : pousse du code, viens aux séances, relis les projets des autres."
                filters
            >
                // Opening a quest is done from the Bureau panel.
                <Suspense fallback=|| ()>
                    {move || {
                        me.get()
                            .and_then(Result::ok)
                            .flatten()
                            .filter(|u| u.can_access_admin)
                            .map(|_| {
                                view! {
                                    <ButtonLink kind=ButtonKind::Primary href="/admin" icon=IconName::Plus>
                                        "Ouvrir une quête"
                                    </ButtonLink>
                                }
                            })
                    }}
                </Suspense>
            </PageHeader>
            <Suspense fallback=|| view! { <RowsSkeleton rows=5 /> }>
                {move || {
                    me.get()
                        .map(|result| match result {
                            Ok(Some(_)) => view! { <QuestList filter /> }.into_any(),
                            Ok(None) => view! { <SignInState what="suivre tes quêtes" /> }.into_any(),
                            Err(_) => {
                                view! {
                                    <ErrorState
                                        message="Impossible de charger les quêtes."
                                        on_retry=Callback::new(move |()| me.refetch())
                                    />
                                }
                                    .into_any()
                            }
                        })
                }}
            </Suspense>
        </Page>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quest(progress: i32, completed: bool) -> QuestItem {
        QuestItem {
            id: "q".into(),
            title: "Relire trois projets".into(),
            description: None,
            xp_reward: 40,
            quest_type: "Weekly".into(),
            condition_label: "Rendre des revues".into(),
            progress,
            target: 3,
            track: None,
            completed,
            time_left: "4 jours".into(),
        }
    }

    #[test]
    fn a_quest_says_where_the_member_stands() {
        assert_eq!(quest_state(&quest(0, false)).0, IconName::Circle);
        assert_eq!(quest_state(&quest(1, false)).0, IconName::CircleHalf);
        assert_eq!(quest_state(&quest(3, true)).0, IconName::CheckCircle);
    }

    #[test]
    fn the_filter_opens_on_what_is_left_to_do() {
        assert!(keep("open", &quest(1, false)));
        assert!(!keep("open", &quest(3, true)));
        assert!(keep("done", &quest(3, true)));
        assert!(keep("all", &quest(0, false)));
    }
}

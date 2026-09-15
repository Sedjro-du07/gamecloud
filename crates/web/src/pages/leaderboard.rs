//! Leaderboard — a dense list, one row per member.
//!
//! **Season is the default**, on purpose: an all-time board freezes, because
//! the founders sit on top of it for good and somebody joining in September
//! can see they will never catch up. The season gives everybody something
//! winnable; the all-time scope keeps the record.
//!
//! Members only. Nobody is told apart by colour: position, name and title.

use leptos::prelude::*;

use crate::{
    api::{format_xp, LeaderboardEntry},
    components::ui::{
        segment, vocab::{plain_title, track_choices}, Avatar, ButtonKind, ButtonLink, Cluster,
        DenseList, EmptyState, ErrorState, Field, FilterBar, IconName, ListRow, Page, PageHeader,
        Pattern, RowValue, RowsSkeleton, SegmentedControl, Stack, Gap, Tag,
    },
    server_fns::get_leaderboard,
};

/// The sentence over the list.
fn caption(scope: &str, label: Option<&str>) -> String {
    match (scope, label) {
        ("season", Some(name)) => format!("Saison en cours : {name}"),
        ("track", Some(name)) => format!("Track {name}"),
        _ => "Toutes saisons confondues".to_string(),
    }
}

/// One member's standing.
#[component]
#[allow(clippy::needless_pass_by_value)] // Leptos prop convention
fn EntryRow(
    /// The row.
    entry: LeaderboardEntry,
) -> impl IntoView {
    let lead = view! {
        <span>{entry.position}</span>
        <Avatar name=entry.display_name.clone() src=entry.avatar_url.clone() />
    }
    .into_any();
    let streak = match entry.streak_days {
        0 => String::new(),
        1 => " · 1 jour de série".to_string(),
        n => format!(" · {n} jours de série"),
    };
    let meta = format!("{}{streak}", plain_title(&entry.rank_title));
    let end = view! { <RowValue text=format!("{} XP", format_xp(entry.xp)) /> }.into_any();
    let me = entry.is_me;
    view! {
        <ListRow lead title=entry.display_name meta end>
            {me.then(|| view! { <Cluster><Tag icon=IconName::User>"Toi"</Tag></Cluster> })}
        </ListRow>
    }
}

/// The standings for the chosen scope, in every state.
#[component]
fn Standings(
    /// The board for the chosen scope.
    board: Resource<Result<crate::api::LeaderboardView, ServerFnError>>,
) -> impl IntoView {
    view! {
            <Transition fallback=|| view! { <RowsSkeleton rows=6 /> }>
                {move || {
                    board
                        .get()
                        .map(|result| match result {
                            Err(_) => {
                                view! {
                                    <ErrorState
                                        message="Impossible de charger le classement."
                                        on_retry=Callback::new(move |()| board.refetch())
                                    />
                                }
                                    .into_any()
                            }
                            Ok(view_model) if view_model.restricted => {
                                view! {
                                    <EmptyState
                                        icon=IconName::LockSimple
                                        title="Le classement est réservé aux membres inscrits"
                                        body="Connecte-toi et vérifie ton adresse Epitech pour le voir."
                                    >
                                        <Cluster>
                                            <ButtonLink
                                                kind=ButtonKind::Primary
                                                href="/api/auth/login"
                                                external=true
                                                icon=IconName::DiscordLogo
                                                hide_label=true
                                            >
                                                "Se connecter avec Discord"
                                            </ButtonLink>
                                            <ButtonLink kind=ButtonKind::Ghost href="/onboarding/email" icon=IconName::Envelope>
                                                "Vérifier mon adresse"
                                            </ButtonLink>
                                        </Cluster>
                                    </EmptyState>
                                }
                                    .into_any()
                            }
                            Ok(view_model) if view_model.entries.is_empty() => {
                                view! {
                                    <EmptyState
                                        icon=IconName::Trophy
                                        title="Personne n'a encore marqué de points ici"
                                        body=caption(&view_model.scope, view_model.label.as_deref())
                                    />
                                }
                                    .into_any()
                            }
                            Ok(view_model) => {
                                let text = caption(&view_model.scope, view_model.label.as_deref());
                                view! {
                                    <Stack gap=Gap::Tight>
                                        <p class="ui-meta">{text}</p>
                                        <DenseList label="Classement">
                                            {view_model
                                                .entries
                                                .into_iter()
                                                .map(|entry| view! { <EntryRow entry /> })
                                                .collect_view()}
                                        </DenseList>
                                    </Stack>
                                }
                                    .into_any()
                            }
                        })
                }}
            </Transition>
    }
}

/// Leaderboard page.
#[component]
pub fn LeaderboardPage() -> impl IntoView {
    let (scope, set_scope) = signal("season".to_string());
    let (track, set_track) = signal("Engineering".to_string());
    let board = Resource::new(
        move || (scope.get(), track.get()),
        |(scope, track)| async move {
            let track = (scope == "track").then_some(track);
            get_leaderboard(scope, track).await
        },
    );

    let filters = view! {
        <FilterBar label="Portée du classement">
            <SegmentedControl
                label="Portée"
                options=vec![segment("season", "Saison"), segment("all", "Depuis toujours"), segment("track", "Par track")]
                value=scope
                on_change=Callback::new(move |v| set_scope.set(v))
            />
            <Show when=move || scope.get() == "track">
                <Field id="classement-track" label="Track" inline=true>
                    <select
                        id="classement-track"
                        class="ui-control"
                        prop:value=move || track.get()
                        on:change=move |ev| set_track.set(event_target_value(&ev))
                    >
                        {track_choices()
                            .into_iter()
                            .map(|(id, label)| view! { <option value=id>{label}</option> })
                            .collect_view()}
                    </select>
                </Field>
            </Show>
        </FilterBar>
    }
    .into_any();

    view! {
        <Page pattern=Pattern::List>
            <PageHeader
                title="Classement"
                lead="L'XP gagnée par chaque membre. La saison repart de zéro : tout le monde peut la gagner."
                filters
            />
            <Standings board />
        </Page>
    }
}

#[cfg(test)]
mod tests {
    use super::caption;

    #[test]
    fn the_caption_names_the_scope() {
        assert!(caption("season", Some("Automne 2026")).contains("Automne 2026"));
        assert!(caption("track", Some("Audio")).contains("Audio"));
        assert!(caption("all", None).contains("Toutes saisons"));
    }
}

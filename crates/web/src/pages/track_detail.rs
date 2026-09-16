//! One track's board — a detail page.
//!
//! The track's people, its sessions and the projects it judged. Readable by
//! anyone; what changes with the viewer's role is what it offers: a reviewer
//! sees the projects awaiting this track's verdict as work to do.

use std::fmt::Write as _;

use leptos::prelude::*;
use leptos_router::hooks::use_params_map;

use crate::{
    api::{format_xp, CalendarEvent, TrackBoard, TrackMemberRow, TrackProjectRow},
    components::ui::{
        vocab::{plain_title, status_label, track_name, verdict},
        ButtonKind, ButtonLink, DenseList, EmptyState, ErrorState, Fact, Facts, Icon, IconName,
        ListRow, MembersOnlyState, Page, PageHeader, PageSkeleton, Pattern, RowValue, Section, Tag,
    },
    server_fns::{get_me, get_track_board},
};

/// The projects this track judged, awaiting ones first.
#[component]
#[allow(clippy::needless_pass_by_value)] // Leptos prop convention
fn Projects(
    /// Rows.
    projects: Vec<TrackProjectRow>,
    /// Whether the viewer reviews here.
    can_review: bool,
) -> impl IntoView {
    let lead = can_review.then_some("Tu es relecteur ici : les projets en attente t'attendent. La note se met sur la page du projet.");
    let list = if projects.is_empty() {
        view! { <EmptyState icon=IconName::Cube title="Aucun projet ne concerne encore cette track" /> }.into_any()
    } else {
        let mut projects = projects;
        projects.sort_by_key(|p| p.verdict != "Pending");
        view! {
            <DenseList label="Projets jugés">
                {projects
                    .into_iter()
                    .map(|row| {
                        let (label, icon) = verdict(&row.verdict);
                        let pending = row.verdict == "Pending";
                        let mut meta = format!("{} · {label}", status_label(&row.status));
                        if let Some(score) = row.score {
                            let _ = write!(meta, " · {score}/100");
                        }
                        if let Some(reviewer) = &row.reviewer_name {
                            let _ = write!(meta, " · relu par {reviewer}");
                        }
                        match row.file_count {
                            0 => {}
                            1 => meta.push_str(" · 1 build"),
                            n => {
            let _ = write!(meta, " · {n} builds");
        }
                        }
                        let lead = view! { <Icon name=icon /> }.into_any();
                        let end = if pending && can_review {
                            view! { <Tag icon=IconName::Eye>"À noter"</Tag> }.into_any()
                        } else {
                            ().into_any()
                        };
                        view! { <ListRow lead title=row.name href=format!("/projects/{}", row.id) meta end /> }
                    })
                    .collect_view()}
            </DenseList>
        }
        .into_any()
    };
    match lead {
        Some(lead) => view! { <Section title="Projets jugés par la track" lead>{list}</Section> }.into_any(),
        None => view! { <Section title="Projets jugés par la track">{list}</Section> }.into_any(),
    }
}

/// The people in the track, strongest title first.
#[component]
#[allow(clippy::needless_pass_by_value)] // Leptos prop convention
fn Members(
    /// Members.
    members: Vec<TrackMemberRow>,
) -> impl IntoView {
    let count = members.len();
    let list = if members.is_empty() {
        view! { <EmptyState icon=IconName::Users title="Personne pour l'instant" /> }.into_any()
    } else {
        view! {
            <DenseList label="Membres">
                {members
                    .into_iter()
                    .map(|m| {
                        let title = plain_title(&m.role_label).to_string();
                        let meta = m.specialization.map_or_else(|| title.clone(), |s| format!("{title} · {s}"));
                        let end = view! { <RowValue text=format!("{} XP", format_xp(m.track_xp)) /> }.into_any();
                        view! { <ListRow title=m.display_name meta end /> }
                    })
                    .collect_view()}
            </DenseList>
        }
        .into_any()
    };
    view! { <Section title="Membres" meta=count.to_string()>{list}</Section> }
}

/// The track's own sessions.
#[component]
#[allow(clippy::needless_pass_by_value)] // Leptos prop convention
fn Sessions(
    /// Upcoming events for the track.
    events: Vec<CalendarEvent>,
    /// Whether the viewer schedules them.
    manages: bool,
) -> impl IntoView {
    let action = manages.then(|| {
        view! {
            <ButtonLink kind=ButtonKind::Ghost href="/calendar" icon=IconName::Plus>"Programmer une séance"</ButtonLink>
        }
        .into_any()
    });
    let list = if events.is_empty() {
        view! {
            <EmptyState icon=IconName::CalendarBlank title="Rien de programmé">
                <ButtonLink kind=ButtonKind::Ghost href="/calendar" trailing_icon=IconName::ArrowRight>"Voir le calendrier"</ButtonLink>
            </EmptyState>
        }
        .into_any()
    } else {
        view! {
            <DenseList label="Séances">
                {events
                    .into_iter()
                    .map(|e| {
                        let mut meta = format!("{} · {}", e.kind_label, e.when_label);
                        if let Some(place) = &e.location {
                            meta.push_str(" · ");
                            meta.push_str(place);
                        }
                        view! { <ListRow title=e.title meta dimmed=e.cancelled /> }
                    })
                    .collect_view()}
            </DenseList>
        }
        .into_any()
    };
    match action {
        Some(action) => view! { <Section title="Séances de la track" action>{list}</Section> }.into_any(),
        None => view! { <Section title="Séances de la track">{list}</Section> }.into_any(),
    }
}

/// The board, once loaded.
#[component]
#[allow(clippy::needless_pass_by_value)] // Leptos prop convention
fn Board(
    /// The loaded board.
    board: TrackBoard,
) -> impl IntoView {
    let pending = board.projects.iter().filter(|p| p.verdict == "Pending").count();
    let average = board.average_score.map_or_else(|| "—".to_string(), |a| format!("{a:.0} / 100"));
    let member_count = board.members.len().to_string();
    let role = board.my_role.as_deref().map(|r| format!("Ton titre ici : {}", plain_title(r)));
    view! {
        <Page pattern=Pattern::Detail>
            <PageHeader title=track_name(&board.id) kicker="Track" lead=board.specializations.join(" · ")>
                {role.map(|r| view! { <Tag>{r}</Tag> })}
                {(!board.joined).then(|| view! {
                    <ButtonLink kind=ButtonKind::Primary href="/tracks" icon=IconName::Plus>"Rejoindre"</ButtonLink>
                })}
            </PageHeader>
            <Facts>
                <Fact label="Membres" value=member_count />
                <Fact label="XP de track" value=format_xp(board.total_xp) />
                <Fact label="Note moyenne" value=average />
                <Fact label="À juger" value=pending.to_string() />
            </Facts>
            <Members members=board.members />
            <Sessions events=board.events manages=board.can_manage_events />
            <Projects projects=board.projects can_review=board.can_review />
        </Page>
    }
}

/// Track board page.
#[component]
pub fn TrackDetailPage() -> impl IntoView {
    let me = Resource::new(|| (), |()| async { get_me().await });
    let params = use_params_map();
    let board = Resource::new(
        move || params.get().get("id").unwrap_or_default(),
        |id| async move { get_track_board(id).await },
    );
    view! {
        <Suspense fallback=|| view! { <Page pattern=Pattern::Detail><PageSkeleton /></Page> }>
            {move || {
                let member = me
                    .get()
                    .and_then(Result::ok)
                    .flatten()
                    .is_some_and(|u| u.email_verified);
                if !member {
                    return view! {
                        <Page pattern=Pattern::Detail>
                            <PageHeader title="Track" />
                            <MembersOnlyState what="voir le tableau d'une track" />
                        </Page>
                    }
                    .into_any();
                }
                board.get().map(|result| match result {
                Err(_) => view! {
                    <Page pattern=Pattern::Detail>
                        <PageHeader title="Track" />
                        <ErrorState message="Impossible de charger cette track." on_retry=Callback::new(move |()| board.refetch()) />
                    </Page>
                }
                .into_any(),
                Ok(board) => view! { <Board board /> }.into_any(),
                })
                .into_any()
            }}
        </Suspense>
    }
}

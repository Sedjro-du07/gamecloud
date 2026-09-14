//! One track's board.
//!
//! A track is the unit the association actually works in, and until now
//! it had no page of its own: you could join one from the onboarding
//! list and then never see it again. This is where a track's people,
//! its verdicts and its own sessions live.
//!
//! The page is readable by anyone. What changes with the viewer's role
//! is what it *offers*: a reviewer sees the queue of projects awaiting
//! this track's verdict as work to do, everyone else sees the same
//! projects as a record of what the track has judged.

use leptos::prelude::*;
use leptos_router::hooks::use_params_map;

use crate::{api::TrackBoard, server_fns::get_track_board};

/// A project row, with the track's verdict and mark.
#[component]
fn ProjectRow(
    /// The row.
    row: crate::api::TrackProjectRow,
    /// Whether the viewer may render verdicts here.
    can_review: bool,
) -> impl IntoView {
    let (verdict_class, verdict_label) = match row.verdict.as_str() {
        "Approved" => ("gc-verdict--ok", "Approuvé"),
        "Rejected" => ("gc-verdict--no", "Refusé"),
        "NotApplicable" => ("gc-verdict--na", "Non applicable"),
        _ => ("gc-verdict--wait", "En attente"),
    };
    let href = format!("/projects/{}", row.id);
    let pending = row.verdict == "Pending";

    view! {
        <tr class=if pending { "gc-track__row gc-track__row--todo" } else { "gc-track__row" }>
            <td>
                <a class="gc-link" href=href>
                    {row.name}
                </a>
            </td>
            <td>
                <span class="gc-chip">{row.status}</span>
            </td>
            <td>
                <span class=format!("gc-chip {verdict_class}")>{verdict_label}</span>
            </td>
            <td class="gc-track__score">
                {row.score.map_or_else(|| "—".to_string(), |n| format!("{n}/100"))}
            </td>
            <td>{row.reviewer_name.unwrap_or_else(|| "—".to_string())}</td>
            <td class="gc-track__files">
                {if row.file_count == 0 {
                    "—".to_string()
                } else {
                    format!("{} build(s)", row.file_count)
                }}
            </td>
            <td>
                {(pending && can_review)
                    .then(|| {
                        view! {
                            <span class="gc-chip gc-chip--call">"à noter"</span>
                        }
                    })}
            </td>
        </tr>
    }
}

/// The people in the track.
#[component]
fn MemberList(
    /// Members, strongest role first.
    members: Vec<crate::api::TrackMemberRow>,
) -> impl IntoView {
    view! {
        <ul class="gc-track__members">
            {members
                .into_iter()
                .map(|m| {
                    let role_class = match m.role.as_str() {
                        "Lead" | "CoLead" => "gc-track__member--lead",
                        "Mentor" | "Reviewer" => "gc-track__member--senior",
                        _ => "gc-track__member",
                    };
                    view! {
                        <li class=role_class>
                            <span class="gc-track__name">{m.display_name}</span>
                            <span class="gc-track__role">{m.role_label}</span>
                            {m
                                .specialization
                                .map(|s| view! { <span class="gc-track__spec">{s}</span> })}
                            <span class="gc-track__xp">{format!("{} XP", m.track_xp)}</span>
                        </li>
                    }
                })
                .collect_view()}
        </ul>
    }
}

/// The track's own sessions.
#[component]
fn TrackAgenda(
    /// Upcoming events scoped to this track.
    events: Vec<crate::api::CalendarEvent>,
) -> impl IntoView {
    view! {
        <section class="gc-track__panel">
            <h2>"Séances de la track"</h2>
            {if events.is_empty() {
                view! {
                    <p class="gc-empty">
                        "Rien de programmé. " <a class="gc-link" href="/calendar">"Voir le calendrier"</a>
                    </p>
                }
                    .into_any()
            } else {
                view! {
                    <ul class="gc-track__agenda">
                        {events
                            .into_iter()
                            .map(|e| {
                                view! {
                                    <li>
                                        <span class="gc-track__when">{e.when_label}</span>
                                        <span class="gc-track__evt">{e.title}</span>
                                        <span class="gc-chip">{e.kind_label}</span>
                                        {e
                                            .location
                                            .map(|l| view! { <span class="gc-track__where">{l}</span> })}
                                    </li>
                                }
                            })
                            .collect_view()}
                    </ul>
                }
                    .into_any()
            }}
        </section>
    }
}

/// The board's header strip: identity and the three numbers that
/// describe the track's health.
#[component]
fn TrackHeader(
    /// The loaded board.
    board: TrackBoard,
) -> impl IntoView {
    let pending = board
        .projects
        .iter()
        .filter(|p| p.verdict == "Pending")
        .count();

    view! {
        <header class="gc-track__head" style=format!("--gc-track: {};", board.color)>
            <h1>
                <span class="gc-track__emoji">{board.emoji}</span>
                {board.id.clone()}
            </h1>
            <div class="gc-track__stats">
                <div class="gc-stat">
                    <span class="gc-stat__value">{board.members.len()}</span>
                    <span class="gc-stat__label">"membres"</span>
                </div>
                <div class="gc-stat">
                    <span class="gc-stat__value">{board.total_xp}</span>
                    <span class="gc-stat__label">"XP de track"</span>
                </div>
                <div class="gc-stat">
                    <span class="gc-stat__value">
                        {board
                            .average_score
                            .map_or_else(|| "—".to_string(), |a| format!("{a:.0}"))}
                    </span>
                    <span class="gc-stat__label">"note moyenne"</span>
                </div>
                <div class="gc-stat">
                    <span class="gc-stat__value">{pending}</span>
                    <span class="gc-stat__label">"à juger"</span>
                </div>
            </div>
            <p class="gc-track__specs">
                {board
                    .specializations
                    .into_iter()
                    .map(|s| view! { <span class="gc-chip">{s}</span> })
                    .collect_view()}
            </p>
            {board
                .my_role
                .map(|r| {
                    view! {
                        <p class="gc-track__mine">"Votre rôle ici : " <strong>{r}</strong></p>
                    }
                })}
        </header>
    }
}

/// Track board page.
#[component]
pub fn TrackDetailPage() -> impl IntoView {
    let params = use_params_map();
    let board = Resource::new(
        move || params.get().get("id").unwrap_or_default(),
        |id| async move { get_track_board(id).await },
    );

    view! {
        <section class="gc-track">
            <Suspense fallback=move || view! { <p class="gc-empty">"Chargement…"</p> }>
                {move || match board.get() {
                    None => view! { <p class="gc-empty">"Chargement…"</p> }.into_any(),
                    Some(Err(e)) => {
                        view! { <p class="gc-empty">{format!("Track indisponible : {e}")}</p> }
                            .into_any()
                    }
                    Some(Ok(b)) => {
                        let can_review = b.can_review;
                        let members = b.members.clone();
                        let projects = b.projects.clone();
                        let events = b.events.clone();
                        let manages = b.can_manage_events;
                        view! {
                            <TrackHeader board=b />

                            <div class="gc-track__cols">
                                <section class="gc-track__panel">
                                    <h2>{format!("Membres ({})", members.len())}</h2>
                                    {if members.is_empty() {
                                        view! { <p class="gc-empty">"Personne pour l'instant."</p> }
                                            .into_any()
                                    } else {
                                        view! { <MemberList members=members /> }.into_any()
                                    }}
                                </section>

                                <TrackAgenda events=events />
                            </div>

                            <section class="gc-track__panel">
                                <h2>"Projets jugés par cette track"</h2>
                                <Show when=move || can_review>
                                    <p class="gc-admin__note">
                                        "Vous êtes relecteur ici : les lignes en attente vous
                                         attendent. La note se met sur la page du projet."
                                    </p>
                                </Show>
                                {if projects.is_empty() {
                                    view! {
                                        <p class="gc-empty">
                                            "Aucun projet ne concerne encore cette track."
                                        </p>
                                    }
                                        .into_any()
                                } else {
                                    view! {
                                        <div class="gc-table-wrap">
                                            <table class="gc-table">
                                                <thead>
                                                    <tr>
                                                        <th>"Projet"</th>
                                                        <th>"Statut"</th>
                                                        <th>"Verdict"</th>
                                                        <th>"Note"</th>
                                                        <th>"Relecteur"</th>
                                                        <th>"Builds"</th>
                                                        <th></th>
                                                    </tr>
                                                </thead>
                                                <tbody>
                                                    {projects
                                                        .into_iter()
                                                        .map(|p| {
                                                            view! {
                                                                <ProjectRow row=p can_review=can_review />
                                                            }
                                                        })
                                                        .collect_view()}
                                                </tbody>
                                            </table>
                                        </div>
                                    }
                                        .into_any()
                                }}
                            </section>

                            <Show when=move || manages>
                                <p class="gc-track__cta">
                                    "Vous pilotez cette track : "
                                    <a class="gc-link" href="/calendar">
                                        "programmez une séance et générez son code de présence"
                                    </a>
                                    "."
                                </p>
                            </Show>
                        }
                            .into_any()
                    }
                }}
            </Suspense>
        </section>
    }
}

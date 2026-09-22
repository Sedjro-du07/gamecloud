//! Projects — a grid of cards.
//!
//! Released work, filterable by track. Pre-release projects are absent: the
//! index is the club's record, and a half-finished build is the team's
//! business until it ships. A card shows the author's 16:9 screenshot
//! blended into the ground, or the track's icon on the surface when there
//! is none.

use std::fmt::Write as _;

use leptos::prelude::*;

use crate::{
    api::ProjectCard,
    components::ui::{
        vocab::{rarity_label, status_label, track_choices, track_icon_by_id, track_name},
        ButtonKind, ButtonLink, Card, CardGrid, CardMedia, EmptyState, ErrorState, Field,
        FilterBar, GridSkeleton, IconName, MembersOnlyState, Page, PageHeader, Pattern,
    },
    server_fns::{get_me, get_projects},
};

/// The line at the bottom of a project card.
fn project_meta(project: &ProjectCard) -> String {
    let mut meta = format!("{} · {}", status_label(&project.status), rarity_label(&project.rarity));
    match project.contributor_count {
        0 => {}
        1 => meta.push_str(" · 1 contributeur"),
        n => {
            let _ = write!(meta, " · {n} contributeurs");
        }
    }
    if let Some(day) = &project.released_on {
        meta.push_str(" · publié le ");
        meta.push_str(day);
    }
    meta
}

/// One project.
#[component]
#[allow(clippy::needless_pass_by_value)] // Leptos prop convention
pub fn ProjectTile(
    /// The project.
    project: ProjectCard,
) -> impl IntoView {
    let media = project.thumbnail_url.clone().map_or_else(
        || CardMedia::Missing(track_icon_by_id(&project.track).unwrap_or(IconName::Cube)),
        |src| CardMedia::Photo { src, alt: format!("Capture de {}", project.name) },
    );
    let meta = project_meta(&project);
    let href = format!("/projects/{}", project.id);
    let kicker = track_name(&project.track);
    match project.short_description {
        Some(text) => view! {
            <Card kicker title=project.name href media meta>{text}</Card>
        }
        .into_any(),
        None => view! { <Card kicker title=project.name href media meta /> }.into_any(),
    }
}

/// Projects page.
#[component]
pub fn ProjectsPage() -> impl IntoView {
    let me = Resource::new(|| (), |()| async { get_me().await });
    let (track, set_track) = signal(String::new());
    let projects = Resource::new(
        move || track.get(),
        |track| async move { get_projects((!track.is_empty()).then_some(track)).await },
    );

    let filters = view! {
        <FilterBar label="Filtrer les projets">
            <Field id="projets-track" label="Track" inline=true>
                <select id="projets-track" class="ui-control" prop:value=move || track.get()
                    on:change=move |ev| set_track.set(event_target_value(&ev))>
                    <option value="">"Toutes"</option>
                    {track_choices()
                        .into_iter()
                        .map(|(id, label)| view! { <option value=id>{label}</option> })
                        .collect_view()}
                </select>
            </Field>
        </FilterBar>
    }
    .into_any();

    view! {
        <Page pattern=Pattern::Grid>
            <PageHeader
                title="Projets"
                lead="Les projets publiés par les membres, une fois validés par chaque track concernée."
                filters
            >
                <ButtonLink kind=ButtonKind::Primary href="/projects/new" icon=IconName::Plus>
                    "Nouveau projet"
                </ButtonLink>
            </PageHeader>
            <Suspense fallback=|| view! { <GridSkeleton cards=6 /> }>
                {move || {
                    me.get()
                        .map(|result| match result.ok().flatten() {
                            Some(user) if user.is_member => view! { <ProjectList projects /> }.into_any(),
                            _ => view! { <MembersOnlyState what="voir les projets de l'association" /> }.into_any(),
                        })
                }}
            </Suspense>
        </Page>
    }
}

/// The projects themselves, once the viewer is known to be a member.
#[component]
fn ProjectList(
    /// The projects for the chosen track.
    projects: Resource<Result<Vec<ProjectCard>, ServerFnError>>,
) -> impl IntoView {
    view! {
            <Transition fallback=|| view! { <GridSkeleton cards=6 /> }>
                {move || {
                    projects
                        .get()
                        .map(|result| match result {
                            Err(_) => view! {
                                <ErrorState message="Impossible de charger les projets." on_retry=Callback::new(move |()| projects.refetch()) />
                            }
                            .into_any(),
                            Ok(list) if list.is_empty() => view! {
                                <EmptyState
                                    icon=IconName::Cube
                                    title="Aucun projet publié pour l'instant"
                                    body="Le premier qui passe la revue de ses tracks arrive ici."
                                />
                            }
                            .into_any(),
                            Ok(list) => view! {
                                <CardGrid>
                                    {list.into_iter().map(|project| view! { <ProjectTile project /> }).collect_view()}
                                </CardGrid>
                            }
                            .into_any(),
                        })
                }}
            </Transition>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_meta_line_counts_people_the_french_way() {
        let mut project = ProjectCard {
            id: "p".into(),
            name: "Aevaryn".into(),
            short_description: None,
            track: "Engineering".into(),
            track_emoji: String::new(),
            status: "Released".into(),
            rarity: "Epic".into(),
            rarity_color: String::new(),
            thumbnail_url: None,
            contributor_count: 1,
            released_on: None,
        };
        assert_eq!(project_meta(&project), "Publié · Épique · 1 contributeur");
        project.contributor_count = 3;
        project.released_on = Some("12/09/2026".into());
        assert_eq!(project_meta(&project), "Publié · Épique · 3 contributeurs · publié le 12/09/2026");
    }
}

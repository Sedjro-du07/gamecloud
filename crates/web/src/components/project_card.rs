//! Project card.
//!
//! The Hall of Fame tile. Rarity drives the border colour, which is the
//! whole point of the rarity tier: a build that satisfied five
//! disciplines should *look* different from one that satisfied one.

use leptos::prelude::*;

use crate::api::ProjectCard as ProjectCardData;

/// French label for a lifecycle status.
fn status_label(status: &str) -> &'static str {
    match status {
        "Draft" => "Brouillon",
        "InReview" => "En revue",
        "PartialOK" => "Partiellement validé",
        "Approved" => "Approuvé",
        "Released" => "Publié",
        "Archived" => "Archivé",
        "Rejected" => "Refusé",
        _ => "Inconnu",
    }
}

/// French label for a rarity tier.
fn rarity_label(rarity: &str) -> &'static str {
    match rarity {
        "Rare" => "Rare",
        "Epic" => "Épique",
        "Legendary" => "Légendaire",
        "Mythic" => "Mythique",
        _ => "Commun",
    }
}

/// Project card component.
#[component]
#[allow(clippy::needless_pass_by_value)] // Leptos prop convention
pub fn ProjectCard(
    /// The project to render.
    project: ProjectCardData,
) -> impl IntoView {
    let href = format!("/projects/{}", project.id);
    let border = project.rarity_color.clone();

    view! {
        <a class="gc-project" href=href style=format!("--gc-rarity: {border};")>
            {project
                .thumbnail_url
                .map(|url| view! {
                    <img class="gc-project__thumb" src=url alt="" loading="lazy" />
                })}
            <div class="gc-project__body">
                <header class="gc-project__head">
                    <span class="gc-project__track">
                        {project.track_emoji} " " {project.track}
                    </span>
                    <span class="gc-project__rarity">{rarity_label(&project.rarity)}</span>
                </header>
                <h3 class="gc-project__name">{project.name}</h3>
                {project
                    .short_description
                    .map(|d| view! { <p class="gc-project__desc">{d}</p> })}
                <footer class="gc-project__foot">
                    <span>{status_label(&project.status)}</span>
                    <span>
                        {format!("{} contributeur(s)", project.contributor_count)}
                    </span>
                    {project
                        .released_on
                        .map(|d| view! { <span>{d}</span> })}
                </footer>
            </div>
        </a>
    }
}

#[cfg(all(test, feature = "ssr"))]
mod tests {
    use super::{rarity_label, status_label};

    #[test]
    fn every_schema_status_has_a_label() {
        // These are exactly the values allowed by the projects_status_valid
        // CHECK constraint.
        for status in [
            "Draft",
            "InReview",
            "PartialOK",
            "Approved",
            "Released",
            "Archived",
            "Rejected",
        ] {
            assert_ne!(status_label(status), "Inconnu", "{status} unlabelled");
        }
    }

    #[test]
    fn unknown_status_degrades_gracefully() {
        assert_eq!(status_label("Sideways"), "Inconnu");
    }

    #[test]
    fn every_rarity_has_a_label() {
        for rarity in ["Common", "Rare", "Epic", "Legendary", "Mythic"] {
            assert!(!rarity_label(rarity).is_empty());
        }
    }
}

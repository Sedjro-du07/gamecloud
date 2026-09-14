//! Badge grid.
//!
//! Shows the full catalogue with the held ones lit up. Displaying the
//! locked badges too is deliberate: a badge you cannot see is not a
//! goal, and half of these exist to tell a member what is worth doing.

use leptos::prelude::*;

use crate::api::BadgeItem;

/// Badge grid component.
#[component]
#[allow(clippy::needless_pass_by_value)] // Leptos prop convention
pub fn BadgeGrid(
    /// Every badge, flagged with whether the member holds it.
    badges: Vec<BadgeItem>,
) -> impl IntoView {
    let held = badges.iter().filter(|b| b.held).count();
    let total = badges.len();

    view! {
        <section class="gc-badges">
            <header class="gc-badges__head">
                <h2>"Badges"</h2>
                <span class="gc-badges__count">{format!("{held} / {total}")}</span>
            </header>
            <ul class="gc-badges__grid">
                {badges
                    .into_iter()
                    .map(|b| {
                        let class = if b.held {
                            "gc-badge gc-badge--held"
                        } else {
                            "gc-badge"
                        };
                        let tooltip = b.description.clone();
                        view! {
                            <li class=class title=tooltip>
                                <span class="gc-badge__title">{b.title}</span>
                                <span class="gc-badge__desc">{b.description}</span>
                            </li>
                        }
                    })
                    .collect_view()}
            </ul>
        </section>
    }
}

//! The ladder of member titles.
//!
//! Every title a member can earn, lowest first, with the XP it takes.
//! Seeing the whole ladder is what turns a title into a goal: "Maître
//! Artisan" means little on its own, "Maître Artisan, 2 500 XP, and you
//! are at 1 900" is something to go and do. The last rung stays hidden
//! until somebody reaches it — a secret title is worth more talked about
//! than listed.

use gamecloud_shared::{roles::GlobalRank, xp::RANK_THRESHOLDS};
use leptos::prelude::*;

use crate::api::format_xp;

/// Where a rung sits relative to the viewer.
fn rung_state(rank: GlobalRank, current: Option<GlobalRank>) -> &'static str {
    match current {
        Some(c) if rank < c => "reached",
        Some(c) if rank == c => "current",
        _ => "locked",
    }
}

/// Title ladder component.
#[component]
#[allow(clippy::needless_pass_by_value)] // Leptos prop convention
pub fn TitleLadder(
    /// The viewer's rank identifier, to mark what they have reached;
    /// `None` for somebody who is not signed in.
    current: Option<String>,
) -> impl IntoView {
    let current = current.as_deref().map(GlobalRank::parse);

    let rungs = GlobalRank::ALL
        .iter()
        .copied()
        .zip(RANK_THRESHOLDS.iter())
        .filter(|(rank, _)| *rank >= GlobalRank::Initiate)
        .map(|(rank, (xp, _))| {
            let state = rung_state(rank, current);
            let hidden = rank == GlobalRank::Myth && state == "locked";
            let title = if hidden {
                "❓ Titre secret".to_string()
            } else {
                rank.title().to_string()
            };
            view! {
                <li
                    class=format!("gc-ladder__rung gc-ladder__rung--{state}")
                    style=format!("--gc-ring: {};", rank.ring_color())
                >
                    <span class="gc-ladder__dot" aria-hidden="true"></span>
                    <span class="gc-ladder__title">{title}</span>
                    <span class="gc-ladder__xp">{format!("{} XP", format_xp(*xp))}</span>
                </li>
            }
        })
        .collect_view();

    view! { <ol class="gc-ladder" aria-label="Échelle des titres">{rungs}</ol> }
}

#[cfg(all(test, feature = "ssr"))]
mod tests {
    use gamecloud_shared::roles::GlobalRank;

    use super::rung_state;

    #[test]
    fn rungs_below_the_viewer_are_reached() {
        let me = Some(GlobalRank::Expert);
        assert_eq!(rung_state(GlobalRank::Apprentice, me), "reached");
        assert_eq!(rung_state(GlobalRank::Expert, me), "current");
        assert_eq!(rung_state(GlobalRank::Legend, me), "locked");
        assert_eq!(rung_state(GlobalRank::Initiate, None), "locked");
    }
}

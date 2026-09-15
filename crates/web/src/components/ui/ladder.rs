//! The ladder of member titles.
//!
//! Every title a member can earn, lowest first, with the XP it takes. The
//! member's current title is the one place its accent tag appears; titles
//! below are checked, titles above wait. The last rung stays secret until
//! somebody reaches it.

use gamecloud_shared::{roles::GlobalRank, xp::RANK_THRESHOLDS};
use leptos::prelude::*;

use super::{
    icon::{Icon, IconName},
    tag::{Tag, TagKind},
    vocab::plain_title,
};
use crate::api::format_xp;

/// Where a rung sits relative to the viewer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Rung {
    Reached,
    Current,
    Locked,
}

fn rung(rank: GlobalRank, current: Option<GlobalRank>) -> Rung {
    match current {
        Some(c) if rank < c => Rung::Reached,
        Some(c) if rank == c => Rung::Current,
        _ => Rung::Locked,
    }
}

/// The title ladder.
#[component]
pub fn TitleLadder(
    /// The viewer's rank; `None` for somebody signed out.
    #[prop(optional)]
    current: Option<GlobalRank>,
) -> impl IntoView {
    let rungs = GlobalRank::ALL
        .iter()
        .copied()
        .zip(RANK_THRESHOLDS.iter())
        .filter(|(rank, _)| *rank >= GlobalRank::Initiate)
        .map(|(rank, (xp, _))| {
            let state = rung(rank, current);
            let title = if rank == GlobalRank::Myth && state == Rung::Locked {
                "Titre secret"
            } else {
                plain_title(rank.title())
            };
            let (class, icon) = match state {
                Rung::Reached => ("ui-ladder__rung ui-ladder__rung--reached", IconName::CheckCircle),
                Rung::Current => ("ui-ladder__rung", IconName::CircleHalf),
                Rung::Locked => ("ui-ladder__rung ui-ladder__rung--locked", IconName::Circle),
            };
            view! {
                <li class=class>
                    <Icon name=icon />
                    <span class="ui-ladder__title">{title}</span>
                    {(state == Rung::Current).then(|| view! { <Tag kind=TagKind::Accent>"Actuel"</Tag> })}
                    <span class="ui-ladder__xp">{format!("{} XP", format_xp(*xp))}</span>
                </li>
            }
        })
        .collect_view();
    view! { <ol class="ui-ladder" aria-label="Échelle des titres">{rungs}</ol> }
}

#[cfg(test)]
mod tests {
    use gamecloud_shared::roles::GlobalRank;

    use super::{rung, Rung};

    #[test]
    fn rungs_below_the_viewer_are_reached() {
        let me = Some(GlobalRank::Expert);
        assert_eq!(rung(GlobalRank::Apprentice, me), Rung::Reached);
        assert_eq!(rung(GlobalRank::Expert, me), Rung::Current);
        assert_eq!(rung(GlobalRank::Legend, me), Rung::Locked);
        assert_eq!(rung(GlobalRank::Initiate, None), Rung::Locked);
    }
}

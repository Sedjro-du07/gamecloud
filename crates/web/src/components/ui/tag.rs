//! Tag — 12px capitals in an outline.
//!
//! Neutral for categories and states; accent for the member's title and
//! for anything achieved.

use leptos::prelude::*;

use super::{
    icon::{Icon, IconName},
    vocab::{track_icon_by_id, track_name},
};

/// A tag's emphasis.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TagKind {
    /// Categories, states.
    #[default]
    Neutral,
    /// The member's title, something achieved.
    Accent,
}

/// A tag.
#[component]
pub fn Tag(
    /// Neutral (default) or accent.
    #[prop(optional)]
    kind: TagKind,
    /// Icon before the text; it says the state, so colour does not have to.
    #[prop(optional)]
    icon: Option<IconName>,
    /// The text.
    children: Children,
) -> impl IntoView {
    let class = match kind {
        TagKind::Neutral => "ui-tag",
        TagKind::Accent => "ui-tag ui-tag--accent",
    };
    view! {
        <span class=class>
            {icon.map(|name| view! { <Icon name /> })}
            {children()}
        </span>
    }
}

/// A track, by its identifier: its icon and its name, no colour.
#[component]
#[allow(clippy::needless_pass_by_value)] // Leptos prop convention
pub fn TrackTag(
    /// The track's identifier, e.g. "GameDesign".
    #[prop(into)]
    track: String,
) -> impl IntoView {
    let icon = track_icon_by_id(&track);
    let name = track_name(&track);
    view! {
        <span class="ui-tag">
            {icon.map(|name| view! { <Icon name /> })}
            {name}
        </span>
    }
}

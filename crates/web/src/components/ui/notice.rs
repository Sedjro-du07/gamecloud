//! Notice — something the page wants read before the rest.
//!
//! Information and success take the accent edge. An error keeps a neutral
//! edge: its icon and its words, in the error hue at 13px, carry it.

use leptos::prelude::*;

use super::{
    icon::{Icon, IconName, IconSize},
    sound::{play, Sound},
};

/// What kind of notice.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NoticeKind {
    /// Something to know or do.
    #[default]
    Info,
    /// It worked.
    Success,
    /// It did not, and why.
    Error,
}

/// A notice.
#[component]
pub fn Notice(
    /// Info (default), success or error.
    #[prop(optional)]
    kind: NoticeKind,
    /// The sound it makes when it appears, instead of the kind's own:
    /// XP earned, a track joined.
    #[prop(optional)]
    sound: Option<Sound>,
    /// The message, and an action if there is one.
    children: Children,
) -> impl IntoView {
    let cue = sound.or(match kind {
        NoticeKind::Info => None,
        NoticeKind::Success => Some(Sound::Success),
        NoticeKind::Error => Some(Sound::Error),
    });
    Effect::new(move |_| {
        if let Some(cue) = cue {
            play(cue);
        }
    });
    let (class, icon, role) = match kind {
        NoticeKind::Info => ("ui-notice", IconName::Info, "status"),
        NoticeKind::Success => ("ui-notice", IconName::CheckCircle, "status"),
        NoticeKind::Error => ("ui-notice ui-notice--error", IconName::WarningCircle, "alert"),
    };
    view! {
        <div class=class role=role>
            <Icon name=icon size=IconSize::Medium />
            <div class="ui-notice__text">{children()}</div>
        </div>
    }
}

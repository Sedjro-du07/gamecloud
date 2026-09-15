//! Notice — something the page wants read before the rest.
//!
//! Information and success take the accent edge. An error keeps a neutral
//! edge: its icon and its words, in the error hue at 13px, carry it.

use leptos::prelude::*;

use super::icon::{Icon, IconName, IconSize};

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
    /// The message, and an action if there is one.
    children: Children,
) -> impl IntoView {
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

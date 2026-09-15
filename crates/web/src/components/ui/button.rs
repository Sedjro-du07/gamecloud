//! Button — an outline, never a fill.
//!
//! Primary: accent outline and accent text. Secondary: neutral outline.
//! Ghost: text only, tinted on hover.

use leptos::prelude::*;

use super::{
    icon::{Icon, IconName},
    DemoState,
};

/// How much a button asks for attention.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ButtonKind {
    /// The one action a screen is for.
    Primary,
    /// Any other action.
    #[default]
    Secondary,
    /// A quiet action beside others.
    Ghost,
}

impl ButtonKind {
    const fn class(self) -> &'static str {
        match self {
            Self::Primary => "ui-btn ui-btn--primary",
            Self::Secondary => "ui-btn",
            Self::Ghost => "ui-btn ui-btn--ghost",
        }
    }
}

/// A button. Event listeners set on it (`on:click`) reach the element.
#[component]
pub fn Button(
    /// Primary, secondary (default) or ghost.
    #[prop(optional)]
    kind: ButtonKind,
    /// Icon before the label.
    #[prop(optional)]
    icon: Option<IconName>,
    /// Icon after the label.
    #[prop(optional)]
    trailing_icon: Option<IconName>,
    /// `button` (default) or `submit`.
    #[prop(default = "button")]
    button_type: &'static str,
    /// Disabled: dimmed, not clickable.
    #[prop(optional, into)]
    disabled: Signal<bool>,
    /// Forced state, for the design preview only.
    #[prop(optional)]
    state: DemoState,
    /// Icon only: the label is kept for screen readers.
    #[prop(optional)]
    hide_label: bool,
    /// The label.
    children: Children,
) -> impl IntoView {
    view! {
        <button
            class=format!("{}{}", kind.class(), state.class())
            type=button_type
            disabled=move || disabled.get()
        >
            {icon.map(|name| view! { <Icon name /> })}
            <span class=hide_label.then_some("ui-sr-only")>{children()}</span>
            {trailing_icon.map(|name| view! { <Icon name /> })}
        </button>
    }
}

/// A link that looks like a button, for navigation.
#[component]
pub fn ButtonLink(
    /// Primary, secondary (default) or ghost.
    #[prop(optional)]
    kind: ButtonKind,
    /// Destination.
    #[prop(into)]
    href: String,
    /// Icon before the label.
    #[prop(optional)]
    icon: Option<IconName>,
    /// Icon after the label.
    #[prop(optional)]
    trailing_icon: Option<IconName>,
    /// A server endpoint rather than a page: the browser navigates for real.
    #[prop(optional)]
    external: bool,
    /// Another site: opens in a new tab.
    #[prop(optional)]
    new_tab: bool,
    /// Icon only: the label is kept for screen readers.
    #[prop(optional)]
    hide_label: bool,
    /// The label.
    children: Children,
) -> impl IntoView {
    view! {
        <a
            class=kind.class()
            href=href
            rel=if new_tab { Some("external noopener") } else { external.then_some("external") }
            target=new_tab.then_some("_blank")
        >
            {icon.map(|name| view! { <Icon name /> })}
            <span class=hide_label.then_some("ui-sr-only")>{children()}</span>
            {trailing_icon.map(|name| view! { <Icon name /> })}
        </a>
    }
}

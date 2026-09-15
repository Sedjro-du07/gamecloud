//! Segmented control — choose one of a few, all visible at once.
//!
//! For filters and small either/or choices. The chosen option takes the
//! text colour and the accent mark under it; no tinted ground.

use leptos::prelude::*;

use super::icon::{Icon, IconName};

/// One option.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Segment {
    /// Value passed back when chosen.
    pub value: &'static str,
    /// What it reads.
    pub label: &'static str,
    /// Optional icon before the label.
    pub icon: Option<IconName>,
}

/// An option with a label only.
#[must_use]
pub const fn segment(value: &'static str, label: &'static str) -> Segment {
    Segment { value, label, icon: None }
}

/// An option with an icon.
#[must_use]
pub const fn segment_with_icon(value: &'static str, label: &'static str, icon: IconName) -> Segment {
    Segment { value, label, icon: Some(icon) }
}

/// A segmented control.
#[component]
pub fn SegmentedControl(
    /// What is being chosen, for screen readers.
    label: &'static str,
    /// The options.
    options: Vec<Segment>,
    /// The chosen value.
    #[prop(into)]
    value: Signal<String>,
    /// Called with the value picked.
    on_change: Callback<String>,
) -> impl IntoView {
    view! {
        <div class="ui-segmented" role="group" aria-label=label>
            {options
                .into_iter()
                .map(|option| {
                    let picked = option.value;
                    view! {
                        <button
                            type="button"
                            class="ui-segmented__option"
                            aria-pressed=move || (value.get() == picked).to_string()
                            on:click=move |_| on_change.run(picked.to_string())
                        >
                            {option.icon.map(|name| view! { <Icon name /> })}
                            {option.label}
                        </button>
                    }
                })
                .collect_view()}
        </div>
    }
}

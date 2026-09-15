//! Page layout — the three patterns and what composes them.
//!
//! Every screen is a [`Page`] in one pattern: a dense list, a card grid, or
//! a detail page on the left. It opens with the same [`PageHeader`] — h1,
//! an optional sentence of context, actions to the right of the title, then
//! the filter bar — and is built from sections, clusters and stacks rather
//! than screen-specific layout.

use leptos::prelude::*;

use super::{
    button::{Button, ButtonKind},
    icon::{Icon, IconName, IconSize},
};

/// The three page patterns.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Pattern {
    /// One row per item, full content width.
    #[default]
    List,
    /// A grid of cards.
    Grid,
    /// One column on the left, for one thing and what can be done with it.
    Detail,
}

/// A screen.
#[component]
pub fn Page(
    /// List (default), grid or detail.
    #[prop(optional)]
    pattern: Pattern,
    /// Header, then content.
    children: Children,
) -> impl IntoView {
    let class = match pattern {
        Pattern::List | Pattern::Grid => "ui-page",
        Pattern::Detail => "ui-page ui-page--detail",
    };
    view! { <div class=class>{children()}</div> }
}

/// The header every screen opens with.
#[component]
pub fn PageHeader(
    /// The h1.
    #[prop(into)]
    title: String,
    /// Small capitals above the title: a step, what kind of page.
    #[prop(optional, into)]
    kicker: Option<String>,
    /// One sentence of context.
    #[prop(optional, into)]
    lead: Option<String>,
    /// The filter bar, under the title.
    #[prop(optional)]
    filters: Option<AnyView>,
    /// Actions, to the right of the title.
    #[prop(optional)]
    children: Option<Children>,
) -> impl IntoView {
    view! {
        <header class="ui-page__head">
            <div class="ui-page__title-row">
                <div class="ui-page__heading">
                    {kicker.map(|k| view! { <span class="ui-label">{k}</span> })}
                    <h1 class="ui-h1">{title}</h1>
                    {lead
                        .filter(|l| !l.is_empty())
                        .map(|l| view! { <p class="ui-body ui-muted">{l}</p> })}
                </div>
                {children.map(|c| view! { <div class="ui-page__actions">{c()}</div> })}
            </div>
            {filters}
        </header>
    }
}

/// Filters under a page header.
#[component]
pub fn FilterBar(
    /// What is being filtered, for screen readers.
    label: &'static str,
    /// Segmented controls, inline fields.
    children: Children,
) -> impl IntoView {
    view! {
        <div class="ui-filters" role="group" aria-label=label>
            {children()}
        </div>
    }
}

/// Previous / label / next, e.g. a month.
#[component]
pub fn Stepper(
    /// What is shown.
    #[prop(into)]
    label: Signal<String>,
    /// Name of the step back, for screen readers.
    previous: &'static str,
    /// Name of the step forward.
    next: &'static str,
    /// Called with -1 or 1.
    on_step: Callback<i32>,
) -> impl IntoView {
    view! {
        <div class="ui-stepper">
            <Button kind=ButtonKind::Ghost icon=IconName::CaretLeft hide_label=true on:click=move |_| on_step.run(-1)>
                {previous}
            </Button>
            <span class="ui-stepper__label" aria-live="polite">{move || label.get()}</span>
            <Button kind=ButtonKind::Ghost icon=IconName::CaretRight hide_label=true on:click=move |_| on_step.run(1)>
                {next}
            </Button>
        </div>
    }
}

/// A titled block of a page.
#[component]
pub fn Section(
    /// The h2.
    #[prop(into)]
    title: String,
    /// A sentence under the title.
    #[prop(optional, into)]
    lead: Option<String>,
    /// A count or state beside the title, e.g. "3 / 15".
    #[prop(optional, into)]
    meta: Option<String>,
    /// A link or button at the end of the heading row.
    #[prop(optional)]
    action: Option<AnyView>,
    /// Content.
    children: Children,
) -> impl IntoView {
    view! {
        <section class="ui-section">
            <div class="ui-section__head">
                <div class="ui-section__heading">
                    <h2 class="ui-h2">{title}</h2>
                    {lead.map(|l| view! { <p class="ui-body ui-muted">{l}</p> })}
                </div>
                {meta.map(|m| view! { <span class="ui-meta">{m}</span> })}
                {action}
            </div>
            {children()}
        </section>
    }
}

/// Things side by side that wrap: tags, buttons.
#[component]
pub fn Cluster(children: Children) -> impl IntoView {
    view! { <div class="ui-cluster">{children()}</div> }
}

/// Space between stacked things.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Gap {
    /// Related lines.
    Tight,
    /// Blocks of one section.
    #[default]
    Normal,
    /// Sections.
    Loose,
}

/// Things one under the other.
#[component]
pub fn Stack(
    /// Space between them.
    #[prop(optional)]
    gap: Gap,
    /// Keep to a reading width.
    #[prop(optional)]
    reading: bool,
    /// Content.
    children: Children,
) -> impl IntoView {
    let mut class = String::from(match gap {
        Gap::Tight => "ui-stack ui-stack--tight",
        Gap::Normal => "ui-stack",
        Gap::Loose => "ui-stack ui-stack--loose",
    });
    if reading {
        class.push_str(" ui-stack--reading");
    }
    view! { <div class=class>{children()}</div> }
}

/// A main column and a narrower one; one column on small screens.
#[component]
pub fn Split(children: Children) -> impl IntoView {
    view! { <div class="ui-split">{children()}</div> }
}

/// A surface for a form that opened or the tools of one item.
#[component]
pub fn Panel(children: Children) -> impl IntoView {
    view! { <div class="ui-panel">{children()}</div> }
}

/// Content shown only when asked for.
#[component]
pub fn Disclosure(
    /// What opens.
    #[prop(into)]
    summary: String,
    /// Content.
    children: Children,
) -> impl IntoView {
    view! {
        <details class="ui-disclosure">
            <summary class="ui-disclosure__summary">
                <span class="ui-disclosure__caret"><Icon name=IconName::CaretRight /></span>
                {summary}
            </summary>
            {children()}
        </details>
    }
}

/// Label-over-value pairs.
#[component]
pub fn Facts(children: Children) -> impl IntoView {
    view! { <dl class="ui-facts">{children()}</dl> }
}

/// One label and its value.
#[component]
pub fn Fact(
    /// Label.
    label: &'static str,
    /// Value.
    #[prop(into)]
    value: String,
) -> impl IntoView {
    view! {
        <div>
            <dt class="ui-label">{label}</dt>
            <dd>{value}</dd>
        </div>
    }
}

/// A short sequence, in order.
#[component]
pub fn Steps(children: Children) -> impl IntoView {
    view! { <ol class="ui-steps">{children()}</ol> }
}

/// One step.
#[component]
pub fn Step(
    /// What the step is about.
    icon: IconName,
    /// Its name.
    title: &'static str,
    /// One or two sentences.
    children: Children,
) -> impl IntoView {
    view! {
        <li>
            <Icon name=icon size=IconSize::Medium />
            <h3 class="ui-h3">{title}</h3>
            <p class="ui-body ui-muted">{children()}</p>
        </li>
    }
}

/// A grid of cards.
#[component]
pub fn CardGrid(children: Children) -> impl IntoView {
    view! { <div class="ui-grid">{children()}</div> }
}

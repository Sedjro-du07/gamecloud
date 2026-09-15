//! Dense list — one row per item.
//!
//! A row reads left to right: a lead (state icon, rank, time), the title
//! with a meta line and anything that belongs under it, and an end column
//! for the value or the actions. Rows are separated by rules that fade at
//! both ends; there is no card per item.

use leptos::prelude::*;

/// A list of rows.
#[component]
pub fn DenseList(
    /// What the list is, for screen readers.
    #[prop(optional)]
    label: Option<&'static str>,
    /// Rows and group labels.
    children: Children,
) -> impl IntoView {
    view! {
        <ul class="ui-list" role="list" aria-label=label>
            {children()}
        </ul>
    }
}

/// A label over the rows that follow it: a day, a state.
#[component]
pub fn ListGroup(
    /// The label.
    #[prop(into)]
    label: String,
) -> impl IntoView {
    view! { <li class="ui-list__group ui-label">{label}</li> }
}

/// One row.
#[component]
pub fn ListRow(
    /// State icon, rank, time.
    #[prop(optional)]
    lead: Option<AnyView>,
    /// What the item is called.
    #[prop(into)]
    title: String,
    /// Makes the title a link.
    #[prop(optional, into)]
    href: Option<String>,
    /// The link leaves the platform: new tab.
    #[prop(optional)]
    external: bool,
    /// The line under the title: dates, counts, author.
    #[prop(optional, into)]
    meta: Option<String>,
    /// Value or actions, at the end of the row.
    #[prop(optional)]
    end: Option<AnyView>,
    /// Past, locked, cancelled: quieter.
    #[prop(optional)]
    dimmed: bool,
    /// Anything else under the title: tags, text, a panel.
    #[prop(optional)]
    children: Option<Children>,
) -> impl IntoView {
    let class = if dimmed {
        "ui-list__row ui-list__row--dimmed"
    } else {
        "ui-list__row"
    };
    let title = match href {
        Some(href) if external => view! {
            <a class="ui-list__title" href=href rel="external noopener" target="_blank">{title}</a>
        }
        .into_any(),
        Some(href) => view! { <a class="ui-list__title" href=href>{title}</a> }.into_any(),
        None => view! { <span class="ui-list__title">{title}</span> }.into_any(),
    };
    view! {
        <li class=class>
            {lead.map(|l| view! { <div class="ui-list__lead">{l}</div> })}
            <div class="ui-list__main">
                {title}
                {meta
                    .filter(|m| !m.is_empty())
                    .map(|m| view! { <span class="ui-list__meta">{m}</span> })}
                {children.map(|c| c())}
            </div>
            {end.map(|e| view! { <div class="ui-list__end">{e}</div> })}
        </li>
    }
}

/// A number at the end of a row: XP, a count.
#[component]
pub fn RowValue(
    /// The value.
    #[prop(into)]
    text: String,
) -> impl IntoView {
    view! { <span class="ui-list__value">{text}</span> }
}

/// A paragraph under a row's title: a description, a review.
#[component]
pub fn RowText(
    /// The text.
    #[prop(into)]
    text: String,
) -> impl IntoView {
    view! { <p class="ui-list__text">{text}</p> }
}

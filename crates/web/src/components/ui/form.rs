//! Form — fields in columns, actions on their own row.
//!
//! The element stays a native `<form>`: plain uploads post without
//! JavaScript. Attributes and listeners set on the component (`attr:action`,
//! `attr:method`, `on:submit`) reach it.

use leptos::prelude::*;

/// A form.
#[component]
pub fn Form(
    /// Fields and a [`FormActions`] row.
    children: Children,
) -> impl IntoView {
    view! { <form class="ui-form">{children()}</form> }
}

/// The row of buttons at the end of a form.
#[component]
pub fn FormActions(children: Children) -> impl IntoView {
    view! { <div class="ui-form__actions">{children()}</div> }
}

/// Anything in a form that is not a field and takes the whole row: a
/// choice between two ways of filling it, a sentence.
#[component]
pub fn FormRow(children: Children) -> impl IntoView {
    view! { <div class="ui-form__full">{children()}</div> }
}

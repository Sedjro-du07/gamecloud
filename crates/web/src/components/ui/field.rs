//! Field — a native control with its label above.
//!
//! The control itself is passed as children, so it stays the browser's own
//! element (input, select, textarea) and keeps working without JavaScript.
//! Give it `class="ui-control"`, the field's `id`, and, when a hint or an
//! error is shown, `aria-describedby` pointing at `<id>-hint` / `<id>-error`
//! and `aria-invalid="true"` for the error.

use leptos::prelude::*;

use super::icon::{Icon, IconName};

/// A labelled field.
#[component]
pub fn Field(
    /// The control's `id`, which the label points at.
    #[prop(into)]
    id: String,
    /// Label, above the control.
    #[prop(into)]
    label: String,
    /// Help under the control.
    #[prop(optional, into)]
    hint: Option<String>,
    /// What is wrong and how to fix it.
    #[prop(optional, into)]
    error: Option<String>,
    /// Dims the whole field.
    #[prop(optional)]
    disabled: bool,
    /// Takes the whole row of a form.
    #[prop(optional)]
    wide: bool,
    /// Label beside the control, for a filter bar.
    #[prop(optional)]
    inline: bool,
    /// The native control.
    children: Children,
) -> impl IntoView {
    let hint_id = format!("{id}-hint");
    let error_id = format!("{id}-error");
    let mut class = String::from("ui-field");
    for (on, modifier) in [(disabled, " ui-field--disabled"), (wide, " ui-field--wide"), (inline, " ui-field--inline")] {
        if on {
            class.push_str(modifier);
        }
    }
    view! {
        <div class=class>
            <label class="ui-field__label" for=id>{label}</label>
            {children()}
            {hint.map(|h| view! { <p class="ui-field__hint" id=hint_id>{h}</p> })}
            {error.map(|e| view! {
                <p class="ui-field__error" id=error_id role="alert">
                    <Icon name=IconName::WarningCircle />
                    <span>{e}</span>
                </p>
            })}
        </div>
    }
}

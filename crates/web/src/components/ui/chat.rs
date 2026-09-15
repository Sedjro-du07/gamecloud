//! Conversation — messages, newest nearest the composer.

use leptos::prelude::*;

/// Who wrote a message, as the viewer sees it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum MessageSide {
    /// Somebody else.
    #[default]
    Theirs,
    /// The viewer.
    Own,
    /// An answer being written.
    Typing,
}

/// The conversation. Give it the messages newest first: it lays them out
/// bottom-up, so the latest sits above the composer and stays in view.
#[component]
pub fn ChatLog(
    /// What the conversation is, for screen readers.
    label: &'static str,
    /// Messages, newest first.
    children: Children,
) -> impl IntoView {
    view! {
        <ol class="ui-chat" aria-label=label aria-live="polite">
            {children()}
        </ol>
    }
}

/// One message.
#[component]
pub fn Message(
    /// Whose.
    #[prop(optional)]
    side: MessageSide,
    /// Author, time, delivery.
    #[prop(into)]
    meta: String,
    /// The text.
    #[prop(into)]
    body: String,
) -> impl IntoView {
    let class = match side {
        MessageSide::Theirs => "ui-message",
        MessageSide::Own => "ui-message ui-message--own",
        MessageSide::Typing => "ui-message ui-message--typing",
    };
    view! {
        <li class=class>
            <p class="ui-message__body">{body}</p>
            <span class="ui-message__meta">{meta}</span>
        </li>
    }
}

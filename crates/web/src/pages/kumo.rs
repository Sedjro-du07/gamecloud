//! Talking to Kumo from the platform.
//!
//! Anybody may write, with or without an account. Somebody who shares no
//! Discord server with the GameCloud OS bot cannot write to it in private,
//! so this page is how they reach Kumo; members may use it too. Behind the
//! page the relay is the same: the platform stores the message, the bot
//! posts it where Kumo reads, and files Kumo's answer back here.
//!
//! Kumo answers in its own time, so the conversation is looked at again
//! every few seconds while the page is open, and it is kept: a visitor
//! finds it again on the same browser, a member on any.

use std::time::Duration;

use leptos::prelude::*;

use crate::{
    api::{ChatMessage, KumoChatView},
    server_fns::{get_kumo_chat, send_kumo_message},
};

/// How often the conversation is looked at again.
const REFRESH: Duration = Duration::from_secs(4);

/// A server function error as a person should read it.
fn human(error: &ServerFnError) -> String {
    let text = error.to_string();
    [
        "error running server function: ",
        "Error running server function: ",
        "ServerFnError: ",
    ]
    .iter()
    .find_map(|prefix| text.strip_prefix(prefix))
    .map_or_else(|| text.clone(), str::to_string)
}

/// One message.
#[component]
#[allow(clippy::needless_pass_by_value)] // Leptos prop convention
fn Bubble(
    /// The message.
    message: ChatMessage,
) -> impl IntoView {
    // An answer is signed by whoever wrote it: Kumo, or a Bureau member
    // who answered in its place.
    let (class, who) = if message.from_kumo {
        ("gc-msg gc-msg--kumo", message.author.clone())
    } else {
        ("gc-msg gc-msg--me", "Toi".to_string())
    };
    let status = match message.status.as_str() {
        "pending" => " · en cours d'envoi",
        "relayed" => " · transmis à Kumo",
        "failed" => " · non transmis, réessaie plus tard",
        _ => "",
    };
    view! {
        <li class=class>
            <p class="gc-msg__body">{message.body}</p>
            <span class="gc-msg__meta">{who} " · " {message.when} {status}</span>
        </li>
    }
}

/// The conversation so far, newest message nearest the composer.
#[component]
fn Conversation(
    /// The conversation, refreshed by the page.
    chat: Resource<Result<KumoChatView, ServerFnError>>,
) -> impl IntoView {
    view! {
        <div class="gc-kumo__chat" aria-live="polite">
            <Transition fallback=|| view! { <p class="gc-empty">"Chargement…"</p> }>
                {move || {
                    chat.get()
                        .map(|result| match result {
                            Err(e) => view! { <p class="gc-kumo__error">{human(&e)}</p> }.into_any(),
                            Ok(view) if view.messages.is_empty() => {
                                view! { <p class="gc-kumo__empty">"Dis bonjour à Kumo 👋"</p> }
                                    .into_any()
                            }
                            Ok(view) => {
                                // Passed on and not answered yet.
                                let waiting = view
                                    .messages
                                    .last()
                                    .is_some_and(|m| m.status == "relayed");
                                // Newest first: the list is laid out bottom-up,
                                // so the latest message sits above the composer.
                                view! {
                                    <ol class="gc-kumo__messages">
                                        {waiting
                                            .then(|| {
                                                view! {
                                                    <li class="gc-msg gc-msg--typing">"Kumo va répondre…"</li>
                                                }
                                            })}
                                        {view
                                            .messages
                                            .into_iter()
                                            .rev()
                                            .map(|message| view! { <Bubble message /> })
                                            .collect_view()}
                                    </ol>
                                }
                                    .into_any()
                            }
                        })
                }}
            </Transition>
        </div>
    }
}

/// Contact Kumo page.
#[component]
pub fn KumoPage() -> impl IntoView {
    let chat = Resource::new(|| (), |()| async { get_kumo_chat().await });
    let (draft, set_draft) = signal(String::new());
    let (error, set_error) = signal(Option::<String>::None);

    // Look again every few seconds while the page is open. Effects only
    // run in the browser, which is the only place an interval makes sense.
    Effect::new(move |started: Option<()>| {
        if started.is_none() {
            if let Ok(handle) = set_interval_with_handle(move || chat.refetch(), REFRESH) {
                on_cleanup(move || handle.clear());
            }
        }
    });

    let send = Action::new(move |text: &String| {
        let text = text.clone();
        async move { send_kumo_message(text).await }
    });
    Effect::new(move |_| {
        if let Some(result) = send.value().get() {
            match result {
                Ok(()) => {
                    set_draft.set(String::new());
                    set_error.set(None);
                    chat.refetch();
                }
                Err(e) => set_error.set(Some(human(&e))),
            }
        }
    });
    let submit = move || {
        let text = draft.get_untracked();
        if text.trim().is_empty() || send.pending().get_untracked() {
            return;
        }
        send.dispatch(text);
    };

    view! {
        <section class="gc-kumo">
            <h1>"💬 Contacter Kumo"</h1>
            <p class="gc-kumo__intro">
                "Une question sur l'association ? Écris-la ici, pas besoin de compte. Kumo te
                 répond dans cette conversation : garde la page ouverte, ou reviens plus tard
                 sur ce navigateur."
            </p>

            <Conversation chat />

            <form
                class="gc-kumo__composer"
                on:submit=move |ev| {
                    ev.prevent_default();
                    submit();
                }
            >
                <textarea
                    rows="2"
                    maxlength="1000"
                    placeholder="Ton message… (Entrée pour envoyer, Maj+Entrée pour aller à la ligne)"
                    aria-label="Ton message pour Kumo"
                    prop:value=move || draft.get()
                    on:input=move |ev| set_draft.set(event_target_value(&ev))
                    on:keydown=move |ev| {
                        if ev.key() == "Enter" && !ev.shift_key() {
                            ev.prevent_default();
                            submit();
                        }
                    }
                ></textarea>
                <button class="gc-btn gc-btn--primary" type="submit" disabled=move || send.pending().get()>
                    {move || if send.pending().get() { "Envoi…" } else { "Envoyer" }}
                </button>
            </form>
            {move || error.get().map(|e| view! { <p class="gc-kumo__error">{e}</p> })}

            <p class="gc-kumo__hint">
                "Déjà sur Discord ? Tu peux aussi écrire en privé au bot GameCloud OS : "
                <a href="/api/contact/kumo" rel="external noopener" target="_blank">
                    "ouvrir la conversation"
                </a>
                "."
            </p>
        </section>
    }
}

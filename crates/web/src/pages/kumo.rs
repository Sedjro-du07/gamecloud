//! Talking to Kumo from the platform — a detail page.
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
    components::ui::{
        Button, ButtonKind, ChatLog, EmptyState, ErrorState, ErrorText, Field, Form, FormActions,
        FormRow, IconName, Message, MessageSide, Page, PageHeader, Pattern, RowsSkeleton,
    },
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

/// Who wrote a message, when, and where it stands.
fn message_meta(message: &ChatMessage) -> String {
    // An answer is signed by whoever wrote it: Kumo, or a Bureau member.
    let who = if message.from_kumo { message.author.as_str() } else { "Toi" };
    let status = match message.status.as_str() {
        "pending" => " · en cours d'envoi",
        "relayed" => " · transmis à Kumo",
        "failed" => " · non transmis, réessaie plus tard",
        _ => "",
    };
    format!("{who} · {}{status}", message.when)
}

/// The conversation so far.
#[component]
fn Conversation(
    /// The conversation, refreshed by the page.
    chat: Resource<Result<KumoChatView, ServerFnError>>,
) -> impl IntoView {
    view! {
        <Transition fallback=|| view! { <RowsSkeleton rows=3 /> }>
            {move || chat.get().map(|result| match result {
                Err(e) => view! {
                    <ErrorState message=human(&e) on_retry=Callback::new(move |()| chat.refetch()) />
                }
                .into_any(),
                Ok(view) if view.messages.is_empty() => view! {
                    <EmptyState icon=IconName::ChatCircle title="Aucun message pour l'instant"
                        body="Dis bonjour à Kumo : ta question part dès que tu l'envoies." />
                }
                .into_any(),
                Ok(view) => {
                    // Passed on and not answered yet.
                    let waiting = view.messages.last().is_some_and(|m| m.status == "relayed");
                    view! {
                        <ChatLog label="Conversation avec Kumo">
                            {waiting.then(|| view! {
                                <Message side=MessageSide::Typing meta="Kumo" body="Kumo va répondre…" />
                            })}
                            {view.messages.into_iter().rev().map(|message| {
                                let side = if message.from_kumo { MessageSide::Theirs } else { MessageSide::Own };
                                view! { <Message side meta=message_meta(&message) body=message.body /> }
                            }).collect_view()}
                        </ChatLog>
                    }
                    .into_any()
                }
            })}
        </Transition>
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
        <Page pattern=Pattern::Detail>
            <PageHeader
                title="Contacter Kumo"
                lead="Une question sur l'association ? Écris-la ici, pas besoin de compte. Kumo répond dans cette conversation : garde la page ouverte, ou reviens plus tard sur ce navigateur."
            />
            <Conversation chat />
            <Form on:submit=move |ev| {
                ev.prevent_default();
                submit();
            }>
                <Field id="kumo-message" label="Ton message" hint="Entrée pour envoyer, Maj+Entrée pour aller à la ligne." wide=true>
                    <textarea
                        id="kumo-message"
                        class="ui-control"
                        rows="3"
                        maxlength="1000"
                        aria-describedby="kumo-message-hint"
                        prop:value=move || draft.get()
                        on:input=move |ev| set_draft.set(event_target_value(&ev))
                        on:keydown=move |ev| {
                            if ev.key() == "Enter" && !ev.shift_key() {
                                ev.prevent_default();
                                submit();
                            }
                        }
                    ></textarea>
                </Field>
                {move || error.get().map(|message| view! { <FormRow><ErrorText message /></FormRow> })}
                <FormActions>
                    <Button kind=ButtonKind::Primary button_type="submit" icon=IconName::PaperPlaneRight
                        disabled=Signal::derive(move || send.pending().get())>
                        {move || if send.pending().get() { "Envoi…" } else { "Envoyer" }}
                    </Button>
                    <span class="ui-meta">
                        "Déjà sur Discord ? "
                        <a href="/api/contact/kumo" rel="external noopener" target="_blank">"Écris en privé au bot GameCloud OS"</a>
                        "."
                    </span>
                </FormActions>
            </Form>
        </Page>
    }
}

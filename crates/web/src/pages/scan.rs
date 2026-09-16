//! QR scan page.
//!
//! Scanning is a phone activity, and until now the only way to claim
//! attendance was to POST JSON by hand. This page gives members a field
//! they can paste into, and — where the browser allows it — a live
//! camera scan.
//!
//! There is no in-page camera. The QR encodes a link to this page with
//! the token in the query string, so a member points their phone's own
//! camera app at the projector, taps the notification, and the claim
//! runs on arrival. That works on every phone, needs no camera
//! permission from the browser, and sidesteps `BarcodeDetector` — which
//! Firefox and Safari do not implement.
//!
//! The paste field stays for the laptop case and for anyone whose
//! camera app is being difficult.

use leptos::prelude::*;
use leptos_router::hooks::use_query_map;

use crate::{
    components::ui::{
        Button, ButtonKind, Field, Form, FormActions, IconName, Notice, NoticeKind, Page,
        PageHeader, Pattern, Sound,
    },
    server_fns::scan_qr,
};

/// QR scan page.
#[component]
pub fn ScanPage() -> impl IntoView {
    let (token, set_token) = signal(String::new());
    let (result, set_result) = signal(Option::<Result<String, String>>::None);
    let query = use_query_map();

    let submit = Action::new(move |token: &String| {
        let token = token.clone();
        async move {
            match scan_qr(token).await {
                Ok(message) => Ok(message),
                Err(e) => Err(friendly_error(&e.to_string())),
            }
        }
    });

    Effect::new(move |_| {
        if let Some(outcome) = submit.value().get() {
            if outcome.is_ok() {
                set_token.set(String::new());
            }
            set_result.set(Some(outcome));
        }
    });

    // Arriving from a scanned QR: claim straight away rather than
    // showing a pre-filled box and asking the member to press a button
    // they have no reason to doubt.
    Effect::new(move |already_ran: Option<bool>| {
        if already_ran == Some(true) {
            return true;
        }
        let Some(from_qr) = query.read().get("token") else {
            return false;
        };
        let value = extract_token(&from_qr);
        if value.is_empty() {
            return false;
        }
        set_token.set(value.clone());
        submit.dispatch(value);
        true
    });

    view! {
        <Page pattern=Pattern::Detail>
            <PageHeader
                title="Présences"
                lead="Vise le QR affiché à l'écran avec l'appareil photo de ton téléphone : le lien s'ouvre et ta présence est enregistrée toute seule. Sinon, colle le code ici."
            />

            {move || {
                result
                    .get()
                    .map(|outcome| match outcome {
                        Ok(message) => view! { <Notice kind=NoticeKind::Success sound=Sound::Xp>{message}</Notice> }.into_any(),
                        Err(message) => view! { <Notice kind=NoticeKind::Error>{message}</Notice> }.into_any(),
                    })
            }}

            <Form on:submit=move |ev| {
                ev.prevent_default();
                let value = extract_token(&token.get());
                if !value.is_empty() {
                    submit.dispatch(value);
                }
            }>
                <Field
                    id="presence-code"
                    label="Code"
                    hint="Le lien complet fonctionne aussi. Chaque membre présent scanne le même code, une fois par personne, tant qu'il est valable."
                    wide=true
                >
                    <input
                        id="presence-code"
                        class="ui-control"
                        type="text"
                        autocomplete="off"
                        placeholder="eyJhbGciOi…"
                        aria-describedby="presence-code-hint"
                        prop:value=move || token.get()
                        on:input=move |ev| set_token.set(event_target_value(&ev))
                    />
                </Field>
                <FormActions>
                    <Button
                        kind=ButtonKind::Primary
                        button_type="submit"
                        icon=IconName::QrCode
                        disabled=Signal::derive(move || submit.pending().get() || token.get().trim().is_empty())
                    >
                        {move || if submit.pending().get() { "Validation…" } else { "Valider ma présence" }}
                    </Button>
                </FormActions>
            </Form>
        </Page>
    }
}

/// Accept either a bare token or a full URL carrying one.
///
/// Members paste whatever their camera app produced, which is usually
/// a URL, so meeting them halfway costs three lines and saves a support
/// question every Monday.
fn extract_token(input: &str) -> String {
    let trimmed = input.trim();
    if let Some((_, query)) = trimmed.split_once("?token=") {
        return query.split('&').next().unwrap_or("").to_string();
    }
    if let Some((_, fragment)) = trimmed.split_once('#') {
        if !fragment.is_empty() && trimmed.starts_with("http") {
            return fragment.to_string();
        }
    }
    trimmed.to_string()
}

/// Turn a server error into something a member can act on.
fn friendly_error(raw: &str) -> String {
    let lower = raw.to_lowercase();
    if lower.contains("already scanned") || lower.contains("déjà") {
        "Tu as déjà scanné ce code.".to_string()
    } else if lower.contains("scan limit") || lower.contains("limit") {
        "Ce code a atteint sa limite de scans.".to_string()
    } else if lower.contains("no longer valid") || lower.contains("qr") {
        "Ce code est expiré ou invalide.".to_string()
    } else if lower.contains("not signed in") {
        "Connecte-toi avec Discord avant de scanner.".to_string()
    } else {
        "Le scan a échoué. Vérifie le code et réessaie.".to_string()
    }
}

#[cfg(all(test, feature = "ssr"))]
mod tests {
    use super::{extract_token, friendly_error};

    #[test]
    fn a_bare_token_passes_through() {
        assert_eq!(extract_token("  abc.def.ghi  "), "abc.def.ghi");
    }

    #[test]
    fn a_query_url_is_unwrapped() {
        assert_eq!(
            extract_token("https://gamecloud.bj/scan?token=abc.def.ghi"),
            "abc.def.ghi"
        );
    }

    #[test]
    fn extra_query_parameters_are_dropped() {
        assert_eq!(
            extract_token("https://gamecloud.bj/scan?token=abc&utm=qr"),
            "abc"
        );
    }

    #[test]
    fn a_fragment_url_is_unwrapped() {
        assert_eq!(extract_token("https://gamecloud.bj/scan#abc.def"), "abc.def");
    }

    #[test]
    fn a_token_containing_a_hash_is_not_mangled() {
        // Only http(s) inputs are treated as URLs, so a raw token that
        // happens to contain '#' survives intact.
        assert_eq!(extract_token("abc#def"), "abc#def");
    }

    #[test]
    fn errors_are_translated_for_members() {
        assert!(friendly_error("you have already scanned this code").contains("déjà"));
        assert!(friendly_error("this code has reached its scan limit").contains("limite"));
        assert!(friendly_error("not signed in").contains("Connecte-toi"));
        assert!(!friendly_error("some internal panic").is_empty());
    }
}

//! QR scan page.
//!
//! Scanning is a phone activity, and until now the only way to claim
//! attendance was to POST JSON by hand. This page gives members a field
//! they can paste into, and — where the browser allows it — a live
//! camera scan.
//!
//! The camera path uses the standard `BarcodeDetector` API, which is
//! available in Chrome and Android WebView but not in Firefox or
//! Safari. The manual field is therefore the primary interface, not a
//! fallback: it always works, on every device, including a laptop
//! reading a code off a projector.

use leptos::prelude::*;

use crate::server_fns::scan_qr;

/// QR scan page.
#[component]
pub fn ScanPage() -> impl IntoView {
    let (token, set_token) = signal(String::new());
    let (result, set_result) = signal(Option::<Result<String, String>>::None);

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

    view! {
        <section class="gc-scan">
            <h1>"Scanner un QR de présence"</h1>
            <p>
                "Colle ici le code affiché à l'écran, ou scanne-le avec
                 l'appareil photo de ton téléphone puis colle le lien."
            </p>

            {move || {
                result
                    .get()
                    .map(|outcome| match outcome {
                        Ok(message) => {
                            view! { <div class="gc-banner gc-banner--ok">{message}</div> }
                                .into_any()
                        }
                        Err(message) => {
                            view! { <div class="gc-banner gc-banner--warning">{message}</div> }
                                .into_any()
                        }
                    })
            }}

            <form
                class="gc-form"
                on:submit=move |ev| {
                    ev.prevent_default();
                    let value = extract_token(&token.get());
                    if !value.is_empty() {
                        submit.dispatch(value);
                    }
                }
            >
                <label class="gc-field">
                    <span>"Code"</span>
                    <input
                        type="text"
                        autocomplete="off"
                        placeholder="eyJhbGciOi…"
                        prop:value=move || token.get()
                        on:input=move |ev| set_token.set(event_target_value(&ev))
                    />
                </label>
                <button
                    class="gc-btn gc-btn--primary"
                    type="submit"
                    disabled=move || submit.pending().get() || token.get().trim().is_empty()
                >
                    {move || if submit.pending().get() { "Validation…" } else { "Valider ma présence" }}
                </button>
            </form>

            <p class="gc-scan__hint">
                "Chaque membre présent peut scanner le même code — il reste
                 valable jusqu'à son expiration. En revanche, une seule fois
                 par personne."
            </p>
        </section>
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

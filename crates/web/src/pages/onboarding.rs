//! Onboarding flow — email submission and OTP verification.
//!
//! Two pages mounted at `/onboarding/email` and `/onboarding/verify`.
//! Both use plain `<form>` POSTs sent as `application/x-www-form-urlencoded`,
//! which the matching server handlers in `routes::auth` accept directly.
//! On success, the server responds with a 303 redirect to the next
//! step. On error, the JSON error envelope is returned; we don't try
//! to render it inline here in phase 4 — phase 5 wires up the
//! WASM-side feedback path.

use leptos::prelude::*;

/// Email submission page.
#[component]
pub fn OnboardingEmailPage() -> impl IntoView {
    view! {
        <section class="gc-onboard">
            <h1>"Étape 1 sur 2 — Email Epitech"</h1>
            <p>
                "Entre ton adresse "
                <code>"prenom.nom@epitech.eu"</code>
                ". Nous t'enverrons un code à 6 chiffres pour confirmer."
            </p>
            <form action="/api/auth/email" method="post" class="gc-form">
                <label class="gc-field">
                    <span>"Email"</span>
                    <input
                        type="email"
                        name="email"
                        autocomplete="email"
                        required=true
                        placeholder="prenom.nom@epitech.eu"
                    />
                </label>
                <button class="gc-btn gc-btn--primary" type="submit">
                    "Envoyer le code"
                </button>
            </form>
            <p class="gc-onboard__hint">
                "Pas encore connecté ? "
                <a href="/api/auth/login" rel="external">"Se connecter avec Discord"</a>
                "."
            </p>
        </section>
    }
}

/// OTP verification page.
#[component]
pub fn OnboardingVerifyPage() -> impl IntoView {
    view! {
        <section class="gc-onboard">
            <h1>"Étape 2 sur 2 — Code reçu"</h1>
            <p>"Entre le code à 6 chiffres reçu par email. Il expire dans 15 minutes."</p>
            <form action="/api/auth/verify" method="post" class="gc-form">
                <label class="gc-field">
                    <span>"Code"</span>
                    <input
                        type="text"
                        name="code"
                        inputmode="numeric"
                        autocomplete="one-time-code"
                        minlength="6"
                        maxlength="6"
                        pattern="[0-9]{6}"
                        required=true
                        placeholder="000000"
                    />
                </label>
                <button class="gc-btn gc-btn--primary" type="submit">
                    "Valider"
                </button>
            </form>
            <p class="gc-onboard__hint">
                "Tu n'as pas reçu de code ? "
                <a href="/onboarding/email" rel="external">"Renvoyer un nouveau code"</a>
                "."
            </p>
        </section>
    }
}

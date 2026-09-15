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
///
/// Somebody who is not on the Discord server and has not been admitted
/// cannot sign up yet: the page sends them to the entrance tests instead
/// of offering a form the server would refuse.
#[component]
pub fn OnboardingEmailPage() -> impl IntoView {
    let me = Resource::new(|| (), |()| async { crate::server_fns::get_me().await });
    view! {
        <Suspense fallback=|| ()>
            {move || {
                me.get()
                    .map(|result| {
                        if result.ok().flatten().is_some_and(|u| u.is_candidate) {
                            view! {
                                <section class="gc-onboard">
                                    <h1>"Pas encore sur le serveur"</h1>
                                    <div class="gc-banner">
                                        "Tu n'es pas sur le serveur Discord de l'association. Pour
                                         t'inscrire, réussis d'abord un test d'entrée : une fois admis,
                                         tu reçois ton invitation et l'inscription s'ouvre."
                                    </div>
                                    <a class="gc-btn gc-btn--primary" href="/tests">
                                        "🎓 Voir les tests d'entrée"
                                    </a>
                                </section>
                            }
                                .into_any()
                        } else {
                            view! { <EmailForm /> }.into_any()
                        }
                    })
            }}
        </Suspense>
    }
}

/// The email form itself.
#[component]
fn EmailForm() -> impl IntoView {
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
            <div class="gc-onboard__hint gc-signin__row">
                <span>"Pas encore connecté ?"</span>
                <crate::components::discord_login::DiscordLogin />
            </div>
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

//! Onboarding — email submission and code verification, as detail pages.
//!
//! Two pages mounted at `/onboarding/email` and `/onboarding/verify`. Both
//! use plain `<form>` POSTs that the handlers in `routes::auth` accept
//! directly and answer with a redirect to the next step.

use leptos::prelude::*;

use crate::{
    components::ui::{
        Button, ButtonKind, ButtonLink, Cluster, Field, Form, FormActions, IconName, Notice, Page,
        PageHeader, PageSkeleton, Pattern,
    },
    server_fns::get_me,
};

/// Email submission page.
///
/// Somebody who is not on the Discord server and has not been admitted
/// cannot sign up yet: the page sends them to the entrance tests instead of
/// offering a form the server would refuse.
#[component]
pub fn OnboardingEmailPage() -> impl IntoView {
    let me = Resource::new(|| (), |()| async { get_me().await });
    view! {
        <Suspense fallback=|| view! { <Page pattern=Pattern::Detail><PageSkeleton /></Page> }>
            {move || me.get().map(|result| {
                if result.ok().flatten().is_some_and(|u| u.is_candidate) {
                    view! {
                        <Page pattern=Pattern::Detail>
                            <PageHeader kicker="Inscription" title="Pas encore sur le serveur" />
                            <Notice>
                                <span>"Pour t'inscrire, réussis d'abord un test d'entrée : une fois admis, tu reçois ton invitation au serveur et l'inscription s'ouvre."</span>
                                <ButtonLink kind=ButtonKind::Primary href="/tests" icon=IconName::GraduationCap>"Voir les tests d'entrée"</ButtonLink>
                            </Notice>
                        </Page>
                    }
                    .into_any()
                } else {
                    view! { <EmailForm /> }.into_any()
                }
            })}
        </Suspense>
    }
}

/// The email form itself.
#[component]
fn EmailForm() -> impl IntoView {
    view! {
        <Page pattern=Pattern::Detail>
            <PageHeader
                kicker="Étape 1 sur 2"
                title="Adresse Epitech"
                lead="Entre ton adresse prenom.nom@epitech.eu : tu reçois un code à 6 chiffres pour la confirmer."
            />
            <Form attr:action="/api/auth/email" attr:method="post">
                <Field id="inscription-email" label="Adresse email" wide=true>
                    <input id="inscription-email" class="ui-control" type="email" name="email" autocomplete="email"
                        required=true placeholder="prenom.nom@epitech.eu" />
                </Field>
                <FormActions>
                    <Button kind=ButtonKind::Primary button_type="submit" icon=IconName::Envelope>"Envoyer le code"</Button>
                </FormActions>
            </Form>
            <Cluster>
                <span class="ui-meta">"Pas encore connecté ?"</span>
                <ButtonLink href="/api/auth/login" external=true icon=IconName::DiscordLogo hide_label=true>
                    "Se connecter avec Discord"
                </ButtonLink>
            </Cluster>
        </Page>
    }
}

/// Code verification page.
#[component]
pub fn OnboardingVerifyPage() -> impl IntoView {
    view! {
        <Page pattern=Pattern::Detail>
            <PageHeader
                kicker="Étape 2 sur 2"
                title="Code reçu"
                lead="Entre le code à 6 chiffres reçu par email. Il expire au bout de 15 minutes."
            />
            <Form attr:action="/api/auth/verify" attr:method="post">
                <Field id="inscription-code" label="Code" wide=true>
                    <input id="inscription-code" class="ui-control" type="text" name="code" inputmode="numeric"
                        autocomplete="one-time-code" minlength="6" maxlength="6" pattern="[0-9]{6}"
                        required=true placeholder="000000" />
                </Field>
                <FormActions>
                    <Button kind=ButtonKind::Primary button_type="submit" icon=IconName::Key>"Valider"</Button>
                    <span class="ui-meta">
                        "Pas reçu de code ? "
                        <a href="/onboarding/email" rel="external">"En demander un nouveau"</a>
                        "."
                    </span>
                </FormActions>
            </Form>
        </Page>
    }
}

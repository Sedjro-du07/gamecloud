//! Empty, error, loading and signed-out states, built from the primitives.
//!
//! Every screen shows one of these instead of a blank area, a browser
//! default or a spinner.

use leptos::prelude::*;

use super::{
    button::{Button, ButtonKind, ButtonLink},
    card::CardSkeleton,
    icon::{Icon, IconName, IconSize},
    layout::CardGrid,
    sound::{play, Sound},
};

/// Nothing to show yet, and what happens next.
#[component]
pub fn EmptyState(
    /// What this is about.
    icon: IconName,
    /// What is empty.
    #[prop(into)]
    title: String,
    /// What will fill it, or what to do.
    #[prop(optional, into)]
    body: Option<String>,
    /// An action.
    #[prop(optional)]
    children: Option<Children>,
) -> impl IntoView {
    view! {
        <div class="ui-state">
            <Icon name=icon size=IconSize::Medium />
            <p class="ui-state__title">{title}</p>
            {body.map(|b| view! { <p class="ui-meta">{b}</p> })}
            {children.map(|c| c())}
        </div>
    }
}

/// An error sentence under what failed: icon and words.
#[component]
pub fn ErrorText(
    /// What went wrong and how to fix it.
    #[prop(into)]
    message: String,
) -> impl IntoView {
    Effect::new(move |_| play(Sound::Error));
    view! {
        <p class="ui-error" role="alert">
            <Icon name=IconName::WarningCircle />
            <span>{message}</span>
        </p>
    }
}

/// Loading failed: what went wrong, and a way to try again.
#[component]
pub fn ErrorState(
    /// What could not be loaded.
    #[prop(into)]
    message: String,
    /// Try again.
    on_retry: Callback<()>,
) -> impl IntoView {
    view! {
        <div class="ui-state" role="alert">
            <ErrorText message />
            <Button on:click=move |_| on_retry.run(())>"Réessayer"</Button>
        </div>
    }
}

/// A page that needs an account, seen by somebody signed out.
#[component]
pub fn SignInState(
    /// What signing in allows, e.g. "voir le calendrier".
    what: &'static str,
) -> impl IntoView {
    view! {
        <div class="ui-state">
            <Icon name=IconName::LockSimple size=IconSize::Medium />
            <p class="ui-state__title">{format!("Connecte-toi avec Discord pour {what}.")}</p>
            <div class="ui-cluster">
                <ButtonLink
                    kind=ButtonKind::Primary
                    href="/api/auth/login"
                    external=true
                    icon=IconName::DiscordLogo
                    hide_label=true
                >
                    "Se connecter avec Discord"
                </ButtonLink>
                <ButtonLink kind=ButtonKind::Ghost href="/kumo" icon=IconName::ChatCircle>
                    "Contacter Kumo"
                </ButtonLink>
            </div>
        </div>
    }
}

/// A page reserved for the association, seen by somebody who is not a
/// member yet: the two ways in, and where to write if neither fits.
#[component]
pub fn MembersOnlyState(
    /// What membership gives access to, e.g. "voir les projets".
    what: &'static str,
) -> impl IntoView {
    view! {
        <div class="ui-state">
            <Icon name=IconName::LockSimple size=IconSize::Medium />
            <p class="ui-state__title">"Réservé aux membres de l'association"</p>
            <p class="ui-meta">
                {format!("Il faut être membre pour {what}. Déjà sur le serveur Discord de l'association ? Connecte-toi, puis vérifie ton adresse Epitech. Sinon, le test d'entrée ouvre la porte.")}
            </p>
            <div class="ui-cluster">
                <ButtonLink
                    kind=ButtonKind::Primary
                    href="/api/auth/login"
                    external=true
                    icon=IconName::DiscordLogo
                    hide_label=true
                >
                    "Se connecter avec Discord"
                </ButtonLink>
                <ButtonLink href="/onboarding/email" icon=IconName::Envelope>
                    "Vérifier mon adresse"
                </ButtonLink>
                <ButtonLink kind=ButtonKind::Ghost href="/tests" icon=IconName::GraduationCap>
                    "Passer le test d'entrée"
                </ButtonLink>
            </div>
        </div>
    }
}

/// Rows of a list while it loads.
#[component]
pub fn RowsSkeleton(
    /// How many rows to suggest.
    #[prop(default = 3)]
    rows: usize,
) -> impl IntoView {
    view! {
        <div class="ui-rows-skeleton" aria-hidden="true">
            {(0..rows)
                .map(|_| {
                    view! {
                        <div class="ui-rows-skeleton__row">
                            <span class="ui-skeleton ui-skeleton--short"></span>
                            <span class="ui-skeleton ui-skeleton--line"></span>
                        </div>
                    }
                })
                .collect_view()}
        </div>
    }
}

/// A grid of cards while it loads.
#[component]
pub fn GridSkeleton(
    /// How many cards to suggest.
    #[prop(default = 6)]
    cards: usize,
) -> impl IntoView {
    view! {
        <CardGrid>
            {(0..cards).map(|_| view! { <CardSkeleton /> }).collect_view()}
        </CardGrid>
    }
}

/// A page's header and first rows while it loads.
#[component]
pub fn PageSkeleton() -> impl IntoView {
    view! {
        <div class="ui-page__head" aria-hidden="true">
            <span class="ui-skeleton ui-skeleton--short"></span>
            <span class="ui-skeleton ui-skeleton--title"></span>
        </div>
        <RowsSkeleton rows=4 />
    }
}

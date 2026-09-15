//! Card — kicker, title, body, meta.
//!
//! A surface with a 1px edge. Achieved (a milestone reached, a quest
//! validated) it gains an accent edge and a check, never a coloured fill.

use leptos::prelude::*;

use super::{
    icon::{Icon, IconName, IconSize},
    DemoState,
};

/// What sits at the top of a card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CardMedia {
    /// A real photo, blended into the dark ground.
    Photo {
        /// Image URL.
        src: String,
        /// Description of the image.
        alt: String,
    },
    /// The visual has not been supplied yet (design preview).
    Pending,
    /// There is no visual: the surface and the thing's icon.
    Missing(IconName),
}

/// A card.
#[component]
pub fn Card(
    /// Small capitals above the title: what kind of thing this is.
    #[prop(optional, into)]
    kicker: Option<String>,
    /// Title.
    #[prop(into)]
    title: String,
    /// Icon before the title.
    #[prop(optional)]
    icon: Option<IconName>,
    /// Line at the bottom: dates, counts, author.
    #[prop(optional, into)]
    meta: Option<String>,
    /// Milestone reached or quest validated.
    #[prop(optional)]
    achieved: bool,
    /// Makes the whole card a link.
    #[prop(optional, into)]
    href: Option<String>,
    /// Photo or pending visual.
    #[prop(optional)]
    media: Option<CardMedia>,
    /// Buttons at the bottom. Not with `href`: a link holds no buttons.
    #[prop(optional)]
    actions: Option<AnyView>,
    /// Forced state, for the design preview only.
    #[prop(optional)]
    state: DemoState,
    /// Body.
    #[prop(optional)]
    children: Option<Children>,
) -> impl IntoView {
    let mut class = String::from("ui-card");
    if href.is_some() {
        class.push_str(" ui-card--link");
    }
    if achieved {
        class.push_str(" ui-card--achieved");
    }
    class.push_str(state.class());

    let media = media.map(|m| match m {
        CardMedia::Photo { src, alt } => view! {
            <div class="ui-card__media"><img src=src alt=alt loading="lazy" /></div>
        }
        .into_any(),
        CardMedia::Pending => view! {
            <div class="ui-card__media">
                <div class="ui-card__placeholder">"Visuel à fournir"</div>
            </div>
        }
        .into_any(),
        CardMedia::Missing(icon) => view! {
            <div class="ui-card__media">
                <div class="ui-card__none"><Icon name=icon size=IconSize::Medium /></div>
            </div>
        }
        .into_any(),
    });
    let head = (kicker.is_some() || achieved).then(|| {
        view! {
            <div class="ui-card__head">
                {kicker.map(|k| view! { <span class="ui-card__kicker">{k}</span> })}
                {achieved.then(|| view! {
                    <span class="ui-card__status" title="Validé"><Icon name=IconName::Check /></span>
                })}
            </div>
        }
    });
    let body = children.map(|c| view! { <div class="ui-card__body">{c()}</div> });
    let meta = meta.map(|m| view! { <p class="ui-card__meta">{m}</p> });
    let actions = actions.map(|a| view! { <div class="ui-card__actions">{a}</div> });
    let inner = view! {
        {media}
        {head}
        <h3 class="ui-card__title">{icon.map(|name| view! { <Icon name /> })}<span>{title}</span></h3>
        {body}
        {meta}
        {actions}
    };

    match href {
        Some(href) => view! { <a class=class href=href>{inner}</a> }.into_any(),
        None => view! { <article class=class>{inner}</article> }.into_any(),
    }
}

/// The shape of a card while its content loads.
#[component]
pub fn CardSkeleton(
    /// Body lines to suggest, two by default.
    #[prop(optional)]
    lines: Option<usize>,
) -> impl IntoView {
    let lines = lines.unwrap_or(2);
    view! {
        <div class="ui-card ui-card--loading" aria-hidden="true">
            <span class="ui-skeleton ui-skeleton--short"></span>
            <span class="ui-skeleton ui-skeleton--title"></span>
            {(0..lines)
                .map(|_| view! { <span class="ui-skeleton ui-skeleton--line"></span> })
                .collect_view()}
        </div>
    }
}

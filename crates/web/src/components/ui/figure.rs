//! Figure and visual.
//!
//! A figure shows something as it is (a QR code). A visual is an author's
//! 16:9 screenshot blended into the dark ground — or, until one is
//! supplied, the surface and the thing's icon, never an empty frame.

use leptos::prelude::*;

use super::icon::{Icon, IconName, IconSize};

/// Something shown as it is, with a caption.
#[component]
pub fn Figure(
    /// Caption.
    #[prop(into)]
    caption: String,
    /// Raw SVG markup, produced by the server by our own code.
    #[prop(into)]
    svg: String,
) -> impl IntoView {
    view! {
        <figure class="ui-figure">
            <div class="ui-figure__frame" inner_html=svg></div>
            <figcaption>{caption}</figcaption>
        </figure>
    }
}

/// A 16:9 visual, or its dignified absence.
#[component]
pub fn Visual(
    /// Image URL, when the author supplied one.
    #[prop(optional_no_strip)]
    src: Option<String>,
    /// What the image shows.
    #[prop(into)]
    alt: String,
    /// Shown when there is no image.
    icon: IconName,
) -> impl IntoView {
    match src {
        Some(src) => view! {
            <div class="ui-visual"><img src=src alt=alt loading="lazy" /></div>
        }
        .into_any(),
        None => view! {
            <div class="ui-visual ui-visual--none">
                <Icon name=icon size=IconSize::Medium />
                <span>"Pas encore de capture"</span>
            </div>
        }
        .into_any(),
    }
}

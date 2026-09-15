//! Avatar — a member's picture, or their initial. No coloured ring.

use leptos::prelude::*;

/// Avatar size.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AvatarSize {
    /// In a row.
    #[default]
    Small,
    /// At the top of a profile.
    Large,
}

/// The first letter of a name, as a capital.
fn initial(name: &str) -> String {
    name.chars()
        .find(|c| c.is_alphanumeric())
        .map(|c| c.to_uppercase().to_string())
        .unwrap_or_default()
}

/// An avatar. Decorative: the name is always written beside it.
#[component]
#[allow(clippy::needless_pass_by_value)] // Leptos prop convention
pub fn Avatar(
    /// Whose.
    #[prop(into)]
    name: String,
    /// Picture, if they have one.
    #[prop(optional_no_strip)]
    src: Option<String>,
    /// Small (default) or large.
    #[prop(optional)]
    size: AvatarSize,
) -> impl IntoView {
    let class = match size {
        AvatarSize::Small => "ui-avatar",
        AvatarSize::Large => "ui-avatar ui-avatar--lg",
    };
    let inner = src.map_or_else(
        || initial(&name).into_any(),
        |src| view! { <img src=src alt="" loading="lazy" /> }.into_any(),
    );
    view! { <span class=class aria-hidden="true">{inner}</span> }
}

#[cfg(test)]
mod tests {
    use super::initial;

    #[test]
    fn the_initial_skips_symbols_and_is_a_capital() {
        assert_eq!(initial("ada"), "A");
        assert_eq!(initial("_élodie"), "É");
        assert_eq!(initial(""), "");
    }
}

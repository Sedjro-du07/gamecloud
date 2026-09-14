//! Character card — the main avatar + rank + XP bar widget.
//!
//! Visual model: a circular avatar surrounded by a neon ring whose
//! color is driven by the user's rank. Below the ring sits the rank
//! title and the XP progress bar. The whole card is keyboard-focusable
//! and announces its content via `aria-label`.

use gamecloud_shared::roles::GlobalRank;
use leptos::prelude::*;

use crate::components::xp_bar::XpBar;

/// Character card component.
#[component]
#[allow(clippy::needless_pass_by_value)] // Leptos prop convention
pub fn CharacterCard(
    /// Display name (Discord username).
    name: String,
    /// Avatar URL; a placeholder is rendered if empty.
    avatar_url: Option<String>,
    /// Total XP.
    xp_total: i64,
    /// Rank string (DB form).
    global_rank: String,
    /// Optional override for the displayed title; defaults to the rank
    /// title.
    title: Option<String>,
) -> impl IntoView {
    let rank = GlobalRank::parse(global_rank.as_str());
    let ring = rank.ring_color().to_string();
    let displayed_title = title.unwrap_or_else(|| rank.title().to_string());
    let aria_label = format!("{name}, {}", rank.title());

    let avatar = avatar_url.unwrap_or_else(|| {
        // Inline 1×1 transparent PNG so we never render a broken
        // <img> on missing avatars.
        "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNkYAAAAAYAAjCB0C8AAAAASUVORK5CYII=".to_string()
    });

    view! {
        <article class="gc-card" tabindex="0" aria-label=aria_label>
            <div class="gc-card__ring"
                 style=format!("--gc-ring: {ring};")>
                <img class="gc-card__avatar" src=avatar alt="" />
            </div>
            <header class="gc-card__header">
                <h2 class="gc-card__name">{name}</h2>
                <p class="gc-card__title">{displayed_title}</p>
            </header>
            <XpBar xp_total color=ring />
        </article>
    }
}

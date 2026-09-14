//! Quest card.
//!
//! One open quest with a progress bar. Quests are the mechanism that
//! tells the club what to do this week, so the card leads with the
//! action and the reward, not with metadata.

use leptos::prelude::*;

use crate::api::{quest_percent, QuestItem};

/// Quest card component.
#[component]
#[allow(clippy::needless_pass_by_value)] // Leptos prop convention
pub fn QuestCard(
    /// The quest to render.
    quest: QuestItem,
) -> impl IntoView {
    let pct = quest_percent(quest.progress, quest.target);
    let card_class = if quest.completed {
        "gc-quest gc-quest--done"
    } else {
        "gc-quest"
    };
    let type_class = match quest.quest_type.as_str() {
        "Special" => "gc-chip gc-chip--special",
        "Hidden" => "gc-chip gc-chip--hidden",
        _ => "gc-chip",
    };
    let type_label = match quest.quest_type.as_str() {
        "Special" => "Spéciale",
        "Hidden" => "Secrète",
        _ => "Hebdo",
    };

    view! {
        <article class=card_class>
            <header class="gc-quest__head">
                <span class=type_class>{type_label}</span>
                {quest.track.map(|t| view! { <span class="gc-chip">{t}</span> })}
                <span class="gc-quest__reward">{format!("+{} XP", quest.xp_reward)}</span>
            </header>

            <h3 class="gc-quest__title">{quest.title}</h3>
            {quest
                .description
                .map(|d| view! { <p class="gc-quest__desc">{d}</p> })}
            <p class="gc-quest__condition">{quest.condition_label}</p>

            <div class="gc-quest__progress"
                 role="progressbar"
                 aria-valuemin="0"
                 aria-valuemax=quest.target.to_string()
                 aria-valuenow=quest.progress.to_string()>
                <div class="gc-quest__track">
                    <div class="gc-quest__fill" style=format!("width: {pct:.1}%;")></div>
                </div>
                <span class="gc-quest__count">
                    {format!("{} / {}", quest.progress, quest.target)}
                </span>
            </div>

            <footer class="gc-quest__foot">
                {if quest.completed {
                    view! { <span class="gc-quest__done">"✅ Accomplie"</span> }.into_any()
                } else {
                    view! { <span class="gc-quest__time">{quest.time_left}</span> }.into_any()
                }}
            </footer>
        </article>
    }
}

//! XP bar and streak marks — the game shown in the interface, not the decor.

use leptos::prelude::*;

use super::vocab::plain_title;
use crate::api::{format_xp, level_percent, MeView};

/// "×1,25" rather than "×1.25".
#[must_use]
pub fn multiplier_label(multiplier: f64) -> String {
    let text = format!("{multiplier:.2}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    text.replace('.', ",")
}

/// How far the member is towards their next title, 0 to 100.
#[must_use]
pub fn rank_percent(me: &MeView) -> f64 {
    if me.rank_xp_needed == 0 {
        100.0
    } else {
        level_percent(me.rank_xp_into, me.rank_xp_needed)
    }
}

/// The line beside the XP bar.
#[must_use]
pub fn xp_caption(me: &MeView) -> String {
    let total = format_xp(me.xp_total);
    me.next_rank_title.as_deref().map_or_else(
        || format!("{total} XP · titre le plus haut atteint"),
        |next| {
            let left = (me.rank_xp_needed - me.rank_xp_into).max(0);
            format!("{total} XP · encore {} XP avant {}", format_xp(left), plain_title(next))
        },
    )
}

/// The line beside the streak marks.
#[must_use]
pub fn streak_caption(me: &MeView) -> String {
    match me.streak_days {
        0 => "Pas de série en cours : gagne de l'XP aujourd'hui pour la lancer.".to_string(),
        1 => format!("1 jour de série · XP ×{}", multiplier_label(me.streak_multiplier)),
        n => format!("{n} jours de série · XP ×{}", multiplier_label(me.streak_multiplier)),
    }
}

/// Progress towards the next title: a 4px rail, the value beside it.
#[component]
pub fn XpProgress(
    /// Share of the way to the next title, 0 to 100.
    percent: f64,
    /// The value and what it leads to, e.g. "7 830 XP · encore 2 170 XP
    /// avant Légende Vivante".
    #[prop(into)]
    caption: String,
) -> impl IntoView {
    let pct = percent.clamp(0.0, 100.0);
    view! {
        <div class="ui-xp">
            <div
                class="ui-xp__rail"
                role="progressbar"
                aria-valuemin="0"
                aria-valuemax="100"
                aria-valuenow=format!("{pct:.0}")
                aria-label=caption.clone()
            >
                <span class="ui-xp__fill" style=format!("width: {pct:.1}%")></span>
            </div>
            <span class="ui-xp__value">{caption}</span>
        </div>
    }
}

/// Consecutive active days as small marks, a week at a time.
///
/// The platform stores the length of the current streak, not a day-by-day
/// history, so the marks count days up to a week rather than drawing
/// which days were active.
#[component]
pub fn StreakMarks(
    /// Consecutive active days.
    days: i32,
    /// Marks drawn; a week by default.
    #[prop(default = 7)]
    span: i32,
) -> impl IntoView {
    let on = days.clamp(0, span);
    let label = match days {
        0 => "Aucun jour de série".to_string(),
        1 => "1 jour de série".to_string(),
        n => format!("{n} jours de série"),
    };
    view! {
        <span class="ui-streak" role="img" aria-label=label>
            {(0..span)
                .map(|i| {
                    view! {
                        <span class=if i < on {
                            "ui-streak__mark ui-streak__mark--on"
                        } else {
                            "ui-streak__mark"
                        }></span>
                    }
                })
                .collect_view()}
        </span>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multipliers_read_the_french_way() {
        assert_eq!(multiplier_label(1.25), "1,25");
        assert_eq!(multiplier_label(1.5), "1,5");
        assert_eq!(multiplier_label(1.0), "1");
    }

    #[test]
    fn the_xp_caption_names_the_next_title_without_its_emoji() {
        let me = MeView {
            xp_total: 7830,
            rank_xp_into: 2830,
            rank_xp_needed: 5000,
            next_rank_title: Some("🌟 Légende Vivante".into()),
            ..MeView::default()
        };
        let caption = xp_caption(&me);
        assert!(caption.contains("Légende Vivante"));
        assert!(!caption.contains('🌟'));
        // Whatever separator `format_xp` uses for thousands.
        assert!(caption.contains(&format_xp(2170)));
    }

    #[test]
    fn the_top_title_says_so() {
        let me = MeView::default();
        assert!(xp_caption(&me).contains("titre le plus haut"));
        assert!((rank_percent(&me) - 100.0).abs() < f64::EPSILON);
    }
}

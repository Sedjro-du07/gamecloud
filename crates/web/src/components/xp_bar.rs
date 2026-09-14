//! XP progress bar.
//!
//! Renders the user's progress toward the next global-rank threshold.
//! All thresholds are pulled from `gamecloud_shared::xp::RANK_THRESHOLDS`
//! so the UI always agrees with the backend.

use gamecloud_shared::xp::RANK_THRESHOLDS;
use leptos::prelude::*;

/// Returns `(lower_bound, upper_bound, percent_in_range)` for the
/// rank window that contains `xp_total`. The last rank caps at 100%.
fn window(xp_total: i64) -> (i64, Option<i64>, f64) {
    // Skip the gating-only entries (`Pending`, `Visitor`, `Initiate`
    // all have threshold 0); the meaningful XP ladder starts at 150.
    let xp_thresholds: Vec<i64> = RANK_THRESHOLDS
        .iter()
        .filter(|(t, _)| *t > 0)
        .map(|(t, _)| *t)
        .collect();

    let mut lower = 0;
    let mut upper = None;
    for &t in &xp_thresholds {
        if xp_total >= t {
            lower = t;
        } else {
            upper = Some(t);
            break;
        }
    }
    let pct = match upper {
        Some(u) if u > lower => {
            #[allow(clippy::cast_precision_loss)]
            let p = (xp_total - lower) as f64 / (u - lower) as f64;
            (p * 100.0).clamp(0.0, 100.0)
        }
        _ => 100.0,
    };
    (lower, upper, pct)
}

/// XP progress bar component.
#[component]
#[allow(clippy::needless_pass_by_value)] // Leptos prop convention
pub fn XpBar(
    /// Total XP earned.
    xp_total: i64,
    /// Color of the filled portion (CSS color).
    color: String,
) -> impl IntoView {
    let (_lower, upper, pct) = window(xp_total);
    let label = upper.map_or_else(
        || format!("{xp_total} XP — rank max"),
        |u| format!("{xp_total} / {u} XP"),
    );

    view! {
        <div class="gc-xp-bar" role="progressbar" aria-valuemin="0" aria-valuemax="100"
             aria-valuenow=move || pct.round().to_string()>
            <div class="gc-xp-bar__track">
                <div class="gc-xp-bar__fill"
                     style=format!("width: {pct:.1}%; background: {color};")></div>
            </div>
            <div class="gc-xp-bar__label">{label}</div>
        </div>
    }
}

#[cfg(all(test, feature = "ssr"))]
mod tests {
    use super::window;

    #[test]
    fn window_at_zero_targets_apprentice() {
        let (lower, upper, pct) = window(0);
        assert_eq!(lower, 0);
        assert_eq!(upper, Some(150));
        assert!(pct.abs() < 0.01);
    }

    #[test]
    fn window_at_threshold_starts_zero_pct() {
        let (lower, upper, pct) = window(150);
        assert_eq!(lower, 150);
        assert_eq!(upper, Some(400));
        assert!(pct.abs() < 0.01);
    }

    #[test]
    fn window_above_max_is_capped() {
        let (_, upper, pct) = window(50_000);
        assert_eq!(upper, None);
        assert!((pct - 100.0).abs() < 0.01);
    }

    #[test]
    fn window_midway() {
        let (_, _, pct) = window(275); // halfway between 150 and 400
        assert!((pct - 50.0).abs() < 0.5);
    }
}

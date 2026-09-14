//! Track standings list.
//!
//! A member's tracks with their per-track XP and role. Each track is
//! tinted with its own colour so the 8 disciplines stay visually
//! distinct across the whole UI.

use leptos::prelude::*;

use gamecloud_shared::roles::TrackRole;

use crate::api::{format_xp, level_percent, TrackView};

/// Title for a track role — the same one the bot shows on Discord.
fn role_label(role: &str) -> &'static str {
    TrackRole::title_of(role)
}

/// Track list component.
#[component]
#[allow(clippy::needless_pass_by_value)] // Leptos prop convention
pub fn TrackList(
    /// Tracks the member belongs to.
    tracks: Vec<TrackView>,
) -> impl IntoView {
    if tracks.is_empty() {
        return view! {
            <section class="gc-tracks">
                <h2>"Tracks"</h2>
                <p class="gc-empty">
                    "Tu n'as encore rejoint aucune track. "
                    <a href="/onboarding/tracks" rel="external">"Choisis-en une"</a>
                    " pour commencer à gagner de l'XP de track."
                </p>
            </section>
        }
        .into_any();
    }

    view! {
        <section class="gc-tracks">
            <h2>"Tracks"</h2>
            <ul class="gc-tracks__list">
                {tracks
                    .into_iter()
                    .map(|t| {
                        // How far the next title in this track is. From
                        // Mentor up there is no bar: those titles are
                        // appointed, not earned.
                        let progress = TrackRole::next_milestone(&t.role).map(|(next, floor, target)| {
                            let pct = level_percent((t.xp - floor).max(0), target - floor);
                            let left = (target - t.xp).max(0);
                            view! {
                                <span class="gc-track__progress">
                                    <span class="gc-track__bar" aria-hidden="true">
                                        <span class="gc-track__fill" style=format!("width: {pct:.1}%;")></span>
                                    </span>
                                    <span class="gc-track__next">
                                        {format!("{} XP avant {next}", format_xp(left))}
                                    </span>
                                </span>
                            }
                        });
                        view! {
                            <li class="gc-track" style=format!("--gc-track: {};", t.color)>
                                <span class="gc-track__emoji">{t.emoji}</span>
                                <span class="gc-track__body">
                                    <span class="gc-track__name">{t.id}</span>
                                    <span class="gc-track__meta">
                                        {role_label(&t.role)}
                                        {t.specialization.map(|s| format!(" · {s}"))}
                                    </span>
                                    {progress}
                                </span>
                                <span class="gc-track__xp">{format_xp(t.xp)} " XP"</span>
                            </li>
                        }
                    })
                    .collect_view()}
            </ul>
        </section>
    }
    .into_any()
}

#[cfg(all(test, feature = "ssr"))]
mod tests {
    use super::role_label;

    #[test]
    fn every_schema_role_has_a_label() {
        // Exactly the values allowed by track_memberships_role_valid.
        for role in [
            "Observer",
            "Contributor",
            "Reviewer",
            "Mentor",
            "CoLead",
            "Lead",
        ] {
            assert!(!role_label(role).is_empty());
        }
    }

    #[test]
    fn an_unknown_role_reads_as_the_lowest_one() {
        // Failing closed: an unrecognised role must never render as
        // something more senior than it is.
        assert_eq!(role_label("Sorcerer"), "Observateur");
    }
}

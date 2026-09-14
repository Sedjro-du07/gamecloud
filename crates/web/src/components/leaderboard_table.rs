//! Leaderboard table.
//!
//! Renders one scope's standings. The viewer's own row is highlighted,
//! which is the point of the component: a board you cannot find
//! yourself on is just a list of other people.

use leptos::prelude::*;

use crate::api::{format_xp, LeaderboardEntry};

/// Medal for the top three, a plain number below.
fn position_label(position: usize) -> String {
    match position {
        1 => "🥇".to_string(),
        2 => "🥈".to_string(),
        3 => "🥉".to_string(),
        n => n.to_string(),
    }
}

/// The top three, standing on a podium.
///
/// A table says who is first; a podium makes it an event. Laid out
/// 2 · 1 · 3, the way podiums stand, so the winner is in the middle and
/// highest.
#[component]
#[allow(clippy::needless_pass_by_value)] // Leptos prop convention
fn Podium(
    /// The first three rows, in position order.
    top: Vec<LeaderboardEntry>,
) -> impl IntoView {
    let places = [1_usize, 0, 2]
        .iter()
        .filter_map(|&i| top.get(i).cloned())
        .map(|e| {
            let place = e.position;
            let initial = e
                .display_name
                .chars()
                .next()
                .map(|c| c.to_uppercase().to_string())
                .unwrap_or_default();
            let avatar = e.avatar_url.clone().map_or_else(
                || view! { <span class="gc-podium__initial">{initial}</span> }.into_any(),
                |src| view! { <img src=src alt="" /> }.into_any(),
            );
            let class = if e.is_me {
                format!("gc-podium__place gc-podium__place--{place} gc-podium__place--me")
            } else {
                format!("gc-podium__place gc-podium__place--{place}")
            };
            view! {
                <li class=class style=format!("--gc-ring: {};", e.rank_color)>
                    <span class="gc-podium__medal" aria-hidden="true">{position_label(place)}</span>
                    <span class="gc-podium__avatar">{avatar}</span>
                    <span class="gc-podium__name">{e.display_name}</span>
                    <span class="gc-podium__title">{e.rank_title}</span>
                    <span class="gc-podium__xp">{format_xp(e.xp)} " XP"</span>
                    <span class="gc-podium__block" aria-hidden="true">{place}</span>
                </li>
            }
        })
        .collect_view();

    view! { <ol class="gc-podium" aria-label="Podium">{places}</ol> }
}

/// Leaderboard table component.
#[component]
#[allow(clippy::needless_pass_by_value)] // Leptos prop convention
pub fn LeaderboardTable(
    /// Rows to render, already ordered and positioned.
    entries: Vec<LeaderboardEntry>,
) -> impl IntoView {
    if entries.is_empty() {
        return view! {
            <p class="gc-empty">
                "Personne n'a encore marqué de points ici. Sois le premier."
            </p>
        }
        .into_any();
    }

    let podium = entries.iter().take(3).cloned().collect::<Vec<_>>();

    view! {
        <Podium top=podium />
        <div class="gc-table-wrap">
            <table class="gc-table">
                <thead>
                    <tr>
                        <th scope="col" class="gc-table__pos">"#"</th>
                        <th scope="col">"Membre"</th>
                        <th scope="col" class="gc-table__num">"XP"</th>
                        <th scope="col" class="gc-table__num">"Série"</th>
                    </tr>
                </thead>
                <tbody>
                    {entries
                        .into_iter()
                        .map(|e| {
                            let row_class = if e.is_me {
                                "gc-table__row gc-table__row--me"
                            } else {
                                "gc-table__row"
                            };
                            let streak = if e.streak_days > 0 {
                                format!("🔥 {}", e.streak_days)
                            } else {
                                "—".to_string()
                            };
                            view! {
                                <tr class=row_class>
                                    <td class="gc-table__pos">{position_label(e.position)}</td>
                                    <td>
                                        <span class="gc-table__name">{e.display_name}</span>
                                        <span
                                            class="gc-table__rank"
                                            style=format!("--gc-ring: {};", e.rank_color)
                                        >
                                            {e.rank_title}
                                        </span>
                                    </td>
                                    <td class="gc-table__num">{format_xp(e.xp)}</td>
                                    <td class="gc-table__num">{streak}</td>
                                </tr>
                            }
                        })
                        .collect_view()}
                </tbody>
            </table>
        </div>
    }
    .into_any()
}

#[cfg(all(test, feature = "ssr"))]
mod tests {
    use super::position_label;

    #[test]
    fn top_three_get_medals() {
        assert_eq!(position_label(1), "🥇");
        assert_eq!(position_label(2), "🥈");
        assert_eq!(position_label(3), "🥉");
    }

    #[test]
    fn everyone_else_gets_a_number() {
        assert_eq!(position_label(4), "4");
        assert_eq!(position_label(25), "25");
    }
}

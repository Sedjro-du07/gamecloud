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

    view! {
        <div class="gc-table-wrap">
            <table class="gc-table">
                <thead>
                    <tr>
                        <th scope="col" class="gc-table__pos">"#"</th>
                        <th scope="col">"Membre"</th>
                        <th scope="col" class="gc-table__num">"XP"</th>
                        <th scope="col" class="gc-table__num">"Niv."</th>
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
                                        <span class="gc-table__rank">{e.rank}</span>
                                    </td>
                                    <td class="gc-table__num">{format_xp(e.xp)}</td>
                                    <td class="gc-table__num">{e.level}</td>
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

//! `/quetes` — the member's open quests and progress.
//!
//! Quests exist to direct effort, so they have to be visible where the
//! club actually lives. Most members will never open the web app on a
//! Monday morning; they will read Discord.

use poise::serenity_prelude as serenity;

use crate::state::Context;

/// Show your open quests and how far along you are.
#[poise::command(slash_command, rename = "quetes")]
pub async fn quetes(ctx: Context<'_>) -> Result<(), anyhow::Error> {
    let discord_id = ctx.author().id.to_string();

    let rows: Vec<(String, i32, i32, i32, Option<String>, bool)> = sqlx::query_as(
        r#"
        SELECT q.title,
               q.xp_reward,
               COALESCE(p.counter, 0) AS progress,
               q.condition_value,
               q.track,
               (c.quest_id IS NOT NULL) AS completed
          FROM quests q
          JOIN users u ON u.discord_id = $1
          LEFT JOIN quest_progress    p ON p.quest_id = q.id AND p.user_id = u.id
          LEFT JOIN quest_completions c ON c.quest_id = q.id AND c.user_id = u.id
         WHERE q.starts_at <= NOW()
           AND q.ends_at   >  NOW()
           AND (q.quest_type <> 'Hidden' OR p.counter > 0)
         ORDER BY (c.quest_id IS NOT NULL), q.ends_at ASC
         LIMIT 10
        "#,
    )
    .bind(&discord_id)
    .fetch_all(ctx.data().pool())
    .await?;

    if rows.is_empty() {
        ctx.reply(
            "Aucune quête en cours pour toi. Le Bureau en publie une nouvelle chaque semaine.",
        )
        .await?;
        return Ok(());
    }

    let mut body = String::new();
    for (title, reward, progress, target, track, completed) in rows {
        let capped = progress.min(target);
        let mark = if completed { "✅" } else { "▫️" };
        let track_label = track.map_or(String::new(), |t| format!(" `[{t}]`"));
        body.push_str(&format!(
            "{mark} **{title}**{track_label} — {}  ·  **+{reward} XP**\n",
            progress_bar(capped, target)
        ));
    }

    let embed = serenity::CreateEmbed::new()
        .title("📜 Tes quêtes")
        .description(body)
        .color(0x00f2ff);
    ctx.send(poise::CreateReply::default().embed(embed)).await?;
    Ok(())
}

/// A ten-cell text progress bar, e.g. `▰▰▰▱▱▱▱▱▱▱ 3/10`.
fn progress_bar(progress: i32, target: i32) -> String {
    const CELLS: i32 = 10;
    let filled = if target <= 0 {
        CELLS
    } else {
        ((progress.max(0) * CELLS) / target).clamp(0, CELLS)
    };
    let bar: String = (0..CELLS)
        .map(|i| if i < filled { '▰' } else { '▱' })
        .collect();
    format!("{bar} {progress}/{target}")
}

#[cfg(test)]
mod tests {
    use super::progress_bar;

    #[test]
    fn an_empty_bar_is_all_hollow() {
        assert!(progress_bar(0, 10).starts_with("▱▱▱▱▱▱▱▱▱▱"));
    }

    #[test]
    fn a_full_bar_is_all_solid() {
        assert!(progress_bar(10, 10).starts_with("▰▰▰▰▰▰▰▰▰▰"));
    }

    #[test]
    fn a_half_bar_is_half_solid() {
        let bar = progress_bar(5, 10);
        assert_eq!(bar.chars().filter(|c| *c == '▰').count(), 5);
    }

    #[test]
    fn overshoot_does_not_overflow_the_bar() {
        let bar = progress_bar(99, 10);
        assert_eq!(bar.chars().filter(|c| *c == '▰').count(), 10);
    }

    #[test]
    fn a_zero_target_never_divides_by_zero() {
        assert_eq!(progress_bar(0, 0).chars().filter(|c| *c == '▰').count(), 10);
    }

    #[test]
    fn the_counter_is_appended() {
        assert!(progress_bar(3, 10).ends_with("3/10"));
    }
}

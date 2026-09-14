//! `/profil` — shows the user's GameCloud profile card.

use gamecloud_shared::{roles::GlobalRank, xp::streak_multiplier};
use poise::serenity_prelude as serenity;

use crate::state::Context;

/// Show your GameCloud profile.
#[poise::command(slash_command, rename = "profil")]
pub async fn profil(
    ctx: Context<'_>,
    #[description = "User to show; defaults to yourself"] user: Option<serenity::User>,
) -> Result<(), anyhow::Error> {
    let target = user.as_ref().unwrap_or_else(|| ctx.author());
    let discord_id = target.id.to_string();

    let row: Option<(String, i64, i32, String, Option<String>, i32)> = sqlx::query_as(
        r#"
        SELECT discord_id, xp_total, level, global_rank, current_title, streak_days
        FROM users WHERE discord_id = $1
        "#,
    )
    .bind(&discord_id)
    .fetch_optional(ctx.data().pool())
    .await?;

    let Some((_, xp, level, rank, title, streak)) = row else {
        ctx.reply("Cet utilisateur n'a pas encore de compte GameCloud.")
            .await?;
        return Ok(());
    };

    let rank_enum = GlobalRank::parse(&rank);
    let title_str = title.as_deref().unwrap_or(rank_enum.title());
    let color = ring_color_to_u32(rank_enum.ring_color());

    let embed = serenity::CreateEmbed::new()
        .title(format!("Profil de {}", target.name))
        .description(title_str)
        .color(color)
        .field("Rang", rank_enum.title(), true)
        .field("Niveau", level.to_string(), true)
        .field("XP totale", xp.to_string(), true)
        .field("Série", streak_label(streak), true);

    ctx.send(poise::CreateReply::default().embed(embed)).await?;
    Ok(())
}

fn ring_color_to_u32(hex: &str) -> u32 {
    u32::from_str_radix(hex.trim_start_matches('#'), 16).unwrap_or(0x9c4dff)
}

/// Describe a streak, including the multiplier it is currently worth.
///
/// The multiplier is the whole reason a streak matters, so showing the
/// day count without it tells a member nothing actionable.
fn streak_label(streak: i32) -> String {
    let multiplier = streak_multiplier(streak);
    if streak <= 0 {
        "Aucune série en cours".to_string()
    } else if multiplier > 1.0 {
        format!("{streak} j — ×{multiplier:.2} sur l'XP")
    } else {
        format!("{streak} j — ×1.25 à 7 jours")
    }
}

#[cfg(test)]
mod tests {
    use super::{ring_color_to_u32, streak_label};

    #[test]
    fn a_cold_streak_says_so() {
        assert!(streak_label(0).contains("Aucune"));
    }

    #[test]
    fn a_short_streak_names_the_next_milestone() {
        assert!(streak_label(3).contains("7 jours"));
    }

    #[test]
    fn an_earned_streak_shows_its_multiplier() {
        assert!(streak_label(7).contains("1.25"));
        assert!(streak_label(14).contains("1.50"));
        assert!(streak_label(30).contains("2.00"));
    }

    #[test]
    fn colors_parse_with_and_without_a_hash() {
        assert_eq!(ring_color_to_u32("#00f2ff"), 0x00f2ff);
        assert_eq!(ring_color_to_u32("00f2ff"), 0x00f2ff);
    }

    #[test]
    fn a_bad_color_falls_back_to_brand_purple() {
        assert_eq!(ring_color_to_u32("nonsense"), 0x9c4dff);
    }
}

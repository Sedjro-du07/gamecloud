//! `/profil` — shows the user's GameCloud profile card.

use gamecloud_shared::roles::GlobalRank;
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

    let rank_enum = parse_rank(&rank);
    let title_str = title.as_deref().unwrap_or(rank_enum.title());
    let color = ring_color_to_u32(rank_enum.ring_color());

    let embed = serenity::CreateEmbed::new()
        .title(format!("Profil de {}", target.name))
        .description(title_str)
        .color(color)
        .field("Rang", rank_enum.title(), true)
        .field("Niveau", level.to_string(), true)
        .field("XP totale", xp.to_string(), true)
        .field("Streak", format!("{streak} j"), true);

    ctx.send(poise::CreateReply::default().embed(embed)).await?;
    Ok(())
}

fn parse_rank(s: &str) -> GlobalRank {
    match s {
        "Visitor" => GlobalRank::Visitor,
        "Initiate" => GlobalRank::Initiate,
        "Apprentice" => GlobalRank::Apprentice,
        "JuniorDev" => GlobalRank::JuniorDev,
        "SeniorDev" => GlobalRank::SeniorDev,
        "Expert" => GlobalRank::Expert,
        "Veteran" => GlobalRank::Veteran,
        "Legend" => GlobalRank::Legend,
        "Myth" => GlobalRank::Myth,
        _ => GlobalRank::Pending,
    }
}

fn ring_color_to_u32(hex: &str) -> u32 {
    u32::from_str_radix(hex.trim_start_matches('#'), 16).unwrap_or(0x9c4dff)
}

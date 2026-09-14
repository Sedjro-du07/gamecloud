//! `/badges` — the member's badge collection.

use gamecloud_shared::roles::SpecialBadge;
use poise::serenity_prelude as serenity;

use crate::state::Context;

/// Show the badges you have unlocked.
#[poise::command(slash_command, rename = "badges")]
pub async fn badges(
    ctx: Context<'_>,
    #[description = "User to inspect; defaults to yourself"] user: Option<serenity::User>,
) -> Result<(), anyhow::Error> {
    let target = user.as_ref().unwrap_or_else(|| ctx.author());
    let discord_id = target.id.to_string();

    let held: Vec<(String,)> = sqlx::query_as(
        r#"
        SELECT b.badge_type
          FROM special_badges b
          JOIN users u ON u.id = b.user_id
         WHERE u.discord_id = $1
         ORDER BY b.awarded_at DESC
        "#,
    )
    .bind(&discord_id)
    .fetch_all(ctx.data().pool())
    .await?;

    let held: Vec<SpecialBadge> = held
        .into_iter()
        .filter_map(|(id,)| SpecialBadge::parse(&id))
        .collect();

    if held.is_empty() {
        ctx.reply(format!(
            "{} n'a encore débloqué aucun badge. Il y en a {} à décrocher.",
            target.name,
            SpecialBadge::ALL.len()
        ))
        .await?;
        return Ok(());
    }

    let mut unlocked = String::new();
    for badge in &held {
        use std::fmt::Write;
        let _ = writeln!(&mut unlocked, "{} — {}", badge.title(), badge.description());
    }

    // Show a couple of near-misses too: a badge nobody knows about is
    // not a goal.
    let mut next = String::new();
    for badge in SpecialBadge::ALL
        .iter()
        .filter(|b| b.is_automatic() && !held.contains(b))
        .take(3)
    {
        use std::fmt::Write;
        let _ = writeln!(&mut next, "🔒 {} — {}", badge.title(), badge.description());
    }

    let mut embed = serenity::CreateEmbed::new()
        .title(format!("🎖️ Badges de {}", target.name))
        .description(unlocked)
        .color(0xffd700)
        .footer(serenity::CreateEmbedFooter::new(format!(
            "{} / {} badges",
            held.len(),
            SpecialBadge::ALL.len()
        )));

    if !next.is_empty() {
        embed = embed.field("À débloquer", next, false);
    }

    ctx.send(poise::CreateReply::default().embed(embed)).await?;
    Ok(())
}

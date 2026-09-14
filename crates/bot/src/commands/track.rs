//! `/track` — show track memberships of a user.

use gamecloud_shared::roles::Track;
use poise::serenity_prelude as serenity;

use crate::state::Context;

/// Show track memberships.
#[poise::command(slash_command, rename = "track")]
pub async fn track(
    ctx: Context<'_>,
    #[description = "User to inspect; defaults to yourself"] user: Option<serenity::User>,
) -> Result<(), anyhow::Error> {
    let target = user.as_ref().unwrap_or_else(|| ctx.author());
    let discord_id = target.id.to_string();

    let rows: Vec<(String, String, Option<String>, i64)> = sqlx::query_as(
        r#"
        SELECT m.track, m.track_role, m.specialization, m.track_xp
        FROM track_memberships m
        JOIN users u ON u.id = m.user_id
        WHERE u.discord_id = $1
        ORDER BY m.track_xp DESC
        "#,
    )
    .bind(&discord_id)
    .fetch_all(ctx.data().pool())
    .await?;

    if rows.is_empty() {
        ctx.reply("Cet utilisateur n'a rejoint aucun track.").await?;
        return Ok(());
    }

    let mut body = String::new();
    for (track, role, spec, xp) in rows {
        let emoji = Track::parse(&track).map_or("•", Track::emoji);
        let spec_label = spec.map_or(String::new(), |s| format!(" — *{s}*"));
        body.push_str(&format!(
            "{emoji} **{track}** ({role}) — {xp} XP{spec_label}\n",
        ));
    }

    let embed = serenity::CreateEmbed::new()
        .title(format!("Tracks — {}", target.name))
        .description(body)
        .color(0xbf00ff);
    ctx.send(poise::CreateReply::default().embed(embed)).await?;
    Ok(())
}

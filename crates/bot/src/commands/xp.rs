//! `/xp` — show the last 10 XP events for a user.

use poise::serenity_prelude as serenity;

use crate::state::Context;

/// Show your recent XP events.
#[poise::command(slash_command, rename = "xp")]
pub async fn xp(
    ctx: Context<'_>,
    #[description = "User to inspect; defaults to yourself"] user: Option<serenity::User>,
) -> Result<(), anyhow::Error> {
    let target = user.as_ref().unwrap_or_else(|| ctx.author());
    let discord_id = target.id.to_string();

    let rows: Vec<(i32, String, Option<String>, Option<String>, chrono::DateTime<chrono::Utc>)> =
        sqlx::query_as(
            r#"
            SELECT x.amount, x.source, x.track, x.description, x.created_at
            FROM xp_logs x
            JOIN users u ON u.id = x.user_id
            WHERE u.discord_id = $1
            ORDER BY x.created_at DESC
            LIMIT 10
            "#,
        )
        .bind(&discord_id)
        .fetch_all(ctx.data().pool())
        .await?;

    if rows.is_empty() {
        ctx.reply("Aucune XP enregistrée pour cet utilisateur.")
            .await?;
        return Ok(());
    }

    let mut body = String::new();
    for (amount, source, track, desc, at) in rows {
        let date = at.format("%d/%m %H:%M");
        let track_label = track.map_or(String::new(), |t| format!(" [{t}]"));
        let desc_label = desc.map_or(String::new(), |d| format!(" — {d}"));
        body.push_str(&format!(
            "`{date}` **{amount:+}** XP `{source}`{track_label}{desc_label}\n"
        ));
    }

    let embed = serenity::CreateEmbed::new()
        .title(format!("XP récente — {}", target.name))
        .description(body)
        .color(0x00f2ff);
    ctx.send(poise::CreateReply::default().embed(embed)).await?;
    Ok(())
}

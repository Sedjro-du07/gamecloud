//! `/leaderboard` — top 10 by global XP, optionally filtered by track.

use gamecloud_shared::roles::Track;
use poise::serenity_prelude as serenity;

use crate::state::Context;

/// Show the top 10 by XP, optionally per track.
#[poise::command(slash_command, rename = "leaderboard")]
pub async fn leaderboard(
    ctx: Context<'_>,
    #[description = "Track to filter on (Engineering, GameDesign, …)"] track: Option<String>,
) -> Result<(), anyhow::Error> {
    let body = if let Some(track_str) = track.as_deref() {
        if Track::parse(track_str).is_none() {
            ctx.reply(format!("Track inconnu : `{track_str}`")).await?;
            return Ok(());
        }
        track_leaderboard(&ctx, track_str).await?
    } else {
        global_leaderboard(&ctx).await?
    };

    let title = match track.as_deref() {
        Some(t) => format!("🏆 Leaderboard — {t}"),
        None => "🏆 Leaderboard global".to_string(),
    };
    let embed = serenity::CreateEmbed::new()
        .title(title)
        .description(body)
        .color(0xffd700);
    ctx.send(poise::CreateReply::default().embed(embed)).await?;
    Ok(())
}

async fn global_leaderboard(ctx: &Context<'_>) -> Result<String, anyhow::Error> {
    let rows: Vec<(String, i64, String)> = sqlx::query_as(
        r#"
        SELECT discord_id, xp_total, global_rank
        FROM users
        WHERE email_verified = TRUE
        ORDER BY xp_total DESC
        LIMIT 10
        "#,
    )
    .fetch_all(ctx.data().pool())
    .await?;

    if rows.is_empty() {
        return Ok("_Aucun joueur classé pour le moment._".into());
    }

    let mut body = String::new();
    for (i, (discord_id, xp, rank)) in rows.iter().enumerate() {
        let medal = match i {
            0 => "🥇",
            1 => "🥈",
            2 => "🥉",
            _ => "  ",
        };
        body.push_str(&format!("{medal} <@{discord_id}> — **{xp}** XP `{rank}`\n"));
    }
    Ok(body)
}

async fn track_leaderboard(ctx: &Context<'_>, track: &str) -> Result<String, anyhow::Error> {
    let rows: Vec<(String, i64, String)> = sqlx::query_as(
        r#"
        SELECT u.discord_id, m.track_xp, m.track_role
        FROM track_memberships m
        JOIN users u ON u.id = m.user_id
        WHERE m.track = $1 AND m.left_at IS NULL
        ORDER BY m.track_xp DESC
        LIMIT 10
        "#,
    )
    .bind(track)
    .fetch_all(ctx.data().pool())
    .await?;

    if rows.is_empty() {
        return Ok("_Aucun membre dans ce track pour le moment._".into());
    }

    let mut body = String::new();
    for (i, (discord_id, xp, role)) in rows.iter().enumerate() {
        let medal = match i {
            0 => "🥇",
            1 => "🥈",
            2 => "🥉",
            _ => "  ",
        };
        body.push_str(&format!("{medal} <@{discord_id}> — **{xp}** XP `{role}`\n"));
    }
    Ok(body)
}

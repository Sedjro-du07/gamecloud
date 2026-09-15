//! Poise slash commands.

use crate::state::BotState;

pub mod badges;
pub mod leaderboard;
pub mod profil;
pub mod quetes;
pub mod track;
pub mod xp;

/// All commands the bot registers.
pub fn all() -> Vec<poise::Command<BotState, anyhow::Error>> {
    vec![
        profil::profil(),
        xp::xp(),
        track::track(),
        leaderboard::leaderboard(),
        quetes::quetes(),
        badges::badges(),
    ]
}

/// Whether a Discord user is a registered member: a platform account
/// with a verified Epitech address.
///
/// The leaderboard names members and their XP, so it is shown to members
/// only. A database error counts as "no": refusing is the safe answer.
pub async fn is_registered(pool: &sqlx::PgPool, discord_id: &str) -> bool {
    sqlx::query_scalar::<_, bool>("SELECT email_verified FROM users WHERE discord_id = $1")
        .bind(discord_id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
        .unwrap_or(false)
}

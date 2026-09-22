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
/// that is not a candidate — on the server, holding an office, or
/// admitted. It used to mean "has a verified Epitech address", which hid
/// the leaderboard from members who were plainly on the server.
///
/// The leaderboard names members and their XP, so it is shown to members
/// only. A database error counts as "no": refusing is the safe answer.
pub async fn is_registered(pool: &sqlx::PgPool, discord_id: &str) -> bool {
    sqlx::query_scalar::<_, bool>("SELECT NOT candidate FROM users WHERE discord_id = $1")
        .bind(discord_id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
        .unwrap_or(false)
}

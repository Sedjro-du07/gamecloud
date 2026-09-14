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

//! Discord → platform: names only.
//!
//! Offices and tracks used to be read from Discord roles. They are now
//! decided on the platform — tracks by each member, offices by the
//! Bureau — and [`super::roles`] mirrors them onto Discord. Reading them
//! back would let a role edited by hand in Discord override the platform,
//! and would put back every track a progression reset had cleared.
//!
//! What is still read is each member's handle and display name, which
//! only the bot can see for members who have not signed in recently.

use serenity::all::{GuildId, Http};
use sqlx::PgPool;

use crate::state::BotState;

/// How many members to pull per page from the Discord API.
const PAGE: u64 = 1000;

/// Store the Discord handle and display name for a member.
///
/// Returns whether anything was written. A member without a platform
/// account matches no row, so nothing is written for them.
async fn record_names(pool: &PgPool, user: &serenity::all::User) -> bool {
    sqlx::query(
        r#"
        UPDATE users
           SET discord_username    = $2,
               discord_global_name = COALESCE($3, users.discord_global_name)
         WHERE discord_id = $1
           AND (users.discord_username IS DISTINCT FROM $2
             OR users.discord_global_name IS DISTINCT FROM
                COALESCE($3, users.discord_global_name))
        "#,
    )
    .bind(user.id.to_string())
    .bind(&user.name)
    .bind(user.global_name.as_deref())
    .execute(pool)
    .await
    .map(|r| r.rows_affected() > 0)
    .unwrap_or(false)
}

/// Refresh the platform's copy of every member's Discord name.
pub async fn run(state: &BotState, http: &Http) {
    let Some(guild_id) = state.config().guild_id else {
        return;
    };
    let guild = GuildId::new(guild_id);

    // Paginate the member list. This needs the GUILD_MEMBERS privileged
    // intent, which must also be enabled in the Developer portal — a
    // failure here is almost always that, so say so plainly.
    let mut after = None;
    let mut names_set = 0_usize;

    loop {
        let members = match guild.members(http, Some(PAGE), after).await {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "import: could not list members — check that the SERVER MEMBERS INTENT \
                     is enabled for this application in the Discord Developer portal"
                );
                return;
            }
        };
        if members.is_empty() {
            break;
        }
        after = members.last().map(|m| m.user.id);

        for member in &members {
            if record_names(state.pool(), &member.user).await {
                names_set += 1;
            }
        }

        if members.len() < usize::try_from(PAGE).unwrap_or(usize::MAX) {
            break;
        }
    }

    if names_set > 0 {
        tracing::info!(names_set, "import: refreshed Discord names");
    }
}

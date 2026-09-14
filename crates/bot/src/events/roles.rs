//! Discord rank-role synchronisation.
//!
//! A rank is otherwise just a string in Postgres. Mirroring it onto a
//! real Discord role is what makes it *socially* real: the member's
//! name changes colour in the server, and everybody else can see it
//! happen. That is a far stronger signal than a number on a web page
//! nobody has open.
//!
//! ## How it works
//!
//! One Discord role per global rank, named after the rank's gamified
//! title. The bot looks roles up **by name** rather than storing ids,
//! so the Bureau can create them by hand with whatever colours and
//! permissions they like and the bot simply finds them. Roles that do
//! not exist are skipped, not created — a bot that invents roles in
//! somebody's guild is a bot nobody trusts.
//!
//! Sync is idempotent: it adds the one role the member should have and
//! removes any other rank role they are carrying. Members whose rank
//! role is already correct cost one comparison and no API call.

use std::collections::HashMap;

use gamecloud_shared::roles::GlobalRank;
use serenity::all::{GuildId, Http, RoleId, UserId};
use sqlx::PgPool;

use crate::state::BotState;

/// Map every rank title to the guild role that carries it.
///
/// Returns an empty map when the guild's roles cannot be read, which
/// makes every downstream call a no-op rather than an error storm.
async fn rank_roles(http: &Http, guild: GuildId) -> HashMap<String, RoleId> {
    let Ok(roles) = guild.roles(http).await else {
        tracing::warn!(%guild, "could not read guild roles; rank sync disabled");
        return HashMap::new();
    };

    let wanted: Vec<&str> = GlobalRank::ALL.iter().map(|r| r.title()).collect();

    roles
        .into_iter()
        .filter(|(_, role)| wanted.iter().any(|w| *w == role.name))
        .map(|(id, role)| (role.name, id))
        .collect()
}

/// Bring one member's Discord roles in line with their platform rank.
///
/// # Errors
/// Returns the Discord API error if a role add/remove fails.
pub async fn sync_member(
    http: &Http,
    guild: GuildId,
    user: UserId,
    rank: GlobalRank,
    roles: &HashMap<String, RoleId>,
) -> anyhow::Result<bool> {
    if roles.is_empty() {
        return Ok(false);
    }

    let member = guild.member(http, user).await?;
    let target = roles.get(rank.title()).copied();

    // Every rank role the member currently holds.
    let held: Vec<RoleId> = member
        .roles
        .iter()
        .copied()
        .filter(|id| roles.values().any(|r| r == id))
        .collect();

    let already_correct = target.is_some_and(|t| held.len() == 1 && held[0] == t);
    if already_correct {
        return Ok(false);
    }

    for stale in held.iter().filter(|id| Some(**id) != target) {
        member.remove_role(http, *stale).await?;
    }
    if let Some(target) = target {
        if !held.contains(&target) {
            member.add_role(http, target).await?;
        }
    }

    Ok(true)
}

/// Sync every verified member of the guild.
///
/// Called once at startup and after each outbox batch that announced a
/// rank change. Failures on individual members are logged and skipped:
/// one member with a role above the bot in the hierarchy must not stop
/// the other ninety-nine from being updated.
pub async fn sync_all(state: &BotState, http: &Http) {
    let Some(guild_id) = state.config().guild_id else {
        return;
    };
    let guild = GuildId::new(guild_id);
    let roles = rank_roles(http, guild).await;
    if roles.is_empty() {
        return;
    }

    let members = match fetch_ranked_members(state.pool()).await {
        Ok(m) => m,
        Err(e) => {
            tracing::error!(error = ?e, "rank sync: could not read members");
            return;
        }
    };

    let mut changed = 0_usize;
    for (discord_id, rank) in members {
        let Ok(raw) = discord_id.parse::<u64>() else {
            continue;
        };
        match sync_member(http, guild, UserId::new(raw), rank, &roles).await {
            Ok(true) => changed += 1,
            Ok(false) => {}
            Err(e) => {
                tracing::debug!(error = %e, discord_id, "rank sync: skipping member");
            }
        }
    }

    if changed > 0 {
        tracing::info!(changed, "rank sync: updated Discord roles");
    }
}

/// Every verified member and the rank they should hold.
async fn fetch_ranked_members(pool: &PgPool) -> sqlx::Result<Vec<(String, GlobalRank)>> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        r#"
        SELECT discord_id, global_rank
          FROM users
         WHERE email_verified = TRUE
        "#,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(id, rank)| (id, GlobalRank::parse(&rank)))
        .collect())
}

#[cfg(test)]
mod tests {
    use gamecloud_shared::roles::GlobalRank;

    #[test]
    fn every_rank_has_a_distinct_role_name() {
        let mut titles: Vec<&str> = GlobalRank::ALL.iter().map(|r| r.title()).collect();
        let before = titles.len();
        titles.sort_unstable();
        titles.dedup();
        assert_eq!(
            titles.len(),
            before,
            "two ranks share a title, so they would map to the same Discord role"
        );
    }

    #[test]
    fn role_names_are_within_discord_limits() {
        // Discord caps role names at 100 characters.
        for rank in GlobalRank::ALL {
            assert!(rank.title().chars().count() <= 100, "{rank:?} too long");
            assert!(!rank.title().is_empty());
        }
    }
}

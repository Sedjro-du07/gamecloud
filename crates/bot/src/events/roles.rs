//! Platform → Discord role synchronisation.
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

use std::collections::{HashMap, HashSet};

use gamecloud_shared::roles::{GlobalRank, Track};
use serenity::all::{GuildId, Http, RoleId, UserId};
use sqlx::PgPool;

use crate::state::BotState;

/// The Discord role names the platform owns for tracks.
///
/// Kept next to [`super::import::track_for_role`], which parses the same
/// names in the other direction. A track whose role does not exist in
/// the guild is skipped rather than created.
#[must_use]
pub fn track_role_name(track: Track) -> String {
    format!("{} {}", track.emoji(), readable(track))
}

/// The human-readable half of a track role name.
const fn readable(track: Track) -> &'static str {
    match track {
        Track::Engineering => "Engineering",
        Track::GameDesign => "Game Design",
        Track::Narrative => "Narrative",
        Track::VisualArt => "Visual Art",
        Track::Audio => "Audio",
        Track::Production => "Production",
        Track::Qa => "QA",
        Track::Marketing => "Marketing",
    }
}

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

/// Map every track role name to the guild role that carries it.
async fn track_roles(http: &Http, guild: GuildId) -> HashMap<String, RoleId> {
    let Ok(roles) = guild.roles(http).await else {
        return HashMap::new();
    };
    let wanted: Vec<String> = Track::ALL.iter().map(|t| track_role_name(*t)).collect();
    roles
        .into_iter()
        .filter(|(_, role)| wanted.contains(&role.name))
        .map(|(id, role)| (role.name, id))
        .collect()
}

/// Bring one member's Discord *track* roles in line with the platform.
///
/// This is the half that was missing: offices and tracks were read from
/// Discord, and ranks were pushed to Discord, but a track joined on the
/// platform never appeared on the member's Discord profile. Both
/// directions now converge — the import applies what Discord says, this
/// applies what the platform says, and each pass leaves the two equal.
///
/// # Errors
/// Returns the Discord API error if a role add/remove fails.
pub async fn sync_member_tracks(
    http: &Http,
    guild: GuildId,
    user: UserId,
    wanted: &HashSet<String>,
    roles: &HashMap<String, RoleId>,
) -> anyhow::Result<bool> {
    if roles.is_empty() {
        return Ok(false);
    }
    let member = guild.member(http, user).await?;
    let owned: HashSet<RoleId> = roles.values().copied().collect();
    let held: HashSet<RoleId> = member.roles.iter().copied().filter(|r| owned.contains(r)).collect();
    let target: HashSet<RoleId> = wanted.iter().filter_map(|n| roles.get(n).copied()).collect();

    let mut changed = false;
    for add in target.difference(&held) {
        member.add_role(http, *add).await?;
        changed = true;
    }
    for remove in held.difference(&target) {
        member.remove_role(http, *remove).await?;
        changed = true;
    }
    Ok(changed)
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

    let tracks = track_roles(http, guild).await;

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
        let user = UserId::new(raw);

        // Everything below rewrites this member's roles, so silence the
        // GuildMemberUpdate echoes it will produce.
        state.suppress_echo(raw);

        match sync_member(http, guild, user, rank, &roles).await {
            Ok(true) => changed += 1,
            Ok(false) => {}
            Err(e) => tracing::debug!(error = %e, discord_id, "rank sync: skipping member"),
        }

        let wanted = match active_tracks(state.pool(), &discord_id).await {
            Ok(w) => w,
            Err(e) => {
                tracing::debug!(error = ?e, discord_id, "track sync: could not read tracks");
                continue;
            }
        };
        match sync_member_tracks(http, guild, user, &wanted, &tracks).await {
            Ok(true) => changed += 1,
            Ok(false) => {}
            Err(e) => tracing::debug!(error = %e, discord_id, "track sync: skipping member"),
        }
    }

    if changed > 0 {
        tracing::info!(changed, "sync: updated Discord roles");
    }
}

/// The track role names a member should currently hold.
async fn active_tracks(pool: &PgPool, discord_id: &str) -> sqlx::Result<HashSet<String>> {
    let rows: Vec<(String,)> = sqlx::query_as(
        r#"
        SELECT m.track
          FROM track_memberships m
          JOIN users u ON u.id = m.user_id
         WHERE u.discord_id = $1 AND m.left_at IS NULL
        "#,
    )
    .bind(discord_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .filter_map(|(t,)| Track::parse(&t))
        .map(track_role_name)
        .collect())
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

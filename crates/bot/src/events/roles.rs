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

use gamecloud_shared::roles::{BureauRole, GlobalRank, Track};
use serenity::all::{GuildId, Http, RoleId, UserId};
use sqlx::PgPool;

use crate::state::BotState;

/// The Discord role names the platform owns for tracks.
///
/// A track whose role does not exist in the guild is skipped rather than
/// created.
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

/// Bring a set of platform-owned Discord roles in line with the platform.
///
/// Used for track roles and for Bureau office roles alike: `roles` is the
/// set the platform owns, `wanted` the names the member should hold.
/// Anything outside `roles` is never touched, so roles the association
/// hands out by hand for other reasons survive every pass.
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

/// Sync every platform account's Discord roles: member title, tracks and
/// Bureau office.
///
/// Runs on the periodic pass and whenever the platform queues a
/// `RoleSync`. Failures on individual members are logged and skipped:
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

    // Offices are handed out on the platform. Discord carries the office
    // role — and the role that opens the Bureau's channels — for exactly
    // as long as the platform says so. Unverified accounts are included:
    // an office can be given before its holder has finished signing up.
    let (bureau, access) = bureau_roles(http, guild).await;
    match fetch_offices(state.pool()).await {
        Ok(rows) => {
            for (discord_id, office) in rows {
                let Ok(raw) = discord_id.parse::<u64>() else {
                    continue;
                };
                let mut wanted = HashSet::new();
                if let Some(office) = office.as_deref().and_then(BureauRole::parse) {
                    wanted.insert(office.title().to_string());
                    if let Some(access) = &access {
                        wanted.insert(access.clone());
                    }
                }
                match sync_member_tracks(http, guild, UserId::new(raw), &wanted, &bureau).await {
                    Ok(true) => changed += 1,
                    Ok(false) => {}
                    Err(e) => tracing::debug!(error = %e, discord_id, "office sync: skipping member"),
                }
            }
        }
        Err(e) => tracing::error!(error = ?e, "office sync: could not read offices"),
    }

    if changed > 0 {
        tracing::info!(changed, "sync: updated Discord roles");
    }
}

/// Map the office titles (the provisional one included), and the role that opens the Bureau's
/// channels, to guild roles.
///
/// Returns the map and the access role's name. The access role is the
/// one whose name contains "Bureau" without being an office — the same
/// rule the outbox uses to ping the Bureau.
async fn bureau_roles(http: &Http, guild: GuildId) -> (HashMap<String, RoleId>, Option<String>) {
    let Ok(roles) = guild.roles(http).await else {
        return (HashMap::new(), None);
    };
    let titles: Vec<&str> = BureauRole::ALL.iter().map(|b| b.title()).collect();
    let access = roles
        .values()
        .find(|r| r.name.contains("Bureau") && !titles.contains(&r.name.as_str()))
        .map(|r| r.name.clone());
    let map = roles
        .into_iter()
        .filter(|(_, r)| titles.contains(&r.name.as_str()) || access.as_ref() == Some(&r.name))
        .map(|(id, r)| (r.name, id))
        .collect();
    (map, access)
}

/// Every platform account and the office it holds, if any.
async fn fetch_offices(pool: &PgPool) -> sqlx::Result<Vec<(String, Option<String>)>> {
    sqlx::query_as("SELECT discord_id, bureau_role FROM users")
        .fetch_all(pool)
        .await
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

/// Every platform account and the member title Discord should show.
///
/// A verified member shows the rank the platform holds. An account that
/// has not verified its email yet — a Bureau member created ahead of their
/// first login, say — is gated on the platform, but the title is earned
/// by progression on the server: with XP it shows the title that XP is
/// worth, and without any it shows nothing. Email verification gates
/// platform rights, not recognition.
async fn fetch_ranked_members(pool: &PgPool) -> sqlx::Result<Vec<(String, GlobalRank)>> {
    let rows: Vec<(String, String, bool, i64)> = sqlx::query_as(
        r#"
        SELECT discord_id, global_rank, email_verified, xp_total
          FROM users
         WHERE email_verified OR xp_total > 0
        "#,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(id, rank, verified, xp)| (id, discord_title(&rank, verified, xp)))
        .collect())
}

/// The member title shown on Discord for an account.
fn discord_title(stored: &str, verified: bool, xp_total: i64) -> GlobalRank {
    if verified {
        GlobalRank::parse(stored)
    } else {
        GlobalRank::from_xp(xp_total).max(GlobalRank::Initiate)
    }
}

#[cfg(test)]
mod title_tests {
    use gamecloud_shared::roles::GlobalRank;

    use super::discord_title;

    #[test]
    fn an_unverified_account_shows_the_title_its_xp_is_worth() {
        // Fred: created ahead of his first login, 15 344 XP from Kumo.
        assert_eq!(discord_title("Pending", false, 15_344), GlobalRank::Legend);
    }

    #[test]
    fn a_verified_member_shows_the_platform_rank() {
        assert_eq!(discord_title("Visitor", true, 15_344), GlobalRank::Visitor);
        assert_eq!(discord_title("Veteran", true, 7_830), GlobalRank::Veteran);
    }
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

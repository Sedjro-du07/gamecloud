//! Discord → platform role import.
//!
//! The companion to [`super::roles`], which pushes *ranks* out to
//! Discord. This module pulls *offices and disciplines* the other way:
//! Discord is where the association actually decides who is Treasurer
//! and who works on audio, so Discord is the source of truth for those,
//! and the platform follows.
//!
//! ## What it reads
//!
//! - **Bureau offices** — a Discord role whose name matches a
//!   [`BureauRole::title`] exactly. The 16 titles are distinct, so the
//!   match is unambiguous.
//! - **Tracks** — either a role named after the canonical track
//!   identifier (`Engineering`, `Audio`, …) or one of the server's own
//!   discipline roles, via [`ALIASES`].
//!
//! ## What it deliberately does not do
//!
//! **It never removes anything.** Two reasons, and they are different:
//!
//! - Dropping a *track* would orphan the XP earned in it —
//!   `track_memberships` holds the pool, not just the membership.
//! - Dropping an *office* on a member whose Discord role simply has not
//!   been assigned yet would silently demote them. On a server where
//!   the roles were only just created, that would strip every office on
//!   the first pass.
//!
//! Revoking an office is therefore an explicit act, through
//! `POST /api/admin/bureau-role`. The import only ever grants.

use gamecloud_shared::roles::{BureauRole, Track};
use serenity::all::{GuildId, Http};
use sqlx::PgPool;
use uuid::Uuid;

use crate::state::BotState;

/// How many members to pull per page from the Discord API.
const PAGE: u64 = 1000;

/// Legacy discipline roles mapped onto platform tracks.
///
/// The Game Cloud guild used to carry these instead of track roles; they
/// were deleted in September 2026 in favour of the roles the platform
/// names itself. The table is kept because it costs nothing and makes the
/// import resilient if a server recreates one of them — [`track_for_role`]
/// tries the canonical spelling first either way.
const ALIASES: &[(&str, Track)] = &[
    ("💻Developer", Track::Engineering),
    ("🔩game engineering", Track::Engineering),
    ("🎮Game design", Track::GameDesign),
    ("✍️Story writing", Track::Narrative),
    ("🎨UI Artist", Track::VisualArt),
    ("🎤🎧Sound Designer", Track::Audio),
];

/// Resolve a Discord role name to a platform track.
///
/// Matching is deliberately tolerant, because a Discord role wants a
/// readable name (`⚙️ Engineering`, `🎨 Visual Art`) while the platform
/// identifier is a bare CamelCase token (`Engineering`, `VisualArt`).
/// Rather than maintain a table for every cosmetic variant, we strip the
/// decoration and close the spaces:
///
/// - `⚙️ Engineering`  → `Engineering`
/// - `🎮 Game Design`  → `GameDesign`
/// - `🐛 QA`           → `QA`
///
/// [`ALIASES`] then covers names that are not a spelling of the track at
/// all, like the guild's own `💻Developer`.
#[must_use]
pub fn track_for_role(name: &str) -> Option<Track> {
    let stripped = name
        .trim_start_matches(|c: char| !c.is_alphanumeric())
        .trim();

    Track::parse(stripped)
        .or_else(|| Track::parse(&stripped.replace(' ', "")))
        .or_else(|| {
            ALIASES
                .iter()
                .find(|(alias, _)| *alias == name)
                .map(|(_, track)| *track)
        })
}

/// Resolve a Discord role name to a Bureau office.
#[must_use]
pub fn bureau_for_role(name: &str) -> Option<BureauRole> {
    BureauRole::ALL.iter().copied().find(|b| b.title() == name)
}

/// What one pass changed.
#[derive(Debug, Default)]
pub struct ImportReport {
    /// Guild members that matched a platform account.
    pub matched: usize,
    /// Members whose office was set or corrected.
    pub offices_set: usize,
    /// Track memberships created.
    pub tracks_added: usize,
}

/// Read every guild member's roles and apply them to the platform.
///
/// Members without a platform account are skipped — there is nothing to
/// grant rights to until they have signed in once through Discord.
pub async fn run(state: &BotState, http: &Http) {
    let Some(guild_id) = state.config().guild_id else {
        return;
    };
    let guild = GuildId::new(guild_id);

    let roles = match guild.roles(http).await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(error = %e, "import: could not read guild roles");
            return;
        }
    };

    // Paginate the member list. This needs the GUILD_MEMBERS privileged
    // intent, which must also be enabled in the Developer portal — a
    // failure here is almost always that, so say so plainly.
    let mut after = None;
    let mut report = ImportReport::default();

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
            let discord_id = member.user.id.to_string();
            let Some(user_id) = platform_account(state.pool(), &discord_id).await else {
                continue;
            };
            report.matched += 1;

            let names: Vec<&str> = member
                .roles
                .iter()
                .filter_map(|id| roles.get(id).map(|r| r.name.as_str()))
                .collect();

            if let Some(office) = names.iter().find_map(|n| bureau_for_role(n)) {
                if set_office(state.pool(), user_id, office).await {
                    report.offices_set += 1;
                    tracing::info!(user = %discord_id, office = office.as_str(), "import: office set");
                }
            }

            for track in names.iter().filter_map(|n| track_for_role(n)) {
                if add_track(state.pool(), user_id, track).await {
                    report.tracks_added += 1;
                    tracing::info!(user = %discord_id, track = track.as_str(), "import: track added");
                }
            }
        }

        if members.len() < usize::try_from(PAGE).unwrap_or(usize::MAX) {
            break;
        }
    }

    if report.offices_set > 0 || report.tracks_added > 0 {
        tracing::info!(
            matched = report.matched,
            offices = report.offices_set,
            tracks = report.tracks_added,
            "import: applied Discord roles to the platform"
        );
    }
}

/// The platform account for a Discord id, if one exists.
async fn platform_account(pool: &PgPool, discord_id: &str) -> Option<Uuid> {
    sqlx::query_scalar::<_, Uuid>("SELECT id FROM users WHERE discord_id = $1")
        .bind(discord_id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
}

/// Set a member's office. Returns whether anything changed.
async fn set_office(pool: &PgPool, user_id: Uuid, office: BureauRole) -> bool {
    sqlx::query(
        r#"
        UPDATE users
           SET bureau_role = $2
         WHERE id = $1
           AND bureau_role IS DISTINCT FROM $2
        "#,
    )
    .bind(user_id)
    .bind(office.as_str())
    .execute(pool)
    .await
    .map(|r| r.rows_affected() > 0)
    .unwrap_or(false)
}

/// Join a track if the member is not already in it. Never removes one:
/// `track_memberships` carries the XP pool, so dropping a row would
/// destroy earned progress.
async fn add_track(pool: &PgPool, user_id: Uuid, track: Track) -> bool {
    sqlx::query(
        r#"
        INSERT INTO track_memberships (user_id, track, track_role, track_xp, last_active_at)
        VALUES ($1, $2, 'Observer', 0, NOW())
        ON CONFLICT (user_id, track) DO UPDATE
            SET left_at = NULL, last_active_at = NOW()
         WHERE track_memberships.left_at IS NOT NULL
        "#,
    )
    .bind(user_id)
    .bind(track.as_str())
    .execute(pool)
    .await
    .map(|r| r.rows_affected() > 0)
    .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// Event-driven reconciliation
// ---------------------------------------------------------------------------

/// Apply a live Discord role change to the platform.
///
/// The periodic [`run`] pass is deliberately additive, because it only
/// ever sees a snapshot: it cannot tell "this member never had the role"
/// from "the role was just removed", and guessing wrong on the first
/// reading would strip everybody.
///
/// A `GuildMemberUpdate` carries the member's *new* role set as the
/// result of a change that happened in Discord, so here the direction is
/// known and Discord can be treated as authoritative for that instant.
/// That is what makes a removal propagate.
///
/// Ranks are excluded on purpose. They are an achievement the platform
/// computes from XP; a rank role removed by hand in Discord is a mistake
/// to be undone by the next push, not an instruction.
pub async fn on_member_update(
    state: &BotState,
    discord_id: &str,
    role_names: &[String],
) {
    // Ignore the bot's own writes. Pushing a member's tracks emits one
    // event per role added, each with a partial snapshot; reading those
    // as removals makes the platform flap.
    if discord_id
        .parse::<u64>()
        .is_ok_and(|raw| state.is_echo(raw))
    {
        tracing::debug!(user = discord_id, "sync: ignoring our own role echo");
        return;
    }

    let Some(user_id) = platform_account(state.pool(), discord_id).await else {
        return;
    };

    // --- offices: Discord says who holds one -------------------------
    let office = role_names.iter().find_map(|n| bureau_for_role(n));
    match office {
        Some(office) => {
            if set_office(state.pool(), user_id, office).await {
                tracing::info!(user = discord_id, office = office.as_str(), "sync: office set");
            }
        }
        None => {
            if clear_office(state.pool(), user_id).await {
                tracing::info!(user = discord_id, "sync: office cleared");
            }
        }
    }

    // --- tracks: the Discord set becomes the active set ---------------
    let wanted: Vec<Track> = role_names.iter().filter_map(|n| track_for_role(n)).collect();
    for track in &wanted {
        if add_track(state.pool(), user_id, *track).await {
            tracing::info!(user = discord_id, track = track.as_str(), "sync: track joined");
        }
    }
    for track in Track::ALL.iter().filter(|t| !wanted.contains(t)) {
        if leave_track(state.pool(), user_id, *track).await {
            tracing::info!(user = discord_id, track = track.as_str(), "sync: track left");
        }
    }
}

/// Clear a member's office. Returns whether anything changed.
async fn clear_office(pool: &PgPool, user_id: Uuid) -> bool {
    sqlx::query("UPDATE users SET bureau_role = NULL WHERE id = $1 AND bureau_role IS NOT NULL")
        .bind(user_id)
        .execute(pool)
        .await
        .map(|r| r.rows_affected() > 0)
        .unwrap_or(false)
}

/// Mark a track membership as left, keeping the XP pool intact.
async fn leave_track(pool: &PgPool, user_id: Uuid, track: Track) -> bool {
    sqlx::query(
        r#"
        UPDATE track_memberships
           SET left_at = NOW()
         WHERE user_id = $1 AND track = $2 AND left_at IS NULL
        "#,
    )
    .bind(user_id)
    .bind(track.as_str())
    .execute(pool)
    .await
    .map(|r| r.rows_affected() > 0)
    .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_bureau_title_resolves() {
        for office in BureauRole::ALL {
            assert_eq!(bureau_for_role(office.title()), Some(office));
        }
    }

    #[test]
    fn bureau_titles_are_distinct() {
        // The match is by name, so two offices sharing a title would make
        // the import ambiguous.
        let mut titles: Vec<&str> = BureauRole::ALL.iter().map(|b| b.title()).collect();
        let before = titles.len();
        titles.sort_unstable();
        titles.dedup();
        assert_eq!(titles.len(), before);
    }

    #[test]
    fn an_ordinary_role_is_not_an_office() {
        for name in ["👑✨️Bureau✨️", "✨️Elder", "@everyone", "DraftBot"] {
            assert_eq!(bureau_for_role(name), None, "{name} matched an office");
        }
    }

    #[test]
    fn canonical_track_names_resolve() {
        for track in Track::ALL {
            assert_eq!(track_for_role(track.as_str()), Some(track));
        }
    }

    #[test]
    fn decorated_role_names_resolve() {
        // The names we give the Discord roles themselves.
        assert_eq!(track_for_role("⚙️ Engineering"), Some(Track::Engineering));
        assert_eq!(track_for_role("🎮 Game Design"), Some(Track::GameDesign));
        assert_eq!(track_for_role("🎨 Visual Art"), Some(Track::VisualArt));
        assert_eq!(track_for_role("🐛 QA"), Some(Track::Qa));
        assert_eq!(track_for_role("📊 Production"), Some(Track::Production));
        assert_eq!(track_for_role("📣 Marketing"), Some(Track::Marketing));
        assert_eq!(track_for_role("📖 Narrative"), Some(Track::Narrative));
        assert_eq!(track_for_role("🎵 Audio"), Some(Track::Audio));
    }

    #[test]
    fn stripping_decoration_does_not_invent_matches() {
        // Loosening the match must not start swallowing unrelated roles.
        for name in ["✨️Elder", "👾Noobie", "🛒Boutique🛒", "🥽VR/AR", "Game"] {
            assert_eq!(track_for_role(name), None, "{name} matched a track");
        }
    }

    #[test]
    fn server_specific_aliases_resolve() {
        assert_eq!(track_for_role("💻Developer"), Some(Track::Engineering));
        assert_eq!(track_for_role("🔩game engineering"), Some(Track::Engineering));
        assert_eq!(track_for_role("🎨UI Artist"), Some(Track::VisualArt));
        assert_eq!(track_for_role("🎤🎧Sound Designer"), Some(Track::Audio));
    }

    #[test]
    fn unrelated_roles_map_to_no_track() {
        for name in ["🥽VR/AR", "🎖Alumni", "👾Noobie", "Punition"] {
            assert_eq!(track_for_role(name), None, "{name} matched a track");
        }
    }

    #[test]
    fn a_rank_role_is_neither_an_office_nor_a_track() {
        // The rank roles this bot creates must not be read back as
        // something else on the return trip.
        use gamecloud_shared::roles::GlobalRank;
        for rank in GlobalRank::ALL {
            assert_eq!(bureau_for_role(rank.title()), None);
            assert_eq!(track_for_role(rank.title()), None);
        }
    }
}

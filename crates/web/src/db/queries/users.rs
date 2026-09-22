//! User and authority queries.

use gamecloud_shared::{
    models::UserRecord,
    roles::{Authority, BureauRole, GlobalRank, Track, TrackMembership, TrackRole},
};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{WebError, WebResult};

/// Fetch a user by ID. Returns `None` if not found.
///
/// # Errors
/// Propagates database errors.
pub async fn find_by_id(pool: &PgPool, id: Uuid) -> WebResult<Option<UserRecord>> {
    let row = sqlx::query_as::<_, UserRecord>(
        "SELECT id, discord_id, discord_username, discord_global_name, github_username, avatar_url, avatar_custom_url, xp_total, level, global_rank, bureau_role, current_title, streak_days, sessions_valid_from, last_activity_at, created_at FROM users WHERE id = $1",
    )
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

/// Fetch a user by Discord ID.
///
/// # Errors
/// Propagates database errors.
pub async fn find_by_discord_id(pool: &PgPool, discord_id: &str) -> WebResult<Option<UserRecord>> {
    let row = sqlx::query_as::<_, UserRecord>(
        "SELECT id, discord_id, discord_username, discord_global_name, github_username, avatar_url, avatar_custom_url, xp_total, level, global_rank, bureau_role, current_title, streak_days, sessions_valid_from, last_activity_at, created_at FROM users WHERE discord_id = $1",
    )
        .bind(discord_id)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

/// Either fetch the existing row for the given Discord ID or create a
/// new `Pending` user. Idempotent.
///
/// # Errors
/// Propagates database errors.
pub async fn upsert_from_discord(
    pool: &PgPool,
    identity: &DiscordIdentity<'_>,
) -> WebResult<UserRecord> {
    let row = sqlx::query_as::<_, UserRecord>(
        r#"
        -- 'Initiate' rather than the old 'Pending': arriving through
        -- Discord OAuth is the membership check, so a new account starts
        -- on the ladder instead of below it waiting for an email.
        INSERT INTO users (discord_id, discord_username, discord_global_name, avatar_url,
                           global_rank)
        VALUES ($1, $2, $3, $4, 'Initiate')
        ON CONFLICT (discord_id) DO UPDATE
            SET avatar_url          = COALESCE(EXCLUDED.avatar_url, users.avatar_url),
                discord_username    = COALESCE(EXCLUDED.discord_username, users.discord_username),
                discord_global_name = COALESCE(EXCLUDED.discord_global_name, users.discord_global_name)
        RETURNING id, discord_id, discord_username, discord_global_name,
                  github_username, avatar_url,
                  avatar_custom_url, xp_total, level, global_rank, bureau_role,
                  current_title, streak_days, sessions_valid_from, last_activity_at, created_at
        "#,
    )
    .bind(identity.discord_id)
    .bind(identity.username)
    .bind(identity.global_name)
    .bind(identity.avatar_url)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

/// What a Discord login tells us about a member.
///
/// Grouped rather than passed as four bare `&str`s, because three of
/// them are optional strings and a caller that swapped the handle and
/// the display name would produce a plausible-looking but wrong row.
#[derive(Debug, Clone, Copy)]
pub struct DiscordIdentity<'a> {
    /// Snowflake, the only field Discord always supplies.
    pub discord_id: &'a str,
    /// Globally unique handle.
    pub username: Option<&'a str>,
    /// Display name the member chose, when they set one.
    pub global_name: Option<&'a str>,
    /// CDN avatar URL built from the avatar hash.
    pub avatar_url: Option<&'a str>,
}

/// Record the Discord names the bot sees for a member.
///
/// The bot meets members the platform has never had a login from — it
/// reads the whole guild — so this fills in names without touching
/// anything else on the row.
///
/// # Errors
/// Propagates database errors.
pub async fn set_discord_names(
    pool: &PgPool,
    discord_id: &str,
    username: &str,
    global_name: Option<&str>,
) -> WebResult<()> {
    sqlx::query(
        r#"
        UPDATE users
           SET discord_username    = $2,
               discord_global_name = COALESCE($3, users.discord_global_name)
         WHERE discord_id = $1
           AND (users.discord_username    IS DISTINCT FROM $2
             OR users.discord_global_name IS DISTINCT FROM COALESCE($3, users.discord_global_name))
        "#,
    )
    .bind(discord_id)
    .bind(username)
    .bind(global_name)
    .execute(pool)
    .await?;
    Ok(())
}

/// Build the `Authority` envelope for a user: rank, bureau role, and
/// the full list of track memberships.
///
/// # Errors
/// Propagates database errors.
pub async fn load_authority(pool: &PgPool, user_id: Uuid) -> WebResult<Authority> {
    let user = find_by_id(pool, user_id).await?;
    let Some(user) = user else {
        return Ok(Authority::anonymous());
    };

    let memberships: Vec<(String, String)> = sqlx::query_as(
        "SELECT track, track_role FROM track_memberships WHERE user_id = $1 AND left_at IS NULL",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let tracks = memberships
        .into_iter()
        .filter_map(|(track, role)| {
            let track = Track::parse(&track)?;
            let role = match role.as_str() {
                "Contributor" => TrackRole::Contributor,
                "Reviewer" => TrackRole::Reviewer,
                "Mentor" => TrackRole::Mentor,
                "CoLead" => TrackRole::CoLead,
                "Lead" => TrackRole::Lead,
                _ => TrackRole::Observer,
            };
            Some(TrackMembership { track, role })
        })
        .collect();

    Ok(Authority {
        rank: parse_rank(&user.global_rank),
        offices: offices_of(pool, user_id).await?,
        tracks,
    })
}

/// Every office a member holds, in protocol order.
///
/// # Errors
/// Propagates database errors.
pub async fn offices_of(pool: &PgPool, user_id: Uuid) -> WebResult<Vec<BureauRole>> {
    let raw: Option<Vec<String>> =
        sqlx::query_scalar("SELECT offices FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_optional(pool)
            .await?;
    Ok(raw
        .unwrap_or_default()
        .iter()
        .filter_map(|o| parse_bureau(o))
        .collect())
}

// Rank and bureau-role parsing now live on the shared enums, so the
// string vocabulary has exactly one definition. These thin wrappers
// keep the call sites in this module readable.
fn parse_rank(s: &str) -> GlobalRank {
    GlobalRank::parse(s)
}

fn parse_bureau(s: &str) -> Option<BureauRole> {
    BureauRole::parse(s)
}

// ===========================================================================
// Profile
// ===========================================================================

/// Fields a member may change about themselves.
///
/// Deliberately narrow: XP, rank, level, streak and bureau role are all
/// platform-owned, and nothing here lets a member touch them.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct ProfileUpdate {
    /// Display title shown instead of the Discord id.
    pub current_title: Option<String>,
    /// Custom avatar URL, overriding the Discord one.
    pub avatar_custom_url: Option<String>,
    /// GitHub login to credit commits to.
    pub github_username: Option<String>,
}

/// Apply a profile update.
///
/// `github_username` is `UNIQUE`-ish in spirit but not in schema, so we
/// check it explicitly: two members claiming the same GitHub login
/// would silently split that account's XP between them.
///
/// # Errors
/// `Validation` on a malformed field, `Conflict` when the GitHub login
/// is already claimed, otherwise database errors.
pub async fn update_profile(
    pool: &PgPool,
    user_id: Uuid,
    update: &ProfileUpdate,
) -> WebResult<UserRecord> {
    if let Some(title) = &update.current_title {
        let trimmed = title.trim();
        if trimmed.is_empty() || trimmed.chars().count() > 48 {
            return Err(WebError::Validation(
                "title must be between 1 and 48 characters".into(),
            ));
        }
    }
    if let Some(url) = &update.avatar_custom_url {
        if !url.starts_with("https://") {
            return Err(WebError::Validation(
                "avatar_custom_url must be an https link".into(),
            ));
        }
    }
    if let Some(login) = &update.github_username {
        if !is_valid_github_login(login) {
            return Err(WebError::Validation(
                "github_username must be 1-39 alphanumeric characters or hyphens".into(),
            ));
        }
        let taken: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM users WHERE github_username = $1 AND id <> $2)",
        )
        .bind(login)
        .bind(user_id)
        .fetch_one(pool)
        .await?;
        if taken {
            return Err(WebError::Conflict(
                "that GitHub account is already linked to another member",
            ));
        }
    }

    let row = sqlx::query_as::<_, UserRecord>(
        r#"
        UPDATE users
           SET current_title     = COALESCE($2, current_title),
               avatar_custom_url = COALESCE($3, avatar_custom_url),
               github_username   = COALESCE($4, github_username)
         WHERE id = $1
        RETURNING id, discord_id, discord_username, discord_global_name,
                  github_username, avatar_url,
                  avatar_custom_url, xp_total, level, global_rank, bureau_role,
                  current_title, streak_days, sessions_valid_from, last_activity_at, created_at
        "#,
    )
    .bind(user_id)
    .bind(update.current_title.as_deref().map(str::trim))
    .bind(&update.avatar_custom_url)
    .bind(&update.github_username)
    .fetch_optional(pool)
    .await?
    .ok_or(WebError::NotFound)?;

    Ok(row)
}

/// GitHub's own rule: 1-39 characters, alphanumeric or single hyphens,
/// not starting or ending with a hyphen.
fn is_valid_github_login(login: &str) -> bool {
    if login.is_empty() || login.len() > 39 {
        return false;
    }
    if login.starts_with('-') || login.ends_with('-') {
        return false;
    }
    if login.contains("--") {
        return false;
    }
    login.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

/// What an office appointment does to the list a member holds.
#[derive(Debug, Clone, Copy)]
pub enum OfficeChange {
    /// Hold this office as well as any others.
    Add(BureauRole),
    /// Stop holding this office; the others stay.
    Remove(BureauRole),
    /// Hold exactly this office, or none — the old single-office
    /// behaviour, kept for the REST route that sends one value.
    Replace(Option<BureauRole>),
}

/// The offices a member ends up with after `change`.
///
/// Pure, so the rules are testable without a database:
/// - the list is kept in protocol order, so the first office is the
///   principal one and `bureau_role` (generated from it) is meaningful;
/// - `Provisional` means "on the Bureau without an office", so it is
///   dropped the moment a real office is added — and never kept alongside
///   one.
#[must_use]
pub fn apply_office_change(current: &[BureauRole], change: OfficeChange) -> Vec<BureauRole> {
    let mut next: Vec<BureauRole> = match change {
        OfficeChange::Add(o) => current.iter().copied().chain(std::iter::once(o)).collect(),
        OfficeChange::Remove(o) => current.iter().copied().filter(|b| *b != o).collect(),
        OfficeChange::Replace(o) => o.into_iter().collect(),
    };
    if next.iter().any(|b| b.holds_office()) {
        next.retain(|b| b.holds_office());
    }
    next.sort_by_key(|b| b.protocol_rank());
    next.dedup();
    next
}

/// Change the offices a member holds.
///
/// Returns the list they now hold.
///
/// # Errors
/// `NotFound` when the id matches nobody — an appointment that quietly
/// lands on no row is how a nomination goes missing while the interface
/// reports success; otherwise database errors.
pub async fn change_offices(
    pool: &PgPool,
    user_id: Uuid,
    change: OfficeChange,
) -> WebResult<Vec<BureauRole>> {
    let mut tx = pool.begin().await?;
    let current: Option<Vec<String>> =
        sqlx::query_scalar("SELECT offices FROM users WHERE id = $1 FOR UPDATE")
            .bind(user_id)
            .fetch_optional(&mut *tx)
            .await?;
    let Some(current) = current else {
        return Err(WebError::NotFound);
    };
    let current: Vec<BureauRole> = current.iter().filter_map(|o| parse_bureau(o)).collect();
    let next = apply_office_change(&current, change);
    let stored: Vec<&str> = next.iter().map(|b| b.as_str()).collect();

    sqlx::query("UPDATE users SET offices = $2 WHERE id = $1")
        .bind(user_id)
        .bind(&stored)
        .execute(&mut *tx)
        .await?;
    // The office roles on Discord follow the platform, immediately.
    crate::services::notifications::request_role_sync(&mut *tx).await?;
    tx.commit().await?;
    Ok(next)
}

/// Assign or clear a single bureau role, replacing any others.
///
/// # Errors
/// `Validation` for an unknown role, `NotFound` for an unknown member.
pub async fn set_bureau_role(
    pool: &PgPool,
    user_id: Uuid,
    role: Option<&str>,
) -> WebResult<()> {
    let office = match role {
        Some(r) => Some(
            BureauRole::parse(r)
                .ok_or_else(|| WebError::Validation(format!("unknown bureau role '{r}'")))?,
        ),
        None => None,
    };
    change_offices(pool, user_id, OfficeChange::Replace(office)).await?;
    Ok(())
}

/// Recent XP ledger lines for a member.
///
/// # Errors
/// Propagates database errors.
pub async fn xp_history(pool: &PgPool, user_id: Uuid, limit: i64) -> WebResult<Vec<XpHistoryRow>> {
    let rows = sqlx::query_as::<_, XpHistoryRow>(
        r#"
        SELECT amount, source, track, description, created_at
          FROM xp_logs
         WHERE user_id = $1
         ORDER BY created_at DESC
         LIMIT $2
        "#,
    )
    .bind(user_id)
    .bind(limit.clamp(1, 200))
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// One line of a member's XP history.
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct XpHistoryRow {
    /// Signed amount.
    pub amount: i32,
    /// Source label.
    pub source: String,
    /// Track, when track-scoped.
    pub track: Option<String>,
    /// Ledger line.
    pub description: Option<String>,
    /// When it happened.
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Resolve a member from their Discord username.
///
/// The Bureau names members the way everybody sees them: by username, with
/// or without a leading `@`, in any case. Identifiers are never asked for,
/// so they never need to be shown.
///
/// # Errors
/// Propagates database errors.
pub async fn find_by_reference(pool: &PgPool, reference: &str) -> WebResult<Option<Uuid>> {
    let username = reference.trim().trim_start_matches('@').trim();
    if username.is_empty() {
        return Ok(None);
    }
    let found: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM users WHERE lower(discord_username) = lower($1)")
            .bind(username)
            .fetch_optional(pool)
            .await?;
    Ok(found)
}

/// The platform account for a server member, created if they have never
/// signed in.
///
/// Lets the Bureau appoint anybody on the Discord server, not only the
/// people who happen to have opened the platform already. The row is the
/// same one Discord OAuth would have made — keyed on the snowflake — so
/// when they do sign in, `upsert_from_discord` finds it and they arrive
/// with their office or track role already in place. They are on the
/// server, so they are not a candidate.
///
/// # Errors
/// Propagates database errors.
pub async fn ensure_for_guild_member(
    pool: &PgPool,
    member: &crate::services::discord::GuildMember,
) -> WebResult<Uuid> {
    let id: Uuid = sqlx::query_scalar(
        r"
        INSERT INTO users (discord_id, discord_username, discord_global_name, global_rank, candidate)
        VALUES ($1, $2, $3, 'Initiate', FALSE)
        ON CONFLICT (discord_id) DO UPDATE
            SET discord_username    = EXCLUDED.discord_username,
                discord_global_name = COALESCE(EXCLUDED.discord_global_name,
                                               users.discord_global_name),
                candidate           = FALSE
        RETURNING id
        ",
    )
    .bind(&member.id)
    .bind(&member.username)
    .bind(member.global_name.as_deref())
    .fetch_one(pool)
    .await?;
    Ok(id)
}

/// Whether this account belongs to the association.
///
/// Membership is being on the association's Discord server — or holding
/// an office, or having been admitted by the Bureau — which is exactly
/// what the stored `candidate` flag records: login sets it from Discord's
/// own answer. It used to be "has verified an Epitech address", which
/// shut out members who were plainly on the server; the address is now
/// optional and decides nothing.
///
/// # Errors
/// Propagates database errors.
pub async fn is_member(pool: &PgPool, user_id: Uuid) -> WebResult<bool> {
    let candidate: Option<bool> =
        sqlx::query_scalar("SELECT candidate FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_optional(pool)
            .await?;
    // An unknown account is not a member.
    Ok(candidate == Some(false))
}

/// Pseudos of the members with these Discord ids, keyed by id, to write
/// mentions out without showing a number.
///
/// # Errors
/// Propagates database errors.
pub async fn pseudos_by_discord_id(
    pool: &PgPool,
    discord_ids: &[String],
) -> WebResult<std::collections::HashMap<String, String>> {
    if discord_ids.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT discord_id, member_display_name(current_title, discord_global_name, discord_username, discord_id) \
           FROM users WHERE discord_id = ANY($1)",
    )
    .bind(discord_ids)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().collect())
}

/// Tracks whose unpublished work a member may see.
///
/// Derived from the permission matrix rather than from the membership
/// table directly, so the answer stays the one `Authority::can` would
/// give and cannot drift from it.
#[must_use]
pub fn visible_tracks(authority: &Authority) -> Vec<String> {
    Track::ALL
        .iter()
        .filter(|t| authority.can(gamecloud_shared::roles::Action::ViewTrackInternalProjects(**t)))
        .map(|t| t.as_str().to_string())
        .collect()
}

#[cfg(test)]
mod office_tests {
    use super::{apply_office_change, OfficeChange};
    use gamecloud_shared::roles::BureauRole::{self, *};

    fn apply(current: &[BureauRole], change: OfficeChange) -> Vec<BureauRole> {
        apply_office_change(current, change)
    }

    #[test]
    fn adding_a_second_office_keeps_the_first() {
        // Inari: Secretary, then Treasurer as well. The single-value
        // column this replaces overwrote the first.
        assert_eq!(apply(&[Secretary], OfficeChange::Add(Treasurer)), vec![Secretary, Treasurer]);
    }

    #[test]
    fn offices_are_kept_in_protocol_order() {
        // Added in the "wrong" order, stored President-first, so the
        // principal office is always the most senior one.
        assert_eq!(apply(&[Treasurer], OfficeChange::Add(Secretary)), vec![Secretary, Treasurer]);
        assert_eq!(apply(&[Moderator], OfficeChange::Add(President)), vec![President, Moderator]);
    }

    #[test]
    fn a_real_office_replaces_the_provisional_seat() {
        assert_eq!(apply(&[Provisional], OfficeChange::Add(Treasurer)), vec![Treasurer]);
    }

    #[test]
    fn the_provisional_seat_is_never_added_beside_an_office() {
        assert_eq!(apply(&[Treasurer], OfficeChange::Add(Provisional)), vec![Treasurer]);
    }

    #[test]
    fn removing_one_office_leaves_the_others() {
        assert_eq!(apply(&[Secretary, Treasurer], OfficeChange::Remove(Secretary)), vec![Treasurer]);
        assert_eq!(apply(&[Treasurer], OfficeChange::Remove(Treasurer)), vec![]);
    }

    #[test]
    fn adding_an_office_already_held_changes_nothing() {
        assert_eq!(apply(&[Secretary, Treasurer], OfficeChange::Add(Treasurer)), vec![Secretary, Treasurer]);
    }

    #[test]
    fn replace_keeps_the_old_single_office_behaviour() {
        assert_eq!(apply(&[Secretary, Treasurer], OfficeChange::Replace(Some(President))), vec![President]);
        assert_eq!(apply(&[Secretary], OfficeChange::Replace(None)), vec![]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_real_github_logins() {
        for login in ["torvalds", "a", "some-user", "User123", "a-b-c"] {
            assert!(is_valid_github_login(login), "{login} should be valid");
        }
    }

    #[test]
    fn rejects_malformed_github_logins() {
        for login in ["", "-leading", "trailing-", "double--hyphen", "has space", "has/slash"] {
            assert!(!is_valid_github_login(login), "{login:?} should be invalid");
        }
    }

    #[test]
    fn rejects_overlong_github_logins() {
        assert!(!is_valid_github_login(&"a".repeat(40)));
        assert!(is_valid_github_login(&"a".repeat(39)));
    }
}

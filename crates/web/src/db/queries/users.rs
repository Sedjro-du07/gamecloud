//! User and authority queries.

use gamecloud_shared::{
    models::UserRecord,
    roles::{Authority, BureauRole, GlobalRank, Track, TrackMembership, TrackRole},
};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::WebResult;

/// Fetch a user by ID. Returns `None` if not found.
///
/// # Errors
/// Propagates database errors.
pub async fn find_by_id(pool: &PgPool, id: Uuid) -> WebResult<Option<UserRecord>> {
    let row = sqlx::query_as::<_, UserRecord>("SELECT * FROM users WHERE id = $1")
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
    let row = sqlx::query_as::<_, UserRecord>("SELECT * FROM users WHERE discord_id = $1")
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
    discord_id: &str,
    avatar_url: Option<&str>,
) -> WebResult<UserRecord> {
    let row = sqlx::query_as::<_, UserRecord>(
        r#"
        INSERT INTO users (discord_id, avatar_url)
        VALUES ($1, $2)
        ON CONFLICT (discord_id) DO UPDATE
            SET avatar_url = COALESCE(EXCLUDED.avatar_url, users.avatar_url)
        RETURNING *
        "#,
    )
    .bind(discord_id)
    .bind(avatar_url)
    .fetch_one(pool)
    .await?;
    Ok(row)
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
        "SELECT track, track_role FROM track_memberships WHERE user_id = $1",
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
        bureau: user.bureau_role.as_deref().and_then(parse_bureau),
        tracks,
    })
}

fn parse_rank(s: &str) -> GlobalRank {
    match s {
        "Visitor" => GlobalRank::Visitor,
        "Initiate" => GlobalRank::Initiate,
        "Apprentice" => GlobalRank::Apprentice,
        "JuniorDev" => GlobalRank::JuniorDev,
        "SeniorDev" => GlobalRank::SeniorDev,
        "Expert" => GlobalRank::Expert,
        "Veteran" => GlobalRank::Veteran,
        "Legend" => GlobalRank::Legend,
        "Myth" => GlobalRank::Myth,
        _ => GlobalRank::Pending,
    }
}

fn parse_bureau(s: &str) -> Option<BureauRole> {
    Some(match s {
        "President" => BureauRole::President,
        "VicePresident" => BureauRole::VicePresident,
        "Secretary" => BureauRole::Secretary,
        "Treasurer" => BureauRole::Treasurer,
        "VpTech" => BureauRole::VpTech,
        "VpCommunity" => BureauRole::VpCommunity,
        "EventManager" => BureauRole::EventManager,
        "AssistantEventManager" => BureauRole::AssistantEventManager,
        "Archiviste" => BureauRole::Archiviste,
        "AssistantArchiviste" => BureauRole::AssistantArchiviste,
        "CommunityManager" => BureauRole::CommunityManager,
        "SocialMediaManager" => BureauRole::SocialMediaManager,
        "Moderator" => BureauRole::Moderator,
        "AssistantModerator" => BureauRole::AssistantModerator,
        "RecruitmentOfficer" => BureauRole::RecruitmentOfficer,
        "PrManager" => BureauRole::PrManager,
        _ => return None,
    })
}

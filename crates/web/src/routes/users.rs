//! Member-facing routes: profile, tracks, badges, history, leaderboards.
//!
//! `ARCHITECTURE.md` advertised `/api/users/*` from the beginning; this
//! is it. The onboarding track-selection endpoint lives here too,
//! because joining a track is what completes onboarding and moves a
//! member from `Visitor` to `Initiate`.

use axum::{
    extract::{Path, Query, State},
    routing::{delete, get},
    Json, Router,
};
use gamecloud_shared::{
    roles::{specializations_for, Action, GlobalRank, Track},
    xp::level_progress,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    db::queries::{badges, leaderboard, qr as qr_q, quests, seasons, tracks, users},
    error::{WebError, WebResult},
    middleware::auth::CurrentUser,
    state::AppState,
};

/// Build the users router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/me", get(me).patch(update_me))
        .route("/me/xp", get(my_xp))
        .route("/me/tracks", get(my_tracks).post(join_track))
        .route("/me/tracks/{track}", delete(leave_track))
        .route("/me/badges", get(my_badges))
        .route("/me/attendance", get(my_attendance))
        .route("/me/quests", get(my_quests))
        .route("/me/sheet", get(character_sheet))
        .route("/catalogue/tracks", get(track_catalogue))
        .route("/catalogue/badges", get(badge_catalogue))
        .route("/leaderboard", get(leaderboard_route))
        .route("/{id}", get(public_profile))
}

// ---------------------------------------------------------------------------
// Profile
// ---------------------------------------------------------------------------

/// The caller's full profile, as the HUD and character sheet need it.
#[derive(Debug, Serialize)]
pub struct ProfileResponse {
    /// Member id.
    pub id: Uuid,
    /// Display name.
    pub display_name: String,
    /// Discord id.
    pub discord_id: String,
    /// Linked GitHub login.
    pub github_username: Option<String>,
    /// Verified Epitech address.
    pub email: Option<String>,
    /// Whether the address is verified.
    pub email_verified: bool,
    /// Avatar.
    pub avatar_url: Option<String>,
    /// Total XP.
    pub xp_total: i64,
    /// Level.
    pub level: i32,
    /// XP into the current level, and XP needed to finish it.
    pub level_progress: (i64, i64),
    /// Rank identifier.
    pub global_rank: String,
    /// Rank display title.
    pub rank_title: String,
    /// Rank accent colour.
    pub rank_color: String,
    /// Bureau role identifier.
    pub bureau_role: Option<String>,
    /// Current streak.
    pub streak_days: i32,
    /// Position on the all-time board.
    pub leaderboard_position: Option<i64>,
}

async fn build_profile(state: &AppState, user_id: Uuid) -> WebResult<ProfileResponse> {
    let record = users::find_by_id(state.pool(), user_id)
        .await?
        .ok_or(WebError::NotFound)?;
    let rank = GlobalRank::parse(&record.global_rank);
    let position = leaderboard::position_of(state.pool(), user_id).await?;

    Ok(ProfileResponse {
        id: record.id,
        display_name: record.display_name(),
        discord_id: record.discord_id,
        github_username: record.github_username,
        email: record.email,
        email_verified: record.email_verified,
        avatar_url: record.avatar_custom_url.or(record.avatar_url),
        xp_total: record.xp_total,
        level: record.level,
        level_progress: level_progress(record.xp_total),
        global_rank: record.global_rank,
        rank_title: rank.title().to_string(),
        rank_color: rank.ring_color().to_string(),
        bureau_role: record.bureau_role,
        streak_days: record.streak_days,
        leaderboard_position: position,
    })
}

async fn me(State(state): State<AppState>, user: CurrentUser) -> WebResult<Json<ProfileResponse>> {
    Ok(Json(build_profile(&state, user.id).await?))
}

async fn public_profile(
    State(state): State<AppState>,
    _user: CurrentUser,
    Path(id): Path<Uuid>,
) -> WebResult<Json<ProfileResponse>> {
    Ok(Json(build_profile(&state, id).await?))
}

async fn update_me(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(update): Json<users::ProfileUpdate>,
) -> WebResult<Json<ProfileResponse>> {
    users::update_profile(state.pool(), user.id, &update).await?;
    Ok(Json(build_profile(&state, user.id).await?))
}

// ---------------------------------------------------------------------------
// XP, badges, attendance, quests
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct LimitQuery {
    limit: Option<i64>,
}

async fn my_xp(
    State(state): State<AppState>,
    user: CurrentUser,
    Query(q): Query<LimitQuery>,
) -> WebResult<Json<Vec<users::XpHistoryRow>>> {
    Ok(Json(
        users::xp_history(state.pool(), user.id, q.limit.unwrap_or(30)).await?,
    ))
}

async fn my_badges(
    State(state): State<AppState>,
    user: CurrentUser,
) -> WebResult<Json<Vec<badges::BadgeView>>> {
    Ok(Json(badges::list_for_user(state.pool(), user.id).await?))
}

async fn my_attendance(
    State(state): State<AppState>,
    user: CurrentUser,
) -> WebResult<Json<Vec<qr_q::AttendanceRow>>> {
    Ok(Json(qr_q::history_for_user(state.pool(), user.id).await?))
}

async fn my_quests(
    State(state): State<AppState>,
    user: CurrentUser,
) -> WebResult<Json<Vec<quests::QuestView>>> {
    Ok(Json(quests::active_for_user(state.pool(), user.id).await?))
}

// ---------------------------------------------------------------------------
// Tracks
// ---------------------------------------------------------------------------

async fn my_tracks(
    State(state): State<AppState>,
    user: CurrentUser,
) -> WebResult<Json<Vec<tracks::MembershipView>>> {
    Ok(Json(tracks::list_for_user(state.pool(), user.id).await?))
}

#[derive(Deserialize)]
struct JoinTrackBody {
    track: String,
    specialization: Option<String>,
}

/// Join a track — the final onboarding step.
///
/// Gated on `CompleteOnboarding` for a `Visitor`. Members who are
/// already `Initiate` or above may join further tracks freely, which is
/// what the multi-track bonus is meant to encourage.
async fn join_track(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(body): Json<JoinTrackBody>,
) -> WebResult<Json<tracks::MembershipView>> {
    if !user.record.email_verified {
        return Err(WebError::Domain(
            gamecloud_shared::DomainError::EmailNotVerified,
        ));
    }

    let authority = users::load_authority(state.pool(), user.id).await?;
    let is_first_track = authority.rank == GlobalRank::Visitor;
    if is_first_track && !authority.can(Action::CompleteOnboarding) {
        return Err(WebError::Forbidden);
    }

    let membership = tracks::join(
        state.pool(),
        state.channels(),
        user.id,
        &body.track,
        body.specialization.as_deref(),
    )
    .await?;

    Ok(Json(membership))
}

async fn leave_track(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(track): Path<String>,
) -> WebResult<Json<serde_json::Value>> {
    tracks::leave(state.pool(), user.id, &track).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

/// The 8 tracks with their specializations — what the onboarding
/// picker renders.
#[derive(Serialize)]
struct TrackCatalogueEntry {
    id: &'static str,
    emoji: &'static str,
    color: &'static str,
    specializations: &'static [&'static str],
}

async fn track_catalogue() -> Json<Vec<TrackCatalogueEntry>> {
    Json(
        Track::ALL
            .iter()
            .map(|t| TrackCatalogueEntry {
                id: t.as_str(),
                emoji: t.emoji(),
                color: t.color_hex(),
                specializations: specializations_for(*t),
            })
            .collect(),
    )
}

async fn badge_catalogue() -> Json<Vec<badges::BadgeView>> {
    Json(badges::catalogue())
}

// ---------------------------------------------------------------------------
// Leaderboard
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct LeaderboardQuery {
    /// `all`, `season` (default) or `track`.
    scope: Option<String>,
    /// Required when `scope=track`.
    track: Option<String>,
    /// Season slug; defaults to the open season.
    season: Option<String>,
    limit: Option<i64>,
}

#[derive(Serialize)]
struct LeaderboardResponse {
    scope: String,
    /// Name of the season or track being shown, when relevant.
    label: Option<String>,
    rows: Vec<leaderboard::LeaderboardRow>,
}

async fn leaderboard_route(
    State(state): State<AppState>,
    Query(q): Query<LeaderboardQuery>,
) -> WebResult<Json<LeaderboardResponse>> {
    let limit = q.limit.unwrap_or(20);
    let scope = q.scope.as_deref().unwrap_or("season");

    match scope {
        "all" => Ok(Json(LeaderboardResponse {
            scope: "all".into(),
            label: None,
            rows: leaderboard::all_time(state.pool(), limit).await?,
        })),
        "track" => {
            let track = q
                .track
                .ok_or_else(|| WebError::Validation("scope=track requires a track".into()))?;
            if Track::parse(&track).is_none() {
                return Err(WebError::Domain(
                    gamecloud_shared::DomainError::UnknownTrack(track),
                ));
            }
            Ok(Json(LeaderboardResponse {
                scope: "track".into(),
                label: Some(track.clone()),
                rows: leaderboard::by_track(state.pool(), &track, limit).await?,
            }))
        }
        _ => {
            let season = match &q.season {
                Some(slug) => Some(seasons::by_slug(state.pool(), slug).await?),
                None => seasons::current(state.pool()).await?,
            };
            match season {
                Some(season) => Ok(Json(LeaderboardResponse {
                    scope: "season".into(),
                    label: Some(season.name.clone()),
                    rows: leaderboard::by_season(state.pool(), season.id, limit).await?,
                })),
                // No season is open — fall back to all-time rather than
                // showing an empty board.
                None => Ok(Json(LeaderboardResponse {
                    scope: "all".into(),
                    label: None,
                    rows: leaderboard::all_time(state.pool(), limit).await?,
                })),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Character sheet
// ---------------------------------------------------------------------------

/// Everything about a member in one payload.
///
/// This is what makes the platform worth something *outside* the club:
/// a member can point at it from a portfolio or an internship
/// application — "Narrative, 3 projects shipped, 2 400 XP".
#[derive(Serialize)]
struct CharacterSheet {
    profile: ProfileResponse,
    tracks: Vec<tracks::MembershipView>,
    badges: Vec<badges::BadgeView>,
    recent_xp: Vec<users::XpHistoryRow>,
    attendance: Vec<qr_q::AttendanceRow>,
    completed_quests: Vec<quests::QuestView>,
}

async fn character_sheet(
    State(state): State<AppState>,
    user: CurrentUser,
) -> WebResult<Json<CharacterSheet>> {
    Ok(Json(CharacterSheet {
        profile: build_profile(&state, user.id).await?,
        tracks: tracks::list_for_user(state.pool(), user.id).await?,
        badges: badges::list_for_user(state.pool(), user.id).await?,
        recent_xp: users::xp_history(state.pool(), user.id, 20).await?,
        attendance: qr_q::history_for_user(state.pool(), user.id).await?,
        completed_quests: quests::completed_by_user(state.pool(), user.id).await?,
    }))
}

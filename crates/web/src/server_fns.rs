//! Leptos server functions.
//!
//! These are the bridge between the pages and the database. Each one
//! compiles twice: on the server it runs the body, in the browser it
//! becomes a typed RPC stub. That is why every return type is a plain
//! owned view model from [`crate::api`].
//!
//! Reads live here; writes that a browser form can express (email, OTP)
//! stay as plain Axum endpoints so the onboarding flow keeps working
//! with JavaScript disabled.

// `#[server]` expands each function's arguments into a generated request
// struct whose fields it does not document. The crate denies missing docs,
// so the lint is relaxed for this module only — every function below still
// carries its own doc comment.
#![allow(missing_docs)]
// Each function body is a `#[cfg(ssr)]` block followed by a
// `#[cfg(not(ssr))]` fallback. Exactly one survives compilation, but the
// parser sees both, so the server block has to `return` rather than fall
// out as a tail expression.
#![allow(clippy::needless_return)]

use leptos::prelude::*;

// Types that appear in the function signatures, so both targets need them.
use crate::api::{
    LeaderboardView, MeView, ProjectCard, ProjectDetailView, QuestItem, SheetView, TrackOption,
};

// Types only constructed inside the server bodies.
#[cfg(feature = "ssr")]
use crate::api::{
    AttendanceEntry, BadgeItem, ContributorItem, LeaderboardEntry, TrackView, VerdictItem, XpEntry,
};

// ---------------------------------------------------------------------------
// Server-only helpers
// ---------------------------------------------------------------------------

/// Resolve the signed-in member from the request cookie.
///
/// Lives behind `ssr` because it touches the Axum request parts that
/// `leptos_routes_with_context` injects.
#[cfg(feature = "ssr")]
mod ctx {
    use leptos::prelude::use_context;
    use uuid::Uuid;

    use crate::{middleware::auth::ACCESS_COOKIE, services::jwt, state::AppState};

    /// The app state, if this call is running inside a request.
    pub fn state() -> Option<AppState> {
        use_context::<AppState>()
    }

    /// The caller's access-token claims, if they present a valid cookie.
    ///
    /// Signature and expiry only — revocation is checked against the
    /// member row by [`current_user_id`], which has to load it anyway.
    fn access_claims(state: &AppState) -> Option<jwt::AccessClaims> {
        let parts = use_context::<axum::http::request::Parts>()?;
        let header = parts.headers.get(axum::http::header::COOKIE)?.to_str().ok()?;
        let prefix = format!("{ACCESS_COOKIE}=");
        let token = header
            .split(';')
            .map(str::trim)
            .find_map(|kv| kv.strip_prefix(prefix.as_str()))?;
        jwt::verify_access_claims(&state.config().jwt_secret, token).ok()
    }

    /// The caller's member id, if their session is valid *and* live.
    ///
    /// The revocation check matters here as much as in the REST
    /// extractor: without it, a signed-out member's profile page would
    /// keep rendering their data server-side.
    pub async fn current_user_id(state: &AppState) -> Option<Uuid> {
        let claims = access_claims(state)?;
        let record = crate::db::queries::users::find_by_id(state.pool(), claims.sub)
            .await
            .ok()??;
        jwt::is_session_live(claims.iat, record.sessions_valid_from).then_some(claims.sub)
    }

    /// Format a timestamp the way the UI shows it.
    pub fn stamp(at: chrono::DateTime<chrono::Utc>) -> String {
        at.format("%d/%m/%Y %H:%M").to_string()
    }

    /// Format a date only.
    pub fn day(at: chrono::DateTime<chrono::Utc>) -> String {
        at.format("%d/%m/%Y").to_string()
    }

    /// Human-readable time remaining until `deadline`.
    pub fn time_left(deadline: chrono::DateTime<chrono::Utc>) -> String {
        let remaining = deadline - chrono::Utc::now();
        if remaining.num_seconds() <= 0 {
            return "terminée".to_string();
        }
        let days = remaining.num_days();
        if days >= 1 {
            return format!("{days} j restants");
        }
        let hours = remaining.num_hours();
        if hours >= 1 {
            return format!("{hours} h restantes");
        }
        format!("{} min restantes", remaining.num_minutes().max(1))
    }
}

// ---------------------------------------------------------------------------
// Me
// ---------------------------------------------------------------------------

/// The signed-in member, or `None` when nobody is signed in.
///
/// # Errors
/// Returns a `ServerFnError` only on an unexpected database failure; a
/// missing or invalid session is `Ok(None)`.
#[server(GetMe, "/api")]
pub async fn get_me() -> Result<Option<MeView>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use gamecloud_shared::{
            roles::{Action, BureauRole, GlobalRank},
            xp::{level_progress, streak_multiplier},
        };

        let Some(state) = ctx::state() else {
            return Ok(None);
        };
        let Some(user_id) = ctx::current_user_id(&state).await else {
            return Ok(None);
        };

        let Some(record) = crate::db::queries::users::find_by_id(state.pool(), user_id)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?
        else {
            return Ok(None);
        };

        let authority = crate::db::queries::users::load_authority(state.pool(), user_id)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
        let position = crate::db::queries::leaderboard::position_of(state.pool(), user_id)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;

        let rank = GlobalRank::parse(&record.global_rank);
        let (into, needed) = level_progress(record.xp_total);

        return Ok(Some(MeView {
            id: record.id.to_string(),
            display_name: record
                .current_title
                .clone()
                .unwrap_or_else(|| record.discord_id.clone()),
            avatar_url: record.avatar_custom_url.clone().or(record.avatar_url.clone()),
            xp_total: record.xp_total,
            level: record.level,
            level_xp_into: into,
            level_xp_needed: needed,
            global_rank: record.global_rank.clone(),
            rank_title: rank.title().to_string(),
            rank_color: rank.ring_color().to_string(),
            bureau_title: record
                .bureau_role
                .as_deref()
                .and_then(BureauRole::parse)
                .map(|r| r.title().to_string()),
            bureau_role: record.bureau_role.clone(),
            streak_days: record.streak_days,
            streak_multiplier: streak_multiplier(record.streak_days),
            email: record.email.clone(),
            email_verified: record.email_verified,
            github_username: record.github_username.clone(),
            leaderboard_position: position,
            // A verified member who has not picked a track is still a
            // Visitor; that is the step the onboarding banner nags about.
            needs_onboarding: record.email_verified && authority.tracks.is_empty(),
            can_access_admin: authority.can(Action::AccessAdminPanel),
            can_generate_qr: authority.can(Action::GenerateQrToken),
        }));
    }

    #[cfg(not(feature = "ssr"))]
    Ok(None)
}

// ---------------------------------------------------------------------------
// Character sheet
// ---------------------------------------------------------------------------

/// Everything about the signed-in member.
///
/// # Errors
/// Returns a `ServerFnError` on database failure.
#[server(GetSheet, "/api")]
pub async fn get_sheet() -> Result<Option<SheetView>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use gamecloud_shared::roles::{SpecialBadge, Track};

        let Some(me) = get_me().await? else {
            return Ok(None);
        };
        let Some(state) = ctx::state() else {
            return Ok(None);
        };
        let Some(user_id) = ctx::current_user_id(&state).await else {
            return Ok(None);
        };

        let err = |e: crate::error::WebError| ServerFnError::new(e.to_string());

        let memberships = crate::db::queries::tracks::list_for_user(state.pool(), user_id)
            .await
            .map_err(err)?;
        let held = crate::db::queries::badges::list_for_user(state.pool(), user_id)
            .await
            .map_err(err)?;
        let history = crate::db::queries::users::xp_history(state.pool(), user_id, 20)
            .await
            .map_err(err)?;
        let attendance = crate::db::queries::qr::history_for_user(state.pool(), user_id)
            .await
            .map_err(err)?;

        let tracks = memberships
            .into_iter()
            .map(|m| {
                let track = Track::parse(&m.track);
                TrackView {
                    emoji: track.map_or("•", Track::emoji).to_string(),
                    color: track.map_or("#9aa0b3", Track::color_hex).to_string(),
                    id: m.track,
                    specialization: m.specialization,
                    role: m.track_role,
                    xp: m.track_xp,
                }
            })
            .collect();

        let held_ids: Vec<String> = held.iter().map(|b| b.badge_type.clone()).collect();
        let badges = SpecialBadge::ALL
            .iter()
            .map(|b| BadgeItem {
                id: b.as_str().to_string(),
                title: b.title().to_string(),
                description: b.description().to_string(),
                held: held_ids.iter().any(|h| h == b.as_str()),
            })
            .collect();

        let recent_xp = history
            .into_iter()
            .map(|row| XpEntry {
                amount: row.amount,
                source: row.source,
                track: row.track,
                description: row.description,
                when: ctx::stamp(row.created_at),
            })
            .collect();

        let attendance = attendance
            .into_iter()
            .map(|a| AttendanceEntry {
                event_name: a.display_name,
                xp: a.xp_rewarded,
                when: ctx::stamp(a.scanned_at),
            })
            .collect();

        return Ok(Some(SheetView {
            me,
            tracks,
            badges,
            recent_xp,
            attendance,
            projects: Vec::new(),
        }));
    }

    #[cfg(not(feature = "ssr"))]
    Ok(None)
}

// ---------------------------------------------------------------------------
// Leaderboard
// ---------------------------------------------------------------------------

/// A leaderboard for the given scope (`all`, `season` or `track`).
///
/// # Errors
/// Returns a `ServerFnError` on database failure.
#[server(GetLeaderboard, "/api")]
pub async fn get_leaderboard(
    scope: String,
    track: Option<String>,
) -> Result<LeaderboardView, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let Some(state) = ctx::state() else {
            return Ok(LeaderboardView::default());
        };
        let viewer = ctx::current_user_id(&state).await;
        let err = |e: crate::error::WebError| ServerFnError::new(e.to_string());

        let (scope, label, rows) = match scope.as_str() {
            "all" => (
                "all".to_string(),
                None,
                crate::db::queries::leaderboard::all_time(state.pool(), 25)
                    .await
                    .map_err(err)?,
            ),
            "track" => {
                let track = track.unwrap_or_default();
                (
                    "track".to_string(),
                    Some(track.clone()),
                    crate::db::queries::leaderboard::by_track(state.pool(), &track, 25)
                        .await
                        .map_err(err)?,
                )
            }
            _ => {
                let season = crate::db::queries::seasons::current(state.pool())
                    .await
                    .map_err(err)?;
                match season {
                    Some(season) => (
                        "season".to_string(),
                        Some(season.name.clone()),
                        crate::db::queries::leaderboard::by_season(state.pool(), season.id, 25)
                            .await
                            .map_err(err)?,
                    ),
                    // Between seasons, show the permanent record rather
                    // than an empty table.
                    None => (
                        "all".to_string(),
                        None,
                        crate::db::queries::leaderboard::all_time(state.pool(), 25)
                            .await
                            .map_err(err)?,
                    ),
                }
            }
        };

        let entries = rows
            .into_iter()
            .enumerate()
            .map(|(i, row)| LeaderboardEntry {
                position: i + 1,
                is_me: viewer == Some(row.user_id),
                user_id: row.user_id.to_string(),
                display_name: row.display_name,
                avatar_url: row.avatar_url,
                xp: row.xp,
                rank: row.global_rank,
                level: row.level,
                streak_days: row.streak_days,
            })
            .collect();

        return Ok(LeaderboardView {
            scope,
            label,
            entries,
        });
    }

    #[cfg(not(feature = "ssr"))]
    {
        let _ = (scope, track);
        Ok(LeaderboardView::default())
    }
}

// ---------------------------------------------------------------------------
// Quests
// ---------------------------------------------------------------------------

/// Open quests with the viewer's progress.
///
/// # Errors
/// Returns a `ServerFnError` on database failure.
#[server(GetQuests, "/api")]
pub async fn get_quests() -> Result<Vec<QuestItem>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let Some(state) = ctx::state() else {
            return Ok(Vec::new());
        };
        let Some(user_id) = ctx::current_user_id(&state).await else {
            return Ok(Vec::new());
        };

        let rows = crate::db::queries::quests::active_for_user(state.pool(), user_id)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;

        return Ok(rows
            .into_iter()
            .map(|q| QuestItem {
                id: q.id.to_string(),
                title: q.title,
                description: q.description,
                xp_reward: q.xp_reward,
                quest_type: q.quest_type,
                condition_label: condition_label(&q.condition_type, q.condition_value),
                progress: q.progress.min(q.condition_value),
                target: q.condition_value,
                track: q.track,
                completed: q.completed,
                time_left: ctx::time_left(q.ends_at),
            })
            .collect());
    }

    #[cfg(not(feature = "ssr"))]
    Ok(Vec::new())
}

/// Turn a condition into something a member can read.
#[cfg(feature = "ssr")]
fn condition_label(condition: &str, target: i32) -> String {
    match condition {
        "Push" => format!("Pousser du code {target} fois"),
        "Attend" => format!("Être présent à {target} événement(s)"),
        "Submit" => format!("Soumettre {target} projet(s)"),
        "Review" => format!("Rendre {target} revue(s)"),
        other => format!("{other} x{target}"),
    }
}

// ---------------------------------------------------------------------------
// Tracks
// ---------------------------------------------------------------------------

/// The 8 tracks with their specializations, flagged with what the
/// viewer already belongs to.
///
/// # Errors
/// Returns a `ServerFnError` on database failure.
#[server(GetTrackOptions, "/api")]
pub async fn get_track_options() -> Result<Vec<TrackOption>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use gamecloud_shared::roles::{specializations_for, Track};

        let joined: Vec<String> = match ctx::state() {
            Some(state) => match ctx::current_user_id(&state).await {
                Some(user_id) => crate::db::queries::tracks::list_for_user(state.pool(), user_id)
                    .await
                    .map_err(|e| ServerFnError::new(e.to_string()))?
                    .into_iter()
                    .map(|m| m.track)
                    .collect(),
                None => Vec::new(),
            },
            None => Vec::new(),
        };

        return Ok(Track::ALL
            .iter()
            .map(|t| TrackOption {
                id: t.as_str().to_string(),
                emoji: t.emoji().to_string(),
                color: t.color_hex().to_string(),
                specializations: specializations_for(*t)
                    .iter()
                    .map(|s| (*s).to_string())
                    .collect(),
                joined: joined.iter().any(|j| j == t.as_str()),
            })
            .collect());
    }

    #[cfg(not(feature = "ssr"))]
    Ok(Vec::new())
}

/// Join a track. This is the step that completes onboarding.
///
/// # Errors
/// Returns a `ServerFnError` carrying the domain message on failure.
#[server(JoinTrack, "/api")]
pub async fn join_track(
    track: String,
    specialization: Option<String>,
) -> Result<(), ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let state = ctx::state().ok_or_else(|| ServerFnError::new("no request context"))?;
        let user_id =
            ctx::current_user_id(&state).await.ok_or_else(|| ServerFnError::new("not signed in"))?;

        let spec = specialization.filter(|s| !s.trim().is_empty());
        crate::db::queries::tracks::join(
            state.pool(),
            state.announce_channel(),
            user_id,
            &track,
            spec.as_deref(),
        )
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
        return Ok(());
    }

    #[cfg(not(feature = "ssr"))]
    {
        let _ = (track, specialization);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Projects
// ---------------------------------------------------------------------------

/// Released projects, optionally filtered by track.
///
/// # Errors
/// Returns a `ServerFnError` on database failure.
#[server(GetProjects, "/api")]
pub async fn get_projects(track: Option<String>) -> Result<Vec<ProjectCard>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let Some(state) = ctx::state() else {
            return Ok(Vec::new());
        };
        let filter = track.filter(|t| !t.is_empty());
        let rows = crate::db::queries::projects::list(state.pool(), filter.as_deref(), false)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
        return Ok(rows.into_iter().map(to_card).collect());
    }

    #[cfg(not(feature = "ssr"))]
    {
        let _ = track;
        Ok(Vec::new())
    }
}

/// One project with its contributors and verdicts.
///
/// # Errors
/// Returns a `ServerFnError` on database failure.
#[server(GetProject, "/api")]
pub async fn get_project(id: String) -> Result<Option<ProjectDetailView>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let Some(state) = ctx::state() else {
            return Ok(None);
        };
        let Ok(uuid) = id.parse::<uuid::Uuid>() else {
            return Ok(None);
        };

        let detail = match crate::db::queries::projects::detail(state.pool(), uuid).await {
            Ok(d) => d,
            Err(crate::error::WebError::NotFound) => return Ok(None),
            Err(e) => return Err(ServerFnError::new(e.to_string())),
        };

        return Ok(Some(ProjectDetailView {
            card: to_card(detail.summary),
            long_description: detail.long_description,
            github_repo_url: detail.github_repo_url,
            itch_url: detail.itch_url,
            screenshots: detail.screenshots,
            contributors: detail
                .contributors
                .into_iter()
                .map(|c| ContributorItem {
                    user_id: c.user_id.to_string(),
                    display_name: c.display_name,
                    avatar_url: c.avatar_url,
                    track: c.track,
                    role: c.role_in_project,
                })
                .collect(),
            validations: detail
                .validations
                .into_iter()
                .map(|v| VerdictItem {
                    track: v.track,
                    status: v.status,
                    reviewer_name: v.reviewer_name,
                    feedback: v.feedback,
                })
                .collect(),
        }));
    }

    #[cfg(not(feature = "ssr"))]
    {
        let _ = id;
        Ok(None)
    }
}

#[cfg(feature = "ssr")]
fn to_card(summary: crate::db::queries::projects::ProjectSummary) -> ProjectCard {
    use gamecloud_shared::{projects::Rarity, roles::Track};

    let rarity_color = match summary.rarity.as_str() {
        "Rare" => Rarity::Rare.color_hex(),
        "Epic" => Rarity::Epic.color_hex(),
        "Legendary" => Rarity::Legendary.color_hex(),
        "Mythic" => Rarity::Mythic.color_hex(),
        _ => Rarity::Common.color_hex(),
    };

    ProjectCard {
        id: summary.id.to_string(),
        name: summary.name,
        short_description: summary.short_description,
        track_emoji: Track::parse(&summary.primary_track)
            .map_or("•", Track::emoji)
            .to_string(),
        track: summary.primary_track,
        status: summary.status,
        rarity: summary.rarity,
        rarity_color: rarity_color.to_string(),
        thumbnail_url: summary.thumbnail_url,
        contributor_count: summary.contributor_count,
        released_on: summary.released_at.map(ctx::day),
    }
}

// ---------------------------------------------------------------------------
// QR scanning
// ---------------------------------------------------------------------------

/// Claim a QR token on behalf of the signed-in member.
///
/// # Errors
/// Returns a `ServerFnError` carrying the domain message — "you have
/// already scanned this code", "this code has reached its scan limit"
/// and so on — so the page can show it verbatim.
#[server(ScanQr, "/api")]
pub async fn scan_qr(token: String) -> Result<String, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let state = ctx::state().ok_or_else(|| ServerFnError::new("no request context"))?;
        let user_id =
            ctx::current_user_id(&state).await.ok_or_else(|| ServerFnError::new("not signed in"))?;

        crate::services::jwt::verify_qr(&state.config().jwt_secret, &token)
            .map_err(|e| ServerFnError::new(e.to_string()))?;

        let claimed = crate::db::queries::qr::claim_qr_token(
            state.pool(),
            state.announce_channel(),
            &token,
            user_id,
        )
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

        return Ok(format!(
            "Présence enregistrée : {} (+{} XP)",
            claimed.event_name, claimed.xp_awarded
        ));
    }

    #[cfg(not(feature = "ssr"))]
    {
        let _ = token;
        Ok(String::new())
    }
}

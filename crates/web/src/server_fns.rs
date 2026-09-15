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
    AuditLine, CalendarEvent, CalendarRights, EventAttendee, EventDraft, FileItem, LeaderboardView, MeView, ProjectCard,
    ProjectDetailView, QrTicket, KumoChatView, QuestItem, ResourceItem, ReviewItem, SharesView, SheetView, SubmissionItem, TestsView, TrackBoard,
    TrackOption,
};

// Types only constructed inside the server bodies.
#[cfg(feature = "ssr")]
use crate::api::{
    AttendanceEntry, BadgeItem, ChatMessage, ContributorItem, LeaderboardEntry, MySubmission, ProjectRights, ShareItem, TestItem, TrackView,
    VerdictItem, XpEntry,
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

    /// A cookie from the current request.
    pub fn cookie(name: &str) -> Option<String> {
        let parts = use_context::<axum::http::request::Parts>()?;
        let header = parts.headers.get(axum::http::header::COOKIE)?.to_str().ok()?;
        let prefix = format!("{name}=");
        header
            .split(';')
            .map(str::trim)
            .find_map(|kv| kv.strip_prefix(prefix.as_str()))
            .map(str::to_string)
    }

    /// Format a timestamp the way the UI shows it.
    pub fn stamp(at: chrono::DateTime<chrono::Utc>) -> String {
        at.format("%d/%m/%Y %H:%M").to_string()
    }

    /// Whether an account is a registered member: it exists and its
    /// Epitech address is verified. A database error reads as "no", so a
    /// hiccup hides members-only data rather than showing it.
    pub async fn is_registered(state: &AppState, id: Option<Uuid>) -> bool {
        let Some(id) = id else {
            return false;
        };
        crate::db::queries::users::find_by_id(state.pool(), id)
            .await
            .ok()
            .flatten()
            .is_some_and(|u| u.email_verified)
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
#[server(GetMe, "/_fn")]
pub async fn get_me() -> Result<Option<MeView>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use gamecloud_shared::{
            roles::{Action, BureauRole, GlobalRank},
            xp::streak_multiplier,
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

        let admission = crate::db::queries::entrance::admission(state.pool(), user_id)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;

        let rank = GlobalRank::parse(&record.global_rank);
        // Progress is shown towards the next title, not a numbered level.
        let next = rank.next_milestone();
        let floor = rank.floor_xp();
        let (into, needed) = next.map_or((0, 0), |(_, at)| {
            ((record.xp_total - floor).clamp(0, at - floor), at - floor)
        });

        return Ok(Some(MeView {
            id: record.id.to_string(),
            display_name: record.display_name(),
            avatar_url: record.avatar_custom_url.clone().or(record.avatar_url.clone()),
            xp_total: record.xp_total,
            next_rank_title: next.map(|(r, _)| r.title().to_string()),
            rank_xp_into: into,
            rank_xp_needed: needed,
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
            can_review: authority
                .tracks
                .iter()
                .any(|m| m.role >= gamecloud_shared::roles::TrackRole::Reviewer),
            can_see_tests: authority.can(Action::AccessAdminPanel) || !record.email_verified,
            is_candidate: admission.candidate,
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
#[server(GetSheet, "/_fn")]
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

/// The member title a leaderboard row shows.
///
/// A verified member shows the rank the platform holds. An account that
/// has not verified its email yet — a Bureau member created ahead of their
/// first login — is gated on the platform but has earned its title by
/// progression, so it shows what its XP is worth, as it does on Discord.
#[cfg(feature = "ssr")]
fn shown_rank(row: &crate::db::queries::leaderboard::LeaderboardRow) -> gamecloud_shared::roles::GlobalRank {
    use gamecloud_shared::roles::GlobalRank;
    if row.email_verified {
        GlobalRank::parse(&row.global_rank)
    } else {
        GlobalRank::from_xp(row.total_xp).max(GlobalRank::Initiate)
    }
}

/// A leaderboard for the given scope (`all`, `season` or `track`).
///
/// # Errors
/// Returns a `ServerFnError` on database failure.
#[server(GetLeaderboard, "/_fn")]
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

        // The board names members and their XP: it is for members. A
        // visitor, or an account that has not verified its address,
        // gets no rows — the page says why.
        if !ctx::is_registered(&state, viewer).await {
            return Ok(LeaderboardView {
                restricted: true,
                ..LeaderboardView::default()
            });
        }

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

        // On a track board the "rank" column holds the track role, so the
        // title and colour come from the track rather than the XP ladder.
        let track_color = (scope == "track")
            .then(|| label.as_deref().and_then(gamecloud_shared::roles::Track::parse))
            .flatten()
            .map(|t| t.color_hex().to_string());
        let entries = rows
            .into_iter()
            .enumerate()
            // Resolved before the row's fields are moved into the entry.
            .map(|(i, row)| (i, shown_rank(&row), row))
            .map(|(i, shown, row)| LeaderboardEntry {
                position: i + 1,
                is_me: viewer == Some(row.user_id),
                user_id: row.user_id.to_string(),
                display_name: row.display_name,
                avatar_url: row.avatar_url,
                xp: row.xp,
                rank_title: match &track_color {
                    Some(_) => gamecloud_shared::roles::TrackRole::title_of(&row.global_rank)
                        .to_string(),
                    None => shown.title().to_string(),
                },
                rank_color: track_color
                    .clone()
                    .unwrap_or_else(|| shown.ring_color().to_string()),
                rank: row.global_rank,
                level: row.level,
                streak_days: row.streak_days,
            })
            .collect();

        return Ok(LeaderboardView {
            scope,
            label,
            entries,
            restricted: false,
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
#[server(GetQuests, "/_fn")]
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
#[server(GetTrackOptions, "/_fn")]
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
#[server(JoinTrack, "/_fn")]
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
            state.channels(),
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
#[server(GetProjects, "/_fn")]
pub async fn get_projects(track: Option<String>) -> Result<Vec<ProjectCard>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let Some(state) = ctx::state() else {
            return Ok(Vec::new());
        };
        let filter = track.filter(|t| !t.is_empty());
        // Signed out is not an error: a visitor simply sees the
        // published record and nothing in progress.
        let viewer = ctx::current_user_id(&state).await;
        let tracks = match viewer {
            Some(id) => {
                let authority = crate::db::queries::users::load_authority(state.pool(), id)
                    .await
                    .map_err(|e| ServerFnError::new(e.to_string()))?;
                crate::db::queries::users::visible_tracks(&authority)
            }
            None => Vec::new(),
        };

        let rows = crate::db::queries::projects::list(
            state.pool(),
            filter.as_deref(),
            &crate::db::queries::projects::Visibility {
                tracks: &tracks,
                viewer,
            },
        )
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
#[server(GetProject, "/_fn")]
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

        let rights = project_rights(&state, &detail).await;

        return Ok(Some(ProjectDetailView {
            card: to_card(detail.summary.clone()),
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
                .iter()
                .map(|v| VerdictItem {
                    track: v.track.clone(),
                    status: v.status.clone(),
                    reviewer_name: v.reviewer_name.clone(),
                    feedback: v.feedback.clone(),
                })
                .collect(),
            rights,
        }));
    }

    #[cfg(not(feature = "ssr"))]
    {
        let _ = id;
        Ok(None)
    }
}

/// What the signed-in member may do on this project.
///
/// Computed here rather than in the browser: the UI decides what to
/// draw, never what is permitted. Every button these flags reveal calls
/// an endpoint that checks the same right again.
#[cfg(feature = "ssr")]
async fn project_rights(
    state: &crate::state::AppState,
    detail: &crate::db::queries::projects::ProjectDetail,
) -> ProjectRights {
    use gamecloud_shared::{projects::ProjectStatus, roles::Action, roles::Track};

    let Some(user_id) = ctx::current_user_id(state).await else {
        return ProjectRights::default();
    };
    let Ok(authority) = crate::db::queries::users::load_authority(state.pool(), user_id).await
    else {
        return ProjectRights::default();
    };

    let status = ProjectStatus::parse(&detail.summary.status);
    let primary = Track::parse(&detail.summary.primary_track);

    // A verdict is only invited for a track that is actually waiting on
    // one, from somebody holding the rank in that track, and never on
    // your own project.
    let is_author = detail.contributors.iter().any(|c| c.user_id == user_id);
    let reviewable_tracks = detail
        .validations
        .iter()
        .filter(|v| v.status == "Pending")
        .filter_map(|v| Track::parse(&v.track))
        .filter(|t| authority.can(Action::ReviewProjectForTrack(*t)))
        .filter(|_| !is_author && status == Some(ProjectStatus::InReview))
        .map(|t| t.as_str().to_string())
        .collect();

    let on_team = detail.contributors.iter().any(|c| c.user_id == user_id);
    let leads_it = primary.is_some_and(|t| {
        authority.has_track_role(t, gamecloud_shared::roles::TrackRole::CoLead)
    });

    ProjectRights {
        can_upload: on_team || leads_it,
        can_submit: status.is_some_and(|s| {
            s.allowed_next().contains(&ProjectStatus::InReview)
        }) && authority.can(Action::SubmitProjectForReview),
        can_release: status == Some(ProjectStatus::Approved)
            && primary.is_some_and(|t| authority.can(Action::PublishProjectAsReleased(t))),
        reviewable_tracks,
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
#[server(ScanQr, "/_fn")]
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
            state.channels(),
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

// ---------------------------------------------------------------------------
// The review circuit
// ---------------------------------------------------------------------------

/// Projects waiting for the caller's verdict.
///
/// # Errors
/// Returns a `ServerFnError` on database failure.
#[server(GetReviewQueue, "/_fn")]
pub async fn get_review_queue() -> Result<Vec<ReviewItem>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let Some(state) = ctx::state() else {
            return Ok(Vec::new());
        };
        let Some(user_id) = ctx::current_user_id(&state).await else {
            return Ok(Vec::new());
        };
        let rows = crate::db::queries::projects::review_queue(state.pool(), user_id)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
        return Ok(rows
            .into_iter()
            .map(|r| ReviewItem {
                project_id: r.project_id.to_string(),
                name: r.name,
                short_description: r.short_description,
                track: r.track,
                author_name: r.author_name,
            })
            .collect());
    }

    #[cfg(not(feature = "ssr"))]
    Ok(Vec::new())
}

/// Render a verdict on one track of a project.
///
/// # Errors
/// Returns a `ServerFnError` carrying the domain message — a rejection
/// with no feedback, a track the caller cannot judge, a project no
/// longer in review.
#[server(ReviewProject, "/_fn")]
pub async fn review_project(
    project_id: String,
    track: String,
    verdict: String,
    feedback: Option<String>,
    score: Option<i32>,
) -> Result<String, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use gamecloud_shared::{projects::Verdict, roles::Action, roles::Track};

        let state = ctx::state().ok_or_else(|| ServerFnError::new("no request context"))?;
        let user_id = ctx::current_user_id(&state)
            .await
            .ok_or_else(|| ServerFnError::new("not signed in"))?;

        let id = project_id
            .parse::<uuid::Uuid>()
            .map_err(|_| ServerFnError::new("unknown project"))?;
        let parsed_track =
            Track::parse(&track).ok_or_else(|| ServerFnError::new("unknown track"))?;
        let parsed_verdict = Verdict::parse(&verdict)
            .ok_or_else(|| ServerFnError::new("unknown verdict"))?;

        // The same check the REST route makes. The UI hides what you
        // cannot do; this is what stops you doing it anyway.
        let authority = crate::db::queries::users::load_authority(state.pool(), user_id)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
        if !authority.can(Action::ReviewProjectForTrack(parsed_track)) {
            return Err(ServerFnError::new(
                "vous n'avez pas le rang de relecteur sur cette track",
            ));
        }

        let status = crate::db::queries::projects::record_verdict(
            state.pool(),
            state.channels(),
            user_id,
            &crate::db::queries::projects::Judgement {
                project_id: id,
                track: parsed_track.as_str(),
                verdict: parsed_verdict,
                feedback: feedback.as_deref().filter(|f| !f.trim().is_empty()),
                score,
            },
        )
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

        return Ok(status.as_str().to_string());
    }

    #[cfg(not(feature = "ssr"))]
    {
        let _ = (project_id, track, verdict, feedback, score);
        Ok(String::new())
    }
}

/// Open a review round on a project.
///
/// # Errors
/// Returns a `ServerFnError` on a refused transition or a missing right.
#[server(SubmitProject, "/_fn")]
pub async fn submit_project(project_id: String) -> Result<Vec<String>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use gamecloud_shared::roles::Action;

        let state = ctx::state().ok_or_else(|| ServerFnError::new("no request context"))?;
        let user_id = ctx::current_user_id(&state)
            .await
            .ok_or_else(|| ServerFnError::new("not signed in"))?;
        let id = project_id
            .parse::<uuid::Uuid>()
            .map_err(|_| ServerFnError::new("unknown project"))?;

        let authority = crate::db::queries::users::load_authority(state.pool(), user_id)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
        if !authority.can(Action::SubmitProjectForReview) {
            return Err(ServerFnError::new("rang insuffisant pour soumettre"));
        }

        return crate::db::queries::projects::submit_for_review(
            state.pool(),
            state.channels(),
            user_id,
            id,
        )
        .await
        .map_err(|e| ServerFnError::new(e.to_string()));
    }

    #[cfg(not(feature = "ssr"))]
    {
        let _ = project_id;
        Ok(Vec::new())
    }
}

// ---------------------------------------------------------------------------
// Authoring and Bureau actions
// ---------------------------------------------------------------------------

/// Create a project, and its GitHub repository when configured.
///
/// # Errors
/// Returns a `ServerFnError` on a validation failure or a missing right.
#[server(CreateProject, "/_fn")]
pub async fn create_project(
    name: String,
    short_description: String,
    primary_track: String,
    epitech_level: String,
) -> Result<String, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use gamecloud_shared::roles::Action;

        let state = ctx::state().ok_or_else(|| ServerFnError::new("no request context"))?;
        let user_id = ctx::current_user_id(&state)
            .await
            .ok_or_else(|| ServerFnError::new("not signed in"))?;

        let authority = crate::db::queries::users::load_authority(state.pool(), user_id)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
        if !authority.can(Action::CreateProject) {
            return Err(ServerFnError::new(
                "il faut le rang Compagnon de Guilde (400 XP) pour créer un projet",
            ));
        }

        let new = crate::db::queries::projects::NewProject {
            name,
            short_description: (!short_description.trim().is_empty())
                .then_some(short_description),
            long_description: None,
            primary_track,
            epitech_level: (!epitech_level.trim().is_empty()).then_some(epitech_level),
            block_number: None,
            github_repo_url: None,
            itch_url: None,
        };

        let id = crate::db::queries::projects::create(state.pool(), user_id, &new)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;

        // The repository is a convenience, not a precondition: GitHub
        // being down must not stop a member starting a project, so a
        // failure is logged and the project stands without one.
        let author_login = crate::db::queries::users::find_by_id(state.pool(), user_id)
            .await
            .ok()
            .flatten()
            .and_then(|u| u.github_username);
        crate::services::github::provision_project_repo(
            &state,
            &crate::services::github::NewRepo {
                project_id: id,
                name: &new.name,
                description: new.short_description.as_deref(),
                author_login: author_login.as_deref(),
            },
        )
        .await;

        return Ok(id.to_string());
    }

    #[cfg(not(feature = "ssr"))]
    {
        let _ = (name, short_description, primary_track, epitech_level);
        Ok(String::new())
    }
}

/// Grant or revoke XP by hand. Bureau only.
///
/// # Errors
/// Returns a `ServerFnError` when the caller lacks `GrantManualXp`.
#[server(GrantXp, "/_fn")]
pub async fn grant_xp(
    member: String,
    amount: i32,
    reason: String,
) -> Result<i32, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use gamecloud_shared::{roles::Action, xp::XpSource};

        let state = ctx::state().ok_or_else(|| ServerFnError::new("no request context"))?;
        let user_id = ctx::current_user_id(&state)
            .await
            .ok_or_else(|| ServerFnError::new("not signed in"))?;

        let authority = crate::db::queries::users::load_authority(state.pool(), user_id)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
        if !authority.can(Action::GrantManualXp) {
            return Err(ServerFnError::new("réservé au Bureau exécutif"));
        }
        if reason.trim().is_empty() {
            return Err(ServerFnError::new("une attribution manuelle doit être motivée"));
        }

        // Accept a platform id or a Discord id, because the Bureau reads
        // names in Discord and ids on the platform.
        let target = crate::db::queries::users::find_by_reference(state.pool(), member.trim())
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?
            .ok_or_else(|| ServerFnError::new("membre introuvable"))?;

        let outcome = crate::db::queries::xp::grant(
            state.pool(),
            state.channels(),
            &crate::db::queries::xp::XpGrant::new(target, amount, XpSource::Manual)
                .describe(reason.trim())
                .exact(),
        )
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

        crate::db::queries::audit::record(
            state.pool(),
            Some(user_id),
            "admin.grant_xp",
            Some("user"),
            Some(target),
            serde_json::json!({ "amount": amount, "reason": reason }),
        )
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

        return Ok(outcome.awarded);
    }

    #[cfg(not(feature = "ssr"))]
    {
        let _ = (member, amount, reason);
        Ok(0)
    }
}

/// Recent audit entries, for the Bureau panel.
///
/// # Errors
/// Returns a `ServerFnError` when the caller lacks `ViewAuditLogs`.
#[server(GetAudit, "/_fn")]
pub async fn get_audit() -> Result<Vec<AuditLine>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use gamecloud_shared::roles::Action;

        let Some(state) = ctx::state() else {
            return Ok(Vec::new());
        };
        let Some(user_id) = ctx::current_user_id(&state).await else {
            return Ok(Vec::new());
        };
        let authority = crate::db::queries::users::load_authority(state.pool(), user_id)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
        if !authority.can(Action::ViewAuditLogs) {
            return Err(ServerFnError::new("réservé au Bureau exécutif"));
        }

        let rows = crate::db::queries::audit::recent(state.pool(), 50)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
        return Ok(rows
            .into_iter()
            .map(|a| AuditLine {
                actor: a.actor_name.unwrap_or_else(|| "la plateforme".into()),
                action: a.action,
                detail: a
                    .metadata
                    .map(|m| m.to_string())
                    .unwrap_or_default(),
                when: ctx::stamp(a.created_at),
            })
            .collect());
    }

    #[cfg(not(feature = "ssr"))]
    Ok(Vec::new())
}

// ---------------------------------------------------------------------------
// Resource library
// ---------------------------------------------------------------------------

/// The resource library, as the viewer may see it.
///
/// # Errors
/// Returns a `ServerFnError` on database failure.
#[server(GetResources, "/_fn")]
pub async fn get_resources() -> Result<Vec<ResourceItem>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use gamecloud_shared::roles::Action;

        let Some(state) = ctx::state() else {
            return Ok(Vec::new());
        };
        let Some(user_id) = ctx::current_user_id(&state).await else {
            return Ok(Vec::new());
        };

        // Unvalidated submissions are shown only to members who can
        // validate them, so what everyone else browses is a library the
        // club has vouched for.
        let authority = crate::db::queries::users::load_authority(state.pool(), user_id)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
        let may_validate = authority.can(Action::ValidateResource);

        let rows = crate::db::queries::resources::list(state.pool(), user_id, None, may_validate)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;

        return Ok(rows
            .into_iter()
            .map(|r| ResourceItem {
                id: r.id.to_string(),
                title: r.title,
                url: r.url,
                kind: r.resource_type,
                tracks: r.tracks,
                level: r.level,
                submitted_by: r.submitted_by_name,
                validated: r.is_validated,
                votes: r.votes,
                has_voted: r.has_voted,
                may_validate,
            })
            .collect());
    }

    #[cfg(not(feature = "ssr"))]
    Ok(Vec::new())
}

/// Submit a resource to the library.
///
/// # Errors
/// Returns a `ServerFnError` on a validation failure.
#[server(SubmitResource, "/_fn")]
pub async fn submit_resource(
    title: String,
    url: String,
    kind: String,
    track: String,
) -> Result<(), ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let state = ctx::state().ok_or_else(|| ServerFnError::new("no request context"))?;
        let user_id = ctx::current_user_id(&state)
            .await
            .ok_or_else(|| ServerFnError::new("connecte-toi pour proposer une ressource"))?;
        // Same rule as the REST route: proposing is for members.
        if !ctx::is_registered(&state, Some(user_id)).await {
            return Err(ServerFnError::new(
                "vérifie ton adresse Epitech pour proposer une ressource",
            ));
        }

        let new = crate::db::queries::resources::NewResource {
            title,
            url,
            resource_type: (!kind.trim().is_empty()).then_some(kind),
            tracks: if track.trim().is_empty() {
                Vec::new()
            } else {
                vec![track]
            },
            specializations: Vec::new(),
            level: None,
        };

        crate::db::queries::resources::submit(state.pool(), state.channels(), user_id, &new)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
        return Ok(());
    }

    #[cfg(not(feature = "ssr"))]
    {
        let _ = (title, url, kind, track);
        Ok(())
    }
}

/// Vote for a resource, or validate it when entitled.
///
/// # Errors
/// Returns a `ServerFnError` carrying the domain message.
#[server(ActOnResource, "/_fn")]
pub async fn act_on_resource(id: String, action: String) -> Result<(), ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use gamecloud_shared::roles::Action;

        let state = ctx::state().ok_or_else(|| ServerFnError::new("no request context"))?;
        let user_id = ctx::current_user_id(&state)
            .await
            .ok_or_else(|| ServerFnError::new("not signed in"))?;
        let resource_id = id
            .parse::<uuid::Uuid>()
            .map_err(|_| ServerFnError::new("unknown resource"))?;

        match action.as_str() {
            "vote" => crate::db::queries::resources::vote(state.pool(), user_id, resource_id)
                .await
                .map(|_| ())
                .map_err(|e| ServerFnError::new(e.to_string())),
            "unvote" => crate::db::queries::resources::unvote(state.pool(), user_id, resource_id)
                .await
                .map(|_| ())
                .map_err(|e| ServerFnError::new(e.to_string())),
            "validate" => {
                let authority =
                    crate::db::queries::users::load_authority(state.pool(), user_id)
                        .await
                        .map_err(|e| ServerFnError::new(e.to_string()))?;
                if !authority.can(Action::ValidateResource) {
                    return Err(ServerFnError::new(
                        "réservé à l'Archiviste et aux responsables de track",
                    ));
                }
                crate::db::queries::resources::validate_entry(
                    state.pool(),
                    state.channels(),
                    user_id,
                    resource_id,
                )
                .await
                .map(|_| ())
                .map_err(|e| ServerFnError::new(e.to_string()))
            }
            other => Err(ServerFnError::new(format!("unknown action {other}"))),
        }
    }

    #[cfg(not(feature = "ssr"))]
    {
        let _ = (id, action);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Member shares
// ---------------------------------------------------------------------------

/// Every share, and what the viewer may do with them.
///
/// The list is visible to anyone, signed in or not: it is the club's
/// shelf. Downloading and posting are for members, and are enforced by
/// the routes in `routes::shares`, not by what this reports.
///
/// # Errors
/// Returns a `ServerFnError` on database failure.
#[server(GetShares, "/_fn")]
pub async fn get_shares() -> Result<SharesView, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use gamecloud_shared::roles::Action;

        let Some(state) = ctx::state() else {
            return Ok(SharesView::default());
        };
        let fail = |e: crate::error::WebError| ServerFnError::new(e.to_string());

        let viewer = ctx::current_user_id(&state).await;
        let (can_upload, moderator) = match viewer {
            Some(id) => {
                let verified = crate::db::queries::users::find_by_id(state.pool(), id)
                    .await
                    .map_err(fail)?
                    .is_some_and(|u| u.email_verified);
                let authority = crate::db::queries::users::load_authority(state.pool(), id)
                    .await
                    .map_err(fail)?;
                (verified, authority.can(Action::ModerateContent))
            }
            None => (false, false),
        };

        let rows = crate::db::queries::shares::list(state.pool())
            .await
            .map_err(fail)?;

        return Ok(SharesView {
            signed_in: viewer.is_some(),
            can_upload,
            items: rows
                .into_iter()
                .map(|s| ShareItem {
                    id: s.id.to_string(),
                    kind_label: crate::db::queries::shares::kind_label(&s.kind).to_string(),
                    host: s
                        .url
                        .as_deref()
                        .and_then(|u| url::Url::parse(u).ok())
                        .and_then(|u| u.host_str().map(|h| h.trim_start_matches("www.").to_string())),
                    is_link: s.url.is_some(),
                    size: s.size_bytes.map(crate::api::format_bytes),
                    filename: s.filename,
                    title: s.title,
                    description: s.description,
                    kind: s.kind,
                    downloads: s.download_count,
                    author: s.uploaded_by_name,
                    when: ctx::day(s.created_at),
                    may_delete: moderator || viewer == Some(s.uploaded_by),
                })
                .collect(),
        });
    }

    #[cfg(not(feature = "ssr"))]
    Ok(SharesView::default())
}

/// Remove a share. Its author, or moderation.
///
/// # Errors
/// Returns a `ServerFnError` when the caller may not remove it.
#[server(DeleteShare, "/_fn")]
pub async fn delete_share(id: String) -> Result<(), ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use gamecloud_shared::roles::Action;

        let fail = |e: crate::error::WebError| ServerFnError::new(e.to_string());
        let state = ctx::state().ok_or_else(|| ServerFnError::new("no request context"))?;
        let user_id = ctx::current_user_id(&state)
            .await
            .ok_or_else(|| ServerFnError::new("connecte-toi d'abord"))?;
        let share_id = id
            .parse::<uuid::Uuid>()
            .map_err(|_| ServerFnError::new("partage introuvable"))?;

        let share = crate::db::queries::shares::find(state.pool(), share_id)
            .await
            .map_err(fail)?;
        if share.uploaded_by != user_id {
            let authority = crate::db::queries::users::load_authority(state.pool(), user_id)
                .await
                .map_err(fail)?;
            if !authority.can(Action::ModerateContent) {
                return Err(ServerFnError::new(
                    "seul l'auteur ou la modération peut retirer ce partage",
                ));
            }
        }

        let stored = crate::db::queries::shares::delete(state.pool(), share_id)
            .await
            .map_err(fail)?;
        if let Some(name) = stored {
            let _ = tokio::fs::remove_file(state.config().shares_dir.join(name)).await;
        }
        return Ok(());
    }

    #[cfg(not(feature = "ssr"))]
    {
        let _ = id;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Chat with Kumo
// ---------------------------------------------------------------------------

/// Cookie that finds a visitor's conversation with Kumo again.
#[cfg(feature = "ssr")]
const KUMO_CHAT_COOKIE: &str = "gc_kumo_chat";

/// The conversation this browser, or this signed-in member, already has.
#[cfg(feature = "ssr")]
async fn current_conversation(
    state: &crate::state::AppState,
) -> Result<Option<uuid::Uuid>, ServerFnError> {
    use crate::db::queries::kumo_chat;

    let fail = |e: crate::error::WebError| ServerFnError::new(e.to_string());
    if let Some(token) = ctx::cookie(KUMO_CHAT_COOKIE) {
        let hash = crate::services::tokens::hash(&token);
        if let Some(id) = kumo_chat::find_by_token(state.pool(), &hash).await.map_err(fail)? {
            return Ok(Some(id));
        }
    }
    match ctx::current_user_id(state).await {
        Some(user) => kumo_chat::latest_for_user(state.pool(), user).await.map_err(fail),
        None => Ok(None),
    }
}

/// Give this browser the cookie of a new conversation.
#[cfg(feature = "ssr")]
fn remember_conversation(state: &crate::state::AppState, token: &str) {
    use axum_extra::extract::cookie::{Cookie, SameSite};

    let Some(response) = use_context::<leptos_axum::ResponseOptions>() else {
        return;
    };
    let cookie = Cookie::build((KUMO_CHAT_COOKIE, token.to_string()))
        .http_only(true)
        .secure(state.config().is_production)
        .same_site(SameSite::Lax)
        .path("/")
        .max_age(time::Duration::days(30))
        .build();
    if let Ok(value) = axum::http::HeaderValue::from_str(&cookie.to_string()) {
        response.append_header(axum::http::header::SET_COOKIE, value);
    }
}

/// The viewer's conversation with Kumo, if they have one.
///
/// # Errors
/// Returns a `ServerFnError` on database failure.
#[server(GetKumoChat, "/_fn")]
pub async fn get_kumo_chat() -> Result<KumoChatView, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let Some(state) = ctx::state() else {
            return Ok(KumoChatView::default());
        };
        let Some(conversation) = current_conversation(&state).await? else {
            return Ok(KumoChatView::default());
        };
        let rows = crate::db::queries::kumo_chat::messages(state.pool(), conversation)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
        return Ok(KumoChatView {
            messages: rows
                .into_iter()
                .map(|m| ChatMessage {
                    status: if m.from_kumo {
                        "kumo"
                    } else if m.relay_failed {
                        "failed"
                    } else if m.relayed_at.is_some() {
                        "relayed"
                    } else {
                        "pending"
                    }
                    .to_string(),
                    author: if m.from_kumo {
                        m.answered_by
                            .clone()
                            .map_or_else(|| "Kumo".to_string(), |name| format!("{name} (Bureau)"))
                    } else {
                        String::new()
                    },
                    from_kumo: m.from_kumo,
                    body: m.body,
                    when: format!("{} UTC", ctx::stamp(m.created_at)),
                })
                .collect(),
        });
    }

    #[cfg(not(feature = "ssr"))]
    Ok(KumoChatView::default())
}

/// Write to Kumo. Needs no account.
///
/// The first message starts the conversation and gives the browser its
/// cookie. The bot relays the message to Kumo within a few seconds.
///
/// # Errors
/// Returns a `ServerFnError` for an empty or overlong message, or when
/// the conversation, or Kumo's channel as a whole, is sending too much.
#[server(SendKumoMessage, "/_fn")]
pub async fn send_kumo_message(body: String) -> Result<(), ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use crate::db::queries::kumo_chat;

        let fail = |e: crate::error::WebError| ServerFnError::new(e.to_string());
        let state = ctx::state().ok_or_else(|| ServerFnError::new("no request context"))?;
        let body = kumo_chat::validate_body(&body).map_err(ServerFnError::new)?;

        if kumo_chat::pending_total(state.pool()).await.map_err(fail)? >= kumo_chat::MAX_PENDING_TOTAL {
            return Err(ServerFnError::new(
                "Kumo reçoit beaucoup de messages en ce moment, réessaie dans quelques minutes",
            ));
        }

        let viewer = ctx::current_user_id(&state).await;
        let conversation = if let Some(id) = current_conversation(&state).await? {
            id
        } else {
            let token = crate::services::tokens::generate();
            let id = kumo_chat::create(state.pool(), &crate::services::tokens::hash(&token), viewer)
                .await
                .map_err(fail)?;
            remember_conversation(&state, &token);
            id
        };
        if let Some(user) = viewer {
            kumo_chat::attach_user(state.pool(), conversation, user).await.map_err(fail)?;
        }

        if kumo_chat::recent_from_visitor(state.pool(), conversation).await.map_err(fail)?
            >= kumo_chat::MAX_RECENT_MESSAGES
        {
            return Err(ServerFnError::new(
                "doucement : attends quelques minutes avant d'envoyer d'autres messages",
            ));
        }

        kumo_chat::post(state.pool(), conversation, &body).await.map_err(fail)?;
        return Ok(());
    }

    #[cfg(not(feature = "ssr"))]
    {
        let _ = body;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Entrance tests
// ---------------------------------------------------------------------------

/// The caller, if they hold a Bureau office.
#[cfg(feature = "ssr")]
async fn require_bureau(state: &crate::state::AppState) -> Result<uuid::Uuid, ServerFnError> {
    let user_id = ctx::current_user_id(state)
        .await
        .ok_or_else(|| ServerFnError::new("connecte-toi d'abord"))?;
    let authority = crate::db::queries::users::load_authority(state.pool(), user_id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    if authority.can(gamecloud_shared::roles::Action::AccessAdminPanel) {
        Ok(user_id)
    } else {
        Err(ServerFnError::new("réservé au Bureau"))
    }
}

#[cfg(feature = "ssr")]
fn present_test(
    test: &crate::db::queries::entrance::EntranceTest,
    mine: Option<&crate::db::queries::entrance::Submission>,
) -> TestItem {
    use crate::api::format_bytes;
    TestItem {
        id: test.id.to_string(),
        title: test.title.clone(),
        description: test.description.clone(),
        closes: format!("{} UTC", ctx::stamp(test.closes_at)),
        time_left: ctx::time_left(test.closes_at),
        open: test.closes_at > chrono::Utc::now(),
        subject_size: format_bytes(test.subject_size),
        submissions: test.submissions,
        mine: mine.map(|m| MySubmission {
            filename: m.filename.clone(),
            size: format_bytes(m.size_bytes),
            when: format!("{} UTC", ctx::stamp(m.submitted_at)),
            verdict: m.verdict.clone(),
        }),
    }
}

/// The entrance tests page.
///
/// For the Bureau, every session. For somebody who is not a verified
/// member yet, the open sessions and the ones they handed work in for.
/// For somebody not signed in — quite possibly an outsider wondering how
/// to join — the open sessions, without the subject or any count, so the
/// page can tell them what to do. For members, nothing: the page is not
/// for them.
///
/// # Errors
/// Returns a `ServerFnError` on database failure.
#[server(GetTests, "/_fn")]
pub async fn get_tests() -> Result<TestsView, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use crate::db::queries::entrance;
        use gamecloud_shared::roles::Action;

        let fail = |e: crate::error::WebError| ServerFnError::new(e.to_string());
        let Some(state) = ctx::state() else {
            return Ok(TestsView::default());
        };
        let sessions = entrance::list(state.pool()).await.map_err(fail)?;
        let now = chrono::Utc::now();
        let signin = TestsView {
            access: "signin".into(),
            tests: sessions
                .iter()
                .filter(|t| t.closes_at > now)
                .map(|t| TestItem {
                    submissions: 0,
                    ..present_test(t, None)
                })
                .collect(),
            ..TestsView::default()
        };
        let Some(user_id) = ctx::current_user_id(&state).await else {
            return Ok(signin);
        };
        let Some(record) = crate::db::queries::users::find_by_id(state.pool(), user_id)
            .await
            .map_err(fail)?
        else {
            return Ok(signin);
        };
        let authority = crate::db::queries::users::load_authority(state.pool(), user_id)
            .await
            .map_err(fail)?;
        let bureau = authority.can(Action::AccessAdminPanel);
        if !bureau && record.email_verified {
            return Ok(TestsView {
                access: "member".into(),
                ..TestsView::default()
            });
        }

        if bureau {
            return Ok(TestsView {
                access: "bureau".into(),
                tests: sessions.iter().map(|t| present_test(t, None)).collect(),
                ..TestsView::default()
            });
        }

        let mine = entrance::my_submissions(state.pool(), user_id).await.map_err(fail)?;
        let admission = entrance::admission(state.pool(), user_id).await.map_err(fail)?;
        return Ok(TestsView {
            access: "candidate".into(),
            tests: sessions
                .iter()
                .filter_map(|t| {
                    let handed_in = mine.iter().find(|m| m.test_id == t.id);
                    (t.closes_at > now || handed_in.is_some()).then(|| present_test(t, handed_in))
                })
                .collect(),
            is_candidate: admission.candidate,
            admitted: admission.admitted_at.is_some(),
            invite_url: admission.discord_invite_url,
        });
    }

    #[cfg(not(feature = "ssr"))]
    Ok(TestsView::default())
}

/// The work handed in for a session. Bureau only.
///
/// # Errors
/// Returns a `ServerFnError` when the caller is not in the Bureau.
#[server(GetTestSubmissions, "/_fn")]
pub async fn get_test_submissions(test_id: String) -> Result<Vec<SubmissionItem>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use crate::api::format_bytes;

        let state = ctx::state().ok_or_else(|| ServerFnError::new("no request context"))?;
        require_bureau(&state).await?;
        let id = test_id
            .parse::<uuid::Uuid>()
            .map_err(|_| ServerFnError::new("test introuvable"))?;
        let rows = crate::db::queries::entrance::submissions(state.pool(), id)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
        return Ok(rows
            .into_iter()
            .map(|s| SubmissionItem {
                id: s.id.to_string(),
                candidate: s.candidate_name,
                handle: s.discord_username,
                filename: s.filename,
                size: format_bytes(s.size_bytes),
                when: format!("{} UTC", ctx::stamp(s.submitted_at)),
                comment: s.comment,
                verdict: s.verdict,
            })
            .collect());
    }

    #[cfg(not(feature = "ssr"))]
    {
        let _ = test_id;
        Ok(Vec::new())
    }
}

/// Admit or turn down a candidate on their work. Bureau only.
///
/// Admitting creates their single-use invitation to the Discord server,
/// which they then see on their tests page. Returns what to tell the
/// Bureau member, invitation link included.
///
/// # Errors
/// Returns a `ServerFnError` when the caller is not in the Bureau or the
/// work is already graded.
#[server(JudgeSubmission, "/_fn")]
pub async fn judge_submission(id: String, verdict: String) -> Result<String, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use crate::db::queries::entrance;

        let fail = |e: crate::error::WebError| ServerFnError::new(e.to_string());
        let state = ctx::state().ok_or_else(|| ServerFnError::new("no request context"))?;
        let reviewer = require_bureau(&state).await?;
        let submission = id
            .parse::<uuid::Uuid>()
            .map_err(|_| ServerFnError::new("rendu introuvable"))?;

        let candidate = entrance::judge(state.pool(), submission, reviewer, &verdict)
            .await
            .map_err(fail)?;
        if verdict != "Admitted" {
            return Ok("Candidature refusée.".into());
        }

        let admission = entrance::admission(state.pool(), candidate).await.map_err(fail)?;
        if let Some(url) = admission.discord_invite_url {
            return Ok(format!("Admis. Son invitation existait déjà : {url}"));
        }
        return Ok(match crate::services::discord::create_invite(state.config()).await {
            Some(url) => {
                entrance::set_invite(state.pool(), candidate, &url).await.map_err(fail)?;
                format!("Admis. Invitation au serveur créée : {url} — le candidat la voit sur sa page Tests.")
            }
            None => "Admis, mais l'invitation au serveur n'a pas pu être créée : \
                     envoie-lui un lien d'invitation à la main."
                .into(),
        });
    }

    #[cfg(not(feature = "ssr"))]
    {
        let _ = (id, verdict);
        Ok(String::new())
    }
}

/// Stop accepting work for a session now. Bureau only.
///
/// # Errors
/// Returns a `ServerFnError` when the caller is not in the Bureau.
#[server(CloseTest, "/_fn")]
pub async fn close_test(id: String) -> Result<(), ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let state = ctx::state().ok_or_else(|| ServerFnError::new("no request context"))?;
        require_bureau(&state).await?;
        let id = id
            .parse::<uuid::Uuid>()
            .map_err(|_| ServerFnError::new("test introuvable"))?;
        crate::db::queries::entrance::close_now(state.pool(), id)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
        return Ok(());
    }

    #[cfg(not(feature = "ssr"))]
    {
        let _ = id;
        Ok(())
    }
}

/// Delete a session, its subject and every piece of work. Bureau only.
///
/// # Errors
/// Returns a `ServerFnError` when the caller is not in the Bureau.
#[server(DeleteTest, "/_fn")]
pub async fn delete_test(id: String) -> Result<(), ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let state = ctx::state().ok_or_else(|| ServerFnError::new("no request context"))?;
        require_bureau(&state).await?;
        let id = id
            .parse::<uuid::Uuid>()
            .map_err(|_| ServerFnError::new("test introuvable"))?;
        let (subject, work) = crate::db::queries::entrance::delete(state.pool(), id)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
        let base = &state.config().tests_dir;
        let _ = tokio::fs::remove_file(base.join("subjects").join(subject)).await;
        for stored in work {
            let _ = tokio::fs::remove_file(base.join("submissions").join(stored)).await;
        }
        return Ok(());
    }

    #[cfg(not(feature = "ssr"))]
    {
        let _ = id;
        Ok(())
    }
}

/// Open a quest. Bureau only.
///
/// # Errors
/// Returns a `ServerFnError` when the caller lacks `GrantManualXp`.
#[server(OpenQuest, "/_fn")]
pub async fn open_quest(
    title: String,
    condition: String,
    target: i32,
    xp_reward: i32,
    days: i32,
) -> Result<String, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use gamecloud_shared::roles::Action;

        let state = ctx::state().ok_or_else(|| ServerFnError::new("no request context"))?;
        let user_id = ctx::current_user_id(&state)
            .await
            .ok_or_else(|| ServerFnError::new("not signed in"))?;

        let authority = crate::db::queries::users::load_authority(state.pool(), user_id)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
        if !authority.can(Action::GrantManualXp) {
            return Err(ServerFnError::new("réservé au Bureau exécutif"));
        }

        let now = chrono::Utc::now();
        let quest = crate::db::queries::quests::NewQuest {
            title,
            description: None,
            xp_reward,
            quest_type: "Weekly".to_string(),
            condition_type: condition,
            condition_value: target,
            track: None,
            starts_at: now,
            ends_at: now + chrono::Duration::days(i64::from(days.clamp(1, 90))),
        };

        let id = crate::db::queries::quests::create(
            state.pool(),
            state.channels(),
            user_id,
            &quest,
        )
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
        return Ok(id.to_string());
    }

    #[cfg(not(feature = "ssr"))]
    {
        let _ = (title, condition, target, xp_reward, days);
        Ok(String::new())
    }
}

/// Credit somebody on a project.
///
/// # Errors
/// Returns a `ServerFnError` on a missing right or unknown member.
#[server(AddContributor, "/_fn")]
pub async fn add_contributor(
    project_id: String,
    member: String,
    track: String,
    role: String,
) -> Result<(), ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use gamecloud_shared::roles::Action;

        let state = ctx::state().ok_or_else(|| ServerFnError::new("no request context"))?;
        let user_id = ctx::current_user_id(&state)
            .await
            .ok_or_else(|| ServerFnError::new("not signed in"))?;

        let authority = crate::db::queries::users::load_authority(state.pool(), user_id)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
        if !authority.can(Action::CreateProject) {
            return Err(ServerFnError::new("rang insuffisant"));
        }

        let id = project_id
            .parse::<uuid::Uuid>()
            .map_err(|_| ServerFnError::new("unknown project"))?;
        let target = crate::db::queries::users::find_by_reference(state.pool(), member.trim())
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?
            .ok_or_else(|| ServerFnError::new("membre introuvable"))?;

        crate::db::queries::projects::add_contributor(
            state.pool(),
            state.channels(),
            id,
            target,
            &track,
            (!role.trim().is_empty()).then_some(role.trim()),
        )
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
        return Ok(());
    }

    #[cfg(not(feature = "ssr"))]
    {
        let _ = (project_id, member, track, role);
        Ok(())
    }
}

/// Appoint somebody to a track role.
///
/// # Errors
/// Returns a `ServerFnError` when the caller may not appoint in that
/// track, or the member is unknown.
#[server(AppointTrackRole, "/_fn")]
pub async fn appoint_track_role(
    member: String,
    track: String,
    role: String,
) -> Result<(), ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use gamecloud_shared::roles::{Action, Track, TrackRole};

        let state = ctx::state().ok_or_else(|| ServerFnError::new("no request context"))?;
        let user_id = ctx::current_user_id(&state)
            .await
            .ok_or_else(|| ServerFnError::new("not signed in"))?;

        let parsed_track =
            Track::parse(&track).ok_or_else(|| ServerFnError::new("track inconnue"))?;
        let parsed_role = match role.as_str() {
            "Lead" => TrackRole::Lead,
            "CoLead" => TrackRole::CoLead,
            "Observer" => TrackRole::Observer,
            other => return Err(ServerFnError::new(format!("rôle inconnu : {other}"))),
        };

        // Naming a Lead is executive business; a CoLead may be named by
        // the track's own Lead. The permission table already draws that
        // line, so we just pick the matching action.
        let action = if parsed_role == TrackRole::Lead {
            Action::AppointTrackLead(parsed_track)
        } else {
            Action::AppointTrackCoLead(parsed_track)
        };
        let authority = crate::db::queries::users::load_authority(state.pool(), user_id)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
        if !authority.can(action) {
            return Err(ServerFnError::new(
                "vous ne pouvez pas nommer sur cette track",
            ));
        }

        let target = crate::db::queries::users::find_by_reference(state.pool(), member.trim())
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?
            .ok_or_else(|| ServerFnError::new("membre introuvable"))?;

        crate::db::queries::tracks::set_role(
            state.pool(),
            state.channels(),
            target,
            parsed_track,
            parsed_role,
        )
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;

        crate::db::queries::audit::record(
            state.pool(),
            Some(user_id),
            "admin.set_track_role",
            Some("user"),
            Some(target),
            serde_json::json!({ "track": parsed_track.as_str(), "role": parsed_role.as_str() }),
        )
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

        return Ok(());
    }

    #[cfg(not(feature = "ssr"))]
    {
        let _ = (member, track, role);
        Ok(())
    }
}

/// Builds attached to a project.
///
/// # Errors
/// Returns a `ServerFnError` on database failure.
#[server(GetProjectFiles, "/_fn")]
pub async fn get_project_files(project_id: String) -> Result<Vec<FileItem>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use crate::api::format_bytes;

        let Some(state) = ctx::state() else {
            return Ok(Vec::new());
        };
        let Ok(id) = project_id.parse::<uuid::Uuid>() else {
            return Ok(Vec::new());
        };
        let rows = crate::db::queries::files::list(state.pool(), id)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;

        return Ok(rows
            .into_iter()
            .map(|f| FileItem {
                id: f.id.to_string(),
                filename: f.filename,
                kind: f.file_type,
                size: format_bytes(f.size_bytes),
                version: f.version,
                changelog: f.changelog,
                when: ctx::day(f.uploaded_at),
            })
            .collect());
    }

    #[cfg(not(feature = "ssr"))]
    {
        let _ = project_id;
        Ok(Vec::new())
    }
}

// ---------------------------------------------------------------------------
// Calendar
// ---------------------------------------------------------------------------

/// The scope a stored event belongs to.
///
/// Falls back to `Bureau` when the row is somehow inconsistent: the
/// most restrictive answer is the safe one, because guessing `Association`
/// on a malformed row would put a private meeting on everybody's
/// calendar.
#[cfg(feature = "ssr")]
fn scope_of(row: &crate::db::queries::events::EventRow) -> gamecloud_shared::roles::EventScope {
    use gamecloud_shared::roles::EventScope;
    EventScope::parse(&row.audience, row.track.as_deref()).unwrap_or(EventScope::Bureau)
}

/// French label for an event kind.
///
/// Lives here rather than in the page so the label is decided once, on
/// the server, and every surface that shows an event agrees.
#[cfg(feature = "ssr")]
fn kind_label(kind: &str) -> &'static str {
    match kind {
        "Session" => "Séance",
        "Workshop" => "Atelier",
        "Jam" => "Game jam",
        "Meeting" => "Réunion",
        "Deadline" => "Échéance",
        "Showcase" => "Présentation",
        _ => "Événement",
    }
}

/// Turn a stored event into what the calendar renders.
#[cfg(feature = "ssr")]
fn present_event(
    row: &crate::db::queries::events::EventRow,
    authority: &gamecloud_shared::roles::Authority,
) -> CalendarEvent {
    use gamecloud_shared::roles::{Action, Track};

    let track = row.track.as_deref().and_then(Track::parse);
    let same_day = row.starts_at.date_naive() == row.ends_at.date_naive();
    let time_label = if row.ends_at == row.starts_at {
        row.starts_at.format("%H:%M").to_string()
    } else if same_day {
        format!(
            "{} – {}",
            row.starts_at.format("%H:%M"),
            row.ends_at.format("%H:%M")
        )
    } else {
        format!(
            "{} → {}",
            row.starts_at.format("%d/%m %H:%M"),
            row.ends_at.format("%d/%m %H:%M")
        )
    };

    CalendarEvent {
        id: row.id.to_string(),
        title: row.title.clone(),
        description: row.description.clone(),
        kind_label: kind_label(&row.kind).to_string(),
        kind: row.kind.clone(),
        track: row.track.clone(),
        track_emoji: track.map(|t| t.emoji().to_string()),
        audience_label: match row.audience.as_str() {
            "Bureau" => "Bureau".to_string(),
            "Track" => track.map_or_else(|| "Track".to_string(), |t| t.as_str().to_string()),
            _ => "Association".to_string(),
        },
        audience: row.audience.clone(),
        day: row.starts_at.format("%Y-%m-%d").to_string(),
        time_label,
        when_label: ctx::stamp(row.starts_at),
        location: row.location.clone(),
        xp_reward: row.xp_reward,
        attendee_count: row.attendee_count,
        cancelled: row.cancelled_at.is_some(),
        past: row.ends_at < chrono::Utc::now(),
        can_manage: authority.can(Action::ManageEvents(scope_of(row))),
    }
}

/// Parse a `YYYY-MM` month into the half-open window it covers.
///
/// Returns `None` for anything unparseable, which the caller turns into
/// the current month rather than an error — a bad query string should
/// show the calendar, not a stack trace.
#[cfg(feature = "ssr")]
fn month_window(
    month: &str,
) -> Option<(
    chrono::DateTime<chrono::Utc>,
    chrono::DateTime<chrono::Utc>,
)> {
    use chrono::{Datelike, NaiveDate, TimeZone};

    let (year, rest) = month.split_once('-')?;
    let year: i32 = year.parse().ok()?;
    let month: u32 = rest.parse().ok()?;
    let first = NaiveDate::from_ymd_opt(year, month, 1)?;
    // Adding a month by hand, because `chrono` has no calendar-aware
    // "next month" and December has to roll the year.
    let next = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1)?
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1)?
    };
    let _ = first.day();
    Some((
        chrono::Utc.from_utc_datetime(&first.and_hms_opt(0, 0, 0)?),
        chrono::Utc.from_utc_datetime(&next.and_hms_opt(0, 0, 0)?),
    ))
}

/// Everything on the calendar for one month.
///
/// `month` is `YYYY-MM`; anything else falls back to the current month.
/// `track` filters to one track's events plus the association-wide ones,
/// because a member looking at the Audio calendar still needs to know
/// about the general assembly.
///
/// # Errors
/// Returns a `ServerFnError` when the database is unreachable.
#[server(GetCalendar, "/_fn")]
pub async fn get_calendar(
    month: String,
    track: Option<String>,
) -> Result<Vec<CalendarEvent>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use chrono::Datelike;

        let Some(state) = ctx::state() else {
            return Ok(Vec::new());
        };

        let (from, to) = month_window(&month).unwrap_or_else(|| {
            let now = chrono::Utc::now();
            month_window(&format!("{:04}-{:02}", now.year(), now.month()))
                .expect("the current month is always a valid YYYY-MM")
        });

        // The calendar is for people signed in; a visitor gets nothing
        // and the page asks them to sign in.
        let Some(viewer) = ctx::current_user_id(&state).await else {
            return Ok(Vec::new());
        };
        let authority = crate::db::queries::users::load_authority(state.pool(), viewer)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;

        let sees_bureau = authority.can(gamecloud_shared::roles::Action::ManageEvents(
            gamecloud_shared::roles::EventScope::Bureau,
        ));
        let rows = crate::db::queries::events::in_window(
            state.pool(),
            from,
            to,
            track.as_deref().filter(|t| !t.is_empty()),
            sees_bureau,
        )
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

        return Ok(rows.iter().map(|r| present_event(r, &authority)).collect());
    }

    #[cfg(not(feature = "ssr"))]
    {
        let _ = (month, track);
        Ok(Vec::new())
    }
}

/// The next few events, for the home page.
///
/// # Errors
/// Returns a `ServerFnError` when the database is unreachable.
#[server(GetUpcoming, "/_fn")]
pub async fn get_upcoming() -> Result<Vec<CalendarEvent>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let Some(state) = ctx::state() else {
            return Ok(Vec::new());
        };
        // Like the calendar itself: nothing for a visitor.
        let Some(viewer) = ctx::current_user_id(&state).await else {
            return Ok(Vec::new());
        };
        let authority = crate::db::queries::users::load_authority(state.pool(), viewer)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
        let sees_bureau = authority.can(gamecloud_shared::roles::Action::ManageEvents(
            gamecloud_shared::roles::EventScope::Bureau,
        ));
        let rows = crate::db::queries::events::upcoming(state.pool(), 5, sees_bureau)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
        return Ok(rows.iter().map(|r| present_event(r, &authority)).collect());
    }

    #[cfg(not(feature = "ssr"))]
    Ok(Vec::new())
}

/// What the viewer may schedule.
///
/// # Errors
/// Returns a `ServerFnError` when the database is unreachable.
#[server(GetCalendarRights, "/_fn")]
pub async fn get_calendar_rights() -> Result<CalendarRights, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use gamecloud_shared::roles::{Action, EventScope, Track};

        let Some(state) = ctx::state() else {
            return Ok(CalendarRights::default());
        };
        let Some(user_id) = ctx::current_user_id(&state).await else {
            return Ok(CalendarRights::default());
        };
        let authority = crate::db::queries::users::load_authority(state.pool(), user_id)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;

        let association = authority.can(Action::ManageEvents(EventScope::Association));
        let bureau = authority.can(Action::ManageEvents(EventScope::Bureau));
        let tracks: Vec<String> = Track::ALL
            .iter()
            .filter(|t| authority.can(Action::ManageEvents(EventScope::Track(**t))))
            .map(|t| t.as_str().to_string())
            .collect();

        return Ok(CalendarRights {
            any: association || bureau || !tracks.is_empty(),
            association,
            bureau,
            tracks,
        });
    }

    #[cfg(not(feature = "ssr"))]
    Ok(CalendarRights::default())
}

/// Create or update an event.
///
/// One function for both, because the form is the same one and the only
/// difference is whether the caller already has an id. `id` empty means
/// create.
///
/// # Errors
/// Returns a `ServerFnError` on a missing right or a malformed event.
#[server(SaveEvent, "/_fn")]
pub async fn save_event(draft: EventDraft) -> Result<String, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use gamecloud_shared::roles::Action;

        let state = ctx::state().ok_or_else(|| ServerFnError::new("no request context"))?;
        let user_id = ctx::current_user_id(&state)
            .await
            .ok_or_else(|| ServerFnError::new("not signed in"))?;

        let EventDraft {
            id,
            title,
            description,
            kind,
            track,
            audience,
            starts_at,
            ends_at,
            location,
            xp_reward,
        } = draft;

        let audience = if audience.trim().is_empty() {
            "Association".to_string()
        } else {
            audience.trim().to_string()
        };
        // A track only belongs on a track-scoped event; carrying one on a
        // Bureau meeting would violate the schema's CHECK and, worse,
        // make the meeting look public.
        let track = (audience == "Track")
            .then(|| track.trim().to_string())
            .filter(|t| !t.is_empty());
        let scope = gamecloud_shared::roles::EventScope::parse(&audience, track.as_deref())
            .ok_or_else(|| ServerFnError::new("portée invalide"))?;

        let authority = crate::db::queries::users::load_authority(state.pool(), user_id)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
        if !authority.can(Action::ManageEvents(scope)) {
            return Err(ServerFnError::new(
                "vous ne pouvez pas modifier le calendrier de cette portée",
            ));
        }

        let new = crate::db::queries::events::NewEvent {
            title,
            description: (!description.trim().is_empty()).then(|| description.clone()),
            kind,
            track,
            audience,
            starts_at: parse_local(&starts_at)?,
            ends_at: parse_local(&ends_at)?,
            location: (!location.trim().is_empty()).then(|| location.clone()),
            xp_reward,
        };

        if id.trim().is_empty() {
            let created = crate::db::queries::events::create(state.pool(), state.channels(), user_id, &new)
                .await
                .map_err(|e| ServerFnError::new(e.to_string()))?;
            return Ok(created.to_string());
        }

        let existing = id
            .parse::<uuid::Uuid>()
            .map_err(|_| ServerFnError::new("événement inconnu"))?;

        // Re-check against the event as it *is*, not only as it is being
        // rewritten: without this, a track Lead could take over an
        // association-wide event by submitting it with their own track.
        let current = crate::db::queries::events::find(state.pool(), existing)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
        if !authority.can(Action::ManageEvents(scope_of(&current))) {
            return Err(ServerFnError::new(
                "cet événement ne relève pas de votre portée",
            ));
        }

        crate::db::queries::events::update(state.pool(), existing, &new)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
        return Ok(existing.to_string());
    }

    #[cfg(not(feature = "ssr"))]
    {
        let _ = draft;
        Ok(String::new())
    }
}

/// Read a `datetime-local` value into an instant.
///
/// The browser sends `2026-09-16T14:00` with no zone. The association is
/// one campus in one timezone, so reading it as UTC keeps the number the
/// organiser typed the number everyone sees — converting through a
/// guessed local zone would shift every event by an hour twice a year.
#[cfg(feature = "ssr")]
fn parse_local(raw: &str) -> Result<chrono::DateTime<chrono::Utc>, ServerFnError> {
    use chrono::TimeZone;

    let raw = raw.trim();
    let naive = chrono::NaiveDateTime::parse_from_str(raw, "%Y-%m-%dT%H:%M")
        .or_else(|_| chrono::NaiveDateTime::parse_from_str(raw, "%Y-%m-%dT%H:%M:%S"))
        .map_err(|_| ServerFnError::new(format!("date illisible : {raw}")))?;
    Ok(chrono::Utc.from_utc_datetime(&naive))
}

/// Call an event off, or put it back on.
///
/// # Errors
/// Returns a `ServerFnError` on a missing right or an unknown event.
#[server(CancelEvent, "/_fn")]
pub async fn cancel_event(id: String, cancelled: bool) -> Result<(), ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use gamecloud_shared::roles::Action;

        let state = ctx::state().ok_or_else(|| ServerFnError::new("no request context"))?;
        let user_id = ctx::current_user_id(&state)
            .await
            .ok_or_else(|| ServerFnError::new("not signed in"))?;
        let event_id = id
            .parse::<uuid::Uuid>()
            .map_err(|_| ServerFnError::new("événement inconnu"))?;

        let current = crate::db::queries::events::find(state.pool(), event_id)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
        let authority = crate::db::queries::users::load_authority(state.pool(), user_id)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
        if !authority.can(Action::ManageEvents(scope_of(&current))) {
            return Err(ServerFnError::new(
                "cet événement ne relève pas de votre portée",
            ));
        }

        crate::db::queries::events::set_cancelled(state.pool(), state.channels(), event_id, cancelled)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
        return Ok(());
    }

    #[cfg(not(feature = "ssr"))]
    {
        let _ = (id, cancelled);
        Ok(())
    }
}

/// Remove an event from the calendar.
///
/// Only possible while nobody has scanned in; after that the event can
/// be cancelled but not erased.
///
/// # Errors
/// Returns a `ServerFnError` on a missing right, an unknown event, or an
/// event that already has attendance.
#[server(DeleteEvent, "/_fn")]
pub async fn delete_event(id: String) -> Result<(), ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use gamecloud_shared::roles::Action;

        let state = ctx::state().ok_or_else(|| ServerFnError::new("no request context"))?;
        let user_id = ctx::current_user_id(&state)
            .await
            .ok_or_else(|| ServerFnError::new("not signed in"))?;
        let event_id = id
            .parse::<uuid::Uuid>()
            .map_err(|_| ServerFnError::new("événement inconnu"))?;

        let current = crate::db::queries::events::find(state.pool(), event_id)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
        let authority = crate::db::queries::users::load_authority(state.pool(), user_id)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
        if !authority.can(Action::ManageEvents(scope_of(&current))) {
            return Err(ServerFnError::new(
                "cet événement ne relève pas de votre portée",
            ));
        }

        crate::db::queries::events::delete(state.pool(), event_id)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
        return Ok(());
    }

    #[cfg(not(feature = "ssr"))]
    {
        let _ = id;
        Ok(())
    }
}

/// Who turned up to an event.
///
/// Visible to whoever may manage the event — an attendance sheet is not
/// something every member needs to read.
///
/// # Errors
/// Returns a `ServerFnError` on a missing right or an unknown event.
#[server(GetEventAttendees, "/_fn")]
pub async fn get_event_attendees(id: String) -> Result<Vec<EventAttendee>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use gamecloud_shared::roles::Action;

        let state = ctx::state().ok_or_else(|| ServerFnError::new("no request context"))?;
        let user_id = ctx::current_user_id(&state)
            .await
            .ok_or_else(|| ServerFnError::new("not signed in"))?;
        let event_id = id
            .parse::<uuid::Uuid>()
            .map_err(|_| ServerFnError::new("événement inconnu"))?;

        let current = crate::db::queries::events::find(state.pool(), event_id)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
        let authority = crate::db::queries::users::load_authority(state.pool(), user_id)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
        if !authority.can(Action::ManageEvents(scope_of(&current))) {
            return Err(ServerFnError::new("réservé aux organisateurs"));
        }

        let rows = crate::db::queries::events::attendees(state.pool(), event_id)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
        return Ok(rows
            .into_iter()
            .map(|a| EventAttendee {
                display_name: a.display_name,
                when: ctx::stamp(a.scanned_at),
                xp_rewarded: a.xp_rewarded,
            })
            .collect());
    }

    #[cfg(not(feature = "ssr"))]
    {
        let _ = id;
        Ok(Vec::new())
    }
}

/// Mint a QR code for an event.
///
/// The code encodes a *link* to the scan page, not the raw token: every
/// phone camera opens a link, and almost none can hand a decoded string
/// to a web page. Firefox and Safari have no `BarcodeDetector` at all,
/// so a design that needed one would work for roughly nobody.
///
/// Generating is gated twice — on `GenerateQrToken`, and on being able
/// to manage the event in question — because the two are genuinely
/// different rights: the event managers mint codes, but a track Lead who
/// runs their own session should be able to mint one for it without
/// being handed the association's whole event calendar.
///
/// # Errors
/// Returns a `ServerFnError` on a missing right or an unknown event.
#[server(GenerateEventQr, "/_fn")]
pub async fn generate_event_qr(
    event_id: String,
    minutes: i32,
    max_scans: Option<i32>,
) -> Result<QrTicket, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use gamecloud_shared::roles::Action;
        use qrcode::{render::svg, QrCode};

        let state = ctx::state().ok_or_else(|| ServerFnError::new("no request context"))?;
        let user_id = ctx::current_user_id(&state)
            .await
            .ok_or_else(|| ServerFnError::new("not signed in"))?;
        let id = event_id
            .parse::<uuid::Uuid>()
            .map_err(|_| ServerFnError::new("événement inconnu"))?;

        let event = crate::db::queries::events::find(state.pool(), id)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
        let authority = crate::db::queries::users::load_authority(state.pool(), user_id)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;

        if !(authority.can(Action::GenerateQrToken)
            || authority.can(Action::ManageEvents(scope_of(&event))))
        {
            return Err(ServerFnError::new(
                "vous ne pouvez pas générer de code pour cet événement",
            ));
        }
        if event.cancelled_at.is_some() {
            return Err(ServerFnError::new("cet événement est annulé"));
        }
        if let Some(max) = max_scans {
            if max <= 0 {
                return Err(ServerFnError::new("le nombre de scans doit être positif"));
            }
        }

        // Capped at a day: a code that outlives the session it admits to
        // is a code somebody can claim from home a week later.
        let ttl = u64::try_from(minutes.clamp(1, 1440)).unwrap_or(60) * 60;

        let qr_id = uuid::Uuid::new_v4();
        let (token, exp) = crate::services::jwt::issue_qr(
            &state.config().jwt_secret,
            qr_id,
            &event.title,
            &event.kind,
            event.xp_reward,
            ttl,
        )
        .map_err(|e| ServerFnError::new(e.to_string()))?;

        crate::db::queries::qr::insert_qr_token(
            state.pool(),
            &crate::db::queries::qr::NewQrToken {
                token: &token,
                event_name: &event.title,
                event_type: &event.kind,
                xp_value: event.xp_reward,
                created_by: user_id,
                expires_at: exp,
                max_scans,
                event_id: Some(id),
            },
        )
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

        let scan_url = format!(
            "{}/scan?token={}",
            state.config().public_origin.trim_end_matches('/'),
            crate::routes::qr::urlencoding(&token)
        );
        let svg = QrCode::new(&scan_url)
            .map_err(|e| ServerFnError::new(format!("qr encode: {e}")))?
            .render::<svg::Color>()
            .min_dimensions(280, 280)
            .build();

        return Ok(QrTicket {
            svg,
            scan_url,
            expires_label: ctx::stamp(exp),
            max_scans,
        });
    }

    #[cfg(not(feature = "ssr"))]
    {
        let _ = (event_id, minutes, max_scans);
        Err(ServerFnError::new("ssr only"))
    }
}

// ---------------------------------------------------------------------------
// Track board
// ---------------------------------------------------------------------------

/// French label for a track role.
#[cfg(feature = "ssr")]
fn track_role_label(role: &str) -> &'static str {
    match role {
        "Lead" => "Responsable",
        "CoLead" => "Co-responsable",
        "Mentor" => "Mentor",
        "Reviewer" => "Relecteur",
        "Contributor" => "Contributeur",
        _ => "Observateur",
    }
}

/// Everything one track's page shows: its people, its projects and the
/// marks it has given, plus its own sessions.
///
/// # Errors
/// Returns a `ServerFnError` on an unknown track or an unreachable
/// database.
#[server(GetTrackBoard, "/_fn")]
pub async fn get_track_board(track: String) -> Result<TrackBoard, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use crate::api::{TrackMemberRow, TrackProjectRow};
        use gamecloud_shared::roles::{specializations_for, Action, Authority, Track};

        let state = ctx::state().ok_or_else(|| ServerFnError::new("no request context"))?;
        let parsed = Track::parse(&track).ok_or_else(|| ServerFnError::new("track inconnue"))?;

        let (authority, user_id) = match ctx::current_user_id(&state).await {
            Some(id) => (
                crate::db::queries::users::load_authority(state.pool(), id)
                    .await
                    .map_err(|e| ServerFnError::new(e.to_string()))?,
                Some(id),
            ),
            None => (Authority::anonymous(), None),
        };

        let board = crate::db::queries::tracks::board(state.pool(), parsed.as_str(), user_id)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;

        let events = crate::db::queries::events::upcoming_for_track(
            state.pool(),
            parsed.as_str(),
            6,
        )
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

        return Ok(TrackBoard {
            id: parsed.as_str().to_string(),
            emoji: parsed.emoji().to_string(),
            color: parsed.color_hex().to_string(),
            specializations: specializations_for(parsed)
                .iter()
                .map(|s| (*s).to_string())
                .collect(),
            joined: board.my_role.is_some(),
            my_role: board.my_role.as_deref().map(|r| track_role_label(r).to_string()),
            can_review: authority.can(Action::ReviewProjectForTrack(parsed)),
            can_manage_events: authority
                .can(Action::ManageEvents(gamecloud_shared::roles::EventScope::Track(parsed))),
            total_xp: board.total_xp,
            average_score: board.average_score,
            members: board
                .members
                .into_iter()
                .map(|m| TrackMemberRow {
                    display_name: m.display_name,
                    role_label: track_role_label(&m.track_role).to_string(),
                    role: m.track_role,
                    track_xp: m.track_xp,
                    specialization: m.specialization,
                })
                .collect(),
            projects: board
                .projects
                .into_iter()
                .map(|p| TrackProjectRow {
                    id: p.id.to_string(),
                    name: p.name,
                    status: p.status,
                    verdict: p.verdict,
                    score: p.score,
                    reviewer_name: p.reviewer_name,
                    file_count: p.file_count,
                })
                .collect(),
            events: events.iter().map(|e| present_event(e, &authority)).collect(),
        });
    }

    #[cfg(not(feature = "ssr"))]
    {
        let _ = track;
        Err(ServerFnError::new("ssr only"))
    }
}

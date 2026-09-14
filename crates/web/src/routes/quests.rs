//! Quest routes.
//!
//! - `GET  /api/quests`           — open quests with the caller's progress.
//! - `GET  /api/quests/completed` — the caller's finished quests.
//! - `POST /api/quests`           — open a quest (Bureau).
//!
//! Progress is advanced by the XP engine as events arrive; nothing here
//! mutates a counter directly.

use axum::{
    extract::State,
    routing::get,
    Json, Router,
};
use gamecloud_shared::roles::Action;
use serde::Serialize;
use uuid::Uuid;

use crate::{
    db::queries::{audit, quests, users},
    error::{WebError, WebResult},
    middleware::auth::CurrentUser,
    state::AppState,
};

/// Build the quests router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(active).post(create))
        .route("/completed", get(completed))
}

async fn active(
    State(state): State<AppState>,
    user: CurrentUser,
) -> WebResult<Json<Vec<quests::QuestView>>> {
    Ok(Json(quests::active_for_user(state.pool(), user.id).await?))
}

async fn completed(
    State(state): State<AppState>,
    user: CurrentUser,
) -> WebResult<Json<Vec<quests::QuestView>>> {
    Ok(Json(
        quests::completed_by_user(state.pool(), user.id).await?,
    ))
}

#[derive(Serialize)]
struct CreatedResponse {
    id: Uuid,
}

/// Open a quest.
///
/// Reuses `GrantManualXp` as the gate: a quest is a promise of XP, so
/// whoever may hand out XP directly may also promise it. That keeps the
/// permission surface small rather than inventing a near-duplicate
/// action.
async fn create(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(body): Json<quests::NewQuest>,
) -> WebResult<Json<CreatedResponse>> {
    let authority = users::load_authority(state.pool(), user.id).await?;
    if !authority.can(Action::GrantManualXp) {
        return Err(WebError::Forbidden);
    }

    let id = quests::create(state.pool(), user.id, &body).await?;

    audit::record(
        state.pool(),
        Some(user.id),
        "quest.created",
        Some("quest"),
        Some(id),
        serde_json::json!({
            "title": body.title,
            "xp_reward": body.xp_reward,
            "condition": body.condition_type,
            "target": body.condition_value,
        }),
    )
    .await?;

    Ok(Json(CreatedResponse { id }))
}

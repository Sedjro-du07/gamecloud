//! Public season routes.
//!
//! - `GET /api/seasons/current` — the season the board is scoped to.
//! - `GET /api/seasons`         — every season, for the archive view.
//!
//! Creating a season is a Bureau action and lives in
//! [`crate::routes::admin`].

use axum::{extract::State, routing::get, Json, Router};

use crate::{
    db::queries::seasons,
    error::WebResult,
    state::AppState,
};

/// Build the seasons router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list))
        .route("/current", get(current))
}

async fn list(State(state): State<AppState>) -> WebResult<Json<Vec<seasons::Season>>> {
    Ok(Json(seasons::list(state.pool()).await?))
}

/// The open season, or `null` between seasons.
///
/// The frontend uses `null` to fall back to the all-time board rather
/// than rendering an empty table.
async fn current(State(state): State<AppState>) -> WebResult<Json<Option<seasons::Season>>> {
    Ok(Json(seasons::current(state.pool()).await?))
}

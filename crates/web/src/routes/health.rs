//! Liveness and readiness probes.

use axum::{extract::State, http::StatusCode, routing::get, Json, Router};
use serde::Serialize;

use crate::state::AppState;

/// Build the health router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/healthz", get(liveness))
        .route("/readyz", get(readiness))
}

/// Always 200 if the process can run a handler. Used for liveness.
async fn liveness() -> &'static str {
    "ok"
}

#[derive(Serialize)]
struct Readiness {
    db: bool,
}

/// Verifies the database is reachable. Used for readiness probes.
async fn readiness(State(state): State<AppState>) -> (StatusCode, Json<Readiness>) {
    let db_ok = sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(state.pool())
        .await
        .is_ok();
    let status = if db_ok {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (status, Json(Readiness { db: db_ok }))
}

//! QR code routes.
//!
//! - `POST /api/qr/generate` — Bureau / event managers create a QR
//!   token and receive `{ token, png_base64 }`.
//! - `POST /api/qr/scan`     — verified members submit a token to
//!   record attendance.

use axum::{extract::State, routing::post, Json, Router};
use base64::Engine;
use chrono::{Duration, Utc};
use gamecloud_shared::roles::Action;
use qrcode::{render::svg, QrCode};
use serde::{Deserialize, Serialize};

use crate::{
    db::queries::{qr as qr_q, users},
    error::{WebError, WebResult},
    middleware::auth::CurrentUser,
    services::jwt,
    state::AppState,
};

/// Build the QR router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/generate", post(generate))
        .route("/scan", post(scan))
}

#[derive(Deserialize)]
struct GenerateBody {
    event_name: String,
    event_type: String,
    xp_value: i32,
    /// TTL in seconds; capped server-side at 24h.
    ttl_seconds: Option<u64>,
}

#[derive(Serialize)]
struct GenerateResponse {
    token: String,
    qr_svg: String,
    expires_at: chrono::DateTime<Utc>,
}

async fn generate(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(body): Json<GenerateBody>,
) -> WebResult<Json<GenerateResponse>> {
    let authority = users::load_authority(state.pool(), user.id).await?;
    if !authority.can(Action::GenerateQrToken) {
        return Err(WebError::Forbidden);
    }

    if !matches!(
        body.event_type.as_str(),
        "Session" | "OfficeHours" | "StandUp" | "GameJam" | "Special"
    ) {
        return Err(WebError::Validation("invalid event_type".into()));
    }
    if body.xp_value < 0 {
        return Err(WebError::Validation("xp_value must be >= 0".into()));
    }

    let ttl = body
        .ttl_seconds
        .unwrap_or(state.config().jwt_qr_ttl_seconds)
        .min(86_400);

    // We sign the JWT with a placeholder qr_id, then INSERT, then sign
    // the *real* one. Two hops are awkward; instead, we pre-generate
    // the row UUID outside.
    let qr_id = uuid::Uuid::new_v4();
    let (token, exp) = jwt::issue_qr(
        &state.config().jwt_secret,
        qr_id,
        &body.event_name,
        &body.event_type,
        body.xp_value,
        ttl,
    )?;

    qr_q::insert_qr_token(
        state.pool(),
        &token,
        &body.event_name,
        &body.event_type,
        body.xp_value,
        user.id,
        exp,
    )
    .await?;

    let svg = QrCode::new(&token)
        .map_err(|e| WebError::Internal(anyhow::anyhow!("qr encode: {e}")))?
        .render::<svg::Color>()
        .min_dimensions(256, 256)
        .build();

    let _ = base64::engine::general_purpose::STANDARD.encode([0u8; 0]); // keep import alive

    Ok(Json(GenerateResponse {
        token,
        qr_svg: svg,
        expires_at: exp,
    }))
}

#[derive(Deserialize)]
struct ScanBody {
    token: String,
}

#[derive(Serialize)]
struct ScanResponse {
    event_name: String,
    event_type: String,
    xp_awarded: i32,
}

async fn scan(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(body): Json<ScanBody>,
) -> WebResult<Json<ScanResponse>> {
    if !user.record.email_verified {
        return Err(WebError::Domain(
            gamecloud_shared::DomainError::EmailNotVerified,
        ));
    }

    // Validate JWT signature/expiry first; then claim the row in DB
    // (which also covers replay).
    let _claims = jwt::verify_qr(&state.config().jwt_secret, &body.token)?;
    let claimed =
        qr_q::claim_qr_token(state.pool(), &body.token, user.id).await?;

    Ok(Json(ScanResponse {
        event_name: claimed.event_name,
        event_type: claimed.event_type,
        xp_awarded: claimed.xp_value,
    }))
}

// Anchor a Duration import we may need for future cap logic.
#[allow(dead_code)]
const _: Duration = Duration::seconds(0);

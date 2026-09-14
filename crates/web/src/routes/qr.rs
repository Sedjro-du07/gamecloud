//! QR attendance routes.
//!
//! - `POST /api/qr/generate`  — Bureau / event managers mint a token.
//! - `POST /api/qr/scan`      — verified members claim it.
//! - `GET  /api/qr/:id/sheet` — who has scanned a given token.
//! - `GET  /api/qr/history`   — the caller's own attendance history.
//!
//! A token is valid for every member until it expires (or hits an
//! optional capacity), and each member may claim it once. Before the
//! audit a single `is_used` flag was flipped by the first scan, so one
//! person got the XP for a session and everybody else got a 410.

use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use gamecloud_shared::roles::Action;
use qrcode::{render::svg, QrCode};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    db::queries::{qr as qr_q, users},
    error::{WebError, WebResult},
    middleware::auth::CurrentUser,
    services::jwt,
    state::AppState,
};

/// Percent-encode a token for use in a query string.
///
/// A JWT is base64url, so only `=` padding and the separators need
/// escaping in practice — but encoding everything outside the unreserved
/// set keeps this correct whatever the token format becomes.
pub(crate) fn urlencoding(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for b in input.bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(b as char);
            }
            _ => {
                use std::fmt::Write;
                let _ = write!(&mut out, "%{b:02X}");
            }
        }
    }
    out
}

/// Build the QR router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/generate", post(generate))
        .route("/scan", post(scan))
        .route("/history", get(history))
        .route("/{id}/sheet", get(sheet))
}

// ---------------------------------------------------------------------------
// Generate
// ---------------------------------------------------------------------------

/// Longest life a QR token may be given, in seconds.
const MAX_TTL_SECONDS: u64 = 86_400;

#[derive(Deserialize)]
struct GenerateBody {
    event_name: String,
    event_type: String,
    xp_value: i32,
    /// TTL in seconds; capped server-side at 24h.
    ttl_seconds: Option<u64>,
    /// Optional ceiling on how many members may claim this token.
    /// `None` means "everyone who is in the room before it expires",
    /// which is the right default for a code on a projector.
    max_scans: Option<i32>,
    /// Calendar event this code admits to. When set, the attendance it
    /// produces is answerable to the calendar rather than to a loose
    /// string, and a member cannot be paid twice for one session by
    /// scanning a reprinted code.
    event_id: Option<Uuid>,
}

#[derive(Serialize)]
struct GenerateResponse {
    /// The signed token. Needed to scan by hand.
    token: String,
    /// The link the QR encodes. Open it on a phone and the scan runs.
    scan_url: String,
    /// Inline SVG rendering, ready to drop into a page.
    qr_svg: String,
    /// Internal id, for the attendance sheet endpoint.
    token_id: Uuid,
    expires_at: chrono::DateTime<chrono::Utc>,
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
    if let Some(max) = body.max_scans {
        if max <= 0 {
            return Err(WebError::Validation("max_scans must be positive".into()));
        }
    }

    let ttl = body
        .ttl_seconds
        .unwrap_or(state.config().jwt_qr_ttl_seconds)
        .min(MAX_TTL_SECONDS);

    // The row id is generated up front so the JWT can carry it and the
    // database row can be keyed by it without a second signing pass.
    let qr_id = Uuid::new_v4();
    let (token, exp) = jwt::issue_qr(
        &state.config().jwt_secret,
        qr_id,
        &body.event_name,
        &body.event_type,
        body.xp_value,
        ttl,
    )?;

    // Only the hash is persisted; the plaintext leaves in this response
    // and is never written down.
    let token_id = qr_q::insert_qr_token(
        state.pool(),
        &qr_q::NewQrToken {
            token: &token,
            event_name: &body.event_name,
            event_type: &body.event_type,
            xp_value: body.xp_value,
            created_by: user.id,
            expires_at: exp,
            max_scans: body.max_scans,
            event_id: body.event_id,
        },
    )
    .await?;

    // The QR encodes a link to the scan page rather than the raw token.
    // Every phone camera opens a link; almost none can be asked to hand
    // a decoded string to a web page. This turns "scan the code" into
    // something that works on every device with no permissions prompt
    // and no BarcodeDetector, which Firefox and Safari lack anyway.
    let scan_url = format!(
        "{}/scan?token={}",
        state.config().public_origin.trim_end_matches('/'),
        urlencoding(&token)
    );

    let qr_svg = QrCode::new(&scan_url)
        .map_err(|e| WebError::Internal(anyhow::anyhow!("qr encode: {e}")))?
        .render::<svg::Color>()
        .min_dimensions(256, 256)
        .build();

    Ok(Json(GenerateResponse {
        token,
        scan_url,
        qr_svg,
        token_id,
        expires_at: exp,
    }))
}

// ---------------------------------------------------------------------------
// Scan
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct ScanBody {
    token: String,
}

#[derive(Serialize)]
struct ScanResponse {
    event_name: String,
    event_type: String,
    /// XP actually credited, after the member's multipliers.
    xp_awarded: i32,
    /// How many members have claimed this token so far.
    scan_count: i32,
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

    // Signature and expiry first — a forged token never reaches the
    // database. The per-user claim check then covers replay.
    let _claims = jwt::verify_qr(&state.config().jwt_secret, &body.token)?;
    let claimed = qr_q::claim_qr_token(
        state.pool(),
        state.channels(),
        &body.token,
        user.id,
    )
    .await?;

    Ok(Json(ScanResponse {
        event_name: claimed.event_name,
        event_type: claimed.event_type,
        xp_awarded: claimed.xp_awarded,
        scan_count: claimed.scan_count,
    }))
}

// ---------------------------------------------------------------------------
// Attendance views
// ---------------------------------------------------------------------------

async fn sheet(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
) -> WebResult<Json<Vec<qr_q::AttendanceRow>>> {
    let authority = users::load_authority(state.pool(), user.id).await?;
    if !authority.can(Action::GenerateQrToken) {
        return Err(WebError::Forbidden);
    }
    Ok(Json(qr_q::attendance_for_token(state.pool(), id).await?))
}

async fn history(
    State(state): State<AppState>,
    user: CurrentUser,
) -> WebResult<Json<Vec<qr_q::AttendanceRow>>> {
    Ok(Json(qr_q::history_for_user(state.pool(), user.id).await?))
}

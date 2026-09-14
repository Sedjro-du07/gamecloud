//! JWT issuance and verification.
//!
//! Two distinct kinds of token are issued from this module:
//!
//! - **Access tokens** carry the user identity for HTTP requests.
//!   `aud = "access"`, short-lived (1h).
//! - **QR tokens** are embedded in attendance QR codes. `aud = "qr"`,
//!   carry the event details, lifetime governed by the event.
//!
//! Refresh tokens are *not* JWTs; they are random strings whose hash
//! is stored in `refresh_tokens`. See `services::tokens`.

use chrono::{DateTime, Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{WebError, WebResult};

/// Audience claim for access tokens.
pub const AUD_ACCESS: &str = "access";
/// Audience claim for QR tokens.
pub const AUD_QR: &str = "qr";

/// Access token claim set.
#[derive(Debug, Serialize, Deserialize)]
pub struct AccessClaims {
    /// Subject — the user UUID.
    pub sub: Uuid,
    /// Audience.
    pub aud: String,
    /// Expiration (Unix seconds).
    pub exp: i64,
    /// Issued-at (Unix seconds).
    pub iat: i64,
    /// JWT ID — random per-token, lets us revoke individual tokens
    /// later if needed.
    pub jti: Uuid,
}

/// QR token claim set.
#[derive(Debug, Serialize, Deserialize)]
pub struct QrClaims {
    /// Internal QR-token row ID.
    pub sub: Uuid,
    /// Audience.
    pub aud: String,
    /// Expiration.
    pub exp: i64,
    /// Issued-at.
    pub iat: i64,
    /// Event display name (echoed back to the scanner UI).
    pub event_name: String,
    /// Event type (`Session`, `OfficeHours`, …).
    pub event_type: String,
    /// XP awarded by this scan.
    pub xp_value: i32,
}

fn now() -> i64 {
    Utc::now().timestamp()
}

/// Issue an access token. Returns the encoded string and its absolute
/// expiration timestamp.
///
/// # Errors
/// Returns `Internal` on encoding failure.
pub fn issue_access(
    secret: &[u8],
    user_id: Uuid,
    ttl_seconds: u64,
) -> WebResult<(String, DateTime<Utc>)> {
    let exp_dt = Utc::now() + Duration::seconds(ttl_seconds.try_into().unwrap_or(3600));
    let claims = AccessClaims {
        sub: user_id,
        aud: AUD_ACCESS.to_string(),
        exp: exp_dt.timestamp(),
        iat: now(),
        jti: Uuid::new_v4(),
    };
    let token = encode(&Header::default(), &claims, &EncodingKey::from_secret(secret))
        .map_err(|e| WebError::Internal(anyhow::anyhow!("jwt encode: {e}")))?;
    Ok((token, exp_dt))
}

/// Verify an access token. Returns the embedded user ID.
///
/// # Errors
/// `Unauthorized` for any signature, audience, or expiration failure.
pub fn verify_access(secret: &[u8], token: &str) -> WebResult<Uuid> {
    let mut validation = Validation::default();
    validation.set_audience(&[AUD_ACCESS]);
    let data = decode::<AccessClaims>(token, &DecodingKey::from_secret(secret), &validation)
        .map_err(|_| WebError::Unauthorized)?;
    Ok(data.claims.sub)
}

/// Issue a QR token.
///
/// # Errors
/// Returns `Internal` on encoding failure.
pub fn issue_qr(
    secret: &[u8],
    qr_id: Uuid,
    event_name: &str,
    event_type: &str,
    xp_value: i32,
    ttl_seconds: u64,
) -> WebResult<(String, DateTime<Utc>)> {
    let exp_dt = Utc::now() + Duration::seconds(ttl_seconds.try_into().unwrap_or(3600));
    let claims = QrClaims {
        sub: qr_id,
        aud: AUD_QR.to_string(),
        exp: exp_dt.timestamp(),
        iat: now(),
        event_name: event_name.to_string(),
        event_type: event_type.to_string(),
        xp_value,
    };
    let token = encode(&Header::default(), &claims, &EncodingKey::from_secret(secret))
        .map_err(|e| WebError::Internal(anyhow::anyhow!("jwt encode: {e}")))?;
    Ok((token, exp_dt))
}

/// Verify a QR token. Returns the parsed claims.
///
/// # Errors
/// `Domain(InvalidQrToken)` for any signature, audience, or expiration
/// failure.
pub fn verify_qr(secret: &[u8], token: &str) -> WebResult<QrClaims> {
    let mut validation = Validation::default();
    validation.set_audience(&[AUD_QR]);
    let data = decode::<QrClaims>(token, &DecodingKey::from_secret(secret), &validation)
        .map_err(|_| {
            WebError::Domain(gamecloud_shared::DomainError::InvalidQrToken)
        })?;
    Ok(data.claims)
}

//! Argon2 password / OTP hashing.

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};

use crate::error::{WebError, WebResult};

/// Hash a secret with Argon2id. Used for OTP codes.
///
/// # Errors
/// Returns `Internal` if the hashing back-end fails.
pub fn hash(secret: &str) -> WebResult<String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(secret.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| WebError::Internal(anyhow::anyhow!("argon2 hash: {e}")))
}

/// Constant-time verification.
///
/// # Errors
/// Returns `Internal` if the stored hash is malformed. Returns
/// `Ok(false)` for an honest mismatch.
pub fn verify(stored_hash: &str, candidate: &str) -> WebResult<bool> {
    let parsed = PasswordHash::new(stored_hash)
        .map_err(|e| WebError::Internal(anyhow::anyhow!("argon2 parse: {e}")))?;
    let ok = Argon2::default()
        .verify_password(candidate.as_bytes(), &parsed)
        .is_ok();
    Ok(ok)
}

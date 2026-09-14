//! Opaque refresh-token generation and hashing.
//!
//! Refresh tokens are 32 random bytes encoded as URL-safe base64. We
//! never persist the plaintext: the database only sees the SHA-256
//! hash. On each refresh request we hash the cookie value and look it
//! up.

use base64::Engine;
use rand::RngCore;
use sha2::{Digest, Sha256};

/// Generate a fresh refresh-token plaintext (~43 character base64 string).
#[must_use]
pub fn generate() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

/// SHA-256 hash a token, returning lowercase hex.
///
/// `token_hash` is what we put in the `refresh_tokens.token_hash`
/// column.
#[must_use]
pub fn hash(plaintext: &str) -> String {
    let digest = Sha256::digest(plaintext.as_bytes());
    hex_encode(&digest)
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write;
        let _ = write!(&mut s, "{b:02x}");
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_tokens_are_unique() {
        let a = generate();
        let b = generate();
        assert_ne!(a, b);
        assert!(a.len() >= 40);
    }

    #[test]
    fn hash_is_deterministic_and_64_chars() {
        let h1 = hash("hello");
        let h2 = hash("hello");
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64);
        assert!(h1.chars().all(|c| c.is_ascii_hexdigit()));
    }
}

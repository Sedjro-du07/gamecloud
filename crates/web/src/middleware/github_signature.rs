//! GitHub webhook HMAC-SHA256 signature verification.
//!
//! GitHub sends an `X-Hub-Signature-256` header of the form
//! `sha256=<hex>`. We recompute the HMAC over the raw body using our
//! shared secret and compare in constant time.
//!
//! Verification happens in the route handler rather than as a Tower
//! layer because we need the raw body bytes after extraction.

use hmac::{Hmac, Mac};
use sha2::Sha256;

use crate::error::{WebError, WebResult};

type HmacSha256 = Hmac<Sha256>;

/// Verify a `sha256=…` signature against the raw body and the secret.
///
/// # Errors
/// `Unauthorized` for any malformed header, length mismatch, or
/// signature mismatch.
pub fn verify(secret: &[u8], body: &[u8], header_value: Option<&str>) -> WebResult<()> {
    let header = header_value.ok_or(WebError::Unauthorized)?;
    let expected = header.strip_prefix("sha256=").ok_or(WebError::Unauthorized)?;
    let expected_bytes = hex_decode(expected).ok_or(WebError::Unauthorized)?;

    let mut mac = HmacSha256::new_from_slice(secret)
        .map_err(|e| WebError::Internal(anyhow::anyhow!("hmac key: {e}")))?;
    mac.update(body);
    mac.verify_slice(&expected_bytes)
        .map_err(|_| WebError::Unauthorized)?;
    Ok(())
}

fn hex_decode(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 {
        return None;
    }
    let mut out = Vec::with_capacity(s.len() / 2);
    let bytes = s.as_bytes();
    for pair in bytes.chunks_exact(2) {
        let hi = hex_nibble(pair[0])?;
        let lo = hex_nibble(pair[1])?;
        out.push((hi << 4) | lo);
    }
    Some(out)
}

fn hex_nibble(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(10 + b - b'a'),
        b'A'..=b'F' => Some(10 + b - b'A'),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_signature_matches() {
        // Computed via: echo -n "hello" | openssl dgst -sha256 -hmac "secret"
        let secret = b"secret";
        let body = b"hello";
        let sig = "sha256=88aab3ede8d3adf94d26ab90d3bafd4a2083070c3bcce9c014ee04a443847c0b";
        assert!(verify(secret, body, Some(sig)).is_ok());
    }

    #[test]
    fn rejects_tampered_body() {
        let secret = b"secret";
        let sig = "sha256=88aab3ede8d3adf94d26ab90d3bafd4a2083070c3bcce9c014ee04a443847c0b";
        assert!(verify(secret, b"goodbye", Some(sig)).is_err());
    }

    #[test]
    fn rejects_missing_header() {
        assert!(verify(b"secret", b"hi", None).is_err());
    }

    #[test]
    fn rejects_wrong_prefix() {
        assert!(verify(b"secret", b"hi", Some("md5=deadbeef")).is_err());
    }
}

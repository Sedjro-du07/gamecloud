//! Streaming a multipart file field to disk.
//!
//! Shared by project builds and member shares. Streaming rather than
//! buffering is the point: a 500 MB upload held in memory would be a
//! denial of service against our own server. The SHA-256 is computed on
//! the way past, which costs nothing and lets a download be checked
//! against the record later.

use std::path::Path;

use crate::error::{WebError, WebResult};

/// Largest file the platform accepts, builds and shares alike.
pub const MAX_UPLOAD_BYTES: u64 = 500 * 1024 * 1024;

/// What was written.
#[derive(Debug, Clone)]
pub struct Written {
    /// Bytes written.
    pub size: u64,
    /// Lowercase hex SHA-256.
    pub checksum: String,
}

/// Write one multipart field to `path`, refusing anything over
/// [`MAX_UPLOAD_BYTES`] mid-stream.
///
/// On any failure the partial file is removed, so a refused upload
/// leaves nothing behind.
///
/// # Errors
/// `Validation` when the upload is too large or interrupted; `Internal`
/// when the disk refuses.
pub async fn stream_to_file(
    field: &mut axum::extract::multipart::Field<'_>,
    path: &Path,
) -> WebResult<Written> {
    let result = write(field, path).await;
    if result.is_err() {
        let _ = tokio::fs::remove_file(path).await;
    }
    result
}

async fn write(
    field: &mut axum::extract::multipart::Field<'_>,
    path: &Path,
) -> WebResult<Written> {
    use sha2::{Digest, Sha256};
    use tokio::io::AsyncWriteExt;

    let mut sink = tokio::fs::File::create(path)
        .await
        .map_err(|e| WebError::Internal(anyhow::anyhow!("upload file: {e}")))?;

    let mut hasher = Sha256::new();
    let mut size: u64 = 0;
    while let Some(chunk) = field
        .chunk()
        .await
        .map_err(|e| WebError::Validation(format!("lecture interrompue : {e}")))?
    {
        size += chunk.len() as u64;
        if size > MAX_UPLOAD_BYTES {
            return Err(WebError::Validation(
                "fichier trop volumineux (500 Mo maximum)".into(),
            ));
        }
        hasher.update(&chunk);
        sink.write_all(&chunk)
            .await
            .map_err(|e| WebError::Internal(anyhow::anyhow!("upload write: {e}")))?;
    }
    sink.flush()
        .await
        .map_err(|e| WebError::Internal(anyhow::anyhow!("upload flush: {e}")))?;

    let digest = hasher.finalize();
    let mut checksum = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write;
        let _ = write!(&mut checksum, "{byte:02x}");
    }
    Ok(Written { size, checksum })
}

/// A filename safe to store and to put in a `Content-Disposition` header.
///
/// Browsers send a bare name, but nothing stops a hand-made request from
/// sending `../../etc/passwd` or a name with a quote that breaks out of
/// the header. Keeps the last path segment and drops control characters,
/// quotes and backslashes.
#[must_use]
pub fn clean_filename(raw: &str) -> String {
    let base = raw.rsplit(['/', '\\']).next().unwrap_or("");
    let cleaned: String = base
        .chars()
        .filter(|c| !c.is_control() && !matches!(c, '"' | '\\'))
        .collect();
    let trimmed = cleaned.trim().trim_start_matches('.').trim();
    if trimmed.is_empty() {
        "fichier".to_string()
    } else {
        trimmed.chars().take(200).collect()
    }
}

/// A `Content-Disposition: attachment` value for `filename`.
///
/// Header values must be plain ASCII, and a name like `forêt.zip` is
/// not, so the name goes twice: an ASCII fallback in `filename`, and the
/// real one percent-encoded in `filename*` (RFC 6266), which every
/// current browser prefers.
#[must_use]
pub fn attachment_header(filename: &str) -> String {
    use std::fmt::Write;

    let clean = clean_filename(filename);
    let fallback: String = clean
        .chars()
        .map(|c| if c.is_ascii() && !c.is_ascii_control() { c } else { '_' })
        .collect();
    let mut encoded = String::with_capacity(clean.len() * 3);
    for byte in clean.bytes() {
        if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
            encoded.push(char::from(byte));
        } else {
            let _ = write!(&mut encoded, "%{byte:02X}");
        }
    }
    format!("attachment; filename=\"{fallback}\"; filename*=UTF-8''{encoded}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_an_ordinary_name() {
        assert_eq!(clean_filename("Aevaryn v1.2.zip"), "Aevaryn v1.2.zip");
    }

    #[test]
    fn drops_any_directory_part() {
        assert_eq!(clean_filename("../../etc/passwd"), "passwd");
        assert_eq!(clean_filename("C:\\Users\\me\\lore.pdf"), "lore.pdf");
    }

    #[test]
    fn cannot_break_out_of_the_header() {
        assert_eq!(clean_filename("a\"; x=\"y.zip"), "a; x=y.zip");
        assert_eq!(clean_filename("a\r\nSet-Cookie: x.zip"), "aSet-Cookie: x.zip");
    }

    #[test]
    fn a_non_ascii_name_still_makes_a_valid_header() {
        let value = attachment_header("forêt 2.zip");
        assert!(value.is_ascii());
        assert!(value.contains("filename=\"for_t 2.zip\""));
        assert!(value.contains("filename*=UTF-8''for%C3%AAt%202.zip"));
        assert!(axum::http::HeaderValue::from_str(&value).is_ok());
    }

    #[test]
    fn never_returns_an_empty_or_hidden_name() {
        assert_eq!(clean_filename(""), "fichier");
        assert_eq!(clean_filename("../"), "fichier");
        assert_eq!(clean_filename(".htaccess"), "htaccess");
    }
}

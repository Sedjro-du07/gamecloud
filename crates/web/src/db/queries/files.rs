//! Project build files.
//!
//! The bytes live in a GitHub release beside the code; this table is the
//! index. `storage_path` holds the release asset id rather than a URL,
//! because a private repository's asset is not fetchable by link — the
//! platform proxies the download and therefore needs the id, not an
//! address that would only work for somebody holding the org token.

use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{WebError, WebResult};

/// Largest build the platform will accept.
///
/// Matches the `project_files_size_max` CHECK. Enforced before the
/// upload starts so an oversized file is refused after a few kilobytes
/// rather than after half a gigabyte.
pub const MAX_FILE_BYTES: u64 = 500 * 1024 * 1024;

/// A build attached to a project.
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct ProjectFile {
    /// Row id.
    pub id: Uuid,
    /// Project it belongs to.
    pub project_id: Uuid,
    /// Original filename.
    pub filename: String,
    /// `Executable`, `Source`, `Asset`, `Doc`, `Audio` or `Video`.
    pub file_type: String,
    /// Size in bytes.
    pub size_bytes: i64,
    /// SHA-256, so a download can be checked against the record.
    pub checksum_sha256: String,
    /// GitHub release asset id.
    pub storage_path: String,
    /// Release tag this build belongs to.
    pub version: String,
    /// What changed in this build.
    pub changelog: Option<String>,
    /// Who uploaded it.
    pub uploaded_by: Uuid,
    /// When.
    pub uploaded_at: chrono::DateTime<chrono::Utc>,
}

/// Guess a file type from its extension.
///
/// The member is not asked: they know what they built, not what the
/// platform's vocabulary calls it. A wrong guess is cosmetic.
#[must_use]
pub fn classify(filename: &str) -> &'static str {
    let lower = filename.to_ascii_lowercase();
    let ext = lower.rsplit('.').next().unwrap_or("");
    match ext {
        "exe" | "app" | "apk" | "dmg" | "appimage" | "x86_64" => "Executable",
        "rs" | "c" | "cpp" | "cs" | "gd" | "py" | "js" | "ts" => "Source",
        "pdf" | "md" | "txt" | "doc" | "docx" | "odt" => "Doc",
        "wav" | "mp3" | "ogg" | "flac" => "Audio",
        "mp4" | "mov" | "webm" | "avi" | "mkv" => "Video",
        // Archives (zip, tar, love, pck…) and anything unrecognised: an
        // odd extension is almost always a packaged build, and the
        // schema's CHECK accepts only these six values, so the fallback
        // has to be one of them rather than a guess that fails to insert.
        _ => "Asset",
    }
}

/// A build about to be recorded.
///
/// Grouped rather than passed as nine positional arguments, so a caller
/// cannot silently swap the checksum and the version.
#[derive(Debug, Clone)]
pub struct NewFile<'a> {
    /// Project it belongs to.
    pub project_id: Uuid,
    /// Who uploaded it.
    pub uploaded_by: Uuid,
    /// Original filename.
    pub filename: &'a str,
    /// Size in bytes.
    pub size_bytes: i64,
    /// SHA-256 computed during the upload.
    pub checksum: &'a str,
    /// GitHub release asset id.
    pub asset_id: u64,
    /// Release tag.
    pub version: &'a str,
    /// What changed.
    pub changelog: Option<&'a str>,
}

/// Record an uploaded build.
///
/// # Errors
/// Propagates database errors.
pub async fn record(pool: &PgPool, new: &NewFile<'_>) -> WebResult<Uuid> {
    let id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO project_files (project_id, filename, file_type, size_bytes,
                                   checksum_sha256, storage_path, version,
                                   changelog, uploaded_by)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        RETURNING id
        "#,
    )
    .bind(new.project_id)
    .bind(new.filename)
    .bind(classify(new.filename))
    .bind(new.size_bytes)
    .bind(new.checksum)
    .bind(new.asset_id.to_string())
    .bind(new.version)
    .bind(new.changelog)
    .bind(new.uploaded_by)
    .fetch_one(pool)
    .await?;
    Ok(id)
}

/// Builds attached to a project, newest first.
///
/// # Errors
/// Propagates database errors.
pub async fn list(pool: &PgPool, project_id: Uuid) -> WebResult<Vec<ProjectFile>> {
    let rows = sqlx::query_as::<_, ProjectFile>(
        r#"
        SELECT id, project_id, filename, file_type, size_bytes, checksum_sha256,
               storage_path, version, changelog, uploaded_by, uploaded_at
          FROM project_files
         WHERE project_id = $1
         ORDER BY uploaded_at DESC
        "#,
    )
    .bind(project_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// One build, by id.
///
/// # Errors
/// `NotFound` when the id does not exist.
pub async fn find(pool: &PgPool, id: Uuid) -> WebResult<ProjectFile> {
    sqlx::query_as::<_, ProjectFile>(
        r#"
        SELECT id, project_id, filename, file_type, size_bytes, checksum_sha256,
               storage_path, version, changelog, uploaded_by, uploaded_at
          FROM project_files
         WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or(WebError::NotFound)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_the_obvious_build_artefacts() {
        assert_eq!(classify("Aevaryn.exe"), "Executable");
        assert_eq!(classify("game.apk"), "Executable");
        assert_eq!(classify("Aevaryn-linux.AppImage"), "Executable");
        assert_eq!(classify("build.zip"), "Asset");
        assert_eq!(classify("design.pdf"), "Doc");
        assert_eq!(classify("theme.ogg"), "Audio");
        assert_eq!(classify("trailer.mp4"), "Video");
        assert_eq!(classify("player.gd"), "Source");
    }

    #[test]
    fn classification_ignores_case() {
        assert_eq!(classify("AEVARYN.EXE"), "Executable");
        assert_eq!(classify("Trailer.MP4"), "Video");
    }

    #[test]
    fn an_unknown_extension_falls_back_to_asset() {
        // Most uploads with an odd extension are archived builds, and
        // the CHECK constraint only accepts the six known types — so the
        // fallback has to be one of them, never a guess that fails.
        for name in ["build.unknown", "noextension", "weird.xyz", ""] {
            assert_eq!(classify(name), "Asset", "{name}");
        }
    }

    #[test]
    fn every_classification_is_a_value_the_schema_accepts() {
        const ALLOWED: [&str; 6] = ["Executable", "Source", "Asset", "Doc", "Audio", "Video"];
        for name in [
            "a.exe", "a.zip", "a.rs", "a.pdf", "a.wav", "a.mp4", "a.???", "",
        ] {
            assert!(ALLOWED.contains(&classify(name)), "{name}");
        }
    }
}

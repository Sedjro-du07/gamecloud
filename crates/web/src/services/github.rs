//! GitHub organisation integration.
//!
//! Creating a project on the platform creates its repository in the
//! association's GitHub organisation, adds the author as a collaborator,
//! and installs the push webhook.
//!
//! That last step is the one that matters most, and it is easy to
//! overlook. The XP economy already awards commits, merged pull
//! requests, reviews and closed issues — but only for repositories whose
//! webhook points at this server, and nobody was ever going to configure
//! that by hand, repository by repository. Creating the hook at the same
//! moment as the repository is what turns GitHub XP from a documented
//! feature into one that actually fires.
//!
//! ## Failure policy
//!
//! GitHub being unreachable must never stop a member creating a project.
//! Every call here returns its error to the caller, which logs it and
//! carries on with a repository-less project; the Bureau can attach a
//! repository by hand afterwards.
//!
//! ## Visibility
//!
//! Repositories are created **private** and made public when the project
//! is released. A draft is unfinished student work, and a repository can
//! always be opened later — code that leaked cannot be un-leaked.

use serde::Deserialize;

use crate::{
    config::Config,
    error::{WebError, WebResult},
};

const API: &str = "https://api.github.com";
const UA: &str = "GameCloudOS";

/// A configured GitHub client.
#[derive(Clone)]
pub struct GitHub {
    client: reqwest::Client,
    org: String,
    token: String,
}

/// What the platform records after creating a repository.
#[derive(Debug, Clone, Deserialize)]
pub struct Repository {
    /// Repository name inside the organisation.
    pub name: String,
    /// Browser URL, stored on the project.
    pub html_url: String,
    /// Whether the repository is private.
    #[serde(default)]
    pub private: bool,
}

impl GitHub {
    /// Build a client, or `None` when the integration is not configured.
    ///
    /// Both the organisation and a token are required: half a
    /// configuration is a misconfiguration, and silently doing nothing
    /// would be worse than saying so.
    #[must_use]
    pub fn from_config(config: &Config) -> Option<Self> {
        let (Some(org), Some(token)) = (config.github_org.clone(), config.github_token.clone())
        else {
            return None;
        };
        Some(Self {
            client: reqwest::Client::new(),
            org,
            token,
        })
    }

    fn request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        self.client
            .request(method, format!("{API}{path}"))
            .bearer_auth(&self.token)
            .header("User-Agent", UA)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
    }

    /// Create a repository in the organisation.
    ///
    /// The name is derived from the project title; a collision gets a
    /// numeric suffix rather than failing, because two teams naming
    /// their game the same thing is a normal event, not an error.
    ///
    /// # Errors
    /// `Upstream` when GitHub refuses or is unreachable.
    pub async fn create_repo(
        &self,
        project_name: &str,
        description: Option<&str>,
    ) -> WebResult<Repository> {
        let base = slugify(project_name);

        for attempt in 0..5 {
            let name = if attempt == 0 {
                base.clone()
            } else {
                format!("{base}-{}", attempt + 1)
            };

            let body = serde_json::json!({
                "name": name,
                "description": description.unwrap_or("Projet GameCloud OS"),
                "private": true,
                "has_issues": true,
                "has_wiki": false,
                "auto_init": true,
            });

            let response = self
                .request(reqwest::Method::POST, &format!("/orgs/{}/repos", self.org))
                .json(&body)
                .send()
                .await
                .map_err(|e| WebError::Upstream(format!("github create repo: {e}")))?;

            if response.status().is_success() {
                return response
                    .json::<Repository>()
                    .await
                    .map_err(|e| WebError::Upstream(format!("github decode repo: {e}")));
            }

            // 422 is GitHub's "name already exists"; anything else is a
            // real failure and retrying under a new name would not help.
            if response.status() != reqwest::StatusCode::UNPROCESSABLE_ENTITY {
                let status = response.status();
                let detail = response.text().await.unwrap_or_default();
                return Err(WebError::Upstream(format!(
                    "github create repo: {status} {}",
                    detail.chars().take(200).collect::<String>()
                )));
            }
        }

        Err(WebError::Upstream(format!(
            "github create repo: {base} and four suffixed variants are all taken"
        )))
    }

    /// Give a member push access to a repository.
    ///
    /// # Errors
    /// `Upstream` when GitHub refuses or is unreachable.
    pub async fn add_collaborator(&self, repo: &str, login: &str) -> WebResult<()> {
        let response = self
            .request(
                reqwest::Method::PUT,
                &format!("/repos/{}/{repo}/collaborators/{login}", self.org),
            )
            .json(&serde_json::json!({ "permission": "push" }))
            .send()
            .await
            .map_err(|e| WebError::Upstream(format!("github add collaborator: {e}")))?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(WebError::Upstream(format!(
                "github add collaborator: {}",
                response.status()
            )))
        }
    }

    /// Install the push webhook so the repository starts paying XP.
    ///
    /// # Errors
    /// `Upstream` when GitHub refuses or is unreachable.
    pub async fn add_webhook(&self, repo: &str, callback: &str, secret: &str) -> WebResult<()> {
        let body = serde_json::json!({
            "name": "web",
            "active": true,
            "events": ["push", "pull_request", "pull_request_review", "issues"],
            "config": {
                "url": callback,
                "content_type": "json",
                "secret": secret,
                "insecure_ssl": "0",
            },
        });

        let response = self
            .request(
                reqwest::Method::POST,
                &format!("/repos/{}/{repo}/hooks", self.org),
            )
            .json(&body)
            .send()
            .await
            .map_err(|e| WebError::Upstream(format!("github add webhook: {e}")))?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(WebError::Upstream(format!(
                "github add webhook: {}",
                response.status()
            )))
        }
    }

    /// Open a repository to the world. Called when a project is released.
    ///
    /// # Errors
    /// `Upstream` when GitHub refuses or is unreachable.
    pub async fn make_public(&self, repo: &str) -> WebResult<()> {
        let response = self
            .request(
                reqwest::Method::PATCH,
                &format!("/repos/{}/{repo}", self.org),
            )
            .json(&serde_json::json!({ "private": false }))
            .send()
            .await
            .map_err(|e| WebError::Upstream(format!("github make public: {e}")))?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(WebError::Upstream(format!(
                "github make public: {}",
                response.status()
            )))
        }
    }
}

/// A release asset, as GitHub reports it after upload.
#[derive(Debug, Clone, Deserialize)]
pub struct Asset {
    /// Asset id, used to download it back through the API.
    pub id: u64,
    /// Filename.
    pub name: String,
    /// Size in bytes.
    pub size: u64,
}

#[derive(Debug, Deserialize)]
struct Release {
    id: u64,
    upload_url: String,
}

impl GitHub {
    /// Find the release for a version tag, creating it when absent.
    ///
    /// Uploading a build is the common case and creating the release is
    /// the rare one, so we look first and only create on a miss.
    ///
    /// # Errors
    /// `Upstream` when GitHub refuses or is unreachable.
    async fn ensure_release(&self, repo: &str, version: &str) -> WebResult<Release> {
        let existing = self
            .request(
                reqwest::Method::GET,
                &format!("/repos/{}/{repo}/releases/tags/{version}", self.org),
            )
            .send()
            .await
            .map_err(|e| WebError::Upstream(format!("github find release: {e}")))?;

        if existing.status().is_success() {
            return existing
                .json::<Release>()
                .await
                .map_err(|e| WebError::Upstream(format!("github decode release: {e}")));
        }

        let response = self
            .request(
                reqwest::Method::POST,
                &format!("/repos/{}/{repo}/releases", self.org),
            )
            .json(&serde_json::json!({
                "tag_name": version,
                "name": version,
                "body": "Publié depuis GameCloud OS.",
                // A draft release would be invisible to the download
                // proxy, and a prerelease flag says something we do not
                // mean. Neither.
                "draft": false,
                "prerelease": false,
            }))
            .send()
            .await
            .map_err(|e| WebError::Upstream(format!("github create release: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let detail = response.text().await.unwrap_or_default();
            return Err(WebError::Upstream(format!(
                "github create release: {status} {}",
                detail.chars().take(200).collect::<String>()
            )));
        }

        response
            .json::<Release>()
            .await
            .map_err(|e| WebError::Upstream(format!("github decode release: {e}")))
    }

    /// Attach a build to a release.
    ///
    /// The body is a file rather than a buffer: a 500 MB build held in
    /// memory would be a denial of service against our own server, so
    /// the upload streams from disk with a known length.
    ///
    /// # Errors
    /// `Upstream` when GitHub refuses or is unreachable.
    pub async fn upload_asset(
        &self,
        repo: &str,
        version: &str,
        filename: &str,
        file: tokio::fs::File,
        size: u64,
    ) -> WebResult<Asset> {
        let release = self.ensure_release(repo, version).await?;

        // `upload_url` is a URI template: ".../assets{?name,label}".
        let base = release
            .upload_url
            .split('{')
            .next()
            .unwrap_or(&release.upload_url)
            .to_string();
        let url = format!("{base}?name={}", urlencode(filename));

        let stream = tokio_util::io::ReaderStream::new(file);
        let response = self
            .client
            .post(&url)
            .bearer_auth(&self.token)
            .header("User-Agent", UA)
            .header("Accept", "application/vnd.github+json")
            .header("Content-Type", "application/octet-stream")
            .header(reqwest::header::CONTENT_LENGTH, size)
            .body(reqwest::Body::wrap_stream(stream))
            .send()
            .await
            .map_err(|e| WebError::Upstream(format!("github upload asset: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let detail = response.text().await.unwrap_or_default();
            return Err(WebError::Upstream(format!(
                "github upload asset: {status} {}",
                detail.chars().take(200).collect::<String>()
            )));
        }

        let _ = release.id;
        response
            .json::<Asset>()
            .await
            .map_err(|e| WebError::Upstream(format!("github decode asset: {e}")))
    }

    /// Stream a release asset back.
    ///
    /// Assets of a private repository are not publicly fetchable, so the
    /// platform proxies the bytes rather than handing out a link. That
    /// is the better shape anyway: access control stays with the
    /// permission model instead of leaking to GitHub's.
    ///
    /// # Errors
    /// `Upstream` when GitHub refuses or is unreachable.
    pub async fn download_asset(&self, repo: &str, asset_id: u64) -> WebResult<reqwest::Response> {
        let response = self
            .client
            .get(format!(
                "{API}/repos/{}/{repo}/releases/assets/{asset_id}",
                self.org
            ))
            .bearer_auth(&self.token)
            .header("User-Agent", UA)
            // This header is what turns the metadata call into the bytes.
            .header("Accept", "application/octet-stream")
            .send()
            .await
            .map_err(|e| WebError::Upstream(format!("github download asset: {e}")))?;

        if response.status().is_success() {
            Ok(response)
        } else {
            Err(WebError::Upstream(format!(
                "github download asset: {}",
                response.status()
            )))
        }
    }
}

/// Percent-encode a filename for a query string.
fn urlencode(input: &str) -> String {
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

/// Turn a project title into a repository name.
///
/// GitHub accepts letters, digits, `.`, `-` and `_`. French titles bring
/// accents and apostrophes, so those are folded rather than dropped —
/// "L'Épopée du Héros" should read `l-epopee-du-heros`, not `l-p-e-du-h-ros`.
#[must_use]
pub fn slugify(title: &str) -> String {
    let mut out = String::with_capacity(title.len());
    let mut last_dash = true; // suppresses a leading dash

    for ch in title.trim().chars() {
        let folded = fold_accent(ch);
        for c in folded.chars() {
            if c.is_ascii_alphanumeric() {
                out.push(c.to_ascii_lowercase());
                last_dash = false;
            } else if !last_dash {
                out.push('-');
                last_dash = true;
            }
        }
    }

    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        out.push_str("projet");
    }
    out.truncate(90);
    while out.ends_with('-') {
        out.pop();
    }
    out
}

/// Map an accented Latin character onto its ASCII base.
fn fold_accent(ch: char) -> String {
    match ch {
        'à' | 'á' | 'â' | 'ä' | 'ã' | 'å' => "a".into(),
        'À' | 'Á' | 'Â' | 'Ä' | 'Ã' | 'Å' => "A".into(),
        'è' | 'é' | 'ê' | 'ë' => "e".into(),
        'È' | 'É' | 'Ê' | 'Ë' => "E".into(),
        'ì' | 'í' | 'î' | 'ï' => "i".into(),
        'Ì' | 'Í' | 'Î' | 'Ï' => "I".into(),
        'ò' | 'ó' | 'ô' | 'ö' | 'õ' => "o".into(),
        'Ò' | 'Ó' | 'Ô' | 'Ö' | 'Õ' => "O".into(),
        'ù' | 'ú' | 'û' | 'ü' => "u".into(),
        'Ù' | 'Ú' | 'Û' | 'Ü' => "U".into(),
        'ÿ' | 'ý' => "y".into(),
        'ç' => "c".into(),
        'Ç' => "C".into(),
        'ñ' => "n".into(),
        'Ñ' => "N".into(),
        'æ' => "ae".into(),
        'Æ' => "AE".into(),
        'œ' => "oe".into(),
        'Œ' => "OE".into(),
        'ß' => "ss".into(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_titles_slug_cleanly() {
        assert_eq!(slugify("Aevaryn"), "aevaryn");
        assert_eq!(slugify("Space Runner 2"), "space-runner-2");
    }

    #[test]
    fn french_accents_are_folded_not_dropped() {
        // The point of folding: dropping would give "l-p-e-du-h-ros".
        assert_eq!(slugify("L'Épopée du Héros"), "l-epopee-du-heros");
        assert_eq!(slugify("Forêt Maudite"), "foret-maudite");
        assert_eq!(slugify("Cœur de Braise"), "coeur-de-braise");
        assert_eq!(slugify("Ça Commence"), "ca-commence");
    }

    #[test]
    fn runs_of_punctuation_collapse_to_one_dash() {
        assert_eq!(slugify("Hello   ---  World!!!"), "hello-world");
    }

    #[test]
    fn leading_and_trailing_junk_is_trimmed() {
        assert_eq!(slugify("  ...Aevaryn...  "), "aevaryn");
        assert!(!slugify("Aevaryn!").ends_with('-'));
    }

    #[test]
    fn a_title_with_no_usable_characters_still_yields_a_name() {
        // GitHub rejects an empty name, so never return one.
        assert_eq!(slugify("!!!"), "projet");
        assert_eq!(slugify(""), "projet");
        assert_eq!(slugify("   "), "projet");
    }

    #[test]
    fn very_long_titles_are_truncated_without_a_trailing_dash() {
        let slug = slugify(&"mot ".repeat(60));
        assert!(slug.len() <= 90);
        assert!(!slug.ends_with('-'));
        assert!(slug.starts_with("mot-mot"));
    }

    #[test]
    fn the_slug_is_always_a_legal_github_name() {
        for title in [
            "Aevaryn", "L'Épopée", "!!!", "  ", "Ça Commence", "a/b\\c", "日本語",
        ] {
            let slug = slugify(title);
            assert!(!slug.is_empty(), "{title:?} gave an empty slug");
            assert!(
                slug.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'),
                "{title:?} gave {slug:?}"
            );
            assert!(!slug.starts_with('-') && !slug.ends_with('-'));
        }
    }
}

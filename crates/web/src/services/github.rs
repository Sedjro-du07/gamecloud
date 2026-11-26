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
    /// The branch everybody merges into.
    #[serde(default = "default_branch")]
    pub default_branch: String,
}

fn default_branch() -> String {
    "main".to_string()
}

/// The labels a project's issues are sorted by: one per track, plus the
/// milestones. Production tags each task with the track that owns it.
const LABELS: [(&str, &str, &str); 9] = [
    ("track:engineering", "1f6feb", "Code, architecture, export"),
    ("track:game-design", "8957e5", "Règles, niveaux, équilibrage"),
    ("track:narrative", "bf3989", "Histoire, dialogues, textes"),
    ("track:visual-art", "d29922", "Sprites, décors, interface"),
    ("track:audio", "2da44e", "Bruitages, musique, mixage"),
    ("track:production", "6e7781", "Planning, périmètre, coordination"),
    ("track:qa", "cf222e", "Tests, bugs, accessibilité"),
    ("track:marketing", "0969da", "Page itch.io, captures, bande-annonce"),
    ("jalon", "000000", "Prototype, alpha, bêta, version 1.0"),
];

/// The files every new project repository starts with, besides the README
/// and the Godot `.gitignore` GitHub writes itself: one folder per kind of
/// work, so each track mostly touches its own files.
fn starter_files(project: &str) -> Vec<(String, String)> {
    let readme = format!(
        "# {project}\n\nProjet de l'association GameCloud (Epitech Bénin), suivi sur la plateforme GameCloud OS.\n\n\
## Où va quoi\n\n\
| Dossier | Contenu | Track |\n|---|---|---|\n\
| `scenes/` | Scènes Godot (une scène = un propriétaire à la fois) | Engineering, Game Design |\n\
| `scripts/` | Code | Engineering |\n\
| `niveaux/` | Un niveau par fichier | Game Design |\n\
| `art/` | Images exportées (PNG) | Visual Art |\n\
| `audio/` | Sons et musiques exportés (OGG, WAV) | Audio |\n\
| `dialogues/` | Fichiers Ink (.ink) | Narrative |\n\
| `docs/` | GDD, planning, plan de test | Game Design, Production, QA |\n\
| `promo/` | Captures, GIF, textes itch.io | Marketing |\n\n\
Les fichiers sources lourds (.kra, .aseprite, projets audio) restent dans le dossier partagé de l'équipe, pas ici.\n\n\
Avant de contribuer : lire [CONTRIBUER.md](CONTRIBUER.md).\n"
    );
    let contribuer = "# Contribuer\n\n\
## La règle\n\n\
`main` est toujours jouable. On n'y pousse jamais directement : tout passe par une pull request relue.\n\n\
## Le chemin d'une tâche\n\n\
1. Une *issue* sur le tableau du projet, étiquetée par track (`track:art`…) et assignée.\n\
2. Une petite branche, qui vit de quelques heures à 2 ou 3 jours : `code/saut-du-joueur`, `art/tileset-foret`, `son/musique-menu`, `recit/dialogue-intro`, `design/niveau-2`.\n\
3. Une pull request, relue par une personne de la track concernée.\n\
4. Fusion dans `main`, puis la branche est supprimée.\n\n\
## Qui relit quoi\n\n\
| Ce qui change | Relu par |\n|---|---|\n\
| Code | un Relecteur Engineering |\n\
| Images | un Relecteur Visual Art |\n\
| Sons | un Relecteur Audio |\n\
| Dialogues | un Relecteur Narrative |\n\
| Niveaux, réglages | un Relecteur Game Design |\n\n\
## Trois règles\n\n\
- Une scène Godot n'a qu'un propriétaire à la fois : découpez en petites scènes.\n\
- Dans le dépôt, les fichiers exportés (PNG, OGG), pas les sources lourdes.\n\
- Pas besoin du terminal : GitHub Desktop, ou « Add file > Upload files » sur le site.\n\n\
## Jalons\n\n\
À chaque jalon (prototype, alpha, bêta, 1.0), une étiquette de version sur `main` et le build déposé sur la plateforme, qui le relit par track.\n";
    let mut files = vec![
        ("README.md".to_string(), readme),
        ("CONTRIBUER.md".to_string(), contribuer.to_string()),
    ];
    for dir in ["scenes", "scripts", "niveaux", "art", "audio", "dialogues", "docs", "promo"] {
        files.push((format!("{dir}/.gitkeep"), String::new()));
    }
    files
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
                // GitHub's own template: keeps Godot's cache and exports out.
                "gitignore_template": "Godot",
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

    /// Lay the starter files on top of the first commit, in one commit,
    /// through the Git data API (a commit per file would bury the
    /// history under nine "create file" entries).
    ///
    /// # Errors
    /// `Upstream` when GitHub refuses or is unreachable.
    pub async fn seed_repo(&self, repo: &Repository, project: &str) -> WebResult<()> {
        let base = format!("/repos/{}/{}", self.org, repo.name);
        let get = |path: String| async move {
            let response = self
                .request(reqwest::Method::GET, &path)
                .send()
                .await
                .map_err(|e| WebError::Upstream(format!("github seed: {e}")))?;
            if !response.status().is_success() {
                return Err(WebError::Upstream(format!("github seed: {} on {path}", response.status())));
            }
            response
                .json::<serde_json::Value>()
                .await
                .map_err(|e| WebError::Upstream(format!("github seed decode: {e}")))
        };
        let post = |method: reqwest::Method, path: String, body: serde_json::Value| async move {
            let response = self
                .request(method, &path)
                .json(&body)
                .send()
                .await
                .map_err(|e| WebError::Upstream(format!("github seed: {e}")))?;
            if !response.status().is_success() {
                return Err(WebError::Upstream(format!("github seed: {} on {path}", response.status())));
            }
            response
                .json::<serde_json::Value>()
                .await
                .map_err(|e| WebError::Upstream(format!("github seed decode: {e}")))
        };

        let branch = &repo.default_branch;
        let head = get(format!("{base}/git/ref/heads/{branch}")).await?;
        let parent = head["object"]["sha"].as_str().unwrap_or_default().to_string();
        let commit = get(format!("{base}/git/commits/{parent}")).await?;
        let base_tree = commit["tree"]["sha"].as_str().unwrap_or_default().to_string();

        let entries: Vec<serde_json::Value> = starter_files(project)
            .into_iter()
            .map(|(path, content)| serde_json::json!({ "path": path, "mode": "100644", "type": "blob", "content": content }))
            .collect();
        let tree = post(reqwest::Method::POST, format!("{base}/git/trees"), serde_json::json!({ "base_tree": base_tree, "tree": entries })).await?;
        let new_commit = post(
            reqwest::Method::POST,
            format!("{base}/git/commits"),
            serde_json::json!({
                "message": "Structure du projet GameCloud",
                "tree": tree["sha"],
                "parents": [parent],
            }),
        )
        .await?;
        post(
            reqwest::Method::PATCH,
            format!("{base}/git/refs/heads/{branch}"),
            serde_json::json!({ "sha": new_commit["sha"] }),
        )
        .await?;
        Ok(())
    }

    /// Create the track and milestone labels. A label that already exists
    /// is not an error.
    pub async fn add_labels(&self, repo: &str) {
        for (name, color, description) in LABELS {
            let result = self
                .request(reqwest::Method::POST, &format!("/repos/{}/{repo}/labels", self.org))
                .json(&serde_json::json!({ "name": name, "color": color, "description": description }))
                .send()
                .await;
            if let Err(e) = result {
                tracing::warn!(error = %e, label = name, "github: label not created");
            }
        }
    }

    /// Protect the default branch: no direct push, one approving review.
    ///
    /// GitHub only allows this on private repositories for paid plans, so
    /// on a free organisation it succeeds once the repository is public —
    /// which is why it is tried again when a project is released.
    ///
    /// # Errors
    /// `Upstream` when GitHub refuses (403 on a private repository of a
    /// free organisation) or is unreachable.
    pub async fn protect_branch(&self, repo: &str, branch: &str) -> WebResult<()> {
        let response = self
            .request(
                reqwest::Method::PUT,
                &format!("/repos/{}/{repo}/branches/{branch}/protection", self.org),
            )
            .json(&serde_json::json!({
                "required_status_checks": null,
                "enforce_admins": false,
                "required_pull_request_reviews": { "required_approving_review_count": 1 },
                "restrictions": null,
            }))
            .send()
            .await
            .map_err(|e| WebError::Upstream(format!("github protect branch: {e}")))?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(WebError::Upstream(format!("github protect branch: {}", response.status())))
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

/// Everything provisioning a repository needs to know.
///
/// Grouped so the two creation paths — the REST endpoint and the
/// Leptos server function — pass the same thing, and so adding a field
/// later is a change in one place.
#[derive(Debug, Clone, Copy)]
pub struct NewRepo<'a> {
    /// The project the repository belongs to.
    pub project_id: uuid::Uuid,
    /// Project name; slugified into the repository name.
    pub name: &'a str,
    /// One-line description, copied to the repository.
    pub description: Option<&'a str>,
    /// The author's GitHub login, when they have linked one.
    pub author_login: Option<&'a str>,
}

/// Create the project's repository, add the author, install the webhook.
///
/// Returns the repository URL when everything that matters succeeded.
/// Each step is independent: a repository with no webhook is still more
/// useful than no repository, so a webhook failure does not discard it.
///
/// Lives here rather than beside one of its callers because a project
/// can be created from two places, and when this was a private helper
/// of the REST route the Leptos path silently created projects with no
/// repository at all — while its own documentation promised one.
pub async fn provision_project_repo(
    state: &crate::state::AppState,
    new: &NewRepo<'_>,
) -> Option<String> {
    let github = state.github()?;

    let repo = match github.create_repo(new.name, new.description).await {
        Ok(repo) => repo,
        Err(e) => {
            tracing::warn!(error = %e, project = %new.project_id, "github: repository not created");
            return None;
        }
    };

    // The structure every project starts with, and the track labels.
    if let Err(e) = github.seed_repo(&repo, new.name).await {
        tracing::warn!(error = %e, repo = %repo.name, "github: starter files not added");
    }
    github.add_labels(&repo.name).await;
    // Refused on a private repository of a free organisation; tried again
    // when the project is released and the repository goes public.
    if let Err(e) = github.protect_branch(&repo.name, &repo.default_branch).await {
        tracing::info!(error = %e, repo = %repo.name, "github: main not protected yet (private repository)");
    }

    // Push access for the author, when they have linked a GitHub login.
    // Without one there is nobody to add — and linking it is also what
    // makes their commits pay XP, so the profile page nags for it.
    if let Some(login) = new.author_login {
        if let Err(e) = github.add_collaborator(&repo.name, login).await {
            tracing::warn!(error = %e, login, "github: collaborator not added");
        }
    } else {
        tracing::info!(
            project = %new.project_id,
            "github: author has no linked GitHub login; repository left without a collaborator"
        );
    }

    // The webhook is what makes commits in this repository pay XP. A
    // localhost origin can never receive one, so do not install a hook
    // that would only ever fail.
    let cfg = state.config();
    let origin = &cfg.public_origin;
    match (&cfg.github_webhook_secret, origin.contains("localhost")) {
        (Some(secret), false) => {
            let callback = format!("{}/api/webhooks/github", origin.trim_end_matches('/'));
            if let Err(e) = github
                .add_webhook(&repo.name, &callback, &String::from_utf8_lossy(secret))
                .await
            {
                tracing::warn!(error = %e, repo = %repo.name, "github: webhook not installed");
            }
        }
        (_, true) => tracing::info!(
            "github: PUBLIC_ORIGIN is localhost, skipping webhook (it could never be delivered)"
        ),
        (None, _) => tracing::info!("github: GITHUB_WEBHOOK_SECRET unset, skipping webhook"),
    }

    if let Err(e) =
        crate::db::queries::projects::set_repo_url(state.pool(), new.project_id, &repo.html_url)
            .await
    {
        tracing::warn!(error = %e, "github: repository created but URL not recorded");
    }
    Some(repo.html_url)
}

/// Give a project contributor push access to its repository, when the
/// project has one and they have linked a GitHub login. Failures are
/// logged: crediting somebody must never fail because GitHub did.
pub async fn grant_repo_access(state: &crate::state::AppState, project_id: uuid::Uuid, user_id: uuid::Uuid) {
    let Some(github) = state.github() else { return };
    let row: Result<Option<(Option<String>, Option<String>)>, _> = sqlx::query_as(
        "SELECT p.github_repo_url, u.github_username FROM projects p, users u WHERE p.id = $1 AND u.id = $2",
    )
    .bind(project_id)
    .bind(user_id)
    .fetch_optional(state.pool())
    .await;
    let Ok(Some((Some(url), Some(login)))) = row else {
        return;
    };
    let Some(repo) = crate::db::queries::projects::repo_name_from_url(&url) else { return };
    if let Err(e) = github.add_collaborator(repo, &login).await {
        tracing::warn!(error = %e, repo, login, "github: contributor not given access");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_project_starts_with_the_track_folders() {
        let files = starter_files("Mon jeu");
        let paths: Vec<&str> = files.iter().map(|(p, _)| p.as_str()).collect();
        for dir in ["scenes", "scripts", "niveaux", "art", "audio", "dialogues", "docs", "promo"] {
            assert!(paths.contains(&format!("{dir}/.gitkeep").as_str()), "{dir}");
        }
        assert!(files[0].1.starts_with("# Mon jeu"));
        assert!(paths.contains(&"CONTRIBUER.md"));
    }

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

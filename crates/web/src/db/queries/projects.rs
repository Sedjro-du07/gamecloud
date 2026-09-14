//! Projects: creation, the review pipeline, and release.
//!
//! The lifecycle rules live in [`gamecloud_shared::projects`]; this
//! module is the persistence and XP side of them. The two things worth
//! knowing:
//!
//! - **Submitting opens one validation row per concerned track.** The
//!   concerned tracks are the project's primary track plus every track
//!   a contributor is credited under, so a build with an artist and a
//!   composer on it genuinely gets judged by Visual Art and Audio.
//!
//! - **Releasing pays everyone at once, in one transaction.** Each
//!   contributor is credited in their own track, the primary track's
//!   Lead gets the oversight bonus, and the Hall of Fame row records
//!   the total actually distributed.

use std::collections::BTreeSet;

use gamecloud_shared::{
    projects::{aggregate, ProjectStatus, Rarity, Verdict},
    roles::Track,
    xp::{
        XpSource, XP_PEER_REVIEW, XP_PROJECT_MASTER, XP_PROJECT_RELEASED, XP_PROJECT_TEK1,
        XP_PROJECT_TEK2, XP_PROJECT_TEK3, XP_TRACKLEAD_RELEASED,
    },
    DomainError,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    db::queries::{
        audit,
        xp::{self, XpGrant},
    },
    error::{WebError, WebResult},
    services::notifications::{self, Announcement},
};

// ---------------------------------------------------------------------------
// Read models
// ---------------------------------------------------------------------------

/// Summary row for listings.
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct ProjectSummary {
    /// Project id.
    pub id: Uuid,
    /// Name.
    pub name: String,
    /// One-liner.
    pub short_description: Option<String>,
    /// Primary track.
    pub primary_track: String,
    /// Lifecycle status.
    pub status: String,
    /// Cosmetic tier.
    pub rarity: String,
    /// Card image.
    pub thumbnail_url: Option<String>,
    /// Epitech level, when the project maps onto one.
    pub epitech_level: Option<String>,
    /// Creation instant.
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Release instant, when released.
    pub released_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Number of credited contributors.
    pub contributor_count: i64,
}

/// One track's verdict, resolved for display.
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct ValidationView {
    /// Track that owes the verdict.
    pub track: String,
    /// `Pending`, `Approved`, `Rejected` or `NotApplicable`.
    pub status: String,
    /// Reviewer's display name, once reviewed.
    pub reviewer_name: Option<String>,
    /// Written feedback.
    pub feedback: Option<String>,
    /// When the verdict was rendered.
    pub reviewed_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// A credited contributor.
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct ContributorView {
    /// Member id.
    pub user_id: Uuid,
    /// Display name.
    pub display_name: String,
    /// Avatar.
    pub avatar_url: Option<String>,
    /// Track they contributed under.
    pub track: String,
    /// Free-text role, e.g. "Lead Artist".
    pub role_in_project: Option<String>,
}

/// Everything the detail page needs.
#[derive(Debug, Clone, Serialize)]
pub struct ProjectDetail {
    /// Summary fields.
    pub summary: ProjectSummary,
    /// Long description.
    pub long_description: Option<String>,
    /// Repository link.
    pub github_repo_url: Option<String>,
    /// itch.io link.
    pub itch_url: Option<String>,
    /// Gallery.
    pub screenshots: Vec<String>,
    /// Trailer.
    pub video_url: Option<String>,
    /// Credited contributors.
    pub contributors: Vec<ContributorView>,
    /// Per-track verdicts.
    pub validations: Vec<ValidationView>,
}

// ---------------------------------------------------------------------------
// Listing
// ---------------------------------------------------------------------------

/// List projects.
///
/// `include_internal` controls whether pre-release work is visible;
/// callers pass `true` only for members who hold the matching track
/// role. Everyone else sees the published record.
///
/// # Errors
/// Propagates database errors.
pub async fn list(
    pool: &PgPool,
    track: Option<&str>,
    include_internal: bool,
) -> WebResult<Vec<ProjectSummary>> {
    let rows = sqlx::query_as::<_, ProjectSummary>(
        r#"
        SELECT p.id,
               p.name,
               p.short_description,
               p.primary_track,
               p.status,
               p.rarity,
               p.thumbnail_url,
               p.epitech_level,
               p.created_at,
               p.released_at,
               COALESCE((SELECT COUNT(DISTINCT c.user_id)
                           FROM project_contributors c
                          WHERE c.project_id = p.id), 0) AS contributor_count
          FROM projects p
         WHERE ($1::TEXT IS NULL OR p.primary_track = $1)
           AND ($2::BOOLEAN OR p.status IN ('Released', 'Archived'))
         ORDER BY p.released_at DESC NULLS LAST, p.created_at DESC
        "#,
    )
    .bind(track)
    .bind(include_internal)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Fetch one project with its contributors and verdicts.
///
/// # Errors
/// `NotFound` when the id does not exist.
pub async fn detail(pool: &PgPool, id: Uuid) -> WebResult<ProjectDetail> {
    let summary = sqlx::query_as::<_, ProjectSummary>(
        r#"
        SELECT p.id, p.name, p.short_description, p.primary_track, p.status,
               p.rarity, p.thumbnail_url, p.epitech_level, p.created_at, p.released_at,
               COALESCE((SELECT COUNT(DISTINCT c.user_id)
                           FROM project_contributors c
                          WHERE c.project_id = p.id), 0) AS contributor_count
          FROM projects p
         WHERE p.id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or(WebError::NotFound)?;

    let extra: (Option<String>, Option<String>, Option<String>, Vec<String>, Option<String>) =
        sqlx::query_as(
            r#"
            SELECT long_description, github_repo_url, itch_url, screenshots, video_url
              FROM projects WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_one(pool)
        .await?;

    let contributors = sqlx::query_as::<_, ContributorView>(
        r#"
        SELECT c.user_id,
               COALESCE(u.current_title, u.discord_id) AS display_name,
               COALESCE(u.avatar_custom_url, u.avatar_url) AS avatar_url,
               c.track,
               c.role_in_project
          FROM project_contributors c
          JOIN users u ON u.id = c.user_id
         WHERE c.project_id = $1
         ORDER BY c.track ASC
        "#,
    )
    .bind(id)
    .fetch_all(pool)
    .await?;

    let validations = sqlx::query_as::<_, ValidationView>(
        r#"
        SELECT v.track,
               v.status,
               COALESCE(u.current_title, u.discord_id) AS reviewer_name,
               v.feedback,
               v.reviewed_at
          FROM track_validations v
          LEFT JOIN users u ON u.id = v.reviewed_by
         WHERE v.project_id = $1
         ORDER BY v.track ASC
        "#,
    )
    .bind(id)
    .fetch_all(pool)
    .await?;

    Ok(ProjectDetail {
        summary,
        long_description: extra.0,
        github_repo_url: extra.1,
        itch_url: extra.2,
        screenshots: extra.3,
        video_url: extra.4,
        contributors,
        validations,
    })
}

// ---------------------------------------------------------------------------
// Creation
// ---------------------------------------------------------------------------

/// Fields accepted when creating a project.
#[derive(Debug, Clone, Deserialize)]
pub struct NewProject {
    /// Name.
    pub name: String,
    /// One-liner.
    pub short_description: Option<String>,
    /// Long description.
    pub long_description: Option<String>,
    /// Primary track.
    pub primary_track: String,
    /// Epitech level, if applicable.
    pub epitech_level: Option<String>,
    /// Block number 1-8, if applicable.
    pub block_number: Option<i32>,
    /// Repository link.
    pub github_repo_url: Option<String>,
    /// itch.io link.
    pub itch_url: Option<String>,
}

/// Validate a project definition.
///
/// # Errors
/// `Validation` or `UnknownTrack` describing the first problem.
pub fn validate(project: &NewProject) -> WebResult<()> {
    if project.name.trim().is_empty() {
        return Err(WebError::Validation("name must not be empty".into()));
    }
    if Track::parse(&project.primary_track).is_none() {
        return Err(WebError::Domain(DomainError::UnknownTrack(
            project.primary_track.clone(),
        )));
    }
    if let Some(level) = &project.epitech_level {
        if !matches!(level.as_str(), "Tek1" | "Tek2" | "Tek3" | "Master") {
            return Err(WebError::Validation(format!(
                "unknown epitech_level '{level}' (expected Tek1, Tek2, Tek3 or Master)"
            )));
        }
    }
    if let Some(block) = project.block_number {
        if !(1..=8).contains(&block) {
            return Err(WebError::Validation(
                "block_number must be between 1 and 8".into(),
            ));
        }
    }
    for (field, url) in [
        ("github_repo_url", &project.github_repo_url),
        ("itch_url", &project.itch_url),
    ] {
        if let Some(u) = url {
            if !u.starts_with("https://") && !u.starts_with("http://") {
                return Err(WebError::Validation(format!("{field} must be an http(s) URL")));
            }
        }
    }
    Ok(())
}

/// Create a project in `Draft`, crediting the author as a contributor
/// under the primary track.
///
/// # Errors
/// Propagates validation and database errors.
pub async fn create(pool: &PgPool, author: Uuid, project: &NewProject) -> WebResult<Uuid> {
    validate(project)?;

    let mut tx = pool.begin().await?;

    let id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO projects (name, short_description, long_description, primary_track,
                              epitech_level, block_number, github_repo_url, itch_url,
                              status, created_by)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'Draft', $9)
        RETURNING id
        "#,
    )
    .bind(&project.name)
    .bind(&project.short_description)
    .bind(&project.long_description)
    .bind(&project.primary_track)
    .bind(&project.epitech_level)
    .bind(project.block_number)
    .bind(&project.github_repo_url)
    .bind(&project.itch_url)
    .bind(author)
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO project_contributors (project_id, user_id, track, role_in_project)
        VALUES ($1, $2, $3, 'Créateur')
        ON CONFLICT DO NOTHING
        "#,
    )
    .bind(id)
    .bind(author)
    .bind(&project.primary_track)
    .execute(&mut *tx)
    .await?;

    audit::record_in_tx(
        &mut tx,
        Some(author),
        "project.created",
        Some("project"),
        Some(id),
        serde_json::json!({ "name": project.name, "track": project.primary_track }),
    )
    .await?;

    tx.commit().await?;
    Ok(id)
}

/// Credit a contributor under a track.
///
/// # Errors
/// `UnknownTrack` or a database error.
pub async fn add_contributor(
    pool: &PgPool,
    project_id: Uuid,
    user_id: Uuid,
    track: &str,
    role_in_project: Option<&str>,
) -> WebResult<()> {
    if Track::parse(track).is_none() {
        return Err(WebError::Domain(DomainError::UnknownTrack(
            track.to_string(),
        )));
    }
    sqlx::query(
        r#"
        INSERT INTO project_contributors (project_id, user_id, track, role_in_project)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (project_id, user_id, track) DO UPDATE
            SET role_in_project = EXCLUDED.role_in_project
        "#,
    )
    .bind(project_id)
    .bind(user_id)
    .bind(track)
    .bind(role_in_project)
    .execute(pool)
    .await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Review pipeline
// ---------------------------------------------------------------------------

/// Current status of a project, as a domain value.
async fn status_of(pool: &PgPool, project_id: Uuid) -> WebResult<ProjectStatus> {
    let raw: String = sqlx::query_scalar("SELECT status FROM projects WHERE id = $1")
        .bind(project_id)
        .fetch_optional(pool)
        .await?
        .ok_or(WebError::NotFound)?;
    ProjectStatus::parse(&raw).ok_or_else(|| {
        WebError::Internal(anyhow::anyhow!("project {project_id} has unknown status {raw}"))
    })
}

/// Submit a project for review.
///
/// Opens one `track_validations` row per concerned track: the primary
/// track, plus every track a contributor is credited under.
///
/// # Errors
/// `InvalidProjectTransition` when the project is not in a submittable
/// state; otherwise propagates database errors.
pub async fn submit_for_review(
    pool: &PgPool,
    announce_channel: Option<u64>,
    actor: Uuid,
    project_id: Uuid,
) -> WebResult<Vec<String>> {
    let current = status_of(pool, project_id).await?;
    let next = current.transition_to(ProjectStatus::InReview)?;

    let mut tx = pool.begin().await?;

    let primary: String = sqlx::query_scalar("SELECT primary_track FROM projects WHERE id = $1")
        .bind(project_id)
        .fetch_one(&mut *tx)
        .await?;

    let contributor_tracks: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT track FROM project_contributors WHERE project_id = $1",
    )
    .bind(project_id)
    .fetch_all(&mut *tx)
    .await?;

    // BTreeSet gives dedup plus a stable order, so the validation rows
    // are created in the same sequence every time.
    let concerned: BTreeSet<String> = std::iter::once(primary)
        .chain(contributor_tracks)
        .collect();

    for track in &concerned {
        sqlx::query(
            r#"
            INSERT INTO track_validations (project_id, track, status)
            VALUES ($1, $2, 'Pending')
            ON CONFLICT (project_id, track) DO UPDATE
                SET status = 'Pending', reviewed_by = NULL,
                    feedback = NULL, reviewed_at = NULL
            "#,
        )
        .bind(project_id)
        .bind(track)
        .execute(&mut *tx)
        .await?;
    }

    sqlx::query("UPDATE projects SET status = $2 WHERE id = $1")
        .bind(project_id)
        .bind(next.as_str())
        .execute(&mut *tx)
        .await?;

    // A submission advances any open `Submit` quest. That condition has
    // no XP source of its own, so it is driven explicitly from here.
    let completed =
        xp::advance_quests_for_conditions(&mut tx, actor, &["Submit"], None).await?;

    audit::record_in_tx(
        &mut tx,
        Some(actor),
        "project.submitted",
        Some("project"),
        Some(project_id),
        serde_json::json!({ "tracks": concerned.iter().collect::<Vec<_>>() }),
    )
    .await?;

    for quest in &completed {
        let display: String =
            sqlx::query_scalar("SELECT COALESCE(current_title, discord_id) FROM users WHERE id = $1")
                .bind(actor)
                .fetch_one(&mut *tx)
                .await?;
        notifications::enqueue(
            &mut tx,
            announce_channel,
            &Announcement::quest(actor, &display, &quest.title, quest.xp_reward),
        )
        .await?;
    }

    tx.commit().await?;
    Ok(concerned.into_iter().collect())
}

/// Record one track's verdict and re-derive the project status.
///
/// The reviewer earns peer-review XP for a substantive verdict.
/// `NotApplicable` pays nothing — declaring a track irrelevant is not
/// review work, and paying for it would make it the rational default.
///
/// # Errors
/// `Validation` when a rejection carries no feedback, `NotFound` when
/// the track is not under review, otherwise database errors.
pub async fn record_verdict(
    pool: &PgPool,
    announce_channel: Option<u64>,
    reviewer: Uuid,
    project_id: Uuid,
    track: &str,
    verdict: Verdict,
    feedback: Option<&str>,
) -> WebResult<ProjectStatus> {
    if verdict == Verdict::Pending {
        return Err(WebError::Validation(
            "Pending is not a verdict a reviewer can submit".into(),
        ));
    }
    if verdict.requires_feedback() && feedback.map_or(true, |f| f.trim().is_empty()) {
        return Err(WebError::Validation(
            "a rejection must explain what needs to change".into(),
        ));
    }

    let current = status_of(pool, project_id).await?;
    if current != ProjectStatus::InReview {
        return Err(WebError::Domain(DomainError::InvalidProjectTransition {
            from: current.as_str(),
            to: "reviewed",
        }));
    }

    let mut tx = pool.begin().await?;

    let updated = sqlx::query(
        r#"
        UPDATE track_validations
           SET status = $3, reviewed_by = $4, feedback = $5, reviewed_at = NOW()
         WHERE project_id = $1 AND track = $2
        "#,
    )
    .bind(project_id)
    .bind(track)
    .bind(verdict.as_str())
    .bind(reviewer)
    .bind(feedback)
    .execute(&mut *tx)
    .await?;

    if updated.rows_affected() == 0 {
        return Err(WebError::NotFound);
    }

    let raw_verdicts: Vec<String> =
        sqlx::query_scalar("SELECT status FROM track_validations WHERE project_id = $1")
            .bind(project_id)
            .fetch_all(&mut *tx)
            .await?;

    let verdicts: Vec<Verdict> = raw_verdicts
        .iter()
        .filter_map(|s| Verdict::parse(s))
        .collect();
    let next = aggregate(&verdicts);

    sqlx::query("UPDATE projects SET status = $2 WHERE id = $1")
        .bind(project_id)
        .bind(next.as_str())
        .execute(&mut *tx)
        .await?;

    if verdict != Verdict::NotApplicable {
        xp::grant_in_tx(
            &mut tx,
            announce_channel,
            &XpGrant::new(reviewer, XP_PEER_REVIEW, XpSource::Review)
                .describe("Revue de projet")
                .in_track(track),
        )
        .await?;
    }

    audit::record_in_tx(
        &mut tx,
        Some(reviewer),
        "project.reviewed",
        Some("project"),
        Some(project_id),
        serde_json::json!({ "track": track, "verdict": verdict.as_str(), "status": next.as_str() }),
    )
    .await?;

    tx.commit().await?;
    Ok(next)
}

// ---------------------------------------------------------------------------
// Release
// ---------------------------------------------------------------------------

/// XP a contributor earns when a project of a given Epitech level ships.
fn contributor_award(epitech_level: Option<&str>) -> i32 {
    match epitech_level {
        Some("Tek1") => XP_PROJECT_TEK1,
        Some("Tek2") => XP_PROJECT_TEK2,
        Some("Tek3") => XP_PROJECT_TEK3,
        Some("Master") => XP_PROJECT_MASTER,
        // An association project with no Epitech level attached is still
        // a shipped game; it pays the base release award.
        _ => XP_PROJECT_RELEASED,
    }
}

/// What a release paid out.
#[derive(Debug, Clone, Serialize)]
pub struct ReleaseReport {
    /// Rarity the project earned.
    pub rarity: String,
    /// Contributors credited.
    pub contributors_paid: usize,
    /// Total XP distributed.
    pub total_xp: i32,
}

/// Move an `Approved` project to `Released`.
///
/// Everything happens in one transaction: the status change, every
/// contributor's XP, the primary track Lead's oversight bonus, the Hall
/// of Fame entry and the announcement.
///
/// # Errors
/// `InvalidProjectTransition` unless the project is `Approved`;
/// otherwise propagates database errors.
pub async fn release(
    pool: &PgPool,
    announce_channel: Option<u64>,
    actor: Uuid,
    project_id: Uuid,
) -> WebResult<ReleaseReport> {
    let current = status_of(pool, project_id).await?;
    let next = current.transition_to(ProjectStatus::Released)?;

    let mut tx = pool.begin().await?;

    let (name, primary_track, epitech_level): (String, String, Option<String>) = sqlx::query_as(
        "SELECT name, primary_track, epitech_level FROM projects WHERE id = $1",
    )
    .bind(project_id)
    .fetch_one(&mut *tx)
    .await?;

    let approved_tracks: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM track_validations WHERE project_id = $1 AND status = 'Approved'",
    )
    .bind(project_id)
    .fetch_one(&mut *tx)
    .await?;

    let rarity = Rarity::from_approved_tracks(usize::try_from(approved_tracks).unwrap_or(0));

    sqlx::query(
        r#"
        UPDATE projects
           SET status = $2, rarity = $3, released_at = NOW()
         WHERE id = $1
        "#,
    )
    .bind(project_id)
    .bind(next.as_str())
    .bind(rarity.as_str())
    .execute(&mut *tx)
    .await?;

    // --- pay the contributors ----------------------------------------
    let contributors: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT user_id, track FROM project_contributors WHERE project_id = $1",
    )
    .bind(project_id)
    .fetch_all(&mut *tx)
    .await?;

    let award = contributor_award(epitech_level.as_deref());
    let description = format!("Projet publié : {name}");
    let mut total_xp = 0_i32;
    let mut paid = BTreeSet::new();

    for (user_id, track) in &contributors {
        let outcome = xp::grant_in_tx(
            &mut tx,
            announce_channel,
            &XpGrant::new(*user_id, award, XpSource::Project)
                .describe(&description)
                .in_track(track),
        )
        .await?;
        total_xp = total_xp.saturating_add(outcome.awarded);
        paid.insert(*user_id);
    }

    // --- pay the primary track's Lead --------------------------------
    let leads: Vec<Uuid> = sqlx::query_scalar(
        r#"
        SELECT user_id FROM track_memberships
         WHERE track = $1 AND track_role = 'Lead'
        "#,
    )
    .bind(&primary_track)
    .fetch_all(&mut *tx)
    .await?;

    for lead in leads {
        let outcome = xp::grant_in_tx(
            &mut tx,
            announce_channel,
            &XpGrant::new(lead, XP_TRACKLEAD_RELEASED, XpSource::Project)
                .describe(&format!("Supervision : {name}"))
                .in_track(&primary_track),
        )
        .await?;
        total_xp = total_xp.saturating_add(outcome.awarded);
    }

    // --- Hall of Fame -------------------------------------------------
    let boss_defeated_by: Vec<Uuid> = paid.iter().copied().collect();
    sqlx::query(
        r#"
        INSERT INTO hall_of_fame (project_id, total_xp_distributed, boss_defeated_by)
        VALUES ($1, $2, $3)
        ON CONFLICT (project_id) DO UPDATE
            SET total_xp_distributed = EXCLUDED.total_xp_distributed,
                boss_defeated_by     = EXCLUDED.boss_defeated_by
        "#,
    )
    .bind(project_id)
    .bind(total_xp.max(0))
    .bind(&boss_defeated_by)
    .execute(&mut *tx)
    .await?;

    notifications::enqueue(
        &mut tx,
        announce_channel,
        &Announcement::project_released(&name, &primary_track, paid.len()),
    )
    .await?;

    audit::record_in_tx(
        &mut tx,
        Some(actor),
        "project.released",
        Some("project"),
        Some(project_id),
        serde_json::json!({
            "rarity": rarity.as_str(),
            "contributors": paid.len(),
            "total_xp": total_xp,
        }),
    )
    .await?;

    tx.commit().await?;

    Ok(ReleaseReport {
        rarity: rarity.as_str().to_string(),
        contributors_paid: paid.len(),
        total_xp,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> NewProject {
        NewProject {
            name: "Aevaryn".into(),
            short_description: Some("Un RPG narratif".into()),
            long_description: None,
            primary_track: "Narrative".into(),
            epitech_level: Some("Tek2".into()),
            block_number: Some(3),
            github_repo_url: Some("https://github.com/gamecloud/aevaryn".into()),
            itch_url: None,
        }
    }

    #[test]
    fn accepts_a_well_formed_project() {
        assert!(validate(&sample()).is_ok());
    }

    #[test]
    fn rejects_an_empty_name() {
        let mut p = sample();
        p.name = "  ".into();
        assert!(validate(&p).is_err());
    }

    #[test]
    fn rejects_an_unknown_track() {
        let mut p = sample();
        p.primary_track = "Pottery".into();
        assert!(validate(&p).is_err());
    }

    #[test]
    fn rejects_an_unknown_epitech_level() {
        let mut p = sample();
        p.epitech_level = Some("Tek9".into());
        assert!(validate(&p).is_err());
    }

    #[test]
    fn rejects_out_of_range_blocks() {
        for bad in [0, 9, -1] {
            let mut p = sample();
            p.block_number = Some(bad);
            assert!(validate(&p).is_err(), "block {bad} should be rejected");
        }
        for good in 1..=8 {
            let mut p = sample();
            p.block_number = Some(good);
            assert!(validate(&p).is_ok(), "block {good} should be accepted");
        }
    }

    #[test]
    fn rejects_non_http_links() {
        let mut p = sample();
        p.itch_url = Some("javascript:alert(1)".into());
        assert!(validate(&p).is_err());
    }

    #[test]
    fn contributor_award_scales_with_level() {
        assert_eq!(contributor_award(Some("Tek1")), XP_PROJECT_TEK1);
        assert_eq!(contributor_award(Some("Tek2")), XP_PROJECT_TEK2);
        assert_eq!(contributor_award(Some("Tek3")), XP_PROJECT_TEK3);
        assert_eq!(contributor_award(Some("Master")), XP_PROJECT_MASTER);
    }

    #[test]
    fn a_project_without_an_epitech_level_still_pays() {
        assert_eq!(contributor_award(None), XP_PROJECT_RELEASED);
        assert!(contributor_award(None) > 0);
    }
}

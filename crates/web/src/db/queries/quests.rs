//! Quests.
//!
//! Quests are the only mechanism on the platform that *directs* effort
//! rather than merely measuring it: XP tells the Bureau what happened,
//! a quest tells the club what to do this week. The schema for them
//! shipped in migration 0006 and was never used.
//!
//! Progress is advanced by the XP engine (see
//! [`crate::db::queries::xp::advance_quests_for_conditions`]); this
//! module owns creation and read models.

use gamecloud_shared::{
    roles::Track,
    xp::QuestCondition,
    DomainError,
};
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{WebError, WebResult};

/// A quest plus the caller's progress on it.
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct QuestView {
    /// Quest id.
    pub id: Uuid,
    /// Title shown in the UI.
    pub title: String,
    /// Longer explanation.
    pub description: Option<String>,
    /// XP paid on completion.
    pub xp_reward: i32,
    /// `Weekly`, `Special` or `Hidden`.
    pub quest_type: String,
    /// What is being counted.
    pub condition_type: String,
    /// How many are needed.
    pub condition_value: i32,
    /// Track this quest is scoped to, if any.
    pub track: Option<String>,
    /// Opening instant.
    pub starts_at: chrono::DateTime<chrono::Utc>,
    /// Closing instant.
    pub ends_at: chrono::DateTime<chrono::Utc>,
    /// The caller's current count.
    pub progress: i32,
    /// Whether the caller has finished it.
    pub completed: bool,
}

/// Quests open right now, with the member's progress attached.
///
/// `Hidden` quests are omitted until the member has made progress on
/// them — that is what makes them hidden.
///
/// # Errors
/// Propagates database errors.
pub async fn active_for_user(pool: &PgPool, user_id: Uuid) -> WebResult<Vec<QuestView>> {
    let rows = sqlx::query_as::<_, QuestView>(
        r#"
        SELECT q.id,
               q.title,
               q.description,
               q.xp_reward,
               q.quest_type,
               q.condition_type,
               q.condition_value,
               q.track,
               q.starts_at,
               q.ends_at,
               COALESCE(p.counter, 0) AS progress,
               (c.quest_id IS NOT NULL) AS completed
          FROM quests q
          LEFT JOIN quest_progress   p ON p.quest_id = q.id AND p.user_id = $1
          LEFT JOIN quest_completions c ON c.quest_id = q.id AND c.user_id = $1
         WHERE q.starts_at <= NOW()
           AND q.ends_at   >  NOW()
           AND (q.quest_type <> 'Hidden' OR p.counter > 0)
         ORDER BY q.ends_at ASC
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Quests the member has finished, most recent first.
///
/// # Errors
/// Propagates database errors.
pub async fn completed_by_user(pool: &PgPool, user_id: Uuid) -> WebResult<Vec<QuestView>> {
    let rows = sqlx::query_as::<_, QuestView>(
        r#"
        SELECT q.id,
               q.title,
               q.description,
               q.xp_reward,
               q.quest_type,
               q.condition_type,
               q.condition_value,
               q.track,
               q.starts_at,
               q.ends_at,
               q.condition_value AS progress,
               TRUE AS completed
          FROM quest_completions c
          JOIN quests q ON q.id = c.quest_id
         WHERE c.user_id = $1
         ORDER BY c.completed_at DESC
         LIMIT 50
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Fields needed to open a quest.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct NewQuest {
    /// Title.
    pub title: String,
    /// Optional description.
    pub description: Option<String>,
    /// XP paid on completion. Must be positive.
    pub xp_reward: i32,
    /// `Weekly`, `Special` or `Hidden`.
    pub quest_type: String,
    /// `Push`, `Attend`, `Submit` or `Review`.
    pub condition_type: String,
    /// Target count. Must be positive.
    pub condition_value: i32,
    /// Optional track restriction.
    pub track: Option<String>,
    /// Opening instant.
    pub starts_at: chrono::DateTime<chrono::Utc>,
    /// Closing instant.
    pub ends_at: chrono::DateTime<chrono::Utc>,
}

/// Validate a quest definition against the domain vocabulary.
///
/// The database has equivalent CHECK constraints; validating here turns
/// what would be an opaque 500 into a precise 422.
///
/// # Errors
/// `Validation` or a domain error describing the first problem found.
pub fn validate(quest: &NewQuest) -> WebResult<()> {
    if quest.title.trim().is_empty() {
        return Err(WebError::Validation("title must not be empty".into()));
    }
    if !matches!(quest.quest_type.as_str(), "Weekly" | "Special" | "Hidden") {
        return Err(WebError::Validation(format!(
            "unknown quest_type '{}' (expected Weekly, Special or Hidden)",
            quest.quest_type
        )));
    }
    if QuestCondition::parse(&quest.condition_type).is_none() {
        return Err(WebError::Validation(format!(
            "unknown condition_type '{}' (expected Push, Attend, Submit or Review)",
            quest.condition_type
        )));
    }
    if quest.xp_reward <= 0 {
        return Err(WebError::Validation("xp_reward must be positive".into()));
    }
    if quest.condition_value <= 0 {
        return Err(WebError::Validation(
            "condition_value must be positive".into(),
        ));
    }
    if quest.ends_at <= quest.starts_at {
        return Err(WebError::Validation(
            "a quest must end after it starts".into(),
        ));
    }
    if let Some(track) = &quest.track {
        if Track::parse(track).is_none() {
            return Err(WebError::Domain(DomainError::UnknownTrack(track.clone())));
        }
    }
    Ok(())
}

/// Create a quest.
///
/// # Errors
/// Propagates validation and database errors.
pub async fn create(pool: &PgPool, author: Uuid, quest: &NewQuest) -> WebResult<Uuid> {
    validate(quest)?;
    let id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO quests (title, description, xp_reward, quest_type, condition_type,
                            condition_value, track, starts_at, ends_at, created_by)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
        RETURNING id
        "#,
    )
    .bind(&quest.title)
    .bind(&quest.description)
    .bind(quest.xp_reward)
    .bind(&quest.quest_type)
    .bind(&quest.condition_type)
    .bind(quest.condition_value)
    .bind(&quest.track)
    .bind(quest.starts_at)
    .bind(quest.ends_at)
    .bind(author)
    .fetch_one(pool)
    .await?;
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> NewQuest {
        let now = chrono::Utc::now();
        NewQuest {
            title: "Trois revues cette semaine".into(),
            description: None,
            xp_reward: 40,
            quest_type: "Weekly".into(),
            condition_type: "Review".into(),
            condition_value: 3,
            track: None,
            starts_at: now,
            ends_at: now + chrono::Duration::days(7),
        }
    }

    #[test]
    fn accepts_a_well_formed_quest() {
        assert!(validate(&sample()).is_ok());
    }

    #[test]
    fn rejects_an_empty_title() {
        let mut q = sample();
        q.title = "   ".into();
        assert!(validate(&q).is_err());
    }

    #[test]
    fn rejects_an_unknown_condition() {
        let mut q = sample();
        q.condition_type = "Vibes".into();
        assert!(validate(&q).is_err());
    }

    #[test]
    fn rejects_an_unknown_quest_type() {
        let mut q = sample();
        q.quest_type = "Daily".into();
        assert!(validate(&q).is_err());
    }

    #[test]
    fn rejects_non_positive_rewards_and_targets() {
        let mut q = sample();
        q.xp_reward = 0;
        assert!(validate(&q).is_err());

        let mut q = sample();
        q.condition_value = -1;
        assert!(validate(&q).is_err());
    }

    #[test]
    fn rejects_an_inverted_window() {
        let mut q = sample();
        std::mem::swap(&mut q.starts_at, &mut q.ends_at);
        assert!(validate(&q).is_err());
    }

    #[test]
    fn rejects_an_unknown_track() {
        let mut q = sample();
        q.track = Some("Cooking".into());
        assert!(validate(&q).is_err());
    }

    #[test]
    fn accepts_every_real_track() {
        for track in Track::ALL {
            let mut q = sample();
            q.track = Some(track.as_str().to_string());
            assert!(validate(&q).is_ok(), "track {track:?} rejected");
        }
    }
}

//! Calendar: sessions, workshops, jams, deadlines.
//!
//! An event is the thing attendance attaches to. Before this module the
//! platform could record that somebody scanned a QR code labelled
//! "Tuesday session", but two weeks of Tuesday sessions were unrelated
//! strings, so no question about attendance over time had an answer.
//!
//! Events are **cancelled, never deleted**. Attendance rows point at
//! them, and a member who turned up should keep the credit even if the
//! event is later called off.

use gamecloud_shared::roles::{EventScope, Track};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    config::DiscordChannels,
    error::{WebError, WebResult},
    services::notifications::{self, Announcement},
};

/// Kinds of gathering the calendar knows about.
///
/// Kept as a small closed list rather than free text so the calendar can
/// colour and filter them, and so the CHECK constraint in the schema has
/// something to match.
pub const KINDS: [&str; 6] = [
    "Session",
    "Workshop",
    "Jam",
    "Meeting",
    "Deadline",
    "Showcase",
];

/// An event as the calendar shows it.
#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct EventRow {
    /// Row id.
    pub id: Uuid,
    /// What it is called.
    pub title: String,
    /// Longer description, optional.
    pub description: Option<String>,
    /// One of [`KINDS`].
    pub kind: String,
    /// Track it belongs to, set only when `audience` is `Track`.
    pub track: Option<String>,
    /// `Association`, `Track` or `Bureau`.
    pub audience: String,
    /// Start instant.
    pub starts_at: chrono::DateTime<chrono::Utc>,
    /// End instant.
    pub ends_at: chrono::DateTime<chrono::Utc>,
    /// Room, campus, or a link.
    pub location: Option<String>,
    /// XP handed to whoever scans in.
    pub xp_reward: i32,
    /// Who put it on the calendar.
    pub created_by: Uuid,
    /// Set when the event was called off.
    pub cancelled_at: Option<chrono::DateTime<chrono::Utc>>,
    /// How many members have scanned in.
    pub attendee_count: i64,
}

/// What is needed to put an event on the calendar.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewEvent {
    /// What it is called.
    pub title: String,
    /// Longer description, optional.
    pub description: Option<String>,
    /// One of [`KINDS`].
    pub kind: String,
    /// Track it belongs to, set only when `audience` is `Track`.
    pub track: Option<String>,
    /// `Association`, `Track` or `Bureau`.
    pub audience: String,
    /// Start instant.
    pub starts_at: chrono::DateTime<chrono::Utc>,
    /// End instant.
    pub ends_at: chrono::DateTime<chrono::Utc>,
    /// Room, campus, or a link.
    pub location: Option<String>,
    /// XP handed to whoever scans in.
    pub xp_reward: i32,
}

/// Reject an event the schema would reject anyway, but with a sentence a
/// member can act on rather than a constraint name.
fn validate(new: &NewEvent) -> WebResult<()> {
    let title = new.title.trim();
    if title.is_empty() || title.chars().count() > 120 {
        return Err(WebError::Validation(
            "Le titre doit faire entre 1 et 120 caractères.".into(),
        ));
    }
    if !KINDS.contains(&new.kind.as_str()) {
        return Err(WebError::Validation("Type d'événement inconnu.".into()));
    }
    if EventScope::parse(&new.audience, new.track.as_deref()).is_none() {
        return Err(WebError::Validation(
            "Portée invalide : une séance de track doit nommer sa track, et une réunion              d'association ou de bureau ne doit pas en nommer."
                .into(),
        ));
    }
    if new.ends_at < new.starts_at {
        return Err(WebError::Validation(
            "La fin ne peut pas précéder le début.".into(),
        ));
    }
    if !(0..=500).contains(&new.xp_reward) {
        return Err(WebError::Validation(
            "La récompense doit être comprise entre 0 et 500 XP.".into(),
        ));
    }
    Ok(())
}

/// Columns every read of an event returns, with the attendee count
/// folded in so the calendar does not issue one query per day.
const SELECT: &str = r"
    SELECT e.id, e.title, e.description, e.kind, e.track, e.audience, e.starts_at, e.ends_at,
           e.location, e.xp_reward, e.created_by, e.cancelled_at,
           (SELECT COUNT(*) FROM attendance a WHERE a.event_id = e.id) AS attendee_count
      FROM events e
";

/// Everything happening in a window, earliest first.
///
/// The window is half-open on the end so a month view asking for
/// `[1 Sep, 1 Oct)` does not also pull the 1st of October.
///
/// # Errors
/// Propagates database errors.
pub async fn in_window(
    pool: &PgPool,
    from: chrono::DateTime<chrono::Utc>,
    to: chrono::DateTime<chrono::Utc>,
    track: Option<&str>,
    bureau: bool,
) -> WebResult<Vec<EventRow>> {
    // Bureau meetings are filtered out in SQL rather than after the
    // fetch. Reading them and then not drawing them would still put
    // them in the response the browser receives, which is not privacy.
    let sql = format!(
        "{SELECT} WHERE e.starts_at >= $1 AND e.starts_at < $2 \
           AND ($4::bool OR e.audience <> 'Bureau') \
           AND ($3::text IS NULL OR e.track = $3 OR e.track IS NULL) \
         ORDER BY e.starts_at"
    );
    let rows = sqlx::query_as::<_, EventRow>(&sql)
        .bind(from)
        .bind(to)
        .bind(track)
        .bind(bureau)
        .fetch_all(pool)
        .await?;
    Ok(rows)
}

/// The next few events, for the home page.
///
/// # Errors
/// Propagates database errors.
pub async fn upcoming(pool: &PgPool, limit: i64, bureau: bool) -> WebResult<Vec<EventRow>> {
    let sql = format!(
        "{SELECT} WHERE e.cancelled_at IS NULL AND e.ends_at >= NOW() \
           AND ($2::bool OR e.audience <> 'Bureau') \
         ORDER BY e.starts_at LIMIT $1"
    );
    let rows = sqlx::query_as::<_, EventRow>(&sql)
        .bind(limit)
        .bind(bureau)
        .fetch_all(pool)
        .await?;
    Ok(rows)
}

/// One event.
///
/// # Errors
/// `NotFound` when the id does not exist.
pub async fn find(pool: &PgPool, id: Uuid) -> WebResult<EventRow> {
    let sql = format!("{SELECT} WHERE e.id = $1");
    sqlx::query_as::<_, EventRow>(&sql)
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or(WebError::NotFound)
}

/// Put an event on the calendar.
///
/// # Errors
/// `Validation` on a malformed event; otherwise propagates database errors.
pub async fn create(
    pool: &PgPool,
    channels: DiscordChannels,
    author: Uuid,
    new: &NewEvent,
) -> WebResult<Uuid> {
    validate(new)?;
    let mut tx = pool.begin().await?;
    let id: Uuid = sqlx::query_scalar(
        r"
        INSERT INTO events (title, description, kind, track, audience, starts_at,
                            ends_at, location, xp_reward, created_by)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
        RETURNING id
        ",
    )
    .bind(new.title.trim())
    .bind(new.description.as_deref().map(str::trim))
    .bind(&new.kind)
    .bind(&new.track)
    .bind(&new.audience)
    .bind(new.starts_at)
    .bind(new.ends_at)
    .bind(new.location.as_deref().map(str::trim))
    .bind(new.xp_reward)
    .bind(author)
    .fetch_one(&mut *tx)
    .await?;

    // Announced in the same transaction as the insert: an event that
    // exists but was never announced is an event nobody attends, and
    // the outbox is what makes the pair atomic.
    let when = new.starts_at.format("%d/%m/%Y à %H:%M").to_string();
    let place = new
        .location
        .as_deref()
        .map(str::trim)
        .filter(|l| !l.is_empty());

    let announcement = if new.audience == "Bureau" {
        Announcement::bureau_meeting(
            new.title.trim(),
            &when,
            place,
            new.description
                .as_deref()
                .map(str::trim)
                .filter(|d| !d.is_empty()),
        )
    } else {
        Announcement::event_scheduled(
            new.title.trim(),
            kind_label(&new.kind),
            &when,
            place,
            new.xp_reward,
            new.track.as_deref().and_then(Track::parse),
        )
    };
    notifications::enqueue(&mut tx, channels, &announcement).await?;

    tx.commit().await?;
    Ok(id)
}

/// French label for an event kind, for the Discord announcement.
fn kind_label(kind: &str) -> &'static str {
    match kind {
        "Workshop" => "Atelier",
        "Jam" => "Game jam",
        "Meeting" => "Réunion",
        "Deadline" => "Échéance",
        "Showcase" => "Présentation",
        _ => "Séance",
    }
}

/// Change an event that is already on the calendar.
///
/// # Errors
/// `Validation` on a malformed event, `NotFound` when the id is unknown.
pub async fn update(pool: &PgPool, id: Uuid, new: &NewEvent) -> WebResult<()> {
    validate(new)?;
    let affected = sqlx::query(
        r"
        UPDATE events
           SET title = $2, description = $3, kind = $4, track = $5, audience = $6,
               starts_at = $7, ends_at = $8, location = $9, xp_reward = $10
         WHERE id = $1
        ",
    )
    .bind(id)
    .bind(new.title.trim())
    .bind(new.description.as_deref().map(str::trim))
    .bind(&new.kind)
    .bind(&new.track)
    .bind(&new.audience)
    .bind(new.starts_at)
    .bind(new.ends_at)
    .bind(new.location.as_deref().map(str::trim))
    .bind(new.xp_reward)
    .execute(pool)
    .await?
    .rows_affected();

    if affected == 0 {
        return Err(WebError::NotFound);
    }
    Ok(())
}

/// Call an event off, or put it back on.
///
/// Toggling rather than deleting, because attendance already recorded
/// against the event has to keep pointing somewhere.
///
/// # Errors
/// `NotFound` when the id is unknown.
pub async fn set_cancelled(
    pool: &PgPool,
    channels: DiscordChannels,
    id: Uuid,
    cancelled: bool,
) -> WebResult<()> {
    let row = find(pool, id).await?;

    let mut tx = pool.begin().await?;
    sqlx::query(
        "UPDATE events SET cancelled_at = CASE WHEN $2 THEN NOW() ELSE NULL END WHERE id = $1",
    )
    .bind(id)
    .bind(cancelled)
    .execute(&mut *tx)
    .await?;

    // Only a cancellation is worth interrupting people for. Putting an
    // event back on is a correction, and announcing it would train
    // everyone to ignore the channel.
    if cancelled {
        let when = row.starts_at.format("%d/%m/%Y à %H:%M").to_string();
        let announcement = if row.audience == "Bureau" {
            Announcement::bureau_meeting_cancelled(&row.title, &when)
        } else {
            Announcement::event_cancelled(
                &row.title,
                &when,
                row.track.as_deref().and_then(Track::parse),
            )
        };
        notifications::enqueue(&mut tx, channels, &announcement).await?;
    }

    tx.commit().await?;
    Ok(())
}

/// One member who turned up.
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct Attendee {
    /// Member id.
    pub user_id: Uuid,
    /// Name to show.
    pub display_name: String,
    /// When they scanned in.
    pub scanned_at: chrono::DateTime<chrono::Utc>,
    /// XP they were given for it.
    pub xp_rewarded: i32,
}

/// Who attended an event, earliest scan first.
///
/// # Errors
/// Propagates database errors.
pub async fn attendees(pool: &PgPool, event_id: Uuid) -> WebResult<Vec<Attendee>> {
    let rows = sqlx::query_as::<_, Attendee>(
        r"
        SELECT a.user_id,
               member_display_name(u.current_title, u.discord_global_name,
                                   u.discord_username, u.discord_id) AS display_name,
               a.scanned_at,
               a.xp_rewarded
          FROM attendance a
          JOIN users u ON u.id = a.user_id
         WHERE a.event_id = $1
         ORDER BY a.scanned_at
        ",
    )
    .bind(event_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Remove an event outright.
///
/// Refused once anybody has scanned in: that attendance is somebody's
/// XP and somebody's record of having been there, and deleting the
/// event would either orphan it or take it away. Cancelling is the
/// answer in that case — the event stays, visibly called off.
///
/// # Errors
/// `Validation` when the event has attendance, `NotFound` when the id is
/// unknown.
pub async fn delete(pool: &PgPool, id: Uuid) -> WebResult<()> {
    let attended: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM attendance WHERE event_id = $1")
            .bind(id)
            .fetch_one(pool)
            .await?;
    if attended > 0 {
        return Err(WebError::Validation(format!(
            "{attended} personne(s) ont déjà scanné pour cet événement : annulez-le au lieu de le supprimer."
        )));
    }

    let affected = sqlx::query("DELETE FROM events WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?
        .rows_affected();

    if affected == 0 {
        return Err(WebError::NotFound);
    }
    Ok(())
}

/// A track's own upcoming sessions.
///
/// Deliberately excludes the association-wide events that
/// [`in_window`] folds in: the track page is about the track, and the
/// general assembly already appears on the main calendar.
///
/// # Errors
/// Propagates database errors.
pub async fn upcoming_for_track(pool: &PgPool, track: &str, limit: i64) -> WebResult<Vec<EventRow>> {
    let sql = format!(
        "{SELECT} WHERE e.track = $1 AND e.audience = 'Track' \
           AND e.cancelled_at IS NULL AND e.ends_at >= NOW() \
         ORDER BY e.starts_at LIMIT $2"
    );
    let rows = sqlx::query_as::<_, EventRow>(&sql)
        .bind(track)
        .bind(limit)
        .fetch_all(pool)
        .await?;
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> NewEvent {
        let now = chrono::Utc::now();
        NewEvent {
            title: "Séance du mardi".into(),
            description: None,
            kind: "Session".into(),
            track: None,
            audience: "Association".into(),
            starts_at: now,
            ends_at: now + chrono::Duration::hours(2),
            location: Some("Salle 204".into()),
            xp_reward: 20,
        }
    }

    #[test]
    fn accepts_a_plain_session() {
        assert!(validate(&sample()).is_ok());
    }

    #[test]
    fn rejects_an_empty_or_oversized_title() {
        let mut e = sample();
        e.title = "   ".into();
        assert!(validate(&e).is_err());
        e.title = "x".repeat(121);
        assert!(validate(&e).is_err());
    }

    #[test]
    fn rejects_an_unknown_kind() {
        let mut e = sample();
        e.kind = "Party".into();
        assert!(validate(&e).is_err());
    }

    #[test]
    fn a_track_event_must_name_a_track_it_recognises() {
        let mut e = sample();
        e.audience = "Track".into();

        e.track = Some("Cooking".into());
        assert!(validate(&e).is_err(), "unknown track");

        e.track = None;
        assert!(validate(&e).is_err(), "track scope with no track");

        e.track = Some("Audio".into());
        assert!(validate(&e).is_ok());
    }

    #[test]
    fn a_non_track_event_must_not_carry_one() {
        // Otherwise a Bureau meeting could be filed under Audio and
        // would surface on the Audio track's agenda — which is exactly
        // the leak the audience column exists to prevent.
        for audience in ["Association", "Bureau"] {
            let mut e = sample();
            e.audience = audience.into();
            e.track = Some("Audio".into());
            assert!(validate(&e).is_err(), "{audience}");

            e.track = None;
            assert!(validate(&e).is_ok(), "{audience}");
        }
    }

    #[test]
    fn rejects_an_audience_nobody_defined() {
        let mut e = sample();
        e.audience = "Everyone".into();
        assert!(validate(&e).is_err());
    }

    #[test]
    fn rejects_an_event_that_ends_before_it_starts() {
        let mut e = sample();
        e.ends_at = e.starts_at - chrono::Duration::minutes(1);
        assert!(validate(&e).is_err());
    }

    #[test]
    fn a_deadline_may_be_worth_nothing_but_not_a_fortune() {
        let mut e = sample();
        e.xp_reward = 0;
        assert!(validate(&e).is_ok());
        e.xp_reward = 501;
        assert!(validate(&e).is_err());
        e.xp_reward = -1;
        assert!(validate(&e).is_err());
    }

    #[test]
    fn every_kind_the_module_advertises_validates() {
        for kind in KINDS {
            let mut e = sample();
            e.kind = kind.to_string();
            assert!(validate(&e).is_ok(), "{kind}");
        }
    }
}

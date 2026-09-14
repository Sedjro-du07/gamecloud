//! Outbound Discord announcements.
//!
//! The bot has always polled `notifications_outbox` and dispatched
//! whatever it finds — but until now nothing on the web side ever wrote
//! a row, so the whole announcement path was inert. This module is the
//! missing writer.
//!
//! Enqueueing participates in the caller's transaction. That matters:
//! an announcement must commit atomically with the event that caused
//! it, so the platform can never award a rank and then fail to announce
//! it (or announce a rank whose grant rolled back).

use gamecloud_shared::roles::Track;
use serde_json::json;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use crate::{config::DiscordChannels, error::WebResult};

/// A message destined for Discord.
#[derive(Debug, Clone)]
pub struct Announcement {
    /// Row `kind`, used by the bot for future per-kind rendering and as
    /// the embed title fallback.
    pub kind: &'static str,
    /// Embed title.
    pub title: String,
    /// Embed body.
    pub description: String,
    /// Embed accent colour as a packed RGB integer.
    pub color: u32,
    /// Member the announcement is about, when there is one.
    pub user_id: Option<Uuid>,
    /// Track the announcement concerns, when it is track-scoped. Drives
    /// routing: a review request belongs in that discipline's channel.
    pub track: Option<Track>,
}

impl Announcement {
    /// A member reached a new global rank.
    #[must_use]
    pub fn rank_up(user_id: Uuid, display: &str, rank_title: &str, ring_color_hex: &str) -> Self {
        Self {
            kind: "RankUp",
            title: "⬆️ Nouveau rang !".to_string(),
            description: format!("**{display}** atteint le rang **{rank_title}**."),
            color: hex_to_rgb(ring_color_hex).unwrap_or(0x9c_4dff),
            user_id: Some(user_id),
            track: None,
        }
    }

    /// A member unlocked a badge.
    #[must_use]
    pub fn badge(user_id: Uuid, display: &str, badge_title: &str, badge_desc: &str) -> Self {
        Self {
            kind: "BadgeAwarded",
            title: "🎖️ Badge débloqué".to_string(),
            description: format!("**{display}** débloque **{badge_title}** — {badge_desc}"),
            color: 0xff_d700,
            user_id: Some(user_id),
            track: None,
        }
    }

    /// A member completed a quest.
    #[must_use]
    pub fn quest(user_id: Uuid, display: &str, quest_title: &str, xp: i32) -> Self {
        Self {
            kind: "QuestCompleted",
            title: "📜 Quête accomplie".to_string(),
            description: format!("**{display}** termine « {quest_title} » — **+{xp} XP**."),
            color: 0x00_f2ff,
            user_id: Some(user_id),
            track: None,
        }
    }

    /// A project reached `Released`.
    #[must_use]
    pub fn project_released(project: &str, track: &str, contributors: usize) -> Self {
        Self {
            kind: "ProjectReleased",
            title: "🚀 Projet publié".to_string(),
            description: format!(
                "**{project}** ({track}) passe en *Released* — {contributors} contributeur(s) récompensé(s)."
            ),
            color: 0x34_d058,
            user_id: None,
            track: None,
        }
    }

    /// A member joined a track.
    #[must_use]
    pub fn track_joined(user_id: Uuid, display: &str, track_title: &str) -> Self {
        Self {
            kind: "TrackJoined",
            title: "🧭 Nouvelle track".to_string(),
            description: format!("**{display}** rejoint la track **{track_title}**."),
            color: 0x3f_a9ff,
            user_id: Some(user_id),
            track: None,
        }
    }
}

impl Announcement {
    /// A project needs one track's verdict.
    ///
    /// Emitted once per concerned track, so each discipline is asked in
    /// its own channel rather than everyone being shouted at in one
    /// queue.
    #[must_use]
    pub fn review_requested(project: &str, track: Track, author: &str) -> Self {
        Self {
            kind: "ProjectSubmitted",
            title: "🔍 Revue demandée".to_string(),
            description: format!(
                "**{project}** attend l'avis de la track **{}** (soumis par {author}).\n                 Un refus doit expliquer ce qui doit changer.",
                track.as_str()
            ),
            color: hex_to_rgb(track.color_hex()).unwrap_or(0xff_aa00),
            user_id: None,
            track: Some(track),
        }
    }

    /// Somebody submitted a resource for validation.
    #[must_use]
    pub fn resource_submitted(title: &str, author: &str, track: Option<Track>) -> Self {
        Self {
            kind: "ResourceSubmitted",
            title: "📚 Ressource à valider".to_string(),
            description: format!("**{author}** propose « {title} »."),
            color: 0x00_ff88,
            user_id: None,
            track,
        }
    }

    /// The Bureau opened a quest.
    #[must_use]
    pub fn quest_opened(title: &str, condition: &str, xp: i32, ends: &str) -> Self {
        Self {
            kind: "QuestOpened",
            title: "🎯 Nouvelle quête".to_string(),
            description: format!("**{title}**\n{condition} — **+{xp} XP**\nJusqu'au {ends}."),
            color: 0xbf_00ff,
            user_id: None,
            track: None,
        }
    }

    /// An action worth recording in the Bureau's journal.
    ///
    /// Never falls back to a public channel: an audit line names who did
    /// what to whom, so if no journal is configured it is dropped rather
    /// than aired.
    #[must_use]
    pub fn audit(actor: &str, action: &str, detail: &str) -> Self {
        Self {
            kind: "AuditEntry",
            title: "🏛 Journal".to_string(),
            description: format!("**{actor}** — {action}\n{detail}"),
            color: 0x9a_a0b3,
            user_id: None,
            track: None,
        }
    }
}

/// Parse `#rrggbb` into a packed integer.
fn hex_to_rgb(hex: &str) -> Option<u32> {
    let cleaned = hex.strip_prefix('#').unwrap_or(hex);
    if cleaned.len() != 6 {
        return None;
    }
    u32::from_str_radix(cleaned, 16).ok()
}

/// Write an announcement into the outbox inside an existing transaction.
///
/// The destination is chosen from the announcement's kind — releases to
/// the Hall of Fame, review requests to the validation queue, audit
/// entries to the Bureau's journal. When the deployment has configured
/// no channel for a kind, the call is a no-op: the platform keeps
/// working and simply stays quiet, which is the right failure mode for
/// a cosmetic feature.
///
/// # Errors
/// Propagates database errors.
pub async fn enqueue(
    tx: &mut Transaction<'_, Postgres>,
    channels: DiscordChannels,
    announcement: &Announcement,
) -> WebResult<()> {
    let Some(channel_id) = channels.route(announcement.kind, announcement.track) else {
        tracing::debug!(
            kind = announcement.kind,
            "no announce channel configured; skipping"
        );
        return Ok(());
    };

    let payload = json!({
        "channel_id": channel_id,
        "title": announcement.title,
        "description": announcement.description,
        "color": announcement.color,
    });

    sqlx::query(
        r#"
        INSERT INTO notifications_outbox (user_id, kind, payload, status)
        VALUES ($1, $2, $3, 'Pending')
        "#,
    )
    .bind(announcement.user_id)
    .bind(announcement.kind)
    .bind(payload)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hex_colors_with_and_without_hash() {
        assert_eq!(hex_to_rgb("#00f2ff"), Some(0x00_f2ff));
        assert_eq!(hex_to_rgb("00f2ff"), Some(0x00_f2ff));
    }

    #[test]
    fn rejects_malformed_hex() {
        assert_eq!(hex_to_rgb("#fff"), None);
        assert_eq!(hex_to_rgb("not-a-color"), None);
        assert_eq!(hex_to_rgb(""), None);
    }

    #[test]
    fn rank_up_falls_back_to_brand_purple_on_a_bad_color() {
        let a = Announcement::rank_up(Uuid::nil(), "Ada", "🌱 L'Initié", "nonsense");
        assert_eq!(a.color, 0x9c_4dff);
    }

    #[test]
    fn announcements_name_the_member_they_concern() {
        let id = Uuid::from_u128(7);
        assert_eq!(Announcement::badge(id, "Ada", "t", "d").user_id, Some(id));
        // A release is about the project, not one member.
        assert_eq!(
            Announcement::project_released("Aevaryn", "Narrative", 3).user_id,
            None
        );
    }
}

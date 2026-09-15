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
    /// Send this to the member privately instead of a channel.
    ///
    /// For anything that is nobody else's business: the feedback on a
    /// rejected project, an XP adjustment, an appointment. A public
    /// channel is the wrong place to tell somebody their work was
    /// refused.
    pub dm: bool,
    /// Who should be pinged, if anyone.
    ///
    /// Carried as an intent rather than a Discord role id, because the
    /// web process has no business knowing role ids — the bot reads the
    /// guild and is the only side that can resolve one correctly. It
    /// also means a renamed or recreated role fixes itself.
    pub mention: Mention,
}

/// Who an announcement should ping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mention {
    /// Ping nobody. The default, and right for most announcements: a
    /// rank-up is pleasant, not urgent.
    #[default]
    Nobody,
    /// Ping the whole association.
    Everyone,
    /// Ping the role belonging to the announcement's track.
    Track,
    /// Ping the Bureau role.
    Bureau,
    /// Ping the member the announcement is about.
    ///
    /// Requires `user_id` to be set; the bot resolves it to a Discord id
    /// itself, because the web process stores platform ids and only the
    /// bot knows how to address somebody on Discord.
    Member,
}

impl Mention {
    /// The wire form the bot reads back.
    const fn as_str(self) -> &'static str {
        match self {
            Self::Nobody => "none",
            Self::Everyone => "everyone",
            Self::Track => "track",
            Self::Bureau => "bureau",
            Self::Member => "member",
        }
    }
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
            mention: Mention::Member,
            dm: false,
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
            mention: Mention::Member,
            dm: false,
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
            mention: Mention::Member,
            dm: false,
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
            mention: Mention::Nobody,
            dm: false,
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
            mention: Mention::Nobody,
            dm: false,
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
            mention: Mention::Nobody,
            dm: false,
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
            mention: Mention::Nobody,
            dm: false,
        }
    }

    /// An event went on the calendar.
    ///
    /// This is one of the few announcements that pings, because it is
    /// one of the few that is *actionable*: a session nobody knows
    /// about is a session nobody attends. Scoped events ping their own
    /// track's role; association-wide ones ping everyone.
    #[must_use]
    pub fn event_scheduled(
        title: &str,
        kind_label: &str,
        when: &str,
        location: Option<&str>,
        xp: i32,
        track: Option<Track>,
    ) -> Self {
        use std::fmt::Write as _;

        let mut description = format!("**{title}**\n🗓️ {when}");
        if let Some(place) = location {
            let _ = write!(description, "\n📍 {place}");
        }
        if xp > 0 {
            let _ = write!(
                description,
                "\n⭐ {xp} XP de présence — scannez le code sur place."
            );
        }
        Self {
            kind: "EventScheduled",
            title: format!("📅 {kind_label} au calendrier"),
            description,
            color: 0x00_c2ff,
            user_id: None,
            track,
            mention: if track.is_some() {
                Mention::Track
            } else {
                Mention::Everyone
            },
            dm: false,
        }
    }

    /// A track rendered its verdict on somebody's project.
    ///
    /// Sent privately. A rejection carries the reason it was rejected,
    /// and telling somebody in public that their work was refused is a
    /// different act from telling them.
    #[must_use]
    pub fn verdict_rendered(
        author: Uuid,
        project: &str,
        track: Track,
        verdict: &str,
        feedback: Option<&str>,
        score: Option<i32>,
    ) -> Self {
        use std::fmt::Write as _;

        let (label, color) = match verdict {
            "Approved" => ("✅ approuvé", 0x34_d058),
            "Rejected" => ("❌ refusé", 0xff_003c),
            _ => ("— non applicable", 0x9a_a0b3),
        };
        let mut description = format!(
            "La track **{}** a {label} votre projet **{project}**.",
            track.as_str()
        );
        if let Some(n) = score {
            let _ = write!(description, "\n\n**Note : {n}/100**");
        }
        if let Some(f) = feedback {
            let _ = write!(description, "\n\n**Retour de l'équipe**\n{f}");
        }
        Self {
            kind: "VerdictRendered",
            title: "🔍 Verdict sur votre projet".to_string(),
            description,
            color,
            user_id: Some(author),
            track: Some(track),
            mention: Mention::Member,
            dm: true,
        }
    }

    /// Somebody was appointed Lead or CoLead of a track.
    #[must_use]
    pub fn track_appointment(member: Uuid, track: Track, role: &str) -> Self {
        let label = match role {
            "Lead" => "responsable",
            "CoLead" => "co-responsable",
            _ => "observateur",
        };
        Self {
            kind: "TrackAppointment",
            title: "🎖️ Nomination".to_string(),
            description: format!(
                "Vous êtes désormais **{label}** de la track **{}**.",
                track.as_str()
            ),
            color: 0x00_f2ff,
            user_id: Some(member),
            track: Some(track),
            mention: Mention::Member,
            dm: true,
        }
    }

    /// The Bureau adjusted somebody's XP by hand.
    ///
    /// Private, and it always names the reason: an unexplained XP change
    /// is the fastest way to make a scoring system feel arbitrary.
    #[must_use]
    pub fn manual_xp(member: Uuid, amount: i32, reason: &str) -> Self {
        let (verb, color) = if amount >= 0 {
            ("crédité de", 0x34_d058)
        } else {
            ("débité de", 0xff_aa00)
        };
        Self {
            kind: "ManualXp",
            title: "⚖️ Ajustement d'XP".to_string(),
            description: format!(
                "Le Bureau vous a {verb} **{} XP**.\n\n**Motif**\n{reason}",
                amount.abs()
            ),
            color,
            user_id: Some(member),
            track: None,
            mention: Mention::Member,
            dm: true,
        }
    }

    /// Somebody was credited on a project.
    #[must_use]
    pub fn contributor_added(member: Uuid, project: &str, track: Track, role: &str) -> Self {
        Self {
            kind: "ContributorAdded",
            title: "🤝 Vous êtes crédité·e".to_string(),
            description: format!(
                "Vous avez été ajouté·e au projet **{project}**                  comme **{role}** sur la track **{}**.",
                track.as_str()
            ),
            color: 0x00_f2ff,
            user_id: Some(member),
            track: Some(track),
            mention: Mention::Member,
            dm: true,
        }
    }

    /// A submitted resource was accepted or turned down.
    #[must_use]
    pub fn resource_reviewed(member: Uuid, title: &str, accepted: bool) -> Self {
        Self {
            kind: "ResourceReviewed",
            title: if accepted {
                "📚 Ressource validée".to_string()
            } else {
                "📚 Ressource écartée".to_string()
            },
            description: if accepted {
                format!("Votre proposition « {title} » rejoint la bibliothèque.")
            } else {
                format!("Votre proposition « {title} » n'a pas été retenue.")
            },
            color: if accepted { 0x34_d058 } else { 0x9a_a0b3 },
            user_id: Some(member),
            track: None,
            mention: Mention::Member,
            dm: true,
        }
    }

    /// A member shared a file or a link.
    ///
    /// Points at the platform rather than at the file: downloading is for
    /// members, and the page is where that is checked and counted.
    #[must_use]
    pub fn share_posted(
        title: &str,
        kind_label: &str,
        author: &str,
        description: Option<&str>,
        detail: &str,
        page_url: &str,
    ) -> Self {
        use std::fmt::Write as _;

        let mut body = format!("**{author}** partage **{title}** · {kind_label}\n{detail}");
        if let Some(text) = description.map(str::trim).filter(|d| !d.is_empty()) {
            let short: String = text.chars().take(300).collect();
            let more = if text.chars().count() > 300 { "…" } else { "" };
            let _ = write!(body, "\n\n{short}{more}");
        }
        let _ = write!(body, "\n\n⬇️ À télécharger sur la plateforme : {page_url}");
        Self {
            kind: "SharePosted",
            title: "📦 Nouveau partage".to_string(),
            description: body,
            color: 0x00_f2ff,
            user_id: None,
            track: None,
            mention: Mention::Nobody,
            dm: false,
        }
    }

    /// A resource was validated and joins the library.
    ///
    /// Only validated resources are announced: until then the library
    /// hides them from members, and the channel must not show what the
    /// page does not.
    #[must_use]
    pub fn resource_published(
        title: &str,
        url: &str,
        kind: Option<&str>,
        author: &str,
        tracks: &[String],
    ) -> Self {
        use std::fmt::Write as _;

        let kind = match kind {
            Some("Tutorial") => "Tutoriel",
            Some("Tool") => "Outil",
            Some("Asset") => "Asset",
            Some("Doc") => "Documentation",
            Some("Video") => "Vidéo",
            _ => "Lien",
        };
        let mut body = format!("**{title}**\n🔗 {url}\n\n{kind} · proposé par {author}");
        if !tracks.is_empty() {
            let _ = write!(body, " · {}", tracks.join(", "));
        }
        Self {
            kind: "ResourcePublished",
            title: "📚 Nouvelle ressource".to_string(),
            description: body,
            color: 0x00_ff88,
            user_id: None,
            track: None,
            mention: Mention::Nobody,
            dm: false,
        }
    }

    /// A meeting was called for the Bureau.
    ///
    /// Separate from [`Self::event_scheduled`] rather than a flag on it,
    /// because it is routed to a different channel, pings a different
    /// role, and must never fall back to the public announce channel.
    /// Folding the two together would make that fallback one careless
    /// edit away.
    #[must_use]
    pub fn bureau_meeting(
        title: &str,
        when: &str,
        location: Option<&str>,
        agenda: Option<&str>,
    ) -> Self {
        use std::fmt::Write as _;

        let mut description = format!("**{title}**\n🗓️ {when}");
        if let Some(place) = location {
            let _ = write!(description, "\n📍 {place}");
        }
        if let Some(agenda) = agenda {
            let _ = write!(description, "\n\n**Ordre du jour**\n{agenda}");
        }
        Self {
            kind: "BureauMeeting",
            title: "🏛️ Réunion du Bureau".to_string(),
            description,
            color: 0xff_aa00,
            user_id: None,
            track: None,
            mention: Mention::Bureau,
            dm: false,
        }
    }

    /// A Bureau meeting was called off.
    #[must_use]
    pub fn bureau_meeting_cancelled(title: &str, when: &str) -> Self {
        Self {
            kind: "BureauMeetingCancelled",
            title: "🏛️ Réunion du Bureau annulée".to_string(),
            description: format!("**{title}** ({when}) n'aura pas lieu."),
            color: 0xff_003c,
            user_id: None,
            track: None,
            mention: Mention::Bureau,
            dm: false,
        }
    }

    /// An event was called off.
    ///
    /// Pings for the same reason the scheduling did: somebody has
    /// planned their afternoon around it.
    #[must_use]
    pub fn event_cancelled(title: &str, when: &str, track: Option<Track>) -> Self {
        Self {
            kind: "EventCancelled",
            title: "🚫 Événement annulé".to_string(),
            description: format!("**{title}** ({when}) n'aura pas lieu."),
            color: 0xff_003c,
            user_id: None,
            track,
            mention: if track.is_some() {
                Mention::Track
            } else {
                Mention::Everyone
            },
            dm: false,
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
            mention: Mention::Nobody,
            dm: false,
        }
    }

    /// A member's attendance was recorded by QR scan.
    ///
    /// Public and silent: it lands in the attendance channel, pings
    /// nobody, and lets organisers watch the room fill up in real time.
    #[must_use]
    pub fn attendance(user_id: Uuid, display: &str, event: &str, xp: i32) -> Self {
        let reward = if xp > 0 {
            format!(" — **+{xp} XP**")
        } else {
            String::new()
        };
        Self {
            kind: "AttendanceRecorded",
            title: "✅ Présence".to_string(),
            description: format!("**{display}** est présent·e à **{event}**{reward}."),
            color: 0x34_d058,
            user_id: Some(user_id),
            track: None,
            mention: Mention::Nobody,
            dm: false,
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
            mention: Mention::Nobody,
            dm: false,
        }
    }
}

/// Ask the bot to bring Discord roles in line with the platform now,
/// rather than at its next periodic pass.
///
/// Offices, tracks and ranks are decided on the platform and Discord
/// only mirrors them. The row posts nothing: the bot handles `RoleSync`
/// silently, and several requests in one batch cost a single pass.
///
/// # Errors
/// Propagates database errors.
pub async fn request_role_sync<'e>(executor: impl sqlx::PgExecutor<'e>) -> WebResult<()> {
    sqlx::query(
        "INSERT INTO notifications_outbox (kind, payload, status) \
         VALUES ('RoleSync', '{}', 'Pending')",
    )
    .execute(executor)
    .await?;
    Ok(())
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
        "mention": announcement.mention.as_str(),
        "dm": announcement.dm,
        "user_id": announcement.user_id,
        "track": announcement.track.map(Track::as_str),
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
    fn a_share_announcement_points_members_to_the_platform() {
        let a = Announcement::share_posted(
            "Pack forêt",
            "Assets",
            "Ada",
            Some(&"x".repeat(400)),
            "📁 foret.zip · 2,0 Mo",
            "https://gamecloud.example/shares",
        );
        assert_eq!(a.kind, "SharePosted");
        assert_eq!(a.mention, Mention::Nobody);
        assert!(a.description.contains("https://gamecloud.example/shares"));
        // A long description is cut, so one upload cannot fill the channel.
        assert!(a.description.contains(&format!("{}…", "x".repeat(300))));
        assert!(!a.description.contains(&"x".repeat(301)));
    }

    #[test]
    fn a_resource_announcement_carries_its_link() {
        let a = Announcement::resource_published(
            "Shaders Godot",
            "https://docs.godotengine.org",
            Some("Tutorial"),
            "Ada",
            &["VisualArt".to_string()],
        );
        assert_eq!(a.kind, "ResourcePublished");
        assert!(a.description.contains("https://docs.godotengine.org"));
        assert!(a.description.contains("Tutoriel"));
        assert!(a.description.contains("VisualArt"));
    }

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

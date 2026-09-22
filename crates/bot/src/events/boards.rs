//! Channels the bot keeps current rather than posts into.
//!
//! `🏅gc-classement` and `📖gc-guide` were created with the platform, but
//! nothing ever wrote to them, so they sat empty. A leaderboard that
//! arrives as a new message every refresh is a channel people mute; one
//! message edited in place is one they can glance at.

use std::fmt::Write as _;

use gamecloud_shared::roles::{BureauRole, GlobalRank, Track};
use serenity::all::{
    Channel, ChannelId, CreateEmbed, CreateEmbedFooter, CreateMessage, EditMessage, GetMessages,
    GuildId, Http, PermissionOverwrite, PermissionOverwriteType, Permissions, RoleId, Timestamp,
};

use crate::state::BotState;

/// Embed title of the leaderboard message. Also how the bot finds its
/// own message again, so changing it leaves the old one behind.
const LEADERBOARD_TITLE: &str = "🏅 Classement GameCloud";

/// Embed title of the guide message.
const GUIDE_TITLE: &str = "📖 Guide de GameCloud OS";

/// Refresh every board that has a channel configured.
pub async fn refresh(state: &BotState, http: &Http) {
    let config = state.config();
    if let Some(id) = config.leaderboard_channel_id {
        if let Some(guild) = config.guild_id {
            members_only(http, GuildId::new(guild), ChannelId::new(id)).await;
        }
        let embed = leaderboard(state).await;
        upsert(http, ChannelId::new(id), LEADERBOARD_TITLE, embed).await;
    }
    if let Some(id) = config.guide_channel_id {
        upsert(http, ChannelId::new(id), GUIDE_TITLE, guide(state)).await;
    }
}

/// Keep `channel` readable by registered members only.
///
/// The leaderboard names members and their XP, so the server at large
/// should not read it. `@everyone` loses the right to see the channel,
/// and every member-title role gets it back — those roles are exactly the
/// people the platform recognises, and the bot keeps them in step. Nobody
/// but the bot may post.
///
/// Checked on every pass, so a title role recreated by hand, or a
/// permission changed in Discord, is put back. When no title role can be
/// read, nothing is touched: locking the channel with nobody let in
/// would be worse than leaving it open.
async fn members_only(http: &Http, guild: GuildId, channel: ChannelId) {
    let titles = super::roles::rank_roles(http, guild).await;
    if titles.is_empty() {
        return;
    }
    let current = match channel.to_channel(http).await {
        Ok(Channel::Guild(c)) => c.permission_overwrites,
        Ok(_) => return,
        Err(e) => {
            tracing::warn!(error = %e, %channel, "boards: could not read channel permissions");
            return;
        }
    };

    // `@everyone` shares the guild's id.
    let mut wanted = vec![PermissionOverwrite {
        allow: Permissions::empty(),
        deny: Permissions::VIEW_CHANNEL | Permissions::SEND_MESSAGES,
        kind: PermissionOverwriteType::Role(RoleId::new(guild.get())),
    }];
    wanted.extend(
        GlobalRank::ALL
            .iter()
            // `Pending` is no longer assigned to anyone; kept out so a
            // stale role left on the server opens nothing.
            .filter(|rank| **rank != GlobalRank::Pending)
            .filter_map(|rank| titles.get(rank.title()))
            .map(|role| PermissionOverwrite {
                allow: Permissions::VIEW_CHANNEL,
                deny: Permissions::empty(),
                kind: PermissionOverwriteType::Role(*role),
            }),
    );

    for overwrite in wanted {
        let in_place = current
            .iter()
            .any(|o| o.kind == overwrite.kind && o.allow == overwrite.allow && o.deny == overwrite.deny);
        if in_place {
            continue;
        }
        if let Err(e) = channel.create_permission(http, overwrite).await {
            tracing::warn!(error = %e, %channel, "boards: could not set channel permission");
        }
    }
}

/// Edit the bot's message titled `title` in `channel`, or post it.
async fn upsert(http: &Http, channel: ChannelId, title: &str, embed: CreateEmbed) {
    let me = match http.get_current_user().await {
        Ok(user) => user.id,
        Err(e) => {
            tracing::warn!(error = %e, "boards: could not read the bot's own user");
            return;
        }
    };
    let existing = match channel.messages(http, GetMessages::new().limit(50)).await {
        Ok(messages) => messages.into_iter().find(|m| {
            m.author.id == me && m.embeds.iter().any(|e| e.title.as_deref() == Some(title))
        }),
        Err(e) => {
            tracing::warn!(error = %e, %channel, "boards: could not read the channel");
            return;
        }
    };

    let result = match existing {
        Some(message) => channel
            .edit_message(http, message.id, EditMessage::new().embed(embed))
            .await
            .map(|_| ()),
        None => channel
            .send_message(http, CreateMessage::new().embed(embed))
            .await
            .map(|_| ()),
    };
    if let Err(e) = result {
        tracing::warn!(error = %e, %channel, "boards: could not update");
    }
}

/// The global top ten, and who leads each track.
async fn leaderboard(state: &BotState) -> CreateEmbed {
    let top: Vec<(String, i64, String, Option<String>)> = sqlx::query_as(
        "SELECT member_display_name(current_title, discord_global_name, \
                                    discord_username, discord_id), xp_total, global_rank, \
                bureau_role \
           FROM users WHERE NOT candidate AND xp_total > 0 \
          ORDER BY xp_total DESC LIMIT 10",
    )
    .fetch_all(state.pool())
    .await
    .unwrap_or_default();

    let leads: Vec<(String, String, i64)> = sqlx::query_as(
        "SELECT DISTINCT ON (m.track) m.track, \
                member_display_name(u.current_title, u.discord_global_name, \
                                    u.discord_username, u.discord_id), m.track_xp \
           FROM track_memberships m JOIN users u ON u.id = m.user_id \
          WHERE m.left_at IS NULL AND m.track_xp > 0 \
          ORDER BY m.track, m.track_xp DESC",
    )
    .fetch_all(state.pool())
    .await
    .unwrap_or_default();

    let mut body = String::new();
    if top.is_empty() {
        body.push_str("_Personne n'a encore marqué de points._");
    }
    for (i, (name, xp, rank, office)) in top.iter().enumerate() {
        let medal = ["🥇", "🥈", "🥉"].get(i).copied().unwrap_or("▫️");
        // Every title, most important first.
        let titles = office
            .as_deref()
            .and_then(BureauRole::parse)
            .map_or_else(
                || GlobalRank::parse(rank).title().to_string(),
                |o| format!("{} · {}", o.title(), GlobalRank::parse(rank).title()),
            );
        let _ = writeln!(body, "{medal} **{name}** — {xp} XP · {titles}");
    }

    let mut embed = CreateEmbed::new()
        .title(LEADERBOARD_TITLE)
        .description(body)
        .color(0xff_d700)
        .footer(CreateEmbedFooter::new("Tenu à jour par le bot · /leaderboard pour le détail"))
        .timestamp(Timestamp::now());

    if !leads.is_empty() {
        let lines = leads
            .iter()
            .map(|(track, name, xp)| {
                let emoji = Track::parse(track).map_or("•", Track::emoji);
                format!("{emoji} {track} — **{name}** ({xp} XP)")
            })
            .collect::<Vec<_>>()
            .join("\n");
        embed = embed.field("En tête de chaque track", lines, false);
    }
    embed
}

/// Where to find what, and how to talk to the bot.
fn guide(state: &BotState) -> CreateEmbed {
    let config = state.config();
    let mut channels = String::new();
    let mut line = |id: Option<u64>, what: &str| {
        if let Some(id) = id {
            let _ = writeln!(channels, "<#{id}> — {what}");
        }
    };
    line(
        config.announce_channel_id,
        "annonces de l'association, séances, rangs et badges",
    );
    line(config.quests_channel_id, "quêtes ouvertes");
    line(
        config.leaderboard_channel_id,
        "classement, tenu à jour (réservé aux membres inscrits)",
    );
    line(config.hall_channel_id, "projets publiés");
    line(
        config.shares_channel_id,
        "partages des membres : scripts, lore, jeux, assets",
    );
    line(config.resources_channel_id, "ressources validées et liens utiles");
    line(config.reviews_channel_id, "ressources en attente de validation");
    line(config.presences_channel_id, "présences enregistrées aux séances");
    if config.draftbot_enabled {
        line(
            Some(config.draftbot_channel_id),
            "niveaux DraftBot, reportés en XP sur la plateforme",
        );
    }

    CreateEmbed::new()
        .title(GUIDE_TITLE)
        .color(0xbf_00ff)
        .field("Où trouver quoi", channels, false)
        .field(
            "Tracks",
            "Chaque discipline a son salon dans 🧭 TRACKS : ses séances et les demandes \
             de revue y arrivent, en taguant le rôle de la track. Les tracks se \
             choisissent sur la plateforme, et le rôle suit ici.",
            false,
        )
        .field(
            "Rôles",
            "Tout suit votre progression : votre titre global monte avec votre XP, \
             et votre titre dans chaque track avec l'XP gagnée dans cette track. Seules \
             les tracks se choisissent. Les postes du Bureau sont attribués depuis la \
             plateforme et ouvrent le salon du Bureau.",
            false,
        )
        .field(
            "Notifications privées",
            "Verdicts sur vos projets, ajustements d'XP, nominations : en message privé. \
             Si vos MP sont fermés, le bot vous tague dans les annonces sans rien \
             révéler, et le détail reste sur la plateforme.",
            false,
        )
        .field(
            "Parler au bot",
            "Taguez-le : « mon profil », « le classement », « les quêtes », « l'agenda ».\n\
             Commandes : `/profil` `/xp` `/track` `/leaderboard` `/quetes` `/badges`",
            false,
        )
}

//! Replying when somebody tags the bot.
//!
//! Slash commands are discoverable but formal: you have to know the
//! command exists and then remember its name. Tagging is how people
//! actually address a bot in a conversation, and a bot that stays silent
//! when tagged reads as broken.
//!
//! The reply is *useful* rather than a menu. The message is skimmed for
//! an intent — profile, ranking, quests, agenda — and answered directly;
//! only when nothing matches does the bot list what it can do. Asking
//! somebody who just asked a question to go and read a help page is the
//! thing that makes bots annoying.

use gamecloud_shared::roles::GlobalRank;
use serenity::all::{Context, CreateEmbed, CreateMessage, Message};

use crate::state::BotState;

/// What the member seems to be asking for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Intent {
    /// Their own standing.
    Profile,
    /// The leaderboard.
    Ranking,
    /// Open quests.
    Quests,
    /// What is coming up on the calendar.
    Agenda,
    /// Nothing recognisable — offer the menu.
    Help,
}

/// Keywords that identify each intent, most specific first.
///
/// Order is meaningful: "classement xp" is a question about the board,
/// not about the asker's own total, so `Ranking` is tested before
/// `Profile`.
const TABLE: [(Intent, &[&str]); 4] = [
    (
        Intent::Ranking,
        &["classement", "leaderboard", "top", "podium", "rang du serveur"],
    ),
    (
        Intent::Agenda,
        &["agenda", "calendrier", "evenement", "seance", "prochaine", "quand"],
    ),
    (
        Intent::Quests,
        &["quete", "quest", "mission", "objectif", "defi"],
    ),
    (
        Intent::Profile,
        &["profil", "xp", "niveau", "titre", "rang", "serie", "streak", "stats", "moi"],
    ),
];

/// Read an intent out of the message text.
///
/// Deliberately generous: this is a chat message, not a command line.
/// Accents are folded and only substrings are matched, so "mes quêtes ?"
/// and "c koi les quetes" both land on the same answer.
fn intent_of(raw: &str) -> Intent {
    let text = fold(raw);

    for (intent, keys) in TABLE {
        if keys.iter().any(|k| text.contains(k)) {
            return intent;
        }
    }
    Intent::Help
}

/// Lowercase and strip the accents French keyboards may or may not carry.
fn fold(raw: &str) -> String {
    raw.to_lowercase()
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            other => other,
        })
        .collect()
}

/// Handle a message that mentions the bot.
///
/// Returns quietly for anything that is not addressed to us — including
/// `@everyone`, which technically "mentions" every member and would
/// otherwise make the bot answer every announcement it posts itself.
pub async fn on_message(ctx: &Context, msg: &Message, state: &BotState) {
    if msg.author.bot {
        return;
    }
    let me = ctx.cache.current_user().id;
    if !msg.mentions.iter().any(|u| u.id == me) {
        return;
    }

    let embed = match intent_of(&msg.content) {
        Intent::Profile => profile_embed(state, &msg.author.id.to_string(), &msg.author.name).await,
        Intent::Ranking => ranking_embed(state).await,
        Intent::Quests => quests_embed(state).await,
        Intent::Agenda => agenda_embed(state).await,
        Intent::Help => help_embed(),
    };

    // A reply rather than a plain message, so the answer stays attached
    // to the question in a busy channel.
    if let Err(e) = msg
        .channel_id
        .send_message(
            &ctx.http,
            CreateMessage::new().embed(embed).reference_message(msg),
        )
        .await
    {
        tracing::warn!(error = %e, "mention: could not reply");
    }
}

/// The asker's own standing.
async fn profile_embed(state: &BotState, discord_id: &str, name: &str) -> CreateEmbed {
    let row: Option<(i64, String, Option<String>, i32)> = sqlx::query_as(
        "SELECT xp_total, global_rank, current_title, streak_days \
           FROM users WHERE discord_id = $1",
    )
    .bind(discord_id)
    .fetch_optional(state.pool())
    .await
    .ok()
    .flatten();

    let Some((xp, rank, title, streak)) = row else {
        return CreateEmbed::new()
            .title("Pas encore de compte")
            .description(
                "Connecte-toi une fois sur GameCloud OS avec Discord et ton profil apparaîtra ici.",
            )
            .color(0xff_aa00);
    };

    let rank = GlobalRank::parse(&rank);
    CreateEmbed::new()
        .title(format!("Profil de {name}"))
        .description(title.unwrap_or_else(|| rank.title().to_string()))
        .color(0x00_f2ff)
        .field("Titre", rank.title(), true)
        .field("XP", xp.to_string(), true)
        .field(
            "Série",
            if streak > 0 {
                format!("{streak} j")
            } else {
                "—".to_string()
            },
            true,
        )
}

/// The top of the leaderboard.
async fn ranking_embed(state: &BotState) -> CreateEmbed {
    let rows: Vec<(String, i64)> = sqlx::query_as(
        "SELECT member_display_name(current_title, discord_global_name, \
                                    discord_username, discord_id), xp_total \
           FROM users WHERE xp_total > 0 ORDER BY xp_total DESC LIMIT 5",
    )
    .fetch_all(state.pool())
    .await
    .unwrap_or_default();

    if rows.is_empty() {
        return CreateEmbed::new()
            .title("🏅 Classement")
            .description("Personne n'a encore marqué de points.")
            .color(0x9a_a0b3);
    }

    let body = rows
        .iter()
        .enumerate()
        .map(|(i, (name, xp))| {
            let medal = ["🥇", "🥈", "🥉", "4.", "5."][i.min(4)];
            format!("{medal} **{name}** — {xp} XP")
        })
        .collect::<Vec<_>>()
        .join("\n");

    CreateEmbed::new()
        .title("🏅 Classement")
        .description(body)
        .color(0x00_f2ff)
}

/// Quests currently open.
async fn quests_embed(state: &BotState) -> CreateEmbed {
    let rows: Vec<(String, i32, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
        "SELECT title, xp_reward, ends_at FROM quests \
          WHERE starts_at <= NOW() AND ends_at >= NOW() ORDER BY ends_at LIMIT 5",
    )
    .fetch_all(state.pool())
    .await
    .unwrap_or_default();

    if rows.is_empty() {
        return CreateEmbed::new()
            .title("🎯 Quêtes")
            .description("Aucune quête ouverte pour l'instant.")
            .color(0x9a_a0b3);
    }

    let body = rows
        .iter()
        .map(|(title, xp, ends)| {
            format!(
                "• **{title}** — {xp} XP, jusqu'au {}",
                ends.format("%d/%m")
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    CreateEmbed::new()
        .title("🎯 Quêtes ouvertes")
        .description(body)
        .color(0x00_f2ff)
}

/// What is coming up.
///
/// Bureau meetings are excluded: this answers in whatever channel the
/// question was asked, which may well be a public one.
async fn agenda_embed(state: &BotState) -> CreateEmbed {
    let rows: Vec<(String, chrono::DateTime<chrono::Utc>, Option<String>, Option<String>)> =
        sqlx::query_as(
            "SELECT title, starts_at, location, track FROM events \
              WHERE cancelled_at IS NULL AND ends_at >= NOW() \
                AND audience <> 'Bureau' \
              ORDER BY starts_at LIMIT 5",
        )
        .fetch_all(state.pool())
        .await
        .unwrap_or_default();

    if rows.is_empty() {
        return CreateEmbed::new()
            .title("📅 À venir")
            .description("Rien de programmé pour le moment.")
            .color(0x9a_a0b3);
    }

    let body = rows
        .iter()
        .map(|(title, at, place, track)| {
            let mut line = format!("• **{title}** — {}", at.format("%d/%m à %H:%M"));
            if let Some(t) = track {
                line.push_str(&format!(" · {t}"));
            }
            if let Some(p) = place {
                line.push_str(&format!(" · {p}"));
            }
            line
        })
        .collect::<Vec<_>>()
        .join("\n");

    CreateEmbed::new()
        .title("📅 À venir")
        .description(body)
        .color(0x00_f2ff)
}

/// What the bot can be asked.
fn help_embed() -> CreateEmbed {
    CreateEmbed::new()
        .title("👋 Je suis GameCloud OS")
        .description(
            "Tague-moi avec ce que tu veux savoir :\n\
             • **mon profil**, **mon xp**, **mon rang**\n\
             • **le classement**\n\
             • **les quêtes**\n\
             • **l'agenda**, **la prochaine séance**\n\n\
             Les commandes `/profil`, `/xp`, `/track`, `/leaderboard`, \
             `/quetes` et `/badges` font la même chose en plus précis.",
        )
        .color(0xbf_00ff)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_a_request_for_the_ranking() {
        for q in ["le classement ?", "montre le TOP", "leaderboard stp"] {
            assert_eq!(intent_of(q), Intent::Ranking, "{q}");
        }
    }

    #[test]
    fn recognises_quests_with_or_without_accents() {
        assert_eq!(intent_of("les quêtes du moment"), Intent::Quests);
        assert_eq!(intent_of("c koi les quetes"), Intent::Quests);
    }

    #[test]
    fn recognises_the_agenda() {
        for q in ["l'agenda", "quand est la prochaine séance", "prochain évènement"] {
            assert_eq!(intent_of(q), Intent::Agenda, "{q}");
        }
    }

    #[test]
    fn recognises_a_request_about_oneself() {
        for q in ["mon profil", "j'ai combien d'xp", "mon niveau"] {
            assert_eq!(intent_of(q), Intent::Profile, "{q}");
        }
    }

    #[test]
    fn ranking_wins_over_profile_when_both_words_appear() {
        // "classement xp" is a question about the board, not about the
        // asker's own total, and the table's order is what decides it.
        assert_eq!(intent_of("le classement xp"), Intent::Ranking);
    }

    #[test]
    fn anything_else_offers_the_menu() {
        for q in ["salut", "", "tu fais quoi", "🎮"] {
            assert_eq!(intent_of(q), Intent::Help, "{q}");
        }
    }

    #[test]
    fn folding_leaves_plain_text_alone() {
        assert_eq!(fold("Quetes"), "quetes");
        assert_eq!(fold("Quêtes"), "quetes");
        assert_eq!(fold("ÉVÈNEMENT"), "evenement");
    }
}

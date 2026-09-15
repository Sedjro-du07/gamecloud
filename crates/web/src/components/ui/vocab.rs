//! The platform's vocabulary as the interface shows it.
//!
//! Tracks and titles carry no colour: a track is recognised by its icon,
//! its place in the list and its name. Titles lose the emoji their Discord
//! role names keep — the bot finds roles by that exact name, so the stored
//! title stays as it is and only its display is plain. Kinds, badges and
//! verdicts get an icon each for the same reason: the icon and the word say
//! it, not a colour.

use gamecloud_shared::roles::{Track, TrackRole};

use super::icon::IconName;

/// A track's name, in words rather than as its identifier.
#[must_use]
pub const fn track_label(track: Track) -> &'static str {
    match track {
        Track::Engineering => "Engineering",
        Track::GameDesign => "Game Design",
        Track::Narrative => "Narrative",
        Track::VisualArt => "Visual Art",
        Track::Audio => "Audio",
        Track::Production => "Production",
        Track::Qa => "QA",
        Track::Marketing => "Marketing",
    }
}

/// A track's icon.
#[must_use]
pub const fn track_icon(track: Track) -> IconName {
    match track {
        Track::Engineering => IconName::Code,
        Track::GameDesign => IconName::GameController,
        Track::Narrative => IconName::BookOpen,
        Track::VisualArt => IconName::PaintBrush,
        Track::Audio => IconName::MusicNotes,
        Track::Production => IconName::Kanban,
        Track::Qa => IconName::Bug,
        Track::Marketing => IconName::Megaphone,
    }
}

/// A track's name from its stored identifier; unknown ones read as stored.
#[must_use]
pub fn track_name(id: &str) -> String {
    Track::parse(id).map_or_else(|| id.to_string(), |t| track_label(t).to_string())
}

/// A track's icon from its stored identifier.
#[must_use]
pub fn track_icon_by_id(id: &str) -> Option<IconName> {
    Track::parse(id).map(track_icon)
}

/// Every track as `(identifier, name)`, for a select.
#[must_use]
pub fn track_choices() -> Vec<(&'static str, &'static str)> {
    Track::ALL.iter().map(|t| (t.as_str(), track_label(*t))).collect()
}

/// A title without its leading emoji: "🌙 Ancien de la Forge" reads
/// "Ancien de la Forge".
#[must_use]
pub fn plain_title(title: &str) -> &str {
    title
        .char_indices()
        .find(|(_, c)| c.is_alphanumeric() || matches!(c, '\'' | '’' | '«' | '"'))
        .map_or(title, |(i, _)| &title[i..])
}

/// The title of a stored track role, e.g. "Relecteur".
#[must_use]
pub fn role_title(stored: &str) -> &'static str {
    plain_title(TrackRole::title_of(stored))
}

/// French label for a project's lifecycle status.
#[must_use]
pub fn status_label(status: &str) -> &'static str {
    match status {
        "Draft" => "Brouillon",
        "InReview" => "En revue",
        "PartialOK" => "Partiellement validé",
        "Approved" => "Approuvé",
        "Released" => "Publié",
        "Archived" => "Archivé",
        "Rejected" => "Refusé",
        _ => "Inconnu",
    }
}

/// French label for a project's rarity tier.
#[must_use]
pub fn rarity_label(rarity: &str) -> &'static str {
    match rarity {
        "Rare" => "Rare",
        "Epic" => "Épique",
        "Legendary" => "Légendaire",
        "Mythic" => "Mythique",
        _ => "Commun",
    }
}

/// A track's verdict on a project: label and icon. Anything unrecognised
/// reads as waiting, never as an approval.
#[must_use]
pub fn verdict(status: &str) -> (&'static str, IconName) {
    match status {
        "Approved" => ("Approuvé", IconName::CheckCircle),
        "Rejected" => ("Refusé", IconName::XCircle),
        "NotApplicable" => ("Non applicable", IconName::MinusCircle),
        _ => ("En attente", IconName::Clock),
    }
}

/// A shared item's kind: stored value, label, icon.
pub const SHARE_KINDS: [(&str, &str, IconName); 5] = [
    ("Script", "Script", IconName::FileCode),
    ("Lore", "Lore", IconName::Scroll),
    ("Game", "Jeu", IconName::Joystick),
    ("Asset", "Assets", IconName::Image),
    ("Other", "Autre", IconName::Package),
];

/// The icon of a shared item's kind.
#[must_use]
pub fn share_kind_icon(kind: &str) -> IconName {
    SHARE_KINDS
        .iter()
        .find(|(value, _, _)| *value == kind)
        .map_or(IconName::Package, |(_, _, icon)| *icon)
}

/// A resource's kind: stored value, label, icon.
pub const RESOURCE_KINDS: [(&str, &str, IconName); 5] = [
    ("Tutorial", "Tutoriel", IconName::Lightbulb),
    ("Tool", "Outil", IconName::Wrench),
    ("Asset", "Asset", IconName::Image),
    ("Doc", "Documentation", IconName::FileText),
    ("Video", "Vidéo", IconName::VideoCamera),
];

/// A resource's kind as label and icon; a resource with no kind is a link.
#[must_use]
pub fn resource_kind(kind: Option<&str>) -> (&'static str, IconName) {
    kind.and_then(|k| RESOURCE_KINDS.iter().find(|(value, _, _)| *value == k))
        .map_or(("Lien", IconName::LinkSimple), |(_, label, icon)| (*label, *icon))
}

/// A badge's icon, by its stored identifier.
#[must_use]
pub fn badge_icon(id: &str) -> IconName {
    match id {
        "FoundingMember" => IconName::Flag,
        "Alumni" => IconName::GraduationCap,
        "ExternalMentor" => IconName::Handshake,
        "GameJamWinner" => IconName::Trophy,
        "GameJamParticipant" => IconName::Hourglass,
        "BugHunter" => IconName::Crosshair,
        "Contributor" => IconName::GitCommit,
        "Streaker" => IconName::Flame,
        "BlockMaster" => IconName::Stack,
        "MultiTracker" => IconName::SquaresFour,
        "Mentor" => IconName::ChalkboardTeacher,
        "TopContributor" => IconName::Medal,
        "NightOwl" => IconName::Moon,
        "SpeedRunner" => IconName::Timer,
        // "Validator", and anything added later.
        _ => IconName::SealCheck,
    }
}

#[cfg(test)]
mod tests {
    use gamecloud_shared::roles::{BureauRole, GlobalRank, SpecialBadge};

    use super::*;

    #[test]
    fn titles_lose_their_emoji_and_nothing_else() {
        assert_eq!(plain_title("🌙 Ancien de la Forge"), "Ancien de la Forge");
        assert_eq!(plain_title("👑⚡ Mythe de la Guilde"), "Mythe de la Guilde");
        assert_eq!(plain_title("⏳ L'Aspirant"), "L'Aspirant");
        assert_eq!(plain_title("Déjà propre"), "Déjà propre");
    }

    #[test]
    fn every_real_title_keeps_a_readable_name() {
        for rank in GlobalRank::ALL {
            let plain = plain_title(rank.title());
            assert!(plain.chars().next().is_some_and(char::is_alphanumeric) || plain.starts_with('L'), "{plain}");
        }
        for office in BureauRole::ALL {
            assert!(!plain_title(office.title()).is_empty());
        }
    }

    #[test]
    fn every_track_has_a_name_and_an_icon() {
        for track in Track::ALL {
            assert!(!track_label(track).is_empty());
            assert_eq!(track_icon_by_id(track.as_str()), Some(track_icon(track)));
        }
        assert_eq!(track_name("GameDesign"), "Game Design");
        assert_eq!(track_name("Sorcery"), "Sorcery");
    }

    #[test]
    fn every_schema_role_has_a_plain_title() {
        for role in ["Observer", "Contributor", "Reviewer", "Mentor", "CoLead", "Lead"] {
            let title = role_title(role);
            assert!(title.chars().next().is_some_and(char::is_alphanumeric), "{title}");
        }
        // Failing closed: an unknown role never reads as something senior.
        assert_eq!(role_title("Sorcerer"), role_title("Observer"));
    }

    #[test]
    fn every_schema_status_has_a_label() {
        for status in ["Draft", "InReview", "PartialOK", "Approved", "Released", "Archived", "Rejected"] {
            assert_ne!(status_label(status), "Inconnu", "{status} unlabelled");
        }
        assert_eq!(status_label("Sideways"), "Inconnu");
        for rarity in ["Common", "Rare", "Epic", "Legendary", "Mythic"] {
            assert!(!rarity_label(rarity).is_empty());
        }
    }

    #[test]
    fn an_unknown_verdict_reads_as_pending() {
        for status in ["Pending", "Approved", "Rejected", "NotApplicable"] {
            assert!(!verdict(status).0.is_empty());
        }
        assert_eq!(verdict("Maybe").0, "En attente");
    }

    #[test]
    fn kinds_fall_back_to_something_honest() {
        assert_eq!(share_kind_icon("Weird"), IconName::Package);
        assert_eq!(resource_kind(None).0, "Lien");
        assert_eq!(resource_kind(Some("Video")).0, "Vidéo");
    }

    #[test]
    fn badges_do_not_share_an_icon() {
        let icons: Vec<IconName> = SpecialBadge::ALL.iter().map(|b| badge_icon(b.as_str())).collect();
        for (i, icon) in icons.iter().enumerate() {
            assert!(!icons[i + 1..].contains(icon), "{icon:?} used twice");
        }
    }
}

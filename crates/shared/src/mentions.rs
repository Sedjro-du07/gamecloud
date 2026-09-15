//! Discord mentions written for people, not as identifiers.
//!
//! Discord stores a mention as `<@123456789012345678>`: readable inside
//! Discord, an eighteen-digit number anywhere else. The platform never
//! shows those numbers — a member mention becomes their pseudo, a role or
//! a channel becomes a word.

/// What a mention points at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mention {
    /// A member, by Discord id.
    User(u64),
    /// A role.
    Role,
    /// A channel.
    Channel,
}

/// The mention at the start of `s`, and its length in bytes.
fn mention_at(s: &str) -> Option<(Mention, usize)> {
    let rest = s.strip_prefix('<')?;
    let (kind, after) = if let Some(r) = rest.strip_prefix("@!") {
        (0, r)
    } else if let Some(r) = rest.strip_prefix("@&") {
        (1, r)
    } else if let Some(r) = rest.strip_prefix('@') {
        (0, r)
    } else if let Some(r) = rest.strip_prefix('#') {
        (2, r)
    } else {
        return None;
    };
    let digits = &after[..after.find(|c: char| !c.is_ascii_digit()).unwrap_or(after.len())];
    if digits.is_empty() || !after[digits.len()..].starts_with('>') {
        return None;
    }
    let id: u64 = digits.parse().ok()?;
    let len = s.len() - after.len() + digits.len() + 1;
    let mention = match kind {
        0 => Mention::User(id),
        1 => Mention::Role,
        _ => Mention::Channel,
    };
    Some((mention, len))
}

/// The members a text mentions, in order, without repeats.
#[must_use]
pub fn mentioned_users(text: &str) -> Vec<u64> {
    let mut ids = Vec::new();
    let mut i = 0;
    while let Some(offset) = text[i..].find('<') {
        let at = i + offset;
        match mention_at(&text[at..]) {
            Some((mention, len)) => {
                if let Mention::User(id) = mention {
                    if !ids.contains(&id) {
                        ids.push(id);
                    }
                }
                i = at + len;
            }
            None => i = at + 1,
        }
    }
    ids
}

/// The text with every mention written out: `@pseudo` for a member whose
/// pseudo `pseudo` knows, `@membre` otherwise, `@rôle` and `#salon` for the
/// rest. Nothing numeric survives.
#[must_use]
pub fn humanize(text: &str, pseudo: impl Fn(u64) -> Option<String>) -> String {
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while let Some(offset) = text[i..].find('<') {
        let at = i + offset;
        out.push_str(&text[i..at]);
        match mention_at(&text[at..]) {
            Some((Mention::User(id), len)) => {
                out.push('@');
                out.push_str(&pseudo(id).unwrap_or_else(|| "membre".to_string()));
                i = at + len;
            }
            Some((Mention::Role, len)) => {
                out.push_str("@rôle");
                i = at + len;
            }
            Some((Mention::Channel, len)) => {
                out.push_str("#salon");
                i = at + len;
            }
            None => {
                out.push('<');
                i = at + 1;
            }
        }
    }
    out.push_str(&text[i..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn known(id: u64) -> Option<String> {
        (id == 865_973_472_223_428_608).then(|| "fred04".to_string())
    }

    #[test]
    fn a_member_mention_becomes_their_pseudo() {
        assert_eq!(humanize("Salut <@865973472223428608> !", known), "Salut @fred04 !");
        assert_eq!(humanize("<@!865973472223428608>", known), "@fred04");
    }

    #[test]
    fn nothing_numeric_survives() {
        let out = humanize("<@123456789012345678> voit <@&42> dans <#77>", known);
        assert_eq!(out, "@membre voit @rôle dans #salon");
        assert!(!out.chars().any(|c| c.is_ascii_digit()));
    }

    #[test]
    fn text_that_only_looks_like_a_mention_is_left_alone() {
        assert_eq!(humanize("a < b et <@> et <@12", known), "a < b et <@> et <@12");
        assert_eq!(humanize("émoji ✨ <@865973472223428608>", known), "émoji ✨ @fred04");
    }

    #[test]
    fn mentioned_members_are_listed_once() {
        assert_eq!(mentioned_users("<@1> <@!1> <@&2> <#3> <@4>"), vec![1, 4]);
    }
}

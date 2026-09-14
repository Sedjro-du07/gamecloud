//! Epitech email validation.
//!
//! The single source of truth for "is this a valid Epitech email" lives
//! here. The DB has the same regex as a defensive `CHECK`, but every
//! application path goes through this module first so we get a clean
//! `DomainError` rather than a database constraint violation.

use std::sync::LazyLock;

use gamecloud_shared::{DomainError, DomainResult};
use regex::Regex;

// Allow hyphens and digits inside both name components (e.g.
// `joachim.goeh-akue@epitech.eu`, `john.doe2@epitech.eu`). We require
// the first character of each part to be a letter so dotted-numeric
// nonsense like `1.2@epitech.eu` is rejected.
static EPITECH_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[a-z][a-z0-9-]*\.[a-z][a-z0-9-]*@epitech\.eu$").expect("valid regex")
});

/// Validate that `input` looks like a `firstname.lastname@epitech.eu`
/// address. Returns the lowercased canonical form on success.
///
/// # Errors
/// `DomainError::InvalidEpitechEmail` if the format is wrong.
pub fn validate(input: &str) -> DomainResult<String> {
    let normalized = input.trim().to_ascii_lowercase();
    if EPITECH_RE.is_match(&normalized) {
        Ok(normalized)
    } else {
        Err(DomainError::InvalidEpitechEmail(input.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_simple_emails() {
        assert!(validate("joachim.goehakue@epitech.eu").is_ok());
        assert!(validate("Joachim.GoehAkue@Epitech.Eu").is_ok());
    }

    #[test]
    fn accepts_hyphenated_names() {
        // The real-world case: a hyphenated last name.
        assert!(validate("joachim.goeh-akue@epitech.eu").is_ok());
        assert!(validate("jean-paul.sartre@epitech.eu").is_ok());
    }

    #[test]
    fn accepts_digits_in_names() {
        // Some Epitech accounts have a numeric suffix.
        assert!(validate("john.doe2@epitech.eu").is_ok());
    }

    #[test]
    fn rejects_other_domains() {
        assert!(validate("joachim.goehakue@gmail.com").is_err());
        assert!(validate("joachim@epitech.eu").is_err());
        assert!(validate("@epitech.eu").is_err());
    }

    #[test]
    fn rejects_leading_digits() {
        assert!(validate("1joachim.goehakue@epitech.eu").is_err());
        assert!(validate("joachim.2doe@epitech.eu").is_err());
    }
}

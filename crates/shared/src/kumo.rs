//! The XP exchange rate between Kumo and the platform, and the counters
//! behind it.
//!
//! Kumo and the platform each keep their own XP total. They are kept in
//! step with two *cumulative* counters rather than per-event deltas, so a
//! retried request, a lost response or two concurrent reports can never
//! pay the same XP twice:
//!
//! - **Kumo → platform.** Kumo reports `organic_total`: everything its
//!   members earned by talking, and nothing else (no `/add-xp`, no `/set-xp`,
//!   nothing mirrored from the platform). The platform pays
//!   [`credit_units`] of it, minus what it already paid.
//! - **Platform → Kumo.** The platform publishes each member's
//!   [`platform_origin`] XP: what the platform itself awarded, with every
//!   Kumo-origin amount taken out so it never bounces back. Kumo applies
//!   the change since the last value it saw, times the ratio.

/// Kumo XP worth one platform XP.
///
/// Kumo pays 15 to 40 XP a message; the platform pays 5 per commit. At
/// 5 to 1, chatting still counts without drowning the rest.
pub const DEFAULT_KUMO_XP_PER_PLATFORM_XP: i64 = 5;

/// Platform XP owed for Kumo activity: whole units of `ratio` Kumo XP
/// earned above `baseline`. The remainder is not lost — it stays in
/// `total - baseline` and is paid once it adds up.
///
/// Never negative: a Kumo total that fell (an admin lowered it) takes
/// nothing back from the platform.
#[must_use]
pub fn credit_units(total: i64, baseline: i64, ratio: i64) -> i64 {
    if ratio <= 0 {
        return 0;
    }
    (total - baseline).max(0) / ratio
}

/// XP the platform itself awarded a member.
///
/// `xp_total` minus the 1:1 carry-over of Kumo's pre-platform XP minus
/// what the Kumo sync paid. Clamped at zero.
#[must_use]
pub fn platform_origin(xp_total: i64, carried_over: i64, kumo_credited: i64) -> i64 {
    (xp_total - carried_over - kumo_credited).max(0)
}

/// Change in Kumo's own XP implied by a change in platform-origin XP.
///
/// `new` and `seen` are cumulative platform-origin values; the result
/// is in Kumo XP and may be negative (the Bureau revoked XP).
#[must_use]
pub fn kumo_delta(new: i64, seen: i64, ratio: i64) -> i64 {
    (new - seen).saturating_mul(ratio)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pays_whole_units_only() {
        assert_eq!(credit_units(1_004, 1_000, 5), 0);
        assert_eq!(credit_units(1_005, 1_000, 5), 1);
        assert_eq!(credit_units(1_841, 1_000, 5), 168);
    }

    #[test]
    fn remainder_is_paid_later_not_lost() {
        // 7 + 8 Kumo XP in two reports: 1 then 2 units in total.
        let first = credit_units(7, 0, 5);
        let second = credit_units(15, 0, 5);
        assert_eq!((first, second - first), (1, 2));
    }

    #[test]
    fn a_lower_kumo_total_takes_nothing_back() {
        assert_eq!(credit_units(500, 1_000, 5), 0);
    }

    #[test]
    fn nonsense_ratio_pays_nothing() {
        assert_eq!(credit_units(1_000, 0, 0), 0);
        assert_eq!(credit_units(1_000, 0, -3), 0);
    }

    #[test]
    fn legacy_carry_over_never_goes_back_to_kumo() {
        // fred040: 15 344 carried over 1:1, plus 120 earned on the platform.
        assert_eq!(platform_origin(15_464, 15_344, 0), 120);
    }

    #[test]
    fn kumo_paid_xp_is_not_echoed() {
        // 168 platform XP came from Kumo chat; 40 were earned here.
        assert_eq!(platform_origin(15_344 + 168 + 40, 15_344, 168), 40);
    }

    #[test]
    fn platform_origin_is_clamped() {
        assert_eq!(platform_origin(10, 15_344, 0), 0);
    }

    #[test]
    fn delta_is_scaled_and_signed() {
        assert_eq!(kumo_delta(50, 40, 5), 50);
        assert_eq!(kumo_delta(30, 40, 5), -50);
        assert_eq!(kumo_delta(40, 40, 5), 0);
    }
}

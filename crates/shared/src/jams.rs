//! Internal game jams: the phases and how winners are decided.
//!
//! A jam runs in four phases set by three dates the Bureau chooses:
//! registration until it starts (the theme stays hidden), the jam itself
//! until submissions close, the vote until voting closes, then results
//! once the Bureau publishes them.
//!
//! Each category is decided half by the Bureau's jury and half by the
//! members: the average score of each group, out of 5, then the mean of
//! the two. A group that cast no vote for a game is left out rather than
//! counted as zero. Ties go to the game with more votes, then to the one
//! handed in first.

/// Where a jam stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Phase {
    /// Teams form; the theme is not revealed yet.
    Registration,
    /// The theme is out; teams make and hand in their game.
    Running,
    /// Submissions are closed; everybody votes.
    Voting,
    /// Voting is closed; the Bureau has not published the results yet.
    Deliberation,
    /// Results are out.
    Results,
}

impl Phase {
    /// The phase at `now`, from the jam's dates (as seconds) and whether
    /// its results are published.
    #[must_use]
    pub const fn at(now: i64, starts: i64, ends: i64, voting_ends: i64, published: bool) -> Self {
        if published {
            Self::Results
        } else if now < starts {
            Self::Registration
        } else if now < ends {
            Self::Running
        } else if now < voting_ends {
            Self::Voting
        } else {
            Self::Deliberation
        }
    }

    /// Stable name, for the page.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Registration => "Registration",
            Self::Running => "Running",
            Self::Voting => "Voting",
            Self::Deliberation => "Deliberation",
            Self::Results => "Results",
        }
    }

    /// Teams can still form and change.
    #[must_use]
    pub const fn teams_open(self) -> bool {
        matches!(self, Self::Registration | Self::Running)
    }

    /// The theme is shown and work can be handed in.
    #[must_use]
    pub const fn submissions_open(self) -> bool {
        matches!(self, Self::Running)
    }

    /// The theme is visible to everybody.
    #[must_use]
    pub const fn theme_revealed(self) -> bool {
        !matches!(self, Self::Registration)
    }
}

/// One vote: a score out of 5 given to one game in one category.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Vote {
    /// The voter sits on the Bureau's jury.
    pub jury: bool,
    /// 1 to 5.
    pub score: u8,
}

/// A game's standing in one category.
#[derive(Debug, Clone, PartialEq)]
pub struct Standing<T> {
    /// Which game.
    pub entry: T,
    /// Jury average out of 5, if the jury voted.
    pub jury: Option<f64>,
    /// Members' average out of 5, if members voted.
    pub members: Option<f64>,
    /// The mean of the two available averages, out of 5.
    pub score: Option<f64>,
    /// Votes cast.
    pub votes: usize,
}

fn average(scores: impl Iterator<Item = u8>) -> Option<f64> {
    let (sum, n) = scores.fold((0_u32, 0_u32), |(s, n), x| (s + u32::from(x), n + 1));
    (n > 0).then(|| f64::from(sum) / f64::from(n))
}

/// Score one game in one category from its votes.
#[must_use]
pub fn standing<T>(entry: T, votes: &[Vote]) -> Standing<T> {
    let jury = average(votes.iter().filter(|v| v.jury).map(|v| v.score));
    let members = average(votes.iter().filter(|v| !v.jury).map(|v| v.score));
    let score = match (jury, members) {
        (Some(j), Some(m)) => Some((j + m) / 2.0),
        (Some(x), None) | (None, Some(x)) => Some(x),
        (None, None) => None,
    };
    Standing { entry, jury, members, score, votes: votes.len() }
}

/// Rank the games of one category, best first. `entries` are in the order
/// they were handed in, which breaks the last ties. Games nobody voted for
/// come last and cannot win.
#[must_use]
pub fn rank<T: Clone>(entries: &[(T, Vec<Vote>)]) -> Vec<Standing<T>> {
    let mut standings: Vec<(usize, Standing<T>)> = entries
        .iter()
        .enumerate()
        .map(|(i, (e, votes))| (i, standing(e.clone(), votes)))
        .collect();
    standings.sort_by(|(ia, a), (ib, b)| {
        b.score
            .unwrap_or(-1.0)
            .total_cmp(&a.score.unwrap_or(-1.0))
            .then(b.votes.cmp(&a.votes))
            .then(ia.cmp(ib))
    });
    standings.into_iter().map(|(_, s)| s).collect()
}

/// The winner of a category, if anybody voted at all.
#[must_use]
pub fn winner<T: Clone>(entries: &[(T, Vec<Vote>)]) -> Option<T> {
    rank(entries).into_iter().find(|s| s.score.is_some()).map(|s| s.entry)
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn jury(score: u8) -> Vote {
        Vote { jury: true, score }
    }
    const fn member(score: u8) -> Vote {
        Vote { jury: false, score }
    }

    #[test]
    fn phases_follow_the_dates() {
        assert_eq!(Phase::at(0, 10, 20, 30, false), Phase::Registration);
        assert_eq!(Phase::at(10, 10, 20, 30, false), Phase::Running);
        assert_eq!(Phase::at(20, 10, 20, 30, false), Phase::Voting);
        assert_eq!(Phase::at(30, 10, 20, 30, false), Phase::Deliberation);
        assert_eq!(Phase::at(5, 10, 20, 30, true), Phase::Results);
        assert!(!Phase::Registration.theme_revealed());
        assert!(Phase::Running.submissions_open() && Phase::Running.teams_open());
        assert!(!Phase::Voting.teams_open());
    }

    #[test]
    fn jury_and_members_weigh_half_each() {
        // Jury 5, members 3 → 4, however many members voted.
        let s = standing("a", &[jury(5), member(3), member(3), member(3)]);
        assert_eq!(s.score, Some(4.0));
        assert_eq!(s.jury, Some(5.0));
        assert_eq!(s.members, Some(3.0));
    }

    #[test]
    fn a_silent_group_is_left_out_not_counted_as_zero() {
        assert_eq!(standing("a", &[member(4), member(2)]).score, Some(3.0));
        assert_eq!(standing("a", &[jury(5)]).score, Some(5.0));
        assert_eq!(standing::<&str>("a", &[]).score, None);
    }

    #[test]
    fn the_best_score_wins_then_more_votes_then_first_handed_in() {
        let entries = vec![
            ("first", vec![member(4)]),
            ("second", vec![member(4), member(4)]),
            ("third", vec![jury(5), member(5)]),
        ];
        assert_eq!(winner(&entries), Some("third"));
        let tied = vec![("first", vec![member(4)]), ("second", vec![member(4), member(4)])];
        assert_eq!(winner(&tied), Some("second"));
        let level = vec![("first", vec![member(3)]), ("second", vec![member(3)])];
        assert_eq!(winner(&level), Some("first"));
    }

    #[test]
    fn nobody_wins_a_category_nobody_voted_in() {
        let entries: Vec<(&str, Vec<Vote>)> = vec![("a", vec![]), ("b", vec![])];
        assert_eq!(winner(&entries), None);
    }
}

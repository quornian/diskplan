use regex::Regex;

/// Criteria for matching against names in a directory
///
#[derive(Debug, PartialEq)]
pub struct MatchCriteria {
    /// Used to sort criteria when matching against directory entries, lower numbers are tried
    /// first with the first successful match winning
    order: i16,
    /// Method to use when testing for a match
    mode: Match,
}

impl MatchCriteria {
    pub fn new(order: i16, mode: Match) -> MatchCriteria {
        MatchCriteria { order, mode }
    }
    pub fn order(&self) -> i16 {
        self.order
    }
    pub fn mode(&self) -> &Match {
        &self.mode
    }
}

#[derive(Debug)]
pub enum Match {
    /// Match the exact name of the item
    Fixed(String),
    /// Match the regular expression
    Regex { pattern: Regex, binding: String },
    /// Match any name
    Any { binding: String },
}

impl Match {
    pub fn from_regex(pattern: &str, binding: &str) -> Result<Match, regex::Error> {
        // Encase as a full string match pattern, but first ensure it cannot escape
        regex::Regex::new(pattern)?;
        let regex = regex::Regex::new(&format!("^(?:{})$", pattern))?;
        Ok(Match::Regex {
            pattern: regex,
            binding: binding.to_owned(),
        })
    }
}

// Regex doesn't implement PartialEq so we have to, by using Regex.as_str()
impl PartialEq for Match {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Match::Fixed(a), Match::Fixed(b)) => a == b,
            (Match::Fixed(_), _) => false,
            (Match::Any { binding: a }, Match::Any { binding: b }) => a == b,
            (Match::Any { .. }, _) => false,
            (
                Match::Regex {
                    pattern: ap,
                    binding: ab,
                },
                Match::Regex {
                    pattern: bp,
                    binding: bb,
                },
            ) => ap.as_str() == bp.as_str() && ab == bb,
            (Match::Regex { .. }, _) => false,
        }
    }
}

use super::expr::{Expression, Identifier};

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

#[derive(Debug, PartialEq)]
pub enum Match {
    /// Match the exact name of the item, not bound
    Fixed(String),
    /// Bind the name to an identifier, and match the (optional) regular expression
    Variable {
        pattern: Option<Expression>,
        binding: Identifier,
    },
}

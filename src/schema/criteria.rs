use super::expr::{Expression, Identifier};

/// Criteria for matching against names in a directory
///
#[derive(Debug, PartialEq)]
pub enum Match {
    /// Match the exact name of the item, not bound
    Fixed(String),
    /// Bind the name to an identifier, and match the (optional) regular expression
    Variable {
        /// Used to sort criteria when matching against directory entries, lower numbers are tried
        /// first with the first successful match winning
        order: i16,
        pattern: Option<Expression>,
        binding: Identifier,
    },
}

impl Match {
    pub fn fixed(s: &str) -> Match {
        Match::Fixed(s.to_owned())
    }
}

impl PartialOrd for Match {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        use std::cmp::Ordering::{Greater, Less};
        use Match::{Fixed, Variable};
        match (self, other) {
            (Fixed(a), Fixed(b)) => None,
            (Fixed(_), _) => Some(Less),
            (_, Fixed(_)) => Some(Greater),
            (Variable { order: a, .. }, Variable { order: b, .. }) => {
                if a == b {
                    None
                } else {
                    Some(a.cmp(b))
                }
            }
        }
    }
}

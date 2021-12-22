use super::expr::{Expression, Identifier};

/// Criteria for matching against names in a directory
///
#[derive(Debug, Clone, PartialEq)]
pub enum Match<'t> {
    /// Match the exact name of the item, not bound
    Fixed(&'t str),
    /// Bind the name to an identifier, and match the (optional) regular expression
    Variable {
        /// Used to sort criteria when matching against directory entries, lower numbers are tried
        /// first with the first successful match winning
        order: i16,
        pattern: Option<Expression<'t>>,
        binding: Identifier<'t>,
    },
}

impl PartialOrd for Match<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        use std::cmp::Ordering::{Greater, Less};
        use Match::{Fixed, Variable};
        match (self, other) {
            (Fixed(a), Fixed(b)) if a == b => None,
            (Fixed(a), Fixed(b)) if a != b => Some(a.cmp(b)),
            (Fixed(_), _) => Some(Less),
            (_, Fixed(_)) => Some(Greater),
            (Variable { order: a, .. }, Variable { order: b, .. }) if a == b => None,
            (Variable { order: a, .. }, Variable { order: b, .. }) => Some(a.cmp(b)),
        }
    }
}

use super::expr::Expression;

/// Criteria for matching against names in a directory
///
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pattern<'t> {
    /// Match the exact name of the item
    Fixed(&'t str),
    /// Match a regular expression
    Regex(Expression<'t>),
}

impl PartialOrd for Pattern<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Pattern<'_> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        use std::cmp::Ordering::{Greater, Less};
        use Pattern::{Fixed, Regex};
        match (self, other) {
            (Fixed(a), Fixed(b)) => a.cmp(b),
            (Fixed(_), Regex(_)) => Less,
            (Regex(_), Fixed(_)) => Greater,
            (Regex(a), Regex(b)) => a.cmp(b),
        }
    }
}

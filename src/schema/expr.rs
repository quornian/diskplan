use std::fmt::Display;

#[derive(Debug, Clone, Eq)]
pub struct Expression<'t>(&'t str, Vec<Token<'t>>);

impl<'t> Expression<'t> {
    pub(super) fn from_parsed(expr: &'t str, tokens: Vec<Token<'t>>) -> Expression<'t> {
        // Note: This is pub(super) because we require the caller to provide the unparsed
        // string and parsed tokens of the same underlying expression. Limiting this trust
        // to the schema parsing code and its tests avoids exposing the issue publicly.
        Expression(expr, tokens)
    }

    pub fn as_str(&self) -> &str {
        self.0
    }

    pub fn tokens(&self) -> &[Token] {
        &self.1[..]
    }
}

impl PartialEq for Expression<'_> {
    fn eq(&self, other: &Self) -> bool {
        // Note: Since we require .0 and .1 to be equivalent we only need to test .0
        self.0 == other.0
    }
}

impl Ord for Expression<'_> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Note: Since we require .0 and .1 to be equivalent we only need to compare .0
        self.0.cmp(other.0)
    }
}

impl PartialOrd for Expression<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Display for Expression<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token<'t> {
    Text(&'t str),
    Variable(Identifier<'t>),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Identifier<'t>(&'t str);

impl<'t> Identifier<'t> {
    pub fn new(s: &'t str) -> Self {
        Identifier(s)
    }

    pub fn value(&self) -> &'t str {
        self.0
    }
}

impl Display for Identifier<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

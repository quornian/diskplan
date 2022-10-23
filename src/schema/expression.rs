use std::{fmt::Display, vec};

#[derive(Debug, Clone, PartialEq, Eq, Ord)]
pub struct Expression<'t>(Vec<Token<'t>>);

impl<'t> Expression<'t> {
    pub fn tokens(&self) -> &[Token<'t>] {
        &self.0[..]
    }

    pub fn from_text(s: &'t str) -> Result<Expression<'t>, String> {
        if s.contains('$') {
            Err(format!("Not a text-only expression: {}", s))
        } else {
            Ok(Expression(vec![Token::Text(s)]))
        }
    }
}

impl<'t> From<Vec<Token<'t>>> for Expression<'t> {
    fn from(tokens: Vec<Token<'t>>) -> Self {
        Expression(tokens)
    }
}

impl PartialOrd for Expression<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Display for Expression<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for token in self.0.iter() {
            write!(f, "{}", token)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Token<'t> {
    Text(&'t str),
    Variable(Identifier<'t>),
    Special(Special),
}

impl Display for Token<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Token::Text(s) => f.write_str(s),
            Token::Variable(v) => write!(f, "${{{}}}", v),
            Token::Special(sp) => write!(f, "{}", sp),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Special {
    PathRelative,
    PathAbsolute,
    PathNameOnly,
    ParentRelative,
    ParentAbsolute,
    ParentNameOnly,
    RootPath,
}

impl Special {
    pub const SAME_PATH_RELATIVE: &'static str = "PATH";
    pub const SAME_PATH_ABSOLUTE: &'static str = "FULL_PATH";
    pub const SAME_PATH_NAME: &'static str = "NAME";
    pub const PARENT_PATH_RELATIVE: &'static str = "PARENT_PATH";
    pub const PARENT_PATH_ABSOLUTE: &'static str = "PARENT_FULL_PATH";
    pub const PARENT_PATH_NAME: &'static str = "PARENT_NAME";
    pub const ROOT_PATH: &'static str = "ROOT_PATH";
}

impl Display for Special {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Special::PathRelative => Special::SAME_PATH_RELATIVE,
            Special::PathAbsolute => Special::SAME_PATH_ABSOLUTE,
            Special::PathNameOnly => Special::SAME_PATH_NAME,
            Special::ParentRelative => Special::PARENT_PATH_RELATIVE,
            Special::ParentAbsolute => Special::PARENT_PATH_ABSOLUTE,
            Special::ParentNameOnly => Special::PARENT_PATH_NAME,
            Special::RootPath => Special::ROOT_PATH,
        })
    }
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

impl<'a> From<&'a str> for Identifier<'a> {
    fn from(s: &'a str) -> Self {
        Identifier::new(s)
    }
}

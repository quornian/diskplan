use std::{fmt::Display, vec};

/// A string expression made from one or more [`Token`]s
#[derive(Debug, Clone, PartialEq, Eq, Ord, PartialOrd)]
pub struct Expression<'t>(Vec<Token<'t>>);

impl<'t> Expression<'t> {
    /// Provides access to the slice of tokens that make up this expression
    pub fn tokens(&self) -> &[Token<'t>] {
        &self.0[..]
    }
}

impl<'t> From<Vec<Token<'t>>> for Expression<'t> {
    fn from(tokens: Vec<Token<'t>>) -> Self {
        Expression(tokens)
    }
}

impl<'t> From<&[Token<'t>]> for Expression<'t> {
    fn from(tokens: &[Token<'t>]) -> Self {
        Expression(tokens.into())
    }
}

impl Display for Expression<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for token in self.0.iter() {
            write!(f, "{token}")?;
        }
        Ok(())
    }
}

impl PartialEq<&str> for Expression<'_> {
    fn eq(&self, other: &&str) -> bool {
        // Expression is equal to a string only if it is a single text token
        // with the same inner value
        match &self.0[..] {
            [Token::Text(text)] => *text == *other,
            _ => false,
        }
    }
}

/// Part of an [`Expression`]; a constant string, or a variable for later expansion to a string
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Token<'t> {
    /// A constant string of plain text
    Text(&'t str),
    /// The name of a variable
    Variable(Identifier<'t>),
    /// A special variable whose value is provided by the current scope
    Special(Special),
}

impl Display for Token<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Token::Text(s) => f.write_str(s),
            Token::Variable(v) => write!(f, "${{{v}}}"),
            Token::Special(sp) => write!(f, "{sp}"),
        }
    }
}

/// A choice of built-in variables that are used to provide context information during traversal
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Special {
    /// The current path relative to the active root
    PathRelative,
    /// The current absolute path
    PathAbsolute,
    /// The final component of the current path
    PathNameOnly,
    /// The current relative path without the final component
    ParentRelative,
    /// The current absolute path without the final component
    ParentAbsolute,
    /// The penultimate component of the current path
    ParentNameOnly,
    /// The absolute path of the active root
    RootPath,
}

impl Special {
    /// The current path relative to the active root
    pub const SAME_PATH_RELATIVE: &'static str = "PATH";
    /// The current absolute path
    pub const SAME_PATH_ABSOLUTE: &'static str = "FULL_PATH";
    /// The final component of the current path
    pub const SAME_PATH_NAME: &'static str = "NAME";
    /// The current relative path without the final component
    pub const PARENT_PATH_RELATIVE: &'static str = "PARENT_PATH";
    /// The current absolute path without the final component
    pub const PARENT_PATH_ABSOLUTE: &'static str = "PARENT_FULL_PATH";
    /// The penultimate component of the current path
    pub const PARENT_PATH_NAME: &'static str = "PARENT_NAME";
    /// The absolute path of the active root
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

/// The name given to a variable
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Identifier<'t>(&'t str);

impl<'t> Identifier<'t> {
    /// Creates a new Identifier from the given string
    pub fn new(s: &'t str) -> Self {
        Identifier(s)
    }

    /// Returns a reference to the underlying string
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

impl<'a> From<Identifier<'a>> for Expression<'a> {
    fn from(identifier: Identifier<'a>) -> Self {
        Expression(vec![Token::Variable(identifier)])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn format_identifier() {
        assert_eq!(&format!("{}", Identifier("something")), "something");
    }

    #[test]
    fn format_variable() {
        let something = Identifier("something");
        assert_eq!(&format!("{}", Token::Variable(something)), "${something}");
    }

    #[test]
    fn format_expression_all_types() {
        let expr = Expression(vec![
            Token::Text("normal text/"),
            Token::Variable(Identifier("a_variable")),
            Token::Text("/"),
            Token::Special(Special::ParentRelative),
        ]);
        assert_eq!(&format!("{expr}"), "normal text/${a_variable}/PARENT_PATH");
    }
}

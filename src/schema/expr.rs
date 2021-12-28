use std::fmt::Display;

#[derive(Debug, Clone, PartialEq, PartialOrd, Eq, Ord)]
pub struct Expression<'t>(Vec<Token<'t>>);

impl Expression<'_> {
    pub fn new(tokens: Vec<Token>) -> Expression {
        Expression(tokens)
    }

    pub fn tokens(&self) -> &Vec<Token> {
        &self.0
    }
}

impl Display for Expression<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for token in &self.0 {
            write!(f, "{}", token)?
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Eq, Ord)]
pub enum Token<'t> {
    Text(&'t str),
    Variable(Identifier<'t>),
}

impl Display for Token<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Token::Text(s) | Token::Variable(Identifier(s)) => write!(f, "{}", s)?,
        }
        Ok(())
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

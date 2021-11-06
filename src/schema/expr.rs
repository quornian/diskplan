use std::fmt::Display;

#[derive(Debug, Clone, PartialEq)]
pub struct Expression(Vec<Token>);

impl Expression {
    pub fn new(tokens: Vec<Token>) -> Expression {
        Expression(tokens)
    }

    pub fn tokens(&self) -> &Vec<Token> {
        &self.0
    }
}

impl Display for Expression {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for token in &self.0 {
            write!(f, "{}", token)?
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Text(String),
    Variable(Identifier),
}

impl Token {
    pub fn text<S: AsRef<str>>(s: S) -> Self {
        Self::Text(s.as_ref().to_owned())
    }
    pub fn variable<S: AsRef<str>>(s: S) -> Self {
        Self::Variable(Identifier(s.as_ref().to_owned()))
    }
}

impl Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Token::Text(s) | Token::Variable(Identifier(s)) => write!(f, "{}", s)?,
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Identifier(String);

impl Identifier {
    pub fn new<S: AsRef<str>>(s: S) -> Self {
        Identifier(s.as_ref().to_owned())
    }

    pub fn value(&self) -> &String {
        &self.0
    }
}

impl From<&Identifier> for String {
    fn from(i: &Identifier) -> Self {
        i.0.clone()
    }
}

impl Display for Identifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

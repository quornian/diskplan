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

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Text(String),
    Variable(String),
}

impl Token {
    pub fn text<S: AsRef<str>>(s: S) -> Self {
        Self::Text(s.as_ref().to_owned())
    }
    pub fn variable<S: AsRef<str>>(s: S) -> Self {
        Self::Variable(s.as_ref().to_owned())
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

impl Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Token::Text(s) | Token::Variable(s) => write!(f, "{}", s)?,
        }
        Ok(())
    }
}

#[derive(thiserror::Error, Debug, PartialEq)]
pub enum EvaluationError {
    #[error("No such variable: {0}")]
    NoSuchVariable(String),

    //FIXME
    // #[error("Built-in variable failed to evaluate: {0:?}")]
    // BuiltinError(Builtin),
    //
    // #[error("Error parsing expression from: {0}")]
    // ExpressionError(String, #[source] ExprError),
    //
    // #[error("Invalid name (no tokens): {0}")]
    // NameHasNoTokens(String),
    //
    // #[error("Invalid name (multiple tokens): {0} ({1} unexpected)")]
    // NameHasMultipleTokens(String, String),
    //
    #[error("Error evaluating {0:?}, replacing @{1} with {2:?}")]
    Recursion(String, String, String, #[source] Box<EvaluationError>),
}

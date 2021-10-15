use crate::{
    context::Context,
    parse::{Builtin, Expr, ExprError, Token},
};

#[derive(thiserror::Error, Debug, PartialEq)]
pub enum EvaluationError {
    #[error("No such variable: {0}")]
    NoSuchVariable(String),

    #[error("Built-in variable failed to evaluate: {0:?}")]
    BuiltinError(Builtin),

    #[error(transparent)]
    ExprError(#[from] ExprError),

    #[error("Invalid name (no tokens): {0}")]
    NameHasNoTokens(String),

    #[error("Invalid name (multiple tokens): {0} ({1} unexpected)")]
    NameHasMultipleTokens(String, String),
}

pub trait Evaluate {
    fn evaluate(&self, expr: &Expr) -> Result<String, EvaluationError>;
}

impl Evaluate for Context<'_> {
    fn evaluate(&self, expr: &Expr) -> Result<String, EvaluationError> {
        let mut buffer = String::new();
        for token in expr.tokens() {
            match token {
                Token::Text(text) => buffer.push_str(text),
                Token::AtVar(var) => buffer.push_str(
                    &self
                        .stack
                        .lookup(var)
                        .ok_or_else(|| EvaluationError::NoSuchVariable(var.to_string()))?,
                ),
                Token::Builtin(builtin) => buffer.push_str(&match builtin {
                    Builtin::Path => self.path.to_string_lossy(),
                    Builtin::Parent => self
                        .path
                        .parent()
                        .ok_or_else(|| EvaluationError::BuiltinError(builtin.clone()))?
                        .to_string_lossy(),
                    Builtin::Name => self
                        .path
                        .file_name()
                        .ok_or_else(|| EvaluationError::BuiltinError(builtin.clone()))?
                        .to_string_lossy(),
                }),
            }
        }
        return Ok(buffer);
    }
}

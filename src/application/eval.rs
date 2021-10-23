use std::convert::TryFrom;

use crate::{
    application::context::Context,
    definition::schema::{Expression, Token},
};

#[derive(thiserror::Error, Debug, PartialEq)]
pub enum EvaluationError {
    #[error("No such variable: {0}")]
    NoSuchVariable(String),

    // #[error("Built-in variable failed to evaluate: {0:?}")]
    // BuiltinError(Builtin),

    // #[error("Error parsing expression from: {0}")]
    // ExpressionError(String, #[source] ExprError),
    //
    #[error("Invalid name (no tokens): {0}")]
    NameHasNoTokens(String),

    #[error("Invalid name (multiple tokens): {0} ({1} unexpected)")]
    NameHasMultipleTokens(String, String),

    #[error("Error evaluating {0:?}, replacing @{1} with {2:?}")]
    Recursion(String, String, String, #[source] Box<EvaluationError>),
}

pub trait Evaluate {
    fn evaluate(&self, expr: &Expression) -> Result<String, EvaluationError>;
}

impl Evaluate for Context<'_> {
    fn evaluate(&self, expr: &Expression) -> Result<String, EvaluationError> {
        let mut buffer = String::new();
        for token in expr.tokens() {
            match token {
                Token::Text(text) => buffer.push_str(text),
                Token::Variable(var) => {
                    let value = self
                        .lookup(var)
                        .ok_or_else(|| EvaluationError::NoSuchVariable(var.to_string()))?;
                    buffer.push_str(&self.evaluate(&value).map_err(|e| {
                        EvaluationError::Recursion(
                            expr.to_string(),
                            var.to_string(),
                            value.to_string(),
                            Box::new(e),
                        )
                    })?);
                }
                //FIXME:
                // Token::Builtin(builtin) => buffer.push_str(&match builtin {
                //     Builtin::Path => self.target.to_string_lossy(),
                //     Builtin::Parent => self
                //         .target
                //         .parent()
                //         .ok_or_else(|| EvaluationError::BuiltinError(builtin.clone()))?
                //         .to_string_lossy(),
                //     Builtin::Name => self
                //         .target
                //         .file_name()
                //         .ok_or_else(|| EvaluationError::BuiltinError(builtin.clone()))?
                //         .to_string_lossy(),
                // }),
            }
        }
        return Ok(buffer);
    }
}

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use super::{Builtin, Expr, ExprError, Token};

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

pub struct Context<'a> {
    pub path: PathBuf,
    pub stack: Stack<'a>,
}

impl Context<'_> {
    pub fn new(path: &Path) -> Context {
        Context {
            path: path.to_owned(),
            stack: Stack::default(),
        }
    }

    pub fn evaluate(&self, expr: &Expr) -> Result<String, EvaluationError> {
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

#[derive(Default)]
pub struct Stack<'a> {
    vars: HashMap<String, String>,
    parent: Option<&'a Stack<'a>>,
}

impl Stack<'_> {
    pub fn lookup<S>(&self, var: S) -> Option<&String>
    where
        S: AsRef<str>,
    {
        self.vars
            .get(var.as_ref())
            .or_else(|| self.parent.as_deref().and_then(|parent| parent.lookup(var)))
    }
}

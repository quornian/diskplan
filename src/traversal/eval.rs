use std::fmt::Display;

use anyhow::{anyhow, Result};

use crate::{
    filesystem::PlantedPath,
    schema::{Expression, Special, Token},
};

use super::stack;

pub enum Value<'a> {
    Expression(&'a Expression<'a>),
    String(&'a str),
}

pub(super) fn evaluate(
    expr: &Expression<'_>,
    stack: Option<&stack::Stack>,
    path: &PlantedPath,
) -> Result<String> {
    log::trace!(r#"Evaluating expression "{}""#, expr);
    let mut value = String::new();
    for token in expr.tokens() {
        match token {
            Token::Text(text) => value.push_str(text),
            Token::Variable(var) => {
                let sub = stack::lookup(var, stack).ok_or_else(|| {
                    anyhow!(r#"Undefined variable "{}" in expression "{}""#, var, expr)
                })?;
                log::trace!(r#"Variable ${{{}}} = "{}""#, var, sub);
                match sub {
                    Value::Expression(expr) => {
                        log::trace!("Going deeper...");
                        value.push_str(&evaluate(expr, stack, path)?)
                    }
                    Value::String(s) => value.push_str(s),
                }
            }
            Token::Special(special) => {
                let it = match special {
                    Special::PathAbsolute => path.absolute().as_str(),
                    Special::PathRelative => path.relative().as_str(),
                    Special::PathNameOnly => path.relative().file_name().unwrap(),
                    Special::ParentAbsolute => path
                        .absolute()
                        .parent()
                        .ok_or_else(|| anyhow!("Path has no parent: {}", path.absolute()))?
                        .as_str(),

                    Special::ParentRelative => path
                        .relative()
                        .parent()
                        .ok_or_else(|| anyhow!("Path has no parent: {}", path.relative()))?
                        .as_str(),
                    Special::ParentNameOnly => path
                        .relative()
                        .parent()
                        .and_then(|p| p.file_name())
                        .ok_or_else(|| anyhow!("Path has no parent: {}", path.relative()))?,
                    Special::RootPath => path.root().as_str(),
                };
                log::trace!(r#"Special {} = "{}""#, special, it);
                value.push_str(it);
            }
        }
    }
    log::trace!(r#"Expression "{}" fully evaluated as "{}""#, expr, value);
    Ok(value)
}

impl Display for Value<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Expression(e) => write!(f, "{}", e),
            Value::String(s) => write!(f, "{}", s),
        }
    }
}

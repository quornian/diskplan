use anyhow::{anyhow, Result};

use crate::{
    filesystem::{name, parent, SplitPath},
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
    path: &SplitPath,
) -> Result<String> {
    log::trace!("Evaluating: {}", expr);
    let mut value = String::new();
    for token in expr.tokens() {
        match token {
            Token::Text(text) => value.push_str(text),
            Token::Variable(var) => {
                let sub = stack::lookup(var, stack).ok_or_else(|| {
                    anyhow!("Undefined variable '{}' in expression '{}'", var, expr)
                })?;
                match sub {
                    Value::Expression(expr) => value.push_str(&evaluate(expr, stack, path)?),
                    Value::String(s) => value.push_str(s),
                }
            }
            Token::Special(special) => value.push_str(match special {
                Special::PathAbsolute => path.absolute(),
                Special::PathRelative => path.relative(),
                Special::PathNameOnly => name(path.relative()),
                Special::ParentAbsolute => parent(path.absolute())
                    .ok_or_else(|| anyhow!("Path has no parent: {}", path.absolute()))?,

                Special::ParentRelative => parent(path.relative())
                    .ok_or_else(|| anyhow!("Path has no parent: {}", path.relative()))?,
                Special::ParentNameOnly => name(
                    parent(path.relative())
                        .ok_or_else(|| anyhow!("Path has no parent: {}", path.relative()))?,
                ),
                Special::RootPath => path.root(),
            }),
        }
    }
    Ok(value)
}

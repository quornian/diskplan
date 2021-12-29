use anyhow::{anyhow, Result};

use crate::schema::{Expression, Identifier, Token};

use super::Scope;

enum Value<'a> {
    Expression(&'a Expression<'a>),
    String(&'a str),
}

pub(super) fn evaluate<'a>(expr: &Expression<'_>, stack: &[Scope]) -> Result<String> {
    let mut value = String::new();
    for token in expr.tokens() {
        match token {
            Token::Text(text) => value.push_str(text),
            Token::Variable(var) => {
                let sub = lookup(var, stack).ok_or_else(|| {
                    anyhow!("Undefined variable '{}' in expression '{}'", var, expr)
                })?;
                match sub {
                    Value::Expression(expr) => value.push_str(&evaluate(expr, stack)?),
                    Value::String(s) => value.push_str(s),
                }
            }
        }
    }
    Ok(value)
}

fn lookup<'a>(var: &Identifier<'a>, stack: &'a [Scope]) -> Option<Value<'a>> {
    stack.last().and_then(|top| {
        match top {
            &Scope::Directory(directory) => directory.get_var(var).map(Value::Expression),
            &Scope::Binding(bind, ref value) => {
                if bind == var {
                    Some(Value::String(value))
                } else {
                    None
                }
            }
        }
        .or_else(|| lookup(var, &stack[..stack.len() - 1]))
    })
}

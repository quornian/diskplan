use crate::schema::{DirectorySchema, Identifier, SchemaNode};

use super::eval::Value;

/// Keeps track of variables and provides access to definitions from parent
/// nodes
#[derive(Debug)]
pub struct Stack<'a> {
    parent: Option<&'a Stack<'a>>,
    scope: Scope<'a>,
}

impl<'a> Stack<'a> {
    pub fn new(parent: Option<&'a Stack<'a>>, scope: Scope<'a>) -> Self {
        Stack { parent, scope }
    }

    pub fn scope(&self) -> &Scope<'a> {
        &self.scope
    }
}

#[derive(Debug)]
pub enum Scope<'a> {
    Directory(&'a DirectorySchema<'a>),
    Binding(&'a Identifier<'a>, String),
}

impl<'a> Scope<'a> {
    pub fn as_binding(&self) -> Option<(&Identifier<'a>, &String)> {
        match self {
            Scope::Binding(id, value) => Some((id, value)),
            _ => None,
        }
    }
}

pub fn lookup<'a>(var: &Identifier<'a>, stack: Option<&'a Stack>) -> Option<Value<'a>> {
    if let Some(Stack { parent, scope, .. }) = stack {
        match scope {
            Scope::Directory(directory) => directory.get_var(var).map(Value::Expression),
            Scope::Binding(bind, ref value) => {
                if *bind == var {
                    Some(Value::String(value))
                } else {
                    None
                }
            }
        }
        .or_else(|| lookup(var, *parent))
    } else {
        None
    }
}

pub fn find_definition<'a>(
    var: &Identifier<'a>,
    stack: Option<&Stack<'a>>,
) -> Option<&'a SchemaNode<'a>> {
    log::trace!("Seeking definition of '{}'", var);
    if let Some(Stack { parent, scope, .. }) = stack {
        match scope {
            Scope::Directory(directory) => directory.get_def(var),
            _ => None,
        }
        .or_else(|| find_definition(var, *parent))
    } else {
        None
    }
}

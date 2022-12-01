use std::{
    collections::HashMap,
    fmt::{Debug, Display},
};

use crate::schema::{DirectorySchema, Identifier, SchemaNode};

use super::eval::Value;

/// Keeps track of variables and provides access to definitions from parent
/// nodes
pub struct Stack<'a> {
    parent: Option<&'a Stack<'a>>,
    frame: Frame<'a>,
}

impl<'a> Stack<'a> {
    pub fn new(parent: Option<&'a Stack<'a>>, frame: Frame<'a>) -> Self {
        Stack { parent, frame }
    }

    pub fn frame(&self) -> &Frame<'a> {
        &self.frame
    }
}

#[derive(Debug)]
pub enum Frame<'a> {
    Directory(&'a DirectorySchema<'a>),
    Binding(&'a Identifier<'a>, String),
    Map(HashMap<String, String>),
}

impl From<HashMap<String, String>> for Frame<'_> {
    fn from(map: HashMap<String, String>) -> Self {
        Frame::Map(map)
    }
}

impl From<HashMap<String, String>> for Stack<'_> {
    fn from(map: HashMap<String, String>) -> Self {
        Stack::new(None, Frame::Map(map))
    }
}

impl<'a> Frame<'a> {
    pub fn as_binding(&self) -> Option<(&Identifier<'a>, &String)> {
        match self {
            Frame::Binding(id, value) => Some((id, value)),
            _ => None,
        }
    }
}

pub fn lookup<'a>(var: &Identifier<'a>, stack: Option<&'a Stack>) -> Option<Value<'a>> {
    if let Some(Stack { parent, frame, .. }) = stack {
        match frame {
            Frame::Directory(directory) => directory.get_var(var).map(Value::Expression),
            Frame::Binding(bind, ref value) => {
                if *bind == var {
                    Some(Value::String(value))
                } else {
                    None
                }
            }
            Frame::Map(map) => map.get(var.value()).map(|s| Value::String(s.as_str())),
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
    if let Some(Stack { parent, frame, .. }) = stack {
        match frame {
            Frame::Directory(directory) => directory.get_def(var),
            _ => None,
        }
        .or_else(|| find_definition(var, *parent))
    } else {
        None
    }
}

impl Display for Stack<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.frame {
            Frame::Directory(directory_schema) => {
                write!(f, "Directory variables:",)?;
                let mut no_vars = true;
                for (ident, expr) in directory_schema.vars() {
                    no_vars = false;
                    write!(f, "\n  ${ident} = \"{expr}\"")?;
                }
                if no_vars {
                    write!(f, "\n  (no variables)",)?;
                }
            }
            Frame::Binding(ident, value) => {
                write!(f, "Schema binding:")?;
                write!(f, "\n  ${ident} = \"{value}\"",)?;
            }
            Frame::Map(map) => {
                write!(f, "Variable map:")?;
                for (key, value) in map.iter() {
                    write!(f, "\n  ${key} = \"{value}\"")?;
                }
            }
        }
        Ok(())
    }
}

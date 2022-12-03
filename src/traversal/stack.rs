use std::{
    collections::HashMap,
    fmt::{Debug, Display},
};

use nix::unistd::{Gid, Uid};

use crate::{
    config::{Config, NameMap},
    schema::{DirectorySchema, Identifier, SchemaNode},
    traversal::eval::Value,
};

/// Keeps track of variables and provides access to definitions from parent
/// nodes
pub struct StackFrame<'p, 'v> {
    parent: Option<&'p StackFrame<'p, 'v>>,

    /// Collection of variables and values at this level of the stack
    variables: VariableSource<'v>,

    /// The owner (after mapping) of this level, inherited by children
    owner: Uid,
    /// The group (after mapping) of this level, inherited by children
    group: Gid,

    /// A reference to the shared config
    pub config: &'v Config<'v>,
}

impl<'p, 'v> StackFrame<'p, 'v> {
    pub fn stack(
        owner: Uid,
        group: Gid,
        config: &'v Config<'v>,
        variables: VariableSource<'v>,
    ) -> Self {
        StackFrame {
            parent: None,
            variables,
            owner,
            group,
            config,
        }
    }

    pub fn push<'s, 'r>(&'s self, variables: VariableSource<'v>) -> StackFrame<'r, 'v>
    where
        'v: 'r,
        's: 'r,
    {
        StackFrame {
            parent: Some(self),
            variables,
            owner: self.owner,
            group: self.group,
            config: self.config,
        }
    }

    pub fn variables(&self) -> &VariableSource<'v> {
        &self.variables
    }

    pub fn lookup<'a>(&'a self, var: &Identifier<'a>) -> Option<Value<'a>> {
        match &self.variables {
            VariableSource::Empty => None,
            VariableSource::Directory(directory) => directory.get_var(var).map(Value::Expression),
            VariableSource::Binding(bind, ref value) => {
                if *bind == var {
                    Some(Value::String(value))
                } else {
                    None
                }
            }
            VariableSource::Map(map) => map.get(var.value()).map(|s| Value::String(s.as_str())),
        }
        .or_else(|| self.parent.and_then(|parent| parent.lookup(var)))
    }

    pub fn find_definition<'a, 'c>(
        &self,
        // stack: &StackFrame<'_, 'v>,
        var: &Identifier<'a>,
    ) -> Option<&'c SchemaNode<'v>>
    where
        'a: 'c,
    {
        match self.variables {
            VariableSource::Directory(directory) => directory.get_def(var),
            _ => None,
        }
        .or_else(|| self.parent.and_then(|parent| parent.find_definition(var)))
    }
}

#[derive(Debug)]
pub enum VariableSource<'a> {
    Empty,
    Directory(&'a DirectorySchema<'a>),
    Binding(&'a Identifier<'a>, String),
    Map(HashMap<String, String>),
}

impl Default for VariableSource<'_> {
    fn default() -> Self {
        VariableSource::Empty
    }
}

impl From<HashMap<String, String>> for VariableSource<'_> {
    fn from(map: HashMap<String, String>) -> Self {
        VariableSource::Map(map)
    }
}

impl From<NameMap> for VariableSource<'_> {
    fn from(map: NameMap) -> Self {
        VariableSource::Map(map.into())
    }
}

impl<'a> VariableSource<'a> {
    pub fn as_binding(&self) -> Option<(&Identifier<'a>, &String)> {
        match self {
            VariableSource::Binding(id, value) => Some((id, value)),
            _ => None,
        }
    }
}

impl Display for StackFrame<'_, '_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.variables {
            VariableSource::Empty => {}
            VariableSource::Directory(directory_schema) => {
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
            VariableSource::Binding(ident, value) => {
                write!(f, "Schema binding:")?;
                write!(f, "\n  ${ident} = \"{value}\"",)?;
            }
            VariableSource::Map(map) => {
                write!(f, "Variable map:")?;
                for (key, value) in map.iter() {
                    write!(f, "\n  ${key} = \"{value}\"")?;
                }
            }
        }
        Ok(())
    }
}

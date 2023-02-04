use std::{
    collections::HashMap,
    fmt::{Debug, Display},
};

use crate::eval::Value;
use diskplan_config::{Config, NameMap};
use diskplan_filesystem::Mode;
use diskplan_schema::{DirectorySchema, Identifier, SchemaNode};

/// Keeps track of variables and provides access to definitions from parent
/// nodes
///
/// Example:
/// ```
/// use diskplan_schema::DirectorySchema;
/// use diskplan_traversal::{StackFrame, VariableSource};
///
/// // The stack lifetimes allow us to have a function that takes a stack...
/// fn __<'g>(stack: &StackFrame<'g, '_, '_>, d: &'g DirectorySchema) {
///     // ...provides access to items referenced by parent scopes...
///     let var = stack.lookup(&"variable".into()).unwrap();
///
///     // ...can be extended with a mutable local scope...
///     let mut local = stack.push(VariableSource::Directory(d));
///
///     // ...and capture local modifications...
///     let owner = "root";
///     local.put_owner(owner);
/// }
/// ```
pub struct StackFrame<'g, 'p, 'l>
where
    'g: 'p, // The shared values pointed to live longer than the whole stack
    'p: 'l, // The local variables live within this frame, so can be shorter lived
{
    parent: Option<&'p StackFrame<'g, 'p, 'p>>,

    /// A reference to the shared config
    pub config: &'g Config<'g>,

    /// Collection of variables and values at this level of the stack
    variables: VariableSource<'g>,

    /// The owner (after mapping) of this level, inherited by children
    owner: &'l str,
    /// The group (after mapping) of this level, inherited by children
    group: &'l str,
    /// The mode of this level, inherited by children
    mode: Mode,
}

impl<'g, 'p, 'l> StackFrame<'g, 'p, 'l> {
    /// Constructs a new stack
    pub fn stack(
        config: &'g Config<'g>,
        variables: VariableSource<'g>,
        owner: &'l str,
        group: &'l str,
        mode: Mode,
    ) -> Self {
        StackFrame {
            parent: None,
            config,
            variables,
            owner,
            group,
            mode,
        }
    }

    /// Adds a new scope onto the stack, returning it
    pub fn push<'s, 'r>(&'s self, variables: VariableSource<'g>) -> StackFrame<'g, 'r, 'r>
    where
        'g: 'r,
        's: 'r,
    {
        StackFrame {
            parent: Some(self),
            variables,
            owner: self.owner,
            group: self.group,
            mode: self.mode,
            config: self.config,
        }
    }

    /// Changes the owner in the current scope
    pub fn put_owner(&mut self, owner: &'l str) {
        self.owner = owner;
    }

    /// Changes the group in the current scope
    pub fn put_group(&mut self, group: &'l str) {
        self.group = group;
    }

    /// Returns the owner in the current scope
    pub fn owner(&self) -> &'l str {
        self.owner
    }

    /// Returns the group in the current scope
    pub fn group(&self) -> &'l str {
        self.group
    }

    /// Returns the UNIX permissions set for the current scope
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// Provides access to variables in the current scope
    pub fn variables(&self) -> &VariableSource<'l> {
        &self.variables
    }

    /// Looks up the value of a variable in the current or parent scope(s)
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

    /// Looks up the definition of a sub-schema in the current or parent scope(s)
    pub fn find_definition<'a>(&self, var: &Identifier<'a>) -> Option<&'a SchemaNode<'g>> {
        match self.variables {
            VariableSource::Directory(directory) => directory.get_def(var),
            _ => None,
        }
        .or_else(|| self.parent.and_then(|parent| parent.find_definition(var)))
    }
}

/// Ways in which variables may be provided by the current scope
#[derive(Debug)]
pub enum VariableSource<'a> {
    /// No available variables
    Empty,
    /// A directory schema description, with its own variables
    Directory(&'a DirectorySchema<'a>),
    /// A binding of a schema to a single name
    Binding(&'a Identifier<'a>, String),
    /// A simple key-value map
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
    /// Convenience for attempting to cast to a single schema binding
    pub fn as_binding(&self) -> Option<(&Identifier<'a>, &String)> {
        match self {
            VariableSource::Binding(id, value) => Some((id, value)),
            _ => None,
        }
    }
}

impl Display for StackFrame<'_, '_, '_> {
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

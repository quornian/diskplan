use std::{collections::HashMap, path::Path};

use crate::definition::schema::Schema;

type Vars = HashMap<String, String>;

pub struct Context<'a> {
    pub schema: &'a Schema,
    pub target: &'a Path,
    pub stack: Stack<'a>,
}

impl<'a> Context<'a> {
    pub fn new(schema: &'a Schema, target: &'a Path) -> Context<'a> {
        Context {
            schema,
            target,
            stack: Stack::default(),
        }
    }
}

#[derive(Default)]
pub struct Stack<'a> {
    vars: Vars,
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

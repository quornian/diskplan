use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use crate::definition::schema::Schema;

// A note on lifetimes:
//  - The Context refers to a Schema, so the Schema must outlive the Context
//  - The Context's Stack refers to variables whose names are owned by the Schema
//    (its values are evaluated and thus owned by the Stack itself)
//  - The Stack has an optional parent Stack which must outlive it

pub struct Context<'a> {
    pub schema: &'a Schema,
    pub target: PathBuf,

    vars: HashMap<&'a str, String>,
    parent: Option<&'a Context<'a>>,
}

impl<'a> Context<'a> {
    pub fn new(schema: &'a Schema, target: &'a Path) -> Context<'a> {
        Context {
            schema,
            target: target.to_owned(),
            vars: HashMap::new(),
            parent: None,
        }
    }

    pub fn child<'ch>(&'a self, target: PathBuf, schema: &'a Schema) -> Context<'ch>
    where
        'a: 'ch,
    {
        Context {
            schema,
            target,
            parent: Some(&self),
            vars: HashMap::new(),
        }
    }

    pub fn lookup<S>(&self, var: S) -> Option<&String>
    where
        S: AsRef<str>,
    {
        self.vars
            .get(var.as_ref())
            .or_else(|| self.parent.as_deref().and_then(|parent| parent.lookup(var)))
    }

    pub fn follow<'ch, S>(&'a self, var: S) -> Option<Context<'ch>>
    where
        'a: 'ch,
        S: AsRef<str>,
    {
        let var = var.as_ref();
        self.follow_schema(var)
            .and_then(|far_schema| Some(self.child(self.target.clone(), far_schema)))
    }

    fn follow_schema(&'a self, var: &str) -> Option<&Schema> {
        if let Schema::Directory(directory_schema) = self.schema {
            if let Some(child_schema) = directory_schema.defs().get(var) {
                return Some(child_schema);
            }
        }
        self.parent.and_then(|parent| parent.follow_schema(var))
    }

    pub fn bind(&mut self, var: &'a str, value: &str) {
        self.vars.insert(var, value.into());
    }
}

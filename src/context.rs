//! Provides [`Context`] for pairing schemas with physical filesystem nodes on disk
//!
use std::{
    borrow::Cow,
    collections::HashMap,
    path::{Path, PathBuf},
};

use crate::schema::{Expression, Identifier, Schema, Token};
use anyhow::{anyhow, Result};

// A note on lifetimes:
//  - The Context refers to a Schema, so the Schema must outlive the Context
//  - The Context's Stack refers to variables whose names are owned by the Schema
//    (its values are evaluated and thus owned by the Stack itself)
//  - The Stack has an optional parent Stack which must outlive it

/// A location on the filesystem paired with the schema tree node being applied
pub struct Context<'t: 'a, 'a> {
    pub schema: &'a Schema<'t>,
    pub root: &'a Path,
    pub target: PathBuf,

    bound_vars: HashMap<Identifier<'t>, String>,
    parent: Option<&'a Context<'t, 'a>>,
}

impl<'t, 'a> Context<'t, 'a> {
    pub fn new(schema: &'a Schema<'t>, root: &'a Path, target: &'a Path) -> Context<'t, 'a> {
        assert!(target.is_relative());
        Context {
            schema,
            root,
            target: target.to_owned(),
            bound_vars: HashMap::new(),
            parent: None,
        }
    }

    pub fn child<'ch>(&'a self, target: PathBuf, schema: &'a Schema<'t>) -> Context<'t, 'ch>
    where
        'a: 'ch,
    {
        Context {
            schema,
            root: self.root,
            target,
            parent: Some(&self),
            bound_vars: HashMap::new(),
        }
    }

    pub fn lookup(&'a self, var: &Identifier<'t>) -> Result<Option<Cow<'a, str>>> {
        if let Some(bound) = self.bound_vars.get(var) {
            return Ok(Some(bound.into()));
        }
        if let Schema::Directory(directory_schema) = self.schema {
            if let Some(expr) = directory_schema.vars().get(var) {
                return self.evaluate(expr).map(Into::into).map(Some);
            }
        }
        if let Some(parent) = self.parent {
            return parent.lookup(var);
        }
        Ok(None)
    }

    pub fn follow<'ch>(&'a self, var: &Identifier<'t>) -> Option<Context<'t, 'ch>>
    where
        'a: 'ch,
    {
        self.follow_schema(var)
            .and_then(|far_schema| Some(self.child(self.target.clone(), far_schema)))
    }

    pub fn follow_schema(&self, var: &Identifier<'t>) -> Option<&Schema<'t>> {
        if let Schema::Directory(directory_schema) = self.schema {
            if let Some(child_schema) = directory_schema.defs().get(var) {
                return Some(child_schema);
            }
        }
        self.parent.and_then(|parent| parent.follow_schema(var))
    }

    // FIXME: Parse and error after parsing refactor
    pub fn bind(&mut self, var: Identifier<'t>, value: String) {
        self.bound_vars.insert(var, value);
    }

    pub fn evaluate(&self, expr: &Expression<'t>) -> Result<String> {
        let mut buffer = String::new();
        for token in expr.tokens() {
            match token {
                Token::Text(text) => buffer.push_str(text),
                Token::Variable(var) => {
                    match var.value().as_ref() {
                        "PATH" => buffer.push_str(self.target.to_string_lossy().as_ref()),
                        "PARENT" => buffer.push_str(
                            self.target
                                .parent()
                                .ok_or_else(|| anyhow!("Path has no parent"))?
                                .to_string_lossy()
                                .as_ref(),
                        ),
                        "NAME" => buffer.push_str(
                            self.target
                                .file_name()
                                .ok_or_else(|| anyhow!("Path has no valid name"))?
                                .to_string_lossy()
                                .as_ref(),
                        ),
                        _ => buffer.push_str(
                            self.lookup(var)?
                                .ok_or_else(|| anyhow!("No such variable: {}", var.value()))?
                                .as_ref(),
                        ),
                    };
                }
            }
        }
        return Ok(buffer);
    }
}

#[cfg(test)]
mod tests {
    use crate::schema::{
        DirectorySchema, Identifier, LinkSchema, Match, Meta, SchemaEntry, Subschema,
    };

    use super::*;

    #[test]
    fn test_full_schema_expr() {
        let schema = Schema::Directory({
            let vars = [(
                Identifier::new("absvar"),
                Expression::new(vec![Token::Text("/tmp/abs")]),
            )]
            .iter()
            .cloned()
            .collect();
            let defs = HashMap::new();
            let meta = Meta::default();
            let entries = vec![SchemaEntry {
                criteria: Match::Fixed("link"),
                subschema: Subschema::Original(Schema::Symlink(LinkSchema::new(
                    Expression::new(vec![
                        Token::Variable(Identifier::new("@absvar")),
                        Token::Text("/sub"),
                    ]),
                    Some(Box::new(Schema::Directory(DirectorySchema::default()))),
                ))),
            }];
            DirectorySchema::new(vars, defs, meta, entries)
        });
        let root = Path::new("/tmp/root");
        let target = Path::new(".");
        let context = Context::new(&schema, root, target);

        assert_eq!(
            context.lookup(&Identifier::new("absvar")).unwrap(),
            Some(Cow::from("/tmp/abs"))
        );

        let expr = Expression::new(vec![Token::Variable(Identifier::new("absvar"))]);
        assert_eq!(context.evaluate(&expr).unwrap(), "/tmp/abs");
    }
}

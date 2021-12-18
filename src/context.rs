use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use crate::schema::{Expression, Identifier, Schema, Token};
use anyhow::{anyhow, Context as _, Result};

// A note on lifetimes:
//  - The Context refers to a Schema, so the Schema must outlive the Context
//  - The Context's Stack refers to variables whose names are owned by the Schema
//    (its values are evaluated and thus owned by the Stack itself)
//  - The Stack has an optional parent Stack which must outlive it

pub struct Context<'a> {
    pub schema: &'a Schema,
    pub root: &'a Path,
    pub target: PathBuf,

    bound_vars: HashMap<Identifier, Expression>,
    parent: Option<&'a Context<'a>>,
}

impl<'a> Context<'a> {
    pub fn new(schema: &'a Schema, root: &'a Path, target: &'a Path) -> Context<'a> {
        assert!(target.is_relative());
        Context {
            schema,
            root,
            target: target.to_owned(),
            bound_vars: HashMap::new(),
            parent: None,
        }
    }

    pub fn child<'ch>(&'a self, target: PathBuf, schema: &'a Schema) -> Context<'ch>
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

    pub fn lookup(&self, var: &'a Identifier) -> Option<&Expression> {
        self.bound_vars
            .get(var)
            .or_else(|| {
                if let Schema::Directory(directory_schema) = self.schema {
                    directory_schema.vars().get(var)
                } else {
                    None
                }
            })
            .or_else(|| self.parent.and_then(|parent| parent.lookup(var)))
    }

    pub fn follow<'ch>(&'a self, var: &'a Identifier) -> Option<Context<'ch>>
    where
        'a: 'ch,
    {
        self.follow_schema(var)
            .and_then(|far_schema| Some(self.child(self.target.clone(), far_schema)))
    }

    pub fn follow_schema(&'a self, var: &Identifier) -> Option<&Schema> {
        if let Schema::Directory(directory_schema) = self.schema {
            if let Some(child_schema) = directory_schema.defs().get(var) {
                return Some(child_schema);
            }
        }
        self.parent.and_then(|parent| parent.follow_schema(var))
    }

    // FIXME: Parse and error after parsing refactor
    pub fn bind(&mut self, var: Identifier, value: Expression) {
        self.bound_vars.insert(var, value);
    }

    pub fn evaluate(&self, expr: &Expression) -> Result<String> {
        let mut buffer = String::new();
        for token in expr.tokens() {
            match token {
                Token::Text(text) => buffer.push_str(text),
                Token::Variable(var) => {
                    let value = match var.value().as_ref() {
                        "PATH" => String::from(self.target.to_string_lossy()),
                        "PARENT" => String::from(
                            self.target
                                .parent()
                                .ok_or_else(|| anyhow!("Path has no parent"))?
                                .to_string_lossy(),
                        ),
                        "NAME" => String::from(
                            self.target
                                .file_name()
                                .ok_or_else(|| anyhow!("Path has no valid name"))?
                                .to_string_lossy(),
                        ),
                        _ => {
                            let value = self
                                .lookup(var)
                                .ok_or_else(|| anyhow!("No such variable: {}", var.value()))?;

                            self.evaluate(&value).with_context(|| {
                                format!(
                                    "Failed to evaluate value of {} in {} (from {})",
                                    var.value(),
                                    value,
                                    expr,
                                )
                            })?
                        }
                    };
                    buffer.push_str(&value);
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
                Expression::new(vec![Token::text("/tmp/abs")]),
            )]
            .iter()
            .cloned()
            .collect();
            let defs = HashMap::new();
            let meta = Meta::default();
            let entries = vec![SchemaEntry {
                criteria: Match::fixed("link"),
                subschema: Subschema::Original(Schema::Symlink(LinkSchema::new(
                    Expression::new(vec![Token::variable("@absvar"), Token::text("/sub")]),
                    Some(Box::new(Schema::Directory(DirectorySchema::default()))),
                ))),
            }];
            DirectorySchema::new(vars, defs, meta, entries)
        });
        let root = Path::new("/tmp/root");
        let target = Path::new(".");
        let context = Context::new(&schema, root, target);

        assert_eq!(
            context.lookup(&Identifier::new("absvar")),
            Some(&Expression::new(vec![Token::text("/tmp/abs")]))
        );

        let expr = Expression::new(vec![Token::variable("absvar")]);
        assert_eq!(context.evaluate(&expr).unwrap(), "/tmp/abs");
    }
}

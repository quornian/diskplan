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

    bound_vars: HashMap<&'a str, String>,
    parent: Option<&'a Context<'a>>,
}

impl<'a> Context<'a> {
    pub fn new(schema: &'a Schema, target: &'a Path) -> Context<'a> {
        Context {
            schema,
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
            target,
            parent: Some(&self),
            bound_vars: HashMap::new(),
        }
    }

    pub fn lookup<S>(&self, var: S) -> Option<&String>
    where
        S: AsRef<str>,
    {
        self.bound_vars
            .get(var.as_ref())
            .or_else(|| {
                if let Schema::Directory(directory_schema) = self.schema {
                    directory_schema.vars().get(var.as_ref())
                } else {
                    None
                }
            })
            .or_else(|| self.parent.and_then(|parent| parent.lookup(var)))
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
        self.bound_vars.insert(var, value.into());
    }
}

#[cfg(test)]
mod tests {
    use std::convert::TryFrom;

    use crate::{
        application::{eval::Evaluate, parse::Expr},
        definition::{
            criteria::{Match, MatchCriteria},
            meta::Meta,
            schema::{DirectorySchema, LinkSchema},
        },
    };

    use super::*;

    #[test]
    fn test_full_schema_expr() {
        let schema = Schema::Directory({
            let vars = [("absvar".to_owned(), "/tmp/abs".to_owned())]
                .iter()
                .cloned()
                .collect();
            let defs = HashMap::new();
            let meta = Meta::default();
            let entries = vec![(
                MatchCriteria::new(0, Match::Fixed("link".to_owned())),
                Schema::Symlink(LinkSchema::new(
                    "@absvar/sub".to_owned(),
                    Schema::Directory(DirectorySchema::default()),
                )),
            )];
            DirectorySchema::new(vars, defs, meta, entries)
        });
        let target = Path::new("/tmp/root");
        let context = Context::new(&schema, target);

        assert_eq!(context.lookup("absvar"), Some(&"/tmp/abs".to_owned()));

        assert_eq!(context.evaluate("@absvar"), Ok("/tmp/abs".to_owned()));
    }
}

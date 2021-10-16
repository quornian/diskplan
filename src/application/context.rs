use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

type Vars = HashMap<String, String>;

pub struct Context<'a> {
    pub path: PathBuf,
    pub stack: Stack<'a>,
}

impl Context<'_> {
    pub fn new(path: &Path) -> Context {
        Context {
            path: path.to_owned(),
            stack: Stack::default(),
        }
    }

    pub fn child(&self, name: &str, vars: Vars) -> Context {
        Context {
            path: self.path.join(name),
            stack: Stack {
                vars: vars,
                parent: Some(&self.stack),
            },
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

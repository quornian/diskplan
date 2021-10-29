use std::collections::HashMap;

use super::expr::Expression;

pub struct SchemaBuilder {
    owner: Option<String>,
    group: Option<String>,
    permissions: Option<u16>,

    vars: HashMap<String, Expression>,
    defs: HashMap<String, SchemaBuilder>,
    entries: Vec<SchemaBuilder>,
}

impl SchemaBuilder {
    pub fn new() -> SchemaBuilder {
        SchemaBuilder {
            owner: None,
            group: None,
            permissions: None,

            vars: HashMap::new(),
            defs: HashMap::new(),
            entries: Vec::new(),
        }
    }

    pub fn owner(&mut self, owner: &str) -> &mut SchemaBuilder {
        self.owner = Some(owner.to_owned());
        self
    }

    pub fn group(&mut self, group: &str) -> &mut SchemaBuilder {
        self.group = Some(group.to_owned());
        self
    }

    pub fn permissions(&mut self, permissions: u16) -> &mut SchemaBuilder {
        self.permissions = Some(permissions);
        self
    }
}

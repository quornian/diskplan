use std::collections::HashMap;

use crate::schema::{
    expr::{Expression, Identifier},
    meta::Meta,
    Binding, DirectorySchema, FileSchema, Schema,
};

use super::ItemType;

pub struct Properties<'t> {
    item_type: ItemType,
    match_expr: Option<Expression<'t>>,
    symlink: Option<Expression<'t>>,
    uses: Vec<Identifier<'t>>,
    vars: HashMap<Identifier<'t>, Expression<'t>>,
    defs: HashMap<Identifier<'t>, Properties<'t>>,
    meta: Meta<'t>,
    // Directory only
    entries: Vec<(Binding<'t>, Properties<'t>)>,
    // File only
    source: Option<Expression<'t>>,
}

impl<'t> Properties<'t> {
    pub(in crate::schema::text) fn new(
        item_type: ItemType,
        symlink: Option<Expression<'t>>,
    ) -> Self {
        Properties {
            item_type,
            match_expr: None,
            symlink,
            uses: Vec::new(),
            vars: HashMap::new(),
            defs: HashMap::new(),
            meta: Meta::default(),
            entries: Vec::new(),
            source: None,
        }
    }

    pub(super) fn has_match(&self) -> bool {
        self.match_expr.is_some()
    }
    pub(super) fn has_use(&self) -> bool {
        !self.uses.is_empty()
    }

    pub fn match_expr(&mut self, expr: Expression<'t>) -> Result<(), String> {
        if let Some(_) = self.match_expr.replace(expr) {
            return Err(format!("#match occurs twice"));
        }
        Ok(())
    }
    pub fn let_var(&mut self, id: Identifier<'t>, expr: Expression<'t>) -> Result<(), String> {
        if let Some(_) = self.vars.insert(id, expr) {
            return Err(format!("#let occurs twice"));
        }
        Ok(())
    }
    pub fn define(&mut self, id: Identifier<'t>, definition: Properties<'t>) -> Result<(), String> {
        if let Some(_) = self.defs.insert(id, definition) {
            return Err(format!("#def occurs twice"));
        }
        Ok(())
    }
    pub fn use_definition(&mut self, id: Identifier<'t>) -> Result<(), String> {
        if self.source.is_some() {
            return Err(format!("#use cannot be used in conjunction with #source"));
        }
        self.uses.push(id);
        self.source = Some(Expression::new(vec![]));
        Ok(())
    }
    pub fn owner(&mut self, owner: &'t str) -> Result<(), String> {
        if let Some(_) = self.meta.owner.replace(owner) {
            return Err(format!("#owner occurs twice"));
        }
        Ok(())
    }
    pub fn group(&mut self, group: &'t str) -> Result<(), String> {
        if let Some(_) = self.meta.group.replace(group) {
            return Err(format!("#group occurs twice"));
        }
        Ok(())
    }
    pub fn mode(&mut self, mode: u16) -> Result<(), String> {
        if let Some(_) = self.meta.mode.replace(mode) {
            return Err(format!("#mode occurs twice"));
        }
        Ok(())
    }
    pub fn source(&mut self, source: Expression<'t>) -> Result<(), String> {
        if !self.uses.is_empty() {
            return Err(format!("#source cannot be used in conjunction with #use"));
        }
        if let Some(_) = self.source.replace(source) {
            return Err(format!("#source occurs twice"));
        }
        Ok(())
    }
    pub fn add_entry(&mut self, binding: Binding<'t>, entry: Properties<'t>) -> Result<(), String> {
        self.entries.push((binding, entry));
        Ok(())
    }

    pub fn try_into_schema(self, parents: &[&DirectorySchema]) -> Result<Schema<'t>, String> {
        let Properties {
            item_type,
            match_expr,
            symlink,
            uses,
            vars,
            defs,
            meta,
            entries,
            source,
        } = self;
        Ok(match item_type {
            ItemType::Directory => {
                let defs = HashMap::new(); // FIXME
                let entries = Vec::new(); // FIXME
                Schema::Directory(DirectorySchema::new(
                    symlink, uses, vars, defs, meta, entries,
                ))
            }
            ItemType::File => Schema::File(FileSchema::new(
                symlink,
                uses,
                meta,
                source.ok_or_else(|| {
                    format!("File has no #source (or #use). Should this have been a directory?")
                })?,
            )),
        })
    }
}

use std::collections::HashMap;

use crate::schema::{
    criteria::Match,
    expr::{Expression, Identifier},
    meta::{Meta, MetaBuilder},
    DirectorySchema, FileSchema, Schema, SchemaEntry, Subschema,
};

use super::ItemType;

pub struct Properties<'t> {
    item_type: ItemType,
    match_expr: Option<Expression<'t>>,
    inner: InnerProperties<'t>,

    // Set if this schema inherits a definition from elsewhere
    use_def: Option<Identifier<'t>>,
}

#[derive(Default)]
struct InnerProperties<'t> {
    symlink: Option<Expression<'t>>,

    vars: HashMap<Identifier<'t>, Expression<'t>>,
    defs: HashMap<Identifier<'t>, Schema<'t>>,

    owner: Option<&'t str>,
    group: Option<&'t str>,
    mode: Option<u16>,

    // Directory only
    entries: Vec<SchemaEntry<'t>>,
    // File only
    source: Option<Expression<'t>>,
}

impl<'t> Properties<'t> {
    pub(in crate::schema::text) fn new(
        item_type: ItemType,
        symlink: Option<Expression<'t>>,
    ) -> Self {
        let inner = InnerProperties {
            symlink,
            ..Default::default()
        };
        Properties {
            item_type,
            match_expr: None,
            inner,
            use_def: None,
        }
    }

    pub fn match_expr(&mut self, expr: Expression<'t>) -> Result<(), String> {
        if let Some(_) = self.match_expr.replace(expr) {
            return Err(format!("#match occurs twice"));
        }
        Ok(())
    }
    pub fn let_var(&mut self, id: Identifier<'t>, expr: Expression<'t>) -> Result<(), String> {
        if let Some(_) = self.inner.vars.insert(id, expr) {
            return Err(format!("#let occurs twice"));
        }
        Ok(())
    }
    pub fn define(&mut self, id: Identifier<'t>, schema: Schema<'t>) -> Result<(), String> {
        if let Some(_) = self.inner.defs.insert(id, schema) {
            return Err(format!("#def occurs twice"));
        }
        Ok(())
    }
    pub fn use_definition(&mut self, id: Identifier<'t>) -> Result<(), String> {
        if self.inner.source.is_some() {
            return Err(format!("#use cannot be used in conjunction with #source"));
        }
        if let Some(_) = self.use_def.replace(id) {
            return Err(format!("#use occurs twice"));
        }
        self.inner.source = Some(Expression::new(vec![]));
        Ok(())
    }
    pub fn owner(&mut self, owner: &'t str) -> Result<(), String> {
        if let Some(_) = self.inner.owner.replace(owner) {
            return Err(format!("#owner occurs twice"));
        }
        Ok(())
    }
    pub fn group(&mut self, group: &'t str) -> Result<(), String> {
        if let Some(_) = self.inner.group.replace(group) {
            return Err(format!("#group occurs twice"));
        }
        Ok(())
    }
    pub fn mode(&mut self, mode: u16) -> Result<(), String> {
        if let Some(_) = self.inner.mode.replace(mode) {
            return Err(format!("#mode occurs twice"));
        }
        Ok(())
    }
    pub fn source(&mut self, source: Expression<'t>) -> Result<(), String> {
        if self.use_def.is_some() {
            return Err(format!("#source cannot be used in conjunction with #use"));
        }
        if let Some(_) = self.inner.source.replace(source) {
            return Err(format!("#source occurs twice"));
        }
        Ok(())
    }
    pub fn add_entry(
        &mut self,
        criteria: Match<'t>,
        subschema: Subschema<'t>,
    ) -> Result<(), String> {
        self.inner.entries.push(SchemaEntry {
            criteria,
            subschema,
        });
        Ok(())
    }

    pub fn to_mapped_subschema(self) -> Result<(Option<Expression<'t>>, Subschema<'t>), String> {
        let schema = match self.item_type {
            ItemType::Directory => Schema::Directory(self.inner.into_directory()?),
            ItemType::File => Schema::File(self.inner.into_file()?),
        };
        let subschema = match self.use_def {
            Some(use_def) => Subschema::Referenced {
                definition: use_def,
                overrides: schema,
            },
            None => Subschema::Original(schema),
        };
        Ok((self.match_expr, subschema))
    }
}

impl<'t> InnerProperties<'t> {
    pub fn build_meta(&self) -> Meta<'t> {
        let mut meta = MetaBuilder::default();
        if let Some(owner) = self.owner {
            meta = meta.owner(owner);
        }
        if let Some(group) = self.group {
            meta = meta.group(group);
        }
        if let Some(mode) = self.mode {
            meta = meta.mode(mode);
        }
        meta.build()
    }

    pub fn into_directory(self) -> Result<DirectorySchema<'t>, String> {
        let meta = self.build_meta();
        Ok(DirectorySchema::new(
            self.symlink,
            self.vars,
            self.defs,
            meta,
            self.entries,
        ))
    }

    pub fn into_file(self) -> Result<FileSchema<'t>, String> {
        // Files must have a #source unless they are #use-ing a definition from elsewhere
        let meta = self.build_meta();
        let source = if let Some(source) = self.source {
            Ok(source)
        } else {
            Err(format!(
                "File has no #source (or #use). Should this have been a directory?"
            ))
        }?;
        Ok(FileSchema::new(self.symlink, meta, source))
    }
}

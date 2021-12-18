use std::collections::HashMap;

use crate::schema::{
    criteria::Match,
    expr::{Expression, Identifier},
    meta::{Meta, MetaBuilder},
    DirectorySchema, FileSchema, LinkSchema, Schema, SchemaEntry, Subschema,
};

use super::ItemType;

pub struct Properties<'a, 'i> {
    item_type: &'i ItemType,
    match_expr: Option<Expression>,
    inner: InnerProperties<'a>,

    // Set if this schema inherits a definition from elsewhere
    use_def: Option<Identifier>,
}

#[derive(Default)]
struct InnerProperties<'a> {
    vars: HashMap<Identifier, Expression>,
    defs: HashMap<Identifier, Schema>,

    owner: Option<&'a str>,
    group: Option<&'a str>,
    mode: Option<u16>,

    // Directory only
    entries: Vec<SchemaEntry>,
    // File only
    source: Option<Expression>,
}

impl<'a, 'i> Properties<'a, 'i> {
    pub(in crate::schema::text) fn new(item_type: &'i ItemType) -> Self {
        Properties {
            item_type,
            match_expr: None,
            inner: Default::default(),
            use_def: None,
        }
    }

    pub fn match_expr(&mut self, expr: Expression) -> Result<(), String> {
        if let Some(_) = self.match_expr.replace(expr) {
            return Err(format!("#match occurs twice"));
        }
        Ok(())
    }
    pub fn let_var(&mut self, id: Identifier, expr: Expression) -> Result<(), String> {
        if let Some(_) = self.inner.vars.insert(id, expr) {
            return Err(format!("#let occurs twice"));
        }
        Ok(())
    }
    pub fn define(&mut self, id: Identifier, schema: Schema) -> Result<(), String> {
        if let Some(_) = self.inner.defs.insert(id, schema) {
            return Err(format!("#def occurs twice"));
        }
        Ok(())
    }
    pub fn use_definition(&mut self, id: Identifier) -> Result<(), String> {
        if self.inner.source.is_some() {
            return Err(format!("#use cannot be used in conjunction with #source"));
        }
        if let Some(_) = self.use_def.replace(id) {
            return Err(format!("#use occurs twice"));
        }
        self.inner.source = Some(Expression::new(vec![]));
        Ok(())
    }
    pub fn owner(&mut self, owner: &'a str) -> Result<(), String> {
        if let Some(_) = self.inner.owner.replace(owner) {
            return Err(format!("#owner occurs twice"));
        }
        Ok(())
    }
    pub fn group(&mut self, group: &'a str) -> Result<(), String> {
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
    pub fn source(&mut self, source: Expression) -> Result<(), String> {
        if self.use_def.is_some() {
            return Err(format!("#source cannot be used in conjunction with #use"));
        }
        if let Some(_) = self.inner.source.replace(source) {
            return Err(format!("#source occurs twice"));
        }
        Ok(())
    }
    pub fn add_entry(&mut self, criteria: Match, subschema: Subschema) -> Result<(), String> {
        self.inner.entries.push(SchemaEntry {
            criteria,
            subschema,
        });
        Ok(())
    }

    pub fn to_mapped_subschema(self) -> Result<(Option<Expression>, Subschema), String> {
        let schema = match self.item_type {
            ItemType::Directory => Schema::Directory(self.inner.into_directory()?),
            ItemType::File => Schema::File(self.inner.into_file()?),
            ItemType::Symlink {
                target,
                is_directory,
            } => Schema::Symlink(self.inner.into_symlink(target.clone(), *is_directory)?),
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

impl InnerProperties<'_> {
    pub fn build_meta(&self) -> Meta {
        let mut meta = MetaBuilder::default();
        if let Some(owner) = self.owner {
            meta.owner(owner);
        }
        if let Some(group) = self.group {
            meta.group(group);
        }
        if let Some(mode) = self.mode {
            meta.mode(mode);
        }
        meta.build()
    }

    pub fn into_directory(self) -> Result<DirectorySchema, String> {
        let meta = self.build_meta();
        Ok(DirectorySchema::new(
            self.vars,
            self.defs,
            meta,
            self.entries,
        ))
    }

    pub fn into_file(self) -> Result<FileSchema, String> {
        // Files must have a #source unless they are #use-ing a definition from elsewhere
        let meta = self.build_meta();
        let source = if let Some(source) = self.source {
            Ok(source)
        } else {
            Err(format!(
                "File has no #source (or #use). Should this have been a directory?"
            ))
        }?;
        Ok(FileSchema::new(meta, source))
    }

    pub fn into_symlink(
        self,
        target: Expression,
        is_directory: bool,
    ) -> Result<LinkSchema, String> {
        let meta = self.build_meta();
        let schema = if self.vars.is_empty()
            && self.defs.is_empty()
            && meta.is_empty()
            && self.entries.is_empty()
        {
            None
        } else if is_directory {
            Some(Box::new(Schema::Directory(DirectorySchema::new(
                self.vars,
                self.defs,
                meta,
                self.entries,
            ))))
        } else {
            Some(Box::new(if let Some(source) = self.source {
                Schema::File(FileSchema::new(meta, source))
            } else {
                return Err(format!(
                    "File has no #source. Should this have been a directory?"
                ));
            }))
        };
        Ok(LinkSchema::new(target.clone(), schema))
    }
}

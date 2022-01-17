use std::collections::HashMap;

use anyhow::{anyhow, Result};

use crate::schema::{
    Attributes, Binding, DirectorySchema, Expression, FileSchema, Identifier, Schema, SchemaNode,
};

use super::NodeType;

#[derive(Debug)]
pub struct SchemaNodeBuilder<'t> {
    match_pattern: Option<Expression<'t>>,
    avoid_pattern: Option<Expression<'t>>,
    symlink: Option<Expression<'t>>,
    uses: Vec<Identifier<'t>>,
    attributes: Attributes<'t>,
    type_specific: TypeSpecific<'t>,
}

#[derive(Debug)]
enum TypeSpecific<'t> {
    Directory {
        vars: HashMap<Identifier<'t>, Expression<'t>>,
        defs: HashMap<Identifier<'t>, SchemaNode<'t>>,
        entries: Vec<(Binding<'t>, SchemaNode<'t>)>,
    },
    File {
        source: Option<Expression<'t>>,
    },
}

impl<'t> SchemaNodeBuilder<'t> {
    pub fn new(node_type: NodeType, symlink: Option<Expression<'t>>) -> Self {
        SchemaNodeBuilder {
            match_pattern: None,
            avoid_pattern: None,
            symlink,
            uses: Vec::new(),
            attributes: Attributes::default(),

            type_specific: match node_type {
                NodeType::Directory => TypeSpecific::Directory {
                    vars: HashMap::new(),
                    defs: HashMap::new(),
                    entries: Vec::new(),
                },
                NodeType::File => TypeSpecific::File { source: None },
            },
        }
    }

    pub fn match_pattern(&mut self, pattern: Expression<'t>) -> Result<()> {
        if self.match_pattern.is_some() {
            return Err(anyhow!("#match occurs twice"));
        }
        self.match_pattern = Some(pattern);
        Ok(())
    }

    pub fn avoid_pattern(&mut self, pattern: Expression<'t>) -> Result<()> {
        if self.avoid_pattern.is_some() {
            return Err(anyhow!("#avoid occurs twice"));
        }
        self.avoid_pattern = Some(pattern);
        Ok(())
    }

    pub fn let_var(&mut self, id: Identifier<'t>, expr: Expression<'t>) -> Result<()> {
        match &mut self.type_specific {
            TypeSpecific::File { .. } => Err(anyhow!(
                "Cannot use #let to set variables inside files (add a '/' to make it a directory)"
            )),
            TypeSpecific::Directory { vars, .. } => {
                if vars.contains_key(&id) {
                    Err(anyhow!("#let {} occurs twice", id))
                } else {
                    vars.insert(id, expr);
                    Ok(())
                }
            }
        }
    }

    pub fn define(&mut self, id: Identifier<'t>, definition: SchemaNode<'t>) -> Result<()> {
        match &mut self.type_specific {
            TypeSpecific::File { .. } => Err(anyhow!(
                "Cannot #define sub-trees inside files (add a '/' to make it a directory)"
            )),
            TypeSpecific::Directory { defs, .. } => {
                if defs.contains_key(&id) {
                    Err(anyhow!("#def {} occurs twice", id))
                } else {
                    defs.insert(id, definition);
                    Ok(())
                }
            }
        }
    }

    pub fn use_definition(&mut self, id: Identifier<'t>) -> Result<()> {
        if let TypeSpecific::File { source, .. } = &self.type_specific {
            if source.is_some() {
                return Err(anyhow!("#use cannot be used in conjunction with #source"));
            }
        }
        self.uses.push(id);
        Ok(())
    }

    pub fn owner(&mut self, owner: Expression<'t>) -> Result<()> {
        if self.attributes.owner.is_some() {
            return Err(anyhow!("#owner occurs twice"));
        }
        self.attributes.owner = Some(owner);
        Ok(())
    }

    pub fn group(&mut self, group: Expression<'t>) -> Result<()> {
        if self.attributes.group.is_some() {
            return Err(anyhow!("#group occurs twice"));
        }
        self.attributes.group = Some(group);
        Ok(())
    }

    pub fn mode(&mut self, mode: u16) -> Result<()> {
        if self.attributes.mode.is_some() {
            return Err(anyhow!("#mode occurs twice"));
        }
        self.attributes.mode = Some(mode);
        Ok(())
    }

    pub fn source(&mut self, source: Expression<'t>) -> Result<()> {
        match self.type_specific {
            TypeSpecific::Directory { .. } => Err(anyhow!(
                "#source can only be used for files, not directories"
            )),
            TypeSpecific::File {
                source: ref mut src,
            } => {
                if !self.uses.is_empty() {
                    Err(anyhow!("#source cannot be used in conjunction with #use"))
                } else if src.is_some() {
                    Err(anyhow!("#source occurs twice"))
                } else {
                    *src = Some(source);
                    Ok(())
                }
            }
        }
    }

    pub fn add_entry(&mut self, binding: Binding<'t>, entry: SchemaNode<'t>) -> Result<()> {
        match &mut self.type_specific {
            TypeSpecific::File { .. } => Err(anyhow!(
                "Files cannot have child items (add a '/' to make it a directory)"
            )),
            TypeSpecific::Directory { entries, .. } => {
                // TODO: Check for duplicates
                entries.push((binding, entry));
                Ok(())
            }
        }
    }

    pub fn build(self) -> Result<SchemaNode<'t>> {
        let SchemaNodeBuilder {
            match_pattern,
            avoid_pattern,
            symlink,
            uses,
            attributes,
            type_specific,
        } = self;
        let schema = match type_specific {
            TypeSpecific::Directory {
                vars,
                defs,
                entries,
            } => Schema::Directory(DirectorySchema::new(vars, defs, entries)),
            TypeSpecific::File { source } => {
                let source = source.ok_or_else(|| anyhow!("File must have a #source"))?;
                Schema::File(FileSchema::new(source))
            }
        };
        Ok(SchemaNode {
            match_pattern,
            avoid_pattern,
            symlink,
            uses,
            attributes,
            schema,
        })
    }
}

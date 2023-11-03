use std::collections::{hash_map::Entry, HashMap};

use anyhow::{anyhow, bail, Result};

use crate::{
    Attributes, Binding, DirectorySchema, Expression, FileSchema, Identifier, SchemaNode,
    SchemaType,
};

use super::NodeType;

#[derive(Debug)]
pub struct SchemaNodeBuilder<'t> {
    line: &'t str,
    is_def: bool,
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
    pub fn new(
        line: &'t str,
        is_def: bool,
        node_type: NodeType,
        symlink: Option<Expression<'t>>,
    ) -> Self {
        SchemaNodeBuilder {
            line,
            is_def,
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
            bail!(":match occurs twice");
        }
        if self.is_def {
            bail!(":match cannot be used in definition");
        }
        self.match_pattern = Some(pattern);
        Ok(())
    }

    pub fn avoid_pattern(&mut self, pattern: Expression<'t>) -> Result<()> {
        if self.avoid_pattern.is_some() {
            bail!(":avoid occurs twice");
        }
        if self.is_def {
            bail!(":avoid cannot be used in definition");
        }
        self.avoid_pattern = Some(pattern);
        Ok(())
    }

    pub fn let_var(&mut self, id: Identifier<'t>, expr: Expression<'t>) -> Result<()> {
        match &mut self.type_specific {
            TypeSpecific::File { .. } => Err(anyhow!(
                "Cannot use :let to set variables inside files (add a '/' to make it a directory)"
            )),
            TypeSpecific::Directory { vars, .. } => {
                if let Entry::Vacant(entry) = vars.entry(id) {
                    entry.insert(expr);
                    Ok(())
                } else {
                    Err(anyhow!(":let {} occurs twice", id))
                }
            }
        }
    }

    pub fn define(&mut self, id: Identifier<'t>, definition: SchemaNode<'t>) -> Result<()> {
        match &mut self.type_specific {
            TypeSpecific::File { .. } => Err(anyhow!(
                "Cannot :define sub-trees inside files (add a '/' to make it a directory)"
            )),
            TypeSpecific::Directory { defs, .. } => {
                if let Entry::Vacant(entry) = defs.entry(id) {
                    entry.insert(definition);
                    Ok(())
                } else {
                    Err(anyhow!(":def {} occurs twice", id))
                }
            }
        }
    }

    pub fn use_definition(&mut self, id: Identifier<'t>) -> Result<()> {
        if let TypeSpecific::File { source, .. } = &self.type_specific {
            if source.is_some() {
                bail!(":use cannot be used in conjunction with :source");
            }
        }
        self.uses.push(id);
        Ok(())
    }

    pub fn owner(&mut self, owner: Expression<'t>) -> Result<()> {
        if self.attributes.owner.is_some() {
            bail!(":owner occurs twice");
        }
        self.attributes.owner = Some(owner);
        Ok(())
    }

    pub fn group(&mut self, group: Expression<'t>) -> Result<()> {
        if self.attributes.group.is_some() {
            bail!(":group occurs twice");
        }
        self.attributes.group = Some(group);
        Ok(())
    }

    pub fn mode(&mut self, mode: u16) -> Result<()> {
        if self.attributes.mode.is_some() {
            bail!(":mode occurs twice");
        }
        self.attributes.mode = Some(mode);
        Ok(())
    }

    pub fn source(&mut self, source: Expression<'t>) -> Result<()> {
        match self.type_specific {
            TypeSpecific::Directory { .. } => Err(anyhow!(
                ":source can only be used for files, not directories"
            )),
            TypeSpecific::File {
                source: ref mut src,
            } => {
                if !self.uses.is_empty() {
                    Err(anyhow!(":source cannot be used in conjunction with :use"))
                } else if src.is_some() {
                    Err(anyhow!(":source occurs twice"))
                } else {
                    *src = Some(source);
                    Ok(())
                }
            }
        }
    }

    pub fn target(&mut self, target: Expression<'t>) -> Result<()> {
        if self.symlink.is_some() {
            bail!(":target occurs twice");
        }
        self.symlink = Some(target);
        Ok(())
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
            line,
            is_def: _,
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
            } => SchemaType::Directory(DirectorySchema::new(vars, defs, entries)),
            TypeSpecific::File { source } => {
                let source = source.ok_or_else(|| {
                    anyhow!("File must have a :source (or add a '/' to make it a directory)")
                })?;
                SchemaType::File(FileSchema::new(source))
            }
        };
        Ok(SchemaNode {
            line,
            match_pattern,
            avoid_pattern,
            symlink,
            uses,
            attributes,
            schema,
        })
    }
}

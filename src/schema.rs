//! Provides the means to constuct a tree of [SchemaNode]s from text form ([parse_schema]).
//!
//! The language of the text form uses significant whitespace (four spaces) for each level. It
//! distinguishes between files and directories by the lack or presence of a `/`, and whether
//! it's a symlink by the lack or presence of `->` (followed by its target path expression).
//!
//! | Syntax                | Description
//! |-----------------------|---------------------------
//! | _str_                 | A file
//! | _str_`/`              | A directory
//! | _str_ `->` _expr_     | A symlink to a file
//! | _str_/ `->` _expr_    | A symlink to a directory
//!
//! Properties of a node in the schema can be set using one of the following tags:
//!
//! | Tag                       | Types     | Description
//! |---------------------------|-----------|---------------------------
//! |`#owner` _str_             | All       | Sets the owner of this file/directory/symlink target
//! |`#group` _str_             | All       | Sets the group of this file, directory or symlink target
//! |`#mode` _octal_            | All       | Sets the permissions of this file/directory/symlink target
//! |`#source` _expr_           | File      | Copy content into this file from the path given by _expr_
//! |`#let` _ident_ `=` _expr_  | Directory | Set a variable at this level to be used by deeper levels
//! |`#def` _ident_             | Directory | Define a sub-schema that can be reused by `#use`
//! |`#use` _ident_             | Directory | Reuse a sub-schema defined by `#def`
//!
//!
//! # Simple Schema
//!
//! The top level of a schema describes a directory, whose [attributes][Attributes] may be set by `#owner`, `#group` and `#mode` tags:
//! ```
//! use diskplan::schema::*;
//!
//! let schema_root = parse_schema("
//!     #owner person
//!     #group user
//!     #mode 777
//! ")?;
//!
//! assert!(matches!(schema_root.schema, Schema::Directory(_)));
//! assert_eq!(schema_root.attributes.owner, Some("person"));
//! assert_eq!(schema_root.attributes.group, Some("user"));
//! assert_eq!(schema_root.attributes.mode, Some(0o777));
//! # Ok::<(), anyhow::Error>(())
//! ```
//!
//! A [DirectorySchema] may contain sub-directories, files...
//! ```
//! # use diskplan::schema::*;
//! #
//! // ...
//! # let text =
//! "
//!     subdirectory/
//!         #owner admin
//!         #mode 700
//!
//!     file_name
//!         #source content/example_file
//! "
//! # ;
//! // ...
//! # match parse_schema(text)?.schema {
//! #     Schema::Directory(directory) => {
//! assert_eq!(directory.entries().len(), 2);
//! #     }
//! #     _ => panic!("Expected directory schema")
//! # }
//! #
//! # Ok::<(), anyhow::Error>(())
//! ```
//!
//! ...and symlinks to directories and files (with its sub-schema applied to the target):
//!
//! ```
//! # use diskplan::schema::*;
//! #
//! // ...
//! # let text =
//! "
//!     example_link/ -> /another/disk/example_target/
//!         #owner admin
//!         #mode 700
//!
//!         file_to_create_at_target_end
//!             #source content/example_file
//! "
//! # ;
//! // ...
//! # match parse_schema(text)?.schema {
//! #     Schema::Directory(directory) => {
//! #
//! let (binding, node) = directory.entries().first().unwrap();
//! assert!(matches!(
//!     binding,
//!     Binding::Static(ref name) if name == &String::from("example_link")
//! ));
//! assert_eq!(
//!     node.symlink.as_ref().unwrap().to_string(),
//!     String::from("/another/disk/example_target/")
//! );
//! assert!(matches!(node.schema, Schema::Directory(_)));
//! #
//! #     }
//! #     _ => panic!("Expected directory schema")
//! # }
//! #
//! # Ok::<(), anyhow::Error>(())
//! ```
//!
//! ## Pattern Matching
//!
//! **TODO**: Document `#match` and `$variable` named entries
//!
//! ## Variable Substitution
//!
//! Variables can be used to drive construction, for example:
//! ```
//! # let text =
//! "
//!     #let asset_type = character
//!     #let asset_name = Monkey
//!
//!     assets/
//!         $asset_type/
//!             $asset/
//!                 reference/
//! "
//! # ;
//! ```
//!
//! ## Schema Reuse
//!
//! **TODO**: Document `#def` and `#use`
//!

use anyhow::{anyhow, Result};
use std::{collections::HashMap, fmt::Display};

mod expr;
pub use expr::{Expression, Identifier, Special, Token};

mod text;
pub use text::{parse_schema, ParseError};

use crate::filesystem::Attributes;

/// A node in an abstract directory hierarchy
#[derive(Debug, Clone, PartialEq)]
pub struct SchemaNode<'t> {
    /// Condition against which to match file/directory names
    pub pattern: Option<Expression<'t>>,

    /// Symlink target - if this produces a symbolic link. Operates on the target end.
    pub symlink: Option<Expression<'t>>,

    /// Links to other schemas `#use`d by this one (found in parent [`DirectorySchema`] definitions)
    pub uses: Vec<Identifier<'t>>,

    /// Properties of this file/directory
    pub attributes: Attributes<'t>,

    /// Properties specific to the underlying (file or directory) type
    pub schema: Schema<'t>,
}

/// File/directory specific aspects of a node in the tree
#[derive(Debug, Clone, PartialEq)]
pub enum Schema<'t> {
    Directory(DirectorySchema<'t>),
    File(FileSchema<'t>),
}

impl Schema<'_> {
    pub fn as_directory(&self) -> Option<&DirectorySchema> {
        match self {
            Schema::Directory(directory) => Some(directory),
            _ => None,
        }
    }

    pub fn as_file(&self) -> Option<&FileSchema> {
        match self {
            Schema::File(file) => Some(file),
            _ => None,
        }
    }
}

pub trait Merge
where
    Self: Sized,
{
    fn merge(&self, other: &Self) -> Result<Self>;
}

impl<'t> Merge for Option<Box<Schema<'t>>> {
    fn merge(&self, other: &Self) -> Result<Self> {
        match (self, other) {
            (_, None) => Ok(self.clone()),
            (None, _) => Ok(other.clone()),
            (Some(a), Some(b)) => a.merge(b).map(Box::new).map(Some),
        }
    }
}

impl<'t> Merge for SchemaNode<'t> {
    fn merge(&self, other: &Self) -> Result<Self> {
        let pattern = match (&self.pattern, &other.pattern) {
            (Some(_), Some(_)) => Err(anyhow!(
                "Cannot merge two entries that both have match conditions"
            )),
            (pattern @ Some(_), _) => Ok(pattern),
            (_, pattern) => Ok(pattern),
        }?;
        let symlink = match (&self.symlink, &other.symlink) {
            (Some(_), Some(_)) => Err(anyhow!("Cannot merge two entries that are both symlinks")),
            (link @ Some(_), _) => Ok(link),
            (_, link) => Ok(link),
        }?;
        let mut uses = Vec::with_capacity(self.uses.len() + other.uses.len());
        for use_ in &self.uses {
            uses.push(use_.clone());
        }
        for use_ in &other.uses {
            uses.push(use_.clone());
        }
        Ok(SchemaNode {
            pattern: pattern.clone(),
            symlink: symlink.clone(),
            uses,
            attributes: self.attributes.merge(&other.attributes),
            schema: self.schema.merge(&other.schema)?,
        })
    }
}

impl<'t> Merge for Schema<'t> {
    fn merge(&self, other: &Schema<'t>) -> Result<Self> {
        match (self, other) {
            (Schema::Directory(schema_a), Schema::Directory(schema_b)) => {
                Ok(Schema::Directory(schema_a.merge(schema_b)?))
            }
            (Schema::File(schema_a), Schema::File(schema_b)) => {
                Ok(Schema::File(schema_a.merge(schema_b)?))
            }
            (Schema::Directory(_), _) | (Schema::File(_), _) => {
                Err(anyhow!("Cannot merge, mismatched types"))
            }
        }
    }
}

/// A DirectorySchema is a container of variables, definitions (named schemas) and a directory listing
#[derive(Debug, Default, Clone, PartialEq)]
pub struct DirectorySchema<'t> {
    /// Text replacement variables
    vars: HashMap<Identifier<'t>, Expression<'t>>,

    /// Definitions of sub-schemas
    defs: HashMap<Identifier<'t>, SchemaNode<'t>>,

    /// Disk entries to be created within this directory
    entries: Vec<(Binding<'t>, SchemaNode<'t>)>,
}

impl<'t> DirectorySchema<'t> {
    pub fn new(
        vars: HashMap<Identifier<'t>, Expression<'t>>,
        defs: HashMap<Identifier<'t>, SchemaNode<'t>>,
        entries: Vec<(Binding<'t>, SchemaNode<'t>)>,
    ) -> Self {
        let mut entries = entries;
        entries.sort_by(|(a, _), (b, _)| a.cmp(b));
        DirectorySchema {
            vars,
            defs,
            entries,
        }
    }
    pub fn vars(&self) -> &HashMap<Identifier, Expression> {
        &self.vars
    }
    pub fn get_var<'a>(&'a self, id: &Identifier<'a>) -> Option<&'a Expression<'t>> {
        self.vars.get(id)
    }
    pub fn defs<'s>(&'s self) -> &'s HashMap<Identifier, SchemaNode> {
        &self.defs
    }
    pub fn get_def<'a>(&'a self, id: &Identifier<'a>) -> Option<&'a SchemaNode<'t>> {
        self.defs.get(id)
    }
    pub fn entries(&self) -> &[(Binding, SchemaNode)] {
        &self.entries[..]
    }
}

impl Merge for DirectorySchema<'_> {
    fn merge(&self, other: &Self) -> Result<Self> {
        let mut vars = HashMap::with_capacity(self.vars.len() + other.vars.len());
        vars.extend(self.vars.iter().map(|(k, v)| (k.clone(), v.clone())));
        vars.extend(other.vars.iter().map(|(k, v)| (k.clone(), v.clone())));
        let mut defs = HashMap::with_capacity(self.defs.len() + other.defs.len());
        defs.extend(self.defs.iter().map(|(k, v)| (k.clone(), v.clone())));
        defs.extend(other.defs.iter().map(|(k, v)| (k.clone(), v.clone())));
        let mut entries = Vec::with_capacity(self.entries.len() + other.entries.len());
        entries.extend(self.entries.iter().cloned());
        entries.extend(other.entries.iter().cloned());
        Ok(DirectorySchema::new(vars, defs, entries))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Binding<'t> {
    Static(&'t str), // Static is ordered first
    Dynamic(Identifier<'t>),
}

impl Display for Binding<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Binding::Static(s) => write!(f, "{}", s),
            Binding::Dynamic(id) => write!(f, "${}", id.value()),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FileSchema<'t> {
    /// Path to the resource to be copied as file content
    // TODO: Make source enum: Enforce(...), Default(...) latter only creates if missing
    source: Expression<'t>,
}

impl<'t> FileSchema<'t> {
    pub fn new(source: Expression<'t>) -> Self {
        FileSchema { source }
    }
    pub fn source(&self) -> &Expression<'t> {
        &self.source
    }
}

impl Merge for FileSchema<'_> {
    fn merge(&self, other: &Self) -> Result<Self> {
        let source = match (self.source.tokens().len(), other.source.tokens().len()) {
            (_, 0) => self.source.clone(),
            (0, _) => other.source.clone(),
            (_, _) => {
                return Err(anyhow!(
                    "Cannot merge file with two sources: {} and {}",
                    self.source().to_string(),
                    other.source().to_string()
                ))
            }
        };
        Ok(FileSchema::new(source))
    }
}

#[cfg(test)]
mod tests;

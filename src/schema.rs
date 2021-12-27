//! Provides [Schema] and the means to constuct a schema from text form ([parse_schema]).
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
//! The top level of a schema describes a directory, whose [metadata][Meta] may be set by `#owner`, `#group` and `#mode` tags:
//! ```
//! use diskplan::schema::*;
//! use indoc::indoc;
//!
//! let text = indoc!(
//! "
//!     #owner person
//!     #group user
//!     #mode 777
//! "
//! );
//!
//! let schema = parse_schema(text)?;
//!
//! let directory: DirectorySchema = match schema {
//!     Schema::Directory(directory) => directory,
//!     _ => panic!("Expected Schema::Directory")
//! };
//!
//! assert_eq!(directory.meta().owner(), Some("person"));
//! assert_eq!(directory.meta().group(), Some("user"));
//! assert_eq!(directory.meta().mode(), Some(0o777));
//! # Ok::<(), anyhow::Error>(())
//! ```
//!
//! A [DirectorySchema] may contain sub-directories, files...
//! ```
//! # use indoc::indoc;
//! # use diskplan::schema::*;
//! #
//! // ...
//! # let text = indoc!(
//! "
//!     subdirectory/
//!         #owner admin
//!         #mode 700
//!
//!     file_name
//!         #source content/example_file
//! "
//! # );
//! // ...
//! # match parse_schema(text)? {
//! #     Schema::Directory(directory) => {
//! assert_eq!(directory.entries().count(), 2);
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
//! # use indoc::indoc;
//! # use diskplan::schema::*;
//! #
//! // ...
//! # let text = indoc!(
//! "
//!     example_link/ -> /another/disk/example_target/
//!         #owner admin
//!         #mode 700
//!
//!         file_to_create_at_target_end
//!             #source content/example_file
//! "
//! # );
//! // ...
//! # match parse_schema(text)? {
//! #     Schema::Directory(directory) => {
//! #
//! let link_entry: &SchemaEntry = directory.entries().next().unwrap();
//! assert!(matches!(
//!     link_entry.criteria,
//!     Match::Fixed(ref name) if name == &String::from("example_link")
//! ));
//! assert!(matches!(
//!     link_entry.subschema,
//!     Subschema::Original(Schema::Directory(ds @ DirectorySchema { .. }))
//!     if matches!(ds.symlink(), Some("/another/disk/example_target/")
//! ));
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
//! **TODO**: Document `#let` and the use of variables in expressions
//!
//! ## Schema Reuse
//!
//! **TODO**: Document `#def` and `#use`
//!

use anyhow::{anyhow, Result};
use std::{collections::HashMap, fmt::Display};

mod criteria;
pub use criteria::Match;

mod expr;
pub use expr::{Expression, Identifier, Token};

mod meta;
pub use meta::Meta;

mod text;
pub use text::{parse_schema, ParseError};

/// A node in an abstract directory hierarchy
#[derive(Debug, Clone, PartialEq)]
pub enum Schema<'t> {
    Directory(DirectorySchema<'t>),
    File(FileSchema<'t>),
}

pub trait Merge
where
    Self: Sized,
{
    fn merge(&self, other: &Self) -> Result<Self>;
}

impl<'t> Merge for Option<Box<Schema<'t>>> {
    fn merge(&self, other: &Option<Box<Schema<'t>>>) -> Result<Self> {
        match (self, other) {
            (_, None) => Ok(self.clone()),
            (None, _) => Ok(other.clone()),
            (Some(a), Some(b)) => a.merge(b).map(Box::new).map(Some),
        }
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
    /// Symlink target, if this is a symbolic link
    symlink: Option<Expression<'t>>,

    /// Links to other schemas `#use`d by this one
    uses: Vec<Identifier<'t>>,

    /// Text replacement variables
    vars: HashMap<Identifier<'t>, Expression<'t>>,

    /// Definitions of sub-schemas
    defs: HashMap<Identifier<'t>, Schema<'t>>,

    /// Properties of this directory
    meta: Meta<'t>,

    /// Disk entries to be created within this directory
    entries: Vec<(Match<'t>, Schema<'t>)>,
}

impl<'t> DirectorySchema<'t> {
    pub fn new(
        symlink: Option<Expression<'t>>,
        uses: Vec<Identifier<'t>>,
        vars: HashMap<Identifier<'t>, Expression<'t>>,
        defs: HashMap<Identifier<'t>, Schema<'t>>,
        meta: Meta<'t>,
        entries: Vec<(Match<'t>, Schema<'t>)>,
    ) -> Self {
        let mut entries = entries;
        entries.sort_by(|(match_a, _), (match_b, _)| {
            match_a
                .partial_cmp(match_b)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        DirectorySchema {
            symlink,
            uses,
            vars,
            defs,
            meta,
            entries,
        }
    }
    pub fn symlink(&self) -> Option<&Expression> {
        self.symlink.as_ref()
    }
    pub fn vars(&self) -> &HashMap<Identifier, Expression> {
        &self.vars
    }
    pub fn defs<'s>(&'s self) -> &'s HashMap<Identifier, Schema> {
        &self.defs
    }
    pub fn meta(&self) -> &Meta<'t> {
        &self.meta
    }
    pub fn entries(&self) -> impl Iterator<Item = &(Match, Schema)> {
        self.entries.iter()
    }
}

impl Merge for DirectorySchema<'_> {
    fn merge(&self, other: &Self) -> Result<Self> {
        let symlink = match (&self.symlink, &other.symlink) {
            (Some(_), Some(_)) => Err(anyhow!("Cannot merge two directories with symlink targets")),
            (link @ Some(_), _) => Ok(link),
            (_, link) => Ok(link),
        }?;
        let mut vars = HashMap::with_capacity(self.vars.len() + other.vars.len());
        vars.extend(self.vars.iter().map(|(k, v)| (k.clone(), v.clone())));
        vars.extend(other.vars.iter().map(|(k, v)| (k.clone(), v.clone())));
        let mut defs = HashMap::with_capacity(self.defs.len() + other.defs.len());
        defs.extend(self.defs.iter().map(|(k, v)| (k.clone(), v.clone())));
        defs.extend(other.defs.iter().map(|(k, v)| (k.clone(), v.clone())));
        let meta = self.meta.merge(&other.meta);
        let mut entries = Vec::with_capacity(self.entries.len() + other.entries.len());
        entries.extend(self.entries.iter().cloned());
        entries.extend(other.entries.iter().cloned());
        Ok(DirectorySchema::new(
            symlink.clone(),
            Vec::new(), // Assuming a merge is the #use'd #def being merged in
            vars,
            defs,
            meta,
            entries,
        ))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Binding<'t> {
    Static(&'t str),
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
    /// Symlink target, if this is a symbolic link
    symlink: Option<Expression<'t>>,

    /// Links to other schemas `#use`d by this one
    uses: Vec<Identifier<'t>>,

    /// Properties of this directory
    meta: Meta<'t>,

    /// Path to the resource to be copied as file content
    // TODO: Make source enum: Enforce(...), Default(...) latter only creates if missing
    source: Expression<'t>,
}

impl<'t> FileSchema<'t> {
    pub fn new(
        symlink: Option<Expression<'t>>,
        uses: Vec<Identifier<'t>>,
        meta: Meta<'t>,
        source: Expression<'t>,
    ) -> Self {
        FileSchema {
            symlink,
            uses,
            meta,
            source,
        }
    }
    pub fn symlink(&self) -> Option<&Expression> {
        self.symlink.as_ref()
    }
    pub fn meta(&self) -> &Meta<'t> {
        &self.meta
    }
    pub fn source(&self) -> &Expression<'t> {
        &self.source
    }
}

impl Merge for FileSchema<'_> {
    fn merge(&self, other: &Self) -> Result<Self> {
        let symlink = match (&self.symlink, &other.symlink) {
            (Some(_), Some(_)) => Err(anyhow!("Cannot merge two directories with symlink targets")),
            (link @ Some(_), _) => Ok(link),
            (_, link) => Ok(link),
        }?;
        let meta = self.meta.merge(&other.meta);
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
        Ok(FileSchema::new(
            symlink.clone(),
            Vec::new(), // Assuming a merge is the #use'd #def being merged in
            meta,
            source,
        ))
    }
}

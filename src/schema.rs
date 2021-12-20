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
//!     Subschema::Original(Schema::Symlink(LinkSchema { .. }))
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
use std::collections::HashMap;

mod criteria;
pub use criteria::Match;

mod expr;
pub use expr::{Expression, Identifier, Token};

mod meta;
pub use meta::{Meta, MetaBuilder};

mod text;
pub use text::{parse_schema, ParseError};

/// A node in an abstract directory hierarchy representing a file, directory or symlink
#[derive(Debug, Clone, PartialEq)]
pub enum Schema {
    Directory(DirectorySchema),
    File(FileSchema),
    Symlink(LinkSchema),
}

pub trait Merge
where
    Self: Sized,
{
    fn merge(&self, other: &Self) -> Result<Self>;
}

impl Merge for Option<Box<Schema>> {
    fn merge(&self, other: &Option<Box<Schema>>) -> Result<Option<Box<Schema>>> {
        match (self, other) {
            (_, None) => Ok(self.clone()),
            (None, _) => Ok(other.clone()),
            (Some(a), Some(b)) => a.merge(b).map(Box::new).map(Some),
        }
    }
}

impl Merge for Schema {
    fn merge(&self, other: &Schema) -> Result<Schema> {
        match (self, other) {
            (Schema::Directory(schema_a), Schema::Directory(schema_b)) => {
                Ok(Schema::Directory(schema_a.merge(schema_b)?))
            }
            (Schema::File(schema_a), Schema::File(schema_b)) => {
                Ok(Schema::File(schema_a.merge(schema_b)?))
            }
            (Schema::Symlink(schema_a), Schema::Symlink(schema_b)) => {
                Ok(Schema::Symlink(schema_a.merge(schema_b)?))
            }
            (Schema::Directory(_), _) | (Schema::File(_), _) | (Schema::Symlink(_), _) => {
                Err(anyhow!("Cannot merge, mismatched types"))
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SchemaEntry {
    pub criteria: Match,
    pub subschema: Subschema,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Subschema {
    Referenced {
        definition: Identifier,
        overrides: Schema,
    },
    Original(Schema),
}

impl PartialOrd for SchemaEntry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.criteria.partial_cmp(&other.criteria)
    }
}

/// A DirectorySchema is a container of variables, definitions (named schemas) and a directory listing
#[derive(Debug, Default, Clone, PartialEq)]
pub struct DirectorySchema {
    /// Text replacement variables
    vars: HashMap<Identifier, Expression>,

    /// Definitions of sub-schemas
    defs: HashMap<Identifier, Schema>,

    /// Properties of this directory
    meta: Meta,

    /// Disk entries to be created within this directory
    entries: Vec<SchemaEntry>,
}

impl DirectorySchema {
    pub fn new(
        vars: HashMap<Identifier, Expression>,
        defs: HashMap<Identifier, Schema>,
        meta: Meta,
        entries: Vec<SchemaEntry>,
    ) -> DirectorySchema {
        let mut entries = entries;
        entries.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        DirectorySchema {
            vars,
            defs,
            meta,
            entries,
        }
    }
    pub fn vars(&self) -> &HashMap<Identifier, Expression> {
        &self.vars
    }
    pub fn defs(&self) -> &HashMap<Identifier, Schema> {
        &self.defs
    }
    pub fn meta(&self) -> &Meta {
        &self.meta
    }
    pub fn entries(&self) -> impl Iterator<Item = &SchemaEntry> {
        self.entries.iter()
    }
}

impl Merge for DirectorySchema {
    fn merge(&self, other: &Self) -> Result<Self> {
        let mut vars = HashMap::with_capacity(self.vars.len() + other.vars.len());
        vars.extend(self.vars.iter().map(|(k, v)| (k.clone(), v.clone())));
        vars.extend(other.vars.iter().map(|(k, v)| (k.clone(), v.clone())));
        let mut defs = HashMap::with_capacity(self.defs.len() + other.defs.len());
        defs.extend(self.defs.iter().map(|(k, v)| (k.clone(), v.clone())));
        defs.extend(other.defs.iter().map(|(k, v)| (k.clone(), v.clone())));
        let meta = MetaBuilder::default()
            .merge(&self.meta)
            .merge(&other.meta)
            .build();
        let mut entries = Vec::with_capacity(self.entries.len() + other.entries.len());
        entries.extend(self.entries.iter().cloned());
        entries.extend(other.entries.iter().cloned());
        Ok(DirectorySchema::new(vars, defs, meta, entries))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FileSchema {
    /// Properties of this directory
    meta: Meta,

    /// Path to the resource to be copied as file content
    // TODO: Make source enum: Enforce(...), Default(...) latter only creates if missing
    source: Expression,
}

impl FileSchema {
    pub fn new(meta: Meta, source: Expression) -> FileSchema {
        FileSchema { meta, source }
    }
    pub fn meta(&self) -> &Meta {
        &self.meta
    }
    pub fn source(&self) -> &Expression {
        &self.source
    }
}

impl Merge for FileSchema {
    fn merge(&self, other: &Self) -> Result<Self> {
        let meta = MetaBuilder::default()
            .merge(&self.meta)
            .merge(&other.meta)
            .build();
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
        Ok(FileSchema::new(meta, source))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LinkSchema {
    /// Symlink target
    target: Expression,

    /// What to ensure, if anything, should be found at the other end
    far_schema: Option<Box<Schema>>,
}

impl LinkSchema {
    pub fn new(target: Expression, far_schema: Option<Box<Schema>>) -> LinkSchema {
        LinkSchema {
            target,
            far_schema: far_schema,
        }
    }
    pub fn target(&self) -> &Expression {
        &self.target
    }
    pub fn far_schema(&self) -> Option<&Schema> {
        self.far_schema.as_deref()
    }
}

impl Merge for LinkSchema {
    fn merge(&self, other: &Self) -> Result<Self> {
        let far_schema = self.far_schema.merge(&other.far_schema)?;
        let target = match (self.target.tokens().len(), other.target.tokens().len()) {
            (_, 0) => self.target.clone(),
            (0, _) => other.target.clone(),
            (_, _) => {
                return Err(anyhow!(
                    "Cannot merge link with two targets: {} and {}",
                    self.target().to_string(),
                    other.target().to_string()
                ))
            }
        };
        Ok(LinkSchema::new(target, far_schema))
    }
}

pub fn print_tree(schema: &Schema) {
    fn print_schema(schema: &Schema, indent: usize) {
        match schema {
            Schema::File(file_schema) => print_file_schema(&file_schema, indent),
            Schema::Directory(dir_schema) => print_dir_schema(&dir_schema, indent),
            Schema::Symlink(link_schema) => print_link_schema(&link_schema, indent),
        }
    }
    fn print_dir_schema(dir_schema: &DirectorySchema, indent: usize) {
        println!("{pad:indent$}[DIRECTORY]", pad = "", indent = indent);
        for (name, value) in dir_schema.vars.iter() {
            println!(
                "{pad:indent$}var {name} = {value}",
                pad = "",
                indent = indent,
                name = String::from(name),
                value = value,
            );
        }
        for (name, def) in dir_schema.defs.iter() {
            println!(
                "{pad:indent$}def {name}:",
                pad = "",
                indent = indent,
                name = String::from(name),
            );
            print_schema(def, indent + 4);
        }
        print_meta(&dir_schema.meta, indent);
        for entry in &dir_schema.entries {
            println!(
                "{pad:indent$}--> {:?}",
                entry.criteria,
                pad = "",
                indent = indent
            );
            match &entry.subschema {
                Subschema::Referenced {
                    definition: def,
                    overrides,
                } => {
                    println!(
                        "{pad:indent$}USE {}",
                        def.value(),
                        pad = "",
                        indent = indent
                    );
                    print_schema(&overrides, indent + 4);
                }
                Subschema::Original(schema) => print_schema(&schema, indent + 4),
            }
        }
    }
    fn print_file_schema(file_schema: &FileSchema, indent: usize) {
        println!(
            "{pad:indent$}[FILE <- {}]",
            file_schema.source().to_string(),
            pad = "",
            indent = indent,
        );
        print_meta(&file_schema.meta, indent);
    }
    fn print_link_schema(link_schema: &LinkSchema, indent: usize) {
        println!(
            "{pad:indent$}[LINK -> {}]",
            link_schema.target,
            pad = "",
            indent = indent
        );
        if let Some(far_schema) = link_schema.far_schema() {
            print_schema(far_schema, indent + 4);
        }
    }
    fn print_meta(meta: &Meta, indent: usize) {
        print!("{pad:indent$}meta ", pad = "", indent = indent);
        match meta.owner() {
            Some(owner) => print!("{}", owner),
            None => print!("(keep)"),
        }
        print!(":");
        match meta.group() {
            Some(group) => print!("{}", group),
            None => print!("(keep)"),
        }
        print!(" mode=");
        match meta.mode() {
            Some(mode) => print!("{:o}", mode),
            None => print!("(keep)"),
        }
        println!();
    }
    print_schema(schema, 0);
}

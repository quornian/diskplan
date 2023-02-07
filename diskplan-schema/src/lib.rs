//! This crate provides the means to constuct a tree of [SchemaNode]s from text form (see
//! [parse_schema]).
//!
//! The language of the text form uses significant whitespace (four spaces) for indentation,
//! distinguishes between files and directories by the presence of a `/`, and whether
//! this is a symlink by presence of an `->` (followed by its target path expression).
//! That is, each indented node of the directory tree takes one of the following forms:
//!
//! | Syntax                | Description
//! |-----------------------|---------------------------
//! | _str_                 | A file
//! | _str_`/`              | A directory
//! | _str_ `->` _expr_     | A symlink to a file
//! | _str_/ `->` _expr_    | A symlink to a directory
//!
//! Properties of a given node are set using the following tags:
//!
//! | Tag                       | Types     | Description
//! |---------------------------|-----------|---------------------------
//! |`:owner` _expr_            | All       | Sets the owner of this file/directory/symlink target
//! |`:group` _expr_            | All       | Sets the group of this file, directory or symlink target
//! |`:mode` _octal_            | All       | Sets the permissions of this file/directory/symlink target
//! |`:source` _expr_           | File      | Copies content into this file from the path given by _expr_
//! |`:let` _ident_ `=` _expr_  | Directory | Sets a variable at this level to be used by deeper levels
//! |`:def` _ident_             | Directory | Defines a sub-schema that can be reused by `:use`
//! |`:use` _ident_             | Directory | Reuses a sub-schema defined by `:def`
//!
//!
//! # Simple Schema
//!
//! The top level of a schema describes a directory, whose [attributes][Attributes] may be set by `:owner`, `:group` and `:mode` tags:
//! ```
//! use diskplan_schema::*;
//!
//! let schema_root = parse_schema("
//!     :owner person
//!     :group user
//!     :mode 777
//! ")?;
//!
//! assert!(matches!(schema_root.schema, SchemaType::Directory(_)));
//! assert_eq!(schema_root.attributes.owner.unwrap(), "person");
//! assert_eq!(schema_root.attributes.group.unwrap(), "user");
//! assert_eq!(schema_root.attributes.mode.unwrap(), 0o777);
//! # Ok::<(), anyhow::Error>(())
//! ```
//!
//! A [DirectorySchema] may contain sub-directories and files:
//! ```
//! # use diskplan_schema::*;
//! #
//! // ...
//! # let text =
//! "
//!     subdirectory/
//!         :owner admin
//!         :mode 700
//!
//!     file_name
//!         :source content/example_file
//! "
//! # ;
//! // ...
//! assert_eq!(
//!     parse_schema(text)?
//!         .schema
//!         .as_directory()
//!         .expect("Not a directory")
//!         .entries()
//!         .len(),
//!     2
//! );
//! # Ok::<(), anyhow::Error>(())
//! ```
//!
//! It may also contain symlinks to directories and files, whose own schemas will apply to the
//! target:
//!
//! ```
//! # use diskplan_schema::*;
//! #
//! // ...
//! # let text =
//! "
//!     example_link/ -> /another/disk/example_target/
//!         :owner admin
//!         :mode 700
//!
//!         file_to_create_at_target_end
//!             :source content/example_file
//! "
//! # ;
//! // ...
//! # match parse_schema(text)?.schema {
//! #     SchemaType::Directory(directory) => {
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
//! assert!(matches!(node.schema, SchemaType::Directory(_)));
//! #
//! #     }
//! #     _ => panic!("Expected directory schema")
//! # }
//! #
//! # Ok::<(), anyhow::Error>(())
//! ```
//!
//! ## Variable Substitution
//!
//! Variables can be used to drive construction, for example:
//! ```
//! # diskplan_schema::parse_schema(
//! "
//!     :let asset_type = character
//!     :let asset_name = Monkey
//!
//!     assets/
//!         $asset_type/
//!             $asset/
//!                 reference/
//! "
//! # ).unwrap();
//! ```
//!
//! Variables will also pick up on names already on disk (even if a `:let` provides a different
//! value). For example, if we had `assets/prop/Banana` on disk already, `$asset_type` would match
//! against and take the value "prop" (as well as "character") and `$asset` would take the value
//! "Banana" (as well as "Monkey"), producing:
//! ```text
//! assets
//! ├── character
//! │   └── Monkey
//! │       └── reference
//! └── prop
//!     └── Banana
//!         └── reference
//! ```
//!
//! ## Pattern Matching
//!
//! Any node of the schema can have a `:match` tag, which, via a Regular Expression, controls the
//! possible values a variable can take.
//!
//! **IMPORTANT:** _No two variables can match the same value_. If they do, an error will occur during
//! execution, so be careful to ensure there is no overlap between patterns. The use of `:avoid`
//! can help restrict the pattern matching and ensure proper partitioning.
//!
//! Static names (without variables) always take precedence and do not need to be unique with
//! respect to variable patterns (and vice versa).
//!
//! For example, this is legal in the schema but will always error in practice:
//! ```text
//! $first/
//! $second/
//! ```
//! For instance, when operating on the path `/test`, it yields:
//! ```text
//! Error: "test" matches multiple dynamic bindings "$first" and "$second" (Any)
//! ```
//!
//! A working example might be:
//! ```text
//! $first/
//!     :match [A-Z].*
//! $second/
//!     :match [^A-Z].*
//! ```
//!
//! ## Schema Reuse
//!
//! Portions of a schema can be built from reusable definitions.
//!
//! A definition is formed using the `:def` keyword, followed by its name and a body like any
//! other schema node:
//! ```text
//! :def reusable/
//!     anything_inside/
//! ```
//! It is used by adding the `:use` tag inside any other (same or deeper level) node:
//! ```text
//! reused_here/
//!     :use reusable
//! ```
//! Multiple `:use` tags may be used. Attributes are resolved in the following order:
//! ```text
//! example/
//!     ## Attributes set here win (before or after any :use lines)
//!     :owner root
//!
//!     ## First :use is next in precedence
//!     :use one
//!
//!     ## Subsequent :use lines take lower precedence
//!     :use two
//! ```
#![warn(missing_docs)]

use std::{collections::HashMap, fmt::Display};

mod attributes;
pub use attributes::Attributes;

mod expression;
pub use expression::{Expression, Identifier, Special, Token};

mod text;
pub use text::{parse_schema, ParseError};

/// A node in an abstract directory hierarchy
#[derive(Debug, Clone, PartialEq)]
pub struct SchemaNode<'t> {
    /// A reference to the line in the text representation where this node was defined
    pub line: &'t str,

    /// Condition against which to match file/directory names
    pub match_pattern: Option<Expression<'t>>,

    /// Condition against which file/directory names must not match
    pub avoid_pattern: Option<Expression<'t>>,

    /// Symlink target - if this produces a symbolic link. Operates on the target end.
    pub symlink: Option<Expression<'t>>,

    /// Links to other schemas `:use`d by this one (found in parent [`DirectorySchema`] definitions)
    pub uses: Vec<Identifier<'t>>,

    /// Properties of this file/directory
    pub attributes: Attributes<'t>,

    /// Properties specific to the underlying (file or directory) type
    pub schema: SchemaType<'t>,
}

impl<'t> std::fmt::Display for SchemaNode<'t> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Schema node \"{}\"", self.line)?;
        if let Some(ref match_pattern) = self.match_pattern {
            write!(f, ", matching \"{match_pattern}\"")?;
        }
        if let Some(ref avoid_pattern) = self.avoid_pattern {
            write!(f, ", avoiding \"{avoid_pattern}\"")?;
        }

        match &self.schema {
            SchemaType::Directory(ds) => {
                let len = ds.entries().len();
                write!(
                    f,
                    " (directory with {} entr{})",
                    len,
                    if len == 1 { "y" } else { "ies" }
                )?
            }
            SchemaType::File(fs) => write!(f, " (file from source: {})", fs.source())?,
        }
        Ok(())
    }
}

/// File/directory specific aspects of a node in the tree
#[derive(Debug, Clone, PartialEq)]
pub enum SchemaType<'t> {
    /// Indicates that this node describes a directory
    Directory(DirectorySchema<'t>),
    /// Indicates that this node describes a file
    File(FileSchema<'t>),
}

impl<'t> SchemaType<'t> {
    /// Returns the inner [`DirectorySchema`] if this node is a directory node
    pub fn as_directory(&self) -> Option<&DirectorySchema<'t>> {
        match self {
            SchemaType::Directory(directory) => Some(directory),
            _ => None,
        }
    }

    /// Returns the inner [`FileSchema`] if this node is a file node
    pub fn as_file(&self) -> Option<&FileSchema<'t>> {
        match self {
            SchemaType::File(file) => Some(file),
            _ => None,
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
    /// Constructs a new description of a directory in the schema
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
    /// Provides access to the variables defined in this node
    pub fn vars(&self) -> &HashMap<Identifier<'t>, Expression<'t>> {
        &self.vars
    }
    /// Returns the expression associated with the given variable, if any was set in the schema
    pub fn get_var<'a>(&'a self, id: &Identifier<'a>) -> Option<&'a Expression<'t>> {
        self.vars.get(id)
    }

    /// Provides access to the sub-schema definitions defined in this node
    pub fn defs(&self) -> &HashMap<Identifier, SchemaNode> {
        &self.defs
    }
    /// Returns the sub-schema associated with the given definition, if any was set in the schema
    pub fn get_def<'a>(&'a self, id: &Identifier<'a>) -> Option<&'a SchemaNode<'t>> {
        self.defs.get(id)
    }

    /// Provides access to the child nodes of this node, with their bindings
    pub fn entries(&self) -> &[(Binding<'t>, SchemaNode<'t>)] {
        &self.entries[..]
    }
}

/// How an entry is bound in a schema, either to a static fixed name or to a variable
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Binding<'t> {
    /// A static, fixed name
    Static(&'t str), // Static is ordered first
    /// A dynamic name bound to the given variable
    Dynamic(Identifier<'t>),
}

impl Display for Binding<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Binding::Static(s) => write!(f, "{s}"),
            Binding::Dynamic(id) => write!(f, "${id}"),
        }
    }
}

/// A description of a file
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileSchema<'t> {
    /// Path to the resource to be copied as file content
    // TODO: Make source enum: Enforce(...), Default(...) latter only creates if missing
    source: Expression<'t>,
}

impl<'t> FileSchema<'t> {
    /// Constructs a new description of a file
    pub fn new(source: Expression<'t>) -> Self {
        FileSchema { source }
    }
    /// Returns the expression of the path from where the file will inherit its content
    pub fn source(&self) -> &Expression<'t> {
        &self.source
    }
}

#[cfg(test)]
mod tests;

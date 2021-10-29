use expr::Expression;
use meta::{Meta, MetaError};
use std::collections::HashMap;
use std::path::PathBuf;

use self::criteria::Match;
use self::expr::Identifier;

pub mod builder;
pub mod criteria;
pub mod expr;
pub mod meta;

#[derive(Debug, PartialEq)]
pub enum Schema {
    Directory(DirectorySchema),
    File(FileSchema),
    Symlink(LinkSchema),
}

#[derive(Debug, PartialEq)]
pub struct SchemaEntry {
    pub criteria: Match,
    pub schema: Subschema,
}

#[derive(Debug, PartialEq)]
pub enum Subschema {
    Referenced(Identifier),
    Original(Schema),
}

impl PartialOrd for SchemaEntry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.criteria.partial_cmp(&other.criteria)
    }
}

/// A DirectorySchema is a container of variables, definitions (named schemas) and a directory listing
#[derive(Debug, Default, PartialEq)]
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
    pub fn entries(&self) -> &Vec<SchemaEntry> {
        &self.entries
    }

    pub fn is_no_op(&self) -> bool {
        self.entries.is_empty() && self.meta.is_no_op()
    }
}

#[derive(Debug, PartialEq)]
pub struct FileSchema {
    /// Properties of this directory
    meta: Meta,

    /// Path to the resource to be copied as file content
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

#[derive(Debug, PartialEq)]
pub struct LinkSchema {
    /// Symlink target
    target: Expression,

    /// What to ensure, if anything, should be found at the other end
    far_schema: Box<Schema>,
}

impl LinkSchema {
    pub fn new(target: Expression, far_schema: Schema) -> LinkSchema {
        LinkSchema {
            target,
            far_schema: Box::new(far_schema),
        }
    }
    pub fn target(&self) -> &Expression {
        &self.target
    }
    pub fn far_schema(&self) -> &Schema {
        &self.far_schema
    }
}

#[derive(thiserror::Error, Debug)]
pub enum SchemaError {
    // SCHEMA --------------------------------
    #[error("Schema entry is not variable (e.g. @varname) but has pattern: {0}")]
    NonVariableWithPattern(PathBuf),

    #[error("Only directories are allowed in the schema; encountered: {0}\nConsider replacing with a directory containing _.is.file or _.is_link")]
    NonDirectorySchemaEntry(PathBuf),

    #[error("Error parsing metadata from: {0}")]
    MetaError(PathBuf, #[source] MetaError),

    // FROM DISK -----------------------------
    #[error("Multiple type annotations found under: {0}")]
    MultipleTypeAnnotation(PathBuf),

    #[error("IO error reading info from: {0}")]
    IOError(PathBuf, #[source] std::io::Error),

    #[error("Unable to parse property value from: {0} ({1})")]
    PropertyParseFailure(PathBuf, String),

    #[error("Unable to parse regular expression value from: {0} ({1})")]
    RegexParseFailure(PathBuf, #[source] regex::Error),

    #[error("An unexpected item was encountered: {0}")]
    UnexpectedItemError(PathBuf),

    #[error("Syntax error in file {path}: {details}")]
    SyntaxError { path: PathBuf, details: String },

    // TODO: Make more specific ones
    #[error("General error: {0}")]
    GeneralError(&'static str),
    #[error("Error in block: {0}")]
    NestedError(String, #[source] Option<Box<SchemaError>>),
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
            match &entry.schema {
                Subschema::Referenced(use_def) => println!(
                    "{pad:indent$}USE {}",
                    use_def.value(),
                    pad = "",
                    indent = indent
                ),
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
        print_schema(link_schema.far_schema(), indent + 4);
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
        match meta.permissions() {
            Some(perms) => print!("{:o}", perms.mode()),
            None => print!("(keep)"),
        }
        println!();
    }
    print_schema(schema, 0);
}

use super::criteria::MatchCriteria;
use super::meta::{Meta, MetaError};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, PartialEq)]
pub enum Schema {
    Directory(DirectorySchema),
    File(FileSchema),
    Symlink(LinkSchema),
    Use(String),
}

/// A DirectorySchema is a container of variables, definitions (named schemas) and a directory listing
#[derive(Debug, PartialEq)]
pub struct DirectorySchema {
    /// Text replacement variables
    vars: HashMap<String, String>,

    /// Definitions of sub-schemas
    defs: HashMap<String, Schema>,

    /// Properties of this directory
    meta: Meta,

    /// Disk entries to be created within this directory
    entries: Vec<(MatchCriteria, Schema)>,
}

impl DirectorySchema {
    pub fn new(
        vars: HashMap<String, String>,
        defs: HashMap<String, Schema>,
        meta: Meta,
        entries: Vec<(MatchCriteria, Schema)>,
    ) -> DirectorySchema {
        let mut entries = entries;
        entries.sort_by(|(a, _), (b, _)| a.order().cmp(&b.order()));
        DirectorySchema {
            vars,
            defs,
            meta,
            entries,
        }
    }
    pub fn vars(&self) -> &HashMap<String, String> {
        &self.vars
    }
    pub fn defs(&self) -> &HashMap<String, Schema> {
        &self.defs
    }
    pub fn meta(&self) -> &Meta {
        &self.meta
    }
    pub fn entries(&self) -> &Vec<(MatchCriteria, Schema)> {
        &self.entries
    }

    pub fn is_no_op(&self) -> bool {
        self.entries.is_empty() && self.meta.is_no_op()
    }
}

#[derive(Debug, Default, PartialEq)]
pub struct FileSchema {
    /// Properties of this directory
    meta: Meta,

    /// Path to the resource to be copied as file content
    source: PathBuf,
}

impl FileSchema {
    pub fn new(meta: Meta, source: PathBuf) -> FileSchema {
        FileSchema { meta, source }
    }
    pub fn meta(&self) -> &Meta {
        &self.meta
    }
    pub fn source(&self) -> &PathBuf {
        &self.source
    }
}

#[derive(Debug, PartialEq)]
pub struct LinkSchema {
    /// Symlink target
    target: String,

    /// What to ensure, if anything, should be found at the other end
    far_schema: Box<Schema>,
}

impl LinkSchema {
    pub fn new(target: String, far_schema: Schema) -> LinkSchema {
        LinkSchema {
            target,
            far_schema: Box::new(far_schema),
        }
    }
    pub fn target(&self) -> &String {
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

    #[error("IO error reading item info from directory: {0}")]
    DirectoryIOError(PathBuf, #[source] std::io::Error),

    #[error("Unable to parse property value from: {0} ({1})")]
    PropertyParseFailure(PathBuf, String),

    #[error("Unable to parse regular expression value from: {0} ({1})")]
    RegexParseFailure(PathBuf, #[source] regex::Error),

    #[error("An unexpected item was encountered: {0}")]
    UnexpectedItemError(PathBuf),
}

pub fn print_tree(schema: &Schema) {
    fn print_schema(schema: &Schema, indent: usize) {
        match schema {
            Schema::File(file_schema) => print_file_schema(&file_schema, indent),
            Schema::Directory(dir_schema) => print_dir_schema(&dir_schema, indent),
            Schema::Symlink(link_schema) => print_link_schema(&link_schema, indent),
            Schema::Use(refname) => println!("[REF == {}]", refname),
        }
    }
    fn print_dir_schema(dir_schema: &DirectorySchema, indent: usize) {
        println!("{pad:indent$}[DIRECTORY]", pad = "", indent = indent);
        for (name, value) in dir_schema.vars.iter() {
            println!(
                "{pad:indent$}var {name} = {value}",
                pad = "",
                indent = indent,
                name = name,
                value = value,
            );
        }
        for (name, def) in dir_schema.defs.iter() {
            println!(
                "{pad:indent$}def {name}:",
                pad = "",
                indent = indent,
                name = name,
            );
            print_schema(def, indent + 4);
        }
        print_meta(&dir_schema.meta, indent);
        for (criteria, entry) in &dir_schema.entries {
            println!(
                "{pad:indent$}--> {:?}",
                criteria.mode(),
                pad = "",
                indent = indent
            );
            print_schema(entry, indent + 4);
        }
    }
    fn print_file_schema(file_schema: &FileSchema, indent: usize) {
        println!(
            "{pad:indent$}[FILE <- {}]",
            file_schema.source().to_string_lossy(),
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

// #[cfg(test)]
// mod tests {
//     use super::*;

//     #[test]
//     fn test_item() {
//         let mut vars = HashMap::new();
//         vars.insert("@var1".to_owned(), "one".to_owned());
//         vars.insert("@var2".to_owned(), "two".to_owned());
//         let expr = "@var1/{@var2}_fixed".to_owned();
//         assert_eq!(
//             Ok("one/two_fixed".to_owned()),
//             evaluate_name(&expr, &[vars])
//         );
//     }
// }

use super::meta::{Meta, MetaError};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, PartialEq)]
pub struct Schema {
    vars: HashMap<String, String>,
    children: HashMap<String, Schema>,
    itemtype: ItemType,
    meta: Meta,
    filter: Filter,
}

#[derive(Debug, PartialEq)]
pub enum ItemType {
    File,
    Directory,
    Symlink(String),
    Reuse(String),
}

#[derive(Debug, PartialEq)]
pub enum Filter {
    /// Match the exact name of the item
    Exact,
    /// Match the regular expression
    Pattern(String),
    /// Match any name
    Freeform,
}

#[derive(thiserror::Error, Debug)]
pub enum ItemError {
    #[error("Unknown item type: {0}")]
    UnknownItemType(PathBuf),

    #[error("Duplicate type annotation: {0}")]
    DuplicateTypeAnnotation(PathBuf),

    #[error("File (or reused) item must not have children: {0}")]
    ItemHasChildren(PathBuf),

    #[error("Usage may not have a type set")]
    UsageTypeError(PathBuf),

    #[error(transparent)]
    MetaError(#[from] MetaError),

    #[error("IO error reading item info from directory: {0}")]
    DirectoryIOError(PathBuf, #[source] std::io::Error),

    #[error("Conflicting filter entry: {0}")]
    ConflictingFilter(PathBuf),

    #[error("Invalid property entry: {0}")]
    InvalidPropertyEntry(PathBuf),

    #[error("IO error reading item info from directory")]
    IOError(#[from] std::io::Error),

    #[error("Unexpected error processing: {0}")]
    UnexpectedItemError(PathBuf),
}

impl Schema {
    pub fn new(
        vars: HashMap<String, String>,
        children: HashMap<String, Schema>,
        itemtype: ItemType,
        meta: Meta,
        filter: Filter,
    ) -> Schema {
        Schema {
            vars,
            children,
            itemtype,
            meta,
            filter,
        }
    }

    pub fn default_typed(itemtype: ItemType) -> Schema {
        Schema {
            vars: HashMap::new(),
            children: HashMap::new(),
            meta: Meta::default(),
            itemtype,
            filter: Filter::Exact,
        }
    }

    pub fn meta(&self) -> &Meta {
        &self.meta
    }

    pub fn itemtype(&self) -> &ItemType {
        &self.itemtype
    }
}

pub fn print_tree(item: &Schema) {
    fn print_item(name: &str, item: &Schema, indent: usize) {
        if !item.vars.is_empty() {
            println!("--[ Variables ]--");
            for (var_name, var_expr) in item.vars.iter() {
                println!(
                    "{pad:indent$}{name} = {value}",
                    pad = "",
                    indent = indent,
                    name = var_name,
                    value = var_expr,
                );
            }
            println!("--[ Tree ]--");
        }
        println!(
            "{pad:indent$}{name:name_width$}{typename:30}{matcher:20}{meta}",
            pad = "",
            indent = indent,
            name = name,
            name_width = 30 - indent,
            typename = format!("{:?}", item.itemtype),
            matcher = format!("{:?}", item.filter),
            meta = format!("{:?}", item.meta)
        );
        for (child_name, child_item) in item.children.iter() {
            print_item(&child_name, child_item, indent + 4);
        }
    }
    print_item("<root>", item, 0);
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

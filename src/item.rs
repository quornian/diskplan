use regex::Captures;

use crate::meta::{ItemMeta, MetaError, RawItemMeta, RawPerms};
use std::convert::TryInto;
use std::path::PathBuf;
use std::{collections::HashMap, fs, path::Path};

#[derive(Debug, PartialEq)]
pub struct Item {
    itemtype: ItemType,
    vars: HashMap<String, String>,
    children: HashMap<String, Item>,
    meta: ItemMeta,
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

#[derive(thiserror::Error, Debug, PartialEq)]
pub enum EvaluationError {
    #[error("No such variable: {0}")]
    NoSuchVariable(String),
}

impl Item {
    pub fn default_typed(itemtype: ItemType) -> Item {
        Item {
            vars: HashMap::new(),
            children: HashMap::new(),
            meta: ItemMeta::default(),
            itemtype: itemtype,
            filter: Filter::Exact,
        }
    }

    /// Construct an Item from a directory
    ///
    /// All Items, including file items, are described by directories containing one or more
    /// "_."-prefixed entries
    ///
    pub fn from_path(path: PathBuf) -> Result<Item, ItemError> {
        item_from_path(path)
    }
}

fn item_from_path(path: PathBuf) -> Result<Item, ItemError> {
    let mut vars = HashMap::new();
    let mut defs = HashMap::new();
    let mut children = HashMap::new();
    let mut meta = RawItemMeta::default();
    let mut itemtype = None;
    let mut filter = None;
    let name = String::from(
        path.file_name()
            .ok_or_else(|| ItemError::UnexpectedItemError(path.clone()))?
            .to_string_lossy(),
    );
    if !name.starts_with("@") {
        filter = Some(Filter::Exact);
    }

    // Add context to directory read errors
    let with_path = |err| ItemError::DirectoryIOError(path.clone(), err);

    for entry in fs::read_dir(&path).map_err(with_path)? {
        let entry = entry.map_err(with_path)?;
        let filename = entry.file_name();
        let filename = filename.to_string_lossy();
        let metadata = entry.metadata()?;
        let filetype = metadata.file_type();

        // Physical type that this item describes (directory by default)
        //
        //   _.is.file    (File with physical content)
        //   _.is.link -> /target/may/@use/@vars
        //   _.is.reuse -> @definition
        //
        // Note that while files may not have children, symlinked targets may.
        //
        if let Some(typename) = filename.strip_prefix("_.is.") {
            if itemtype.is_some() {
                return Err(ItemError::DuplicateTypeAnnotation(entry.path()));
            }
            match typename {
                "file" => itemtype = Some(ItemType::File),
                "link" => itemtype = Some(ItemType::Symlink(parse_linked_string(&entry)?)),
                "reuse" => itemtype = Some(ItemType::Reuse(parse_linked_string(&entry)?)),
                _ => {
                    return Err(ItemError::UnknownItemType(entry.path()));
                }
            }
            continue;
        }

        // Variables defined at this level:
        //
        //   _.let.@somevar = an/@expr
        //
        if let Some(at_name) = filename.strip_prefix("_.let.") {
            // TODO: Validate variable name
            let expr = parse_linked_string(&entry)?;
            vars.insert(at_name.to_owned(), expr);
            continue;
        }

        // Sub-structures defined at this level:
        //
        //   _.def.@reusableitem/
        //
        // These are used by:
        //
        //   something/
        //     _.is.reuse -> @reusableitem
        //
        if let Some(at_name) = filename.strip_prefix("_.def.") {
            // TODO: Validate variable name
            let sub_item = Item::from_path(entry.path())?;
            defs.insert(at_name.to_owned(), sub_item);
            continue;
        }

        // Properties defined at this level. For example,
        //
        //   _.owner = admin
        //   _.group = admin
        //   _.perms = 0o755
        //
        if filename.starts_with("_.") {
            match &filename[2..] {
                "owner" => meta.owner = Some(parse_linked_string(&entry)?),
                "group" => meta.group = Some(parse_linked_string(&entry)?),
                "perms" => meta.permissions = Some(RawPerms(parse_linked_string(&entry)?)),
                "match" => {
                    if filter.is_some() {
                        return Err(ItemError::ConflictingFilter(entry.path()));
                    }
                    filter = Some(Filter::Pattern(parse_linked_string(&entry)?));
                }
                _ => return Err(ItemError::InvalidPropertyEntry(entry.path())),
            }
            continue;
        }

        // Loose files, directories and symlinks
        //
        let filename = filename.into_owned();
        if filetype.is_file() {
            children.insert(filename, Item::default_typed(ItemType::File));
        } else if filetype.is_dir() {
            let child = item_from_path(entry.path())?;
            children.insert(filename, child);
        } else if filetype.is_symlink() {
            let target = parse_linked_string(&entry)?;
            children.insert(filename, Item::default_typed(ItemType::Symlink(target)));
        } else {
            eprintln!("Invalid filetype: {}", filename);
        }
    }
    // If not otherwise set, default item type is directory
    let itemtype = itemtype.unwrap_or(ItemType::Directory);

    // Validate based on item type
    match itemtype {
        ItemType::File => {
            if !children.is_empty() {
                return Err(ItemError::ItemHasChildren(path));
            }
        }
        ItemType::Directory => {}
        ItemType::Symlink(_) => {}
        ItemType::Reuse(_) => {}
    }
    Ok(Item {
        vars: vars,
        children: children,
        meta: meta.try_into()?,
        itemtype: itemtype,
        filter: filter.unwrap_or(Filter::Freeform),
    })
}

fn parse_linked_string(entry: &fs::DirEntry) -> Result<String, ItemError> {
    Ok(String::from(fs::read_link(entry.path())?.to_string_lossy()))
}

pub fn print_item_tree(item: &Item) {
    fn print_item(name: &str, item: &Item, indent: usize) {
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

pub fn apply_tree(
    root: &PathBuf,
    name: &str,
    item: &Item,
    vars: &[HashMap<String, String>],
) -> Result<(), EvaluationError> {
    let mut install_args = vec!["install".to_owned()];
    if let Some(owner) = item.meta.owner() {
        install_args.push(format!("--owner={}", owner));
    }
    if let Some(group) = item.meta.group() {
        install_args.push(format!("--group={}", group));
    }
    if let Some(perms) = item.meta.permissions() {
        install_args.push(format!("--mode={:o}", perms.mode()));
    }
    let action = match item.itemtype {
        ItemType::Directory => {
            let mut path = root.to_owned();
            path.push(name);
            install_args.push("--directory".to_owned());
            install_args.push(String::from(path.to_string_lossy()));
            println!("Run: {:?}", install_args);

            for (name, child) in item.children.iter() {
                let name = evaluate_name(name, vars)?;
                apply_tree(&path, &name, child, vars)?;
            }
        }
        _ => eprintln!("NOT IMPLEMENTED"),
    };
    Ok(())
}

fn evaluate_name<S>(
    name: S,
    var_stack: &[HashMap<String, String>],
) -> Result<String, EvaluationError>
where
    S: AsRef<str>,
{
    // No variables: "some value"
    if !name.as_ref().contains("@") {
        return Ok(name.as_ref().to_owned());
    }
    // Simple expression: "@varname"
    let pattern = regex::Regex::new(r"@\w+$").unwrap();
    if pattern.is_match(name.as_ref()) {
        for vars in var_stack {
            if let Some(value) = vars.get(name.as_ref()) {
                return Ok(value.clone());
            }
        }
        return Err(EvaluationError::NoSuchVariable(name.as_ref().to_owned()));
    }
    // Complex expression: "@var1/{@var2}_fixed"
    let pattern = regex::Regex::new(r"\{([^{}]+)\}|@\w+|[^{}@]*").unwrap();
    let result = pattern.replace_all(name.as_ref(), |captures: &Captures| {
        evaluate_name(
            captures
                .get(1)
                .or(captures.get(0))
                .unwrap()
                .as_str()
                .to_owned(),
            var_stack,
        )
        .unwrap() // TODO: Lost my nice error handling due to closure :(
    });
    return Ok(String::from(result));
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

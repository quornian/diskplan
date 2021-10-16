use std::{collections::HashMap, convert::TryInto, fs, path::Path};

use super::{
    meta::{RawItemMeta, RawPerms},
    schema::{Filter, ItemError, ItemType, Schema},
};

pub fn item_from_path(path: &Path) -> Result<Schema, ItemError> {
    let mut vars = HashMap::new();
    let mut defs = HashMap::new();
    let mut children = HashMap::new();
    let mut meta = RawItemMeta::default();
    let mut itemtype = None;
    let mut filter = None;
    let name = String::from(
        path.file_name()
            .ok_or_else(|| ItemError::UnexpectedItemError(path.to_owned()))?
            .to_string_lossy(),
    );
    if !name.starts_with("@") {
        filter = Some(Filter::Exact);
    }

    // Add context to directory read errors
    let with_path = |err| ItemError::DirectoryIOError(path.to_owned(), err);

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
            let sub_item = item_from_path(&entry.path())?;
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
            children.insert(filename, Schema::default_typed(ItemType::File));
        } else if filetype.is_dir() {
            let child = item_from_path(&entry.path())?;
            children.insert(filename, child);
        } else if filetype.is_symlink() {
            let target = parse_linked_string(&entry)?;
            children.insert(filename, Schema::default_typed(ItemType::Symlink(target)));
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
                return Err(ItemError::ItemHasChildren(path.to_owned()));
            }
        }
        ItemType::Directory => {}
        ItemType::Symlink(_) => {}
        ItemType::Reuse(_) => {}
    }
    Ok(Schema::new(
        vars,
        children,
        itemtype,
        meta.try_into()?,
        filter.unwrap_or(Filter::Freeform),
    ))
}

fn parse_linked_string(entry: &fs::DirEntry) -> Result<String, ItemError> {
    Ok(String::from(fs::read_link(entry.path())?.to_string_lossy()))
}

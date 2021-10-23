use std::{
    collections::HashMap,
    convert::TryInto,
    fmt::Display,
    fs,
    path::{Path, PathBuf},
    str::FromStr,
};

use crate::schema::{
    criteria::{Match, MatchCriteria},
    expr::{Expression, Token},
    meta::{Meta, RawItemMeta, RawPerms},
    DirectorySchema, FileSchema, LinkSchema, Schema, SchemaError,
};

pub fn schema_from_path(path: &Path) -> Result<Schema, SchemaError> {
    // Physical type that this item describes (directory by default)
    //
    //   _.is.file    (File with physical content)
    //   _.is.link -> /target/may/@use/@vars
    //   _.is.reuse -> @definition
    //
    // Note that while files may not have children, symlinked targets may.
    //
    if !path.is_dir() {
        return Err(SchemaError::NonDirectorySchemaEntry(path.to_owned()));
    }
    let file_indicator = path.join("_.is.file");
    let link_indicator = path.join("_.is.link");
    let use_indicator = path.join("_.is.use");
    let parse_linked_use = |ind| {
        let value = parse_linked_string(ind)?;
        value
            .strip_prefix("@")
            .map(|s| s.to_owned())
            .ok_or_else(|| SchemaError::PropertyParseFailure(ind.into(), value))
    };
    let schema = match (
        file_indicator.present(),
        link_indicator.present(),
        use_indicator.present(),
    ) {
        (true, true, _) | (true, _, true) | (_, true, true) => {
            return Err(SchemaError::MultipleTypeAnnotation(path.to_owned()))
        }
        (true, _, _) => Schema::File(file_schema_from_path(path, file_indicator)?),
        (_, true, _) => Schema::Symlink(link_schema_from_path(path, link_indicator)?),
        (_, _, true) => Schema::Use(parse_linked_use(&use_indicator)?),
        (_, _, _) => Schema::Directory(directory_schema_from_path(path)?),
    };

    fn directory_schema_from_path(path: &Path) -> Result<DirectorySchema, SchemaError> {
        let meta = meta_from_path(path)?;
        let mut vars = HashMap::new();
        let mut defs = HashMap::new();
        let mut entries = Vec::new();

        // Add context to directory read errors
        let with_path = |err| SchemaError::IOError(path.to_owned(), err);

        for dir_entry in fs::read_dir(&path).map_err(with_path)? {
            let dir_entry = dir_entry.map_err(with_path)?;
            let name = String::from(dir_entry.file_name().to_string_lossy());
            if name.starts_with("_.") {
                // Variables defined at this level:
                //
                //   _.let.@somevar = an/@expr
                //
                if let Some(var) = name.strip_prefix("_.let.@") {
                    // TODO: Validate variable name
                    let expr = parse_linked_string(&dir_entry.path())?;
                    // FIXME: Parse expression
                    assert!(!expr.contains("$"));
                    let expr = Expression::new(vec![Token::text(expr)]);
                    vars.insert(var.to_owned(), expr);
                    continue;
                }

                // Defined sub-schemas at this level:
                //
                //   _.def.@reusableitem/
                //
                if let Some(def) = name.strip_prefix("_.def.@") {
                    // TODO: Validate variable name
                    let sub_item = schema_from_path(&dir_entry.path())?;
                    defs.insert(def.to_owned(), sub_item);
                    continue;
                }

                // Validate remaining options, skipping those already handled
                match name.as_ref() {
                    "_.is.file" | "_.is.link" | "_.is.use" => (),
                    "_.owner" | "_.group" | "_.perms" => (), // Handled by Meta
                    "_.match" | "_.order" => (),             // Handled by MatchCriteria
                    _ => return Err(SchemaError::UnexpectedItemError(dir_entry.path())),
                }
            } else {
                let order_indicator = dir_entry.path().join("_.order");
                let pattern_indicator = dir_entry.path().join("_.match");

                let order = {
                    if order_indicator.present() {
                        parse_linked(&order_indicator)?
                    } else {
                        0i16
                    }
                };
                let map_regex_err = |e| SchemaError::RegexParseFailure(path.to_owned(), e);
                let binding = name.strip_prefix("@");
                let mode = {
                    match (pattern_indicator.present(), binding) {
                        (false, Some(binding)) => Match::Any {
                            binding: binding.to_owned(),
                        },
                        (true, Some(binding)) => {
                            let pattern = parse_linked_string(&pattern_indicator)?;
                            Match::from_regex(&pattern, binding).map_err(map_regex_err)?
                        }
                        (false, None) => Match::Fixed(String::from(name)),
                        (true, None) => {
                            return Err(SchemaError::NonVariableWithPattern(path.to_owned()))
                        }
                    }
                };
                let criteria = MatchCriteria::new(order, mode);
                let schema = schema_from_path(&dir_entry.path())?;
                entries.push((criteria, schema));
            }
        }
        Ok(DirectorySchema::new(vars, defs, meta, entries))
    }

    fn file_schema_from_path(path: &Path, ind: PathBuf) -> Result<FileSchema, SchemaError> {
        let meta = meta_from_path(path)?;
        Ok(FileSchema::new(meta, ind))
    }

    fn link_schema_from_path(path: &Path, ind: PathBuf) -> Result<LinkSchema, SchemaError> {
        let with_path = |err| SchemaError::IOError(path.to_owned(), err);
        let target = String::from(ind.read_link().map_err(with_path)?.to_string_lossy());
        // FIXME: Parse expression
        assert!(!target.contains("$"));
        let expr = Expression::new(vec![Token::text(target)]);
        // For now we always assume the other end of a link is a directory, and has a directory
        // schema, but will only create this target if the schema is not a no-op
        Ok(LinkSchema::new(
            expr,
            Schema::Directory(directory_schema_from_path(path)?),
        ))
    }

    fn meta_from_path(path: &Path) -> Result<Meta, SchemaError> {
        // Properties defined at this level. For example,
        //
        //   _.owner = admin
        //   _.group = admin
        //   _.perms = 0o755
        //
        let owner_link = path.join("_.owner");
        let group_link = path.join("_.group");
        let perms_link = path.join("_.perms");

        fn parse_meta_link(path: &Path) -> Result<Option<String>, SchemaError> {
            match fs::read_link(path) {
                Ok(path) => Ok(Some(String::from(path.to_string_lossy()))),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
                Err(e) => Err(SchemaError::IOError(path.to_owned(), e)),
            }
        }
        let raw = RawItemMeta {
            owner: parse_meta_link(&owner_link)?,
            group: parse_meta_link(&group_link)?,
            permissions: parse_meta_link(&perms_link)?.map(|p| RawPerms(p)),
        };
        raw.try_into()
            .map_err(|e| SchemaError::MetaError(path.to_owned(), e))
    }
    Ok(schema)
}

fn parse_linked_string(path: &Path) -> Result<String, SchemaError> {
    fs::read_link(path)
        .map(|s| String::from(s.to_string_lossy()))
        .map_err(|e| SchemaError::IOError(path.to_owned(), e))
}

fn parse_linked<T>(path: &Path) -> Result<T, SchemaError>
where
    T: FromStr,
    <T as FromStr>::Err: Display,
{
    fs::read_link(path)
        .map_err(|e| SchemaError::IOError(path.to_owned(), e))
        .map(|s| str::parse::<T>(&s.to_string_lossy()))?
        .map_err(|err| SchemaError::PropertyParseFailure(path.to_owned(), format!("{}", err)))
}

trait Present {
    fn present(&self) -> bool;
}

impl Present for PathBuf {
    fn present(&self) -> bool {
        self.symlink_metadata().is_ok()
    }
}

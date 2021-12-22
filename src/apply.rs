//! Provides [`Action`] to describe ordered, actionable events to realize a [`schema`][crate::schema] on disk
//!
use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Context as _, Result};
use regex::Regex;

use crate::{
    context::Context,
    schema::{
        DirectorySchema, FileSchema, LinkSchema, Match, Merge, Meta, Schema, SchemaEntry, Subschema,
    },
};

/// The process to perform to apply a schema node to the target location on the filesystem
#[derive(Debug, Clone, PartialEq)]
pub enum Action<'t> {
    CreateDirectory {
        path: PathBuf,
        meta: Meta<'t>,
    },
    CreateSymlink {
        path: PathBuf,
        target: PathBuf,
    },
    CreateFile {
        path: PathBuf,
        source: PathBuf,
        meta: Meta<'t>,
    },
}

pub fn gather_actions<'t>(context: &Context<'t, '_>) -> Result<Vec<Action<'t>>> {
    let mut actions = Vec::new();
    apply_tree(context, &mut actions).map(|()| actions)
}

fn apply_tree<'t>(context: &'t Context<'t, '_>, actions: &mut Vec<Action<'t>>) -> Result<()> {
    eprintln!(
        "Applying to {}: {}",
        &context.root.to_str().unwrap(),
        &context.target.to_str().unwrap()
    );
    match context.schema {
        Schema::File(file_schema) => apply_file(file_schema, context, actions),
        Schema::Symlink(link_schema) => apply_link(link_schema, context, actions),
        Schema::Directory(dir_schema) => apply_directory(dir_schema, context, actions),
    }
    .with_context(|| format!("Failed to apply tree {:?}", context.target))
}

fn apply_file<'t>(
    file_schema: &FileSchema<'t>,
    context: &Context<'t, '_>,
    actions: &mut Vec<Action<'t>>,
) -> Result<()> {
    // Ensure the file exists with the correct permissions and ownership
    // TODO: Consider skipping subprocess call if metadata already matches
    // install::install_file(&context.target, file_schema.source(), file_schema.meta());
    let source = context.evaluate(file_schema.source())?;

    actions.push(Action::CreateFile {
        path: normalize(&context.root.join(&context.target)),
        source: source.into(),
        meta: (*file_schema.meta()).clone(),
    });
    Ok(())
}

fn apply_link<'t>(
    link_schema: &LinkSchema<'t>,
    context: &Context<'t, '_>,
    actions: &mut Vec<Action<'t>>,
) -> Result<()> {
    // Ensure the link exists and its evaluated target path is absolute
    let link_target = context.evaluate(link_schema.target())?;
    let link_target_path = Path::new(&link_target);

    if !link_target_path.is_absolute() {
        return Err(anyhow!(
            "Link target is not absolute: {} ({})\n{:?}",
            link_schema.target(),
            link_target,
            link_schema.far_schema(),
        ));
    }

    // TODO: Consider skipping if link already exists
    // install::install_link(&context.target, link_target_path)?;
    actions.push(Action::CreateSymlink {
        path: normalize(&context.root.join(&context.target)),
        target: normalize(link_target_path),
    });
    if let Some(far_schema) = link_schema.far_schema() {
        // TODO: Check root/target is okay like this
        let far_context = Context::new(&far_schema, link_target_path, Path::new("."));
        apply_tree(&far_context, actions)?;
    }
    Ok(())
}

fn apply_directory<'t>(
    directory_schema: &'t DirectorySchema<'t>,
    context: &Context<'t, '_>,
    actions: &mut Vec<Action<'t>>,
) -> Result<()> {
    // Ensure the directory exists with the correct permissions and ownership
    // TODO: Consider skipping subprocess call if metadata already matches
    // install::install_directory(&context.target, directory_schema.meta())?;
    actions.push(Action::CreateDirectory {
        path: normalize(&context.root.join(&context.target)),
        meta: (*directory_schema.meta()).clone(),
    });

    handle_entries(directory_schema.entries(), context, &mut actions)
}

struct DirectoryMap {
    listing: BTreeMap<OsString, bool>,
}

impl DirectoryMap {
    pub fn from_directory(path: PathBuf) -> Result<DirectoryMap> {
        let listing: Result<BTreeMap<_, _>, _> = match fs::read_dir(&path) {
            Ok(read_dir) => read_dir
                .map(|dir_ent_res| dir_ent_res.map(|dir_ent| (dir_ent.file_name(), false)))
                .collect(),
            // If we fail to read the directory we assume it doesn't exist yet
            Err(_) => Ok(BTreeMap::default()),
        };
        Ok(DirectoryMap { listing: listing? })
    }

    /// Returns an error if already handled
    pub fn about_to_handle(&mut self, key: OsString) -> Result<(), ()> {
        match self.listing.insert(key, true) {
            None | Some(false) => Ok(()),
            Some(true) => Err(()),
        }
    }

    pub fn unhandled(&mut self) -> impl Iterator<Item = (&OsString, &mut bool)> + '_ {
        self.listing.iter_mut().filter(|(_, &mut handled)| !handled)
    }
}

fn handle_entries<'t: 'i, 'i, I: Iterator<Item = &'i SchemaEntry<'t>>>(
    entries: I,
    context: &Context<'t, '_>,
    actions: &mut Vec<Action<'t>>,
) -> Result<()> {
    // Handle entries within this directory
    let mut map = DirectoryMap::from_directory(context.root.join(&context.target))?;

    // Algorithm overview:
    //  - Loop over schema entries at this level (sorted fixed first)
    //  - For a fixed name, apply to directory entry of this name, mark as handled
    //  - For variable names, loop over directory and apply to all matching entries that were
    //    not already handled, mark as handled

    for entry in entries {
        match &entry.criteria {
            Match::Fixed(name) => {
                match map.about_to_handle(name.into()) {
                    Ok(()) => {
                        // New | Update
                        let child_path = context.target.join(name);
                        let merged;
                        let schema = match &entry.subschema {
                            Subschema::Original(schema) => schema,
                            Subschema::Referenced {
                                definition: use_def,
                                overrides,
                            } => {
                                merged = context
                                    .follow_schema(&use_def)
                                    .ok_or_else(|| anyhow!("No matching #def for {}", use_def))?
                                    .merge(&overrides)?;
                                &merged
                            }
                        };
                        apply_tree(&context.child(child_path, schema), actions)?;
                    }
                    Err(()) => {
                        // Earlier rule handled this, but this is a Fixed match. Seems suspicious...
                        let child_path = context.target.join(name);
                        eprintln!(
                            "Warning: Fixed({}) rule matches an entry in directory {}, but was already handled by earlier rule",
                            name, child_path.to_string_lossy());
                    }
                }
            }
            Match::Variable {
                order: _order,
                pattern,
                binding,
            } => {
                // Turn pattern into a regular expression
                let pattern = match pattern {
                    None => None,
                    Some(pattern) => Some(full_regex(&context.evaluate(&pattern)?)?),
                };
                // If we have this binding in our variables already, resolve it and use the fixed result
                // issuing an error if the pattern doesn't match (no need to re-bind)
                let name = context.lookup(&binding)?;
                if let Some(name) = name {
                    if let Some(pattern) = pattern {
                        if !pattern.is_match(&name) {
                            return Err(anyhow!(
                                "Pattern mismatch: {} against {}",
                                pattern.to_string(),
                                name,
                            ));
                        }
                    }
                    // TODO: Reduce this mayhem...
                    match map.about_to_handle(name.clone().into_owned().into()) {
                        Ok(()) => {
                            // New | Update
                            let child_path = context.target.join(name.as_ref());
                            let merged;
                            let schema = match &entry.subschema {
                                Subschema::Original(ref schema) => schema,
                                Subschema::Referenced {
                                    definition: use_def,
                                    overrides,
                                } => {
                                    merged = context
                                        .follow_schema(&use_def)
                                        .ok_or_else(|| anyhow!("No matching #def for {}", use_def))?
                                        .merge(&overrides)?;
                                    &merged
                                }
                            };
                            apply_tree(&context.child(child_path, schema), actions)?;
                        }
                        Err(()) => (), // Earlier rule handled
                    }
                }
                // Otherwise match the pattern against all unhandled entries, mark matches as handled,
                // and bind their names to the variable for the child schemas
                else {
                    for (name, handled_ref) in map.unhandled() {
                        if let Some(name) = name.to_str() {
                            let matched = match pattern {
                                None => true,
                                Some(ref pattern) => pattern.is_match(name),
                            };
                            if matched {
                                *handled_ref = true;
                                let child_path = context.target.join(name);
                                let merged;
                                let schema = &match &entry.subschema {
                                    Subschema::Original(schema) => schema,
                                    Subschema::Referenced {
                                        definition: use_def,
                                        overrides,
                                    } => {
                                        merged = context
                                            .follow_schema(&use_def)
                                            .ok_or_else(|| {
                                                anyhow!("No matching #def for {}", use_def)
                                            })?
                                            .merge(&overrides)?;
                                        &merged
                                    }
                                };
                                let mut child_context = context.child(child_path, schema);
                                // No need to parse, we know this is a Text token
                                child_context.bind(binding.clone(), name.into());
                                apply_tree(&child_context, actions)?;
                            }
                        }
                        // else: Ignore file names we couldn't read
                    }
                }
            }
        }
    }
    Ok(())
}

fn full_regex(pattern: &str) -> Result<Regex, regex::Error> {
    Regex::new(&pattern)?;
    regex::Regex::new(&format!("^(?:{})$", pattern))
}

fn normalize(path: &Path) -> PathBuf {
    path.components().collect()
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use crate::schema::{Identifier, MetaBuilder};

    use super::*;

    #[test]
    fn test_use() {
        let schema = Schema::Directory({
            let vars = HashMap::default();
            let mut defs = HashMap::default();
            defs.insert(
                Identifier::new("thing"),
                Schema::Directory(DirectorySchema::new(
                    HashMap::new(),
                    HashMap::new(),
                    MetaBuilder::default().mode(0o777).build(),
                    vec![],
                )),
            );
            let meta = MetaBuilder::default().owner("user1").build();
            DirectorySchema::new(
                vars,
                defs,
                meta,
                vec![SchemaEntry {
                    criteria: Match::Fixed("place"),
                    subschema: Subschema::Referenced {
                        definition: Identifier::new("thing"),
                        overrides: Schema::Directory(DirectorySchema::new(
                            HashMap::new(),
                            HashMap::new(),
                            MetaBuilder::default().owner("user2").build(),
                            vec![],
                        )),
                    },
                }],
            )
        });

        let context = Context::new(&schema, &Path::new("/tmp/root"), &Path::new("."));
        let mut actions = Vec::new();
        apply_tree(&context, &mut actions).unwrap();
        assert_eq!(
            actions,
            vec![
                Action::CreateDirectory {
                    meta: MetaBuilder::default().owner("user1").build(),
                    path: PathBuf::from("/tmp/root")
                },
                Action::CreateDirectory {
                    meta: MetaBuilder::default().owner("user2").mode(0o777).build(),
                    path: PathBuf::from("/tmp/root/place")
                }
            ]
        );
    }
}

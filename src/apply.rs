use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Context as _, Result};
use regex::Regex;

use crate::{
    context::Context,
    schema::{
        criteria::Match,
        expr::{Expression, Token},
        meta::Meta,
        DirectorySchema, FileSchema, LinkSchema, Merge, Schema, SchemaEntry, Subschema,
    },
};

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    CreateDirectory {
        path: PathBuf,
        meta: Meta,
    },
    CreateSymlink {
        path: PathBuf,
        target: PathBuf,
    },
    CreateFile {
        path: PathBuf,
        source: PathBuf,
        meta: Meta,
    },
}

pub fn gather_actions(context: &Context) -> Result<Vec<Action>> {
    let mut actions = Vec::new();
    apply_tree(context, &mut actions).map(|()| actions)
}

fn apply_tree(context: &Context, actions: &mut Vec<Action>) -> Result<()> {
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

fn apply_file(
    file_schema: &FileSchema,
    context: &Context,
    actions: &mut Vec<Action>,
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

fn apply_link(
    link_schema: &LinkSchema,
    context: &Context,
    actions: &mut Vec<Action>,
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

fn apply_directory(
    directory_schema: &DirectorySchema,
    context: &Context,
    actions: &mut Vec<Action>,
) -> Result<()> {
    // Ensure the directory exists with the correct permissions and ownership
    // TODO: Consider skipping subprocess call if metadata already matches
    // install::install_directory(&context.target, directory_schema.meta())?;
    actions.push(Action::CreateDirectory {
        path: normalize(&context.root.join(&context.target)),
        meta: directory_schema.meta().clone(),
    });

    handle_entries(directory_schema.entries(), context, actions)
}

fn handle_entries(
    entries: &Vec<SchemaEntry>,
    context: &Context,
    actions: &mut Vec<Action>,
) -> Result<()> {
    // Handle entries within this directory
    let mut entries_handled = (|| {
        let listing: Result<HashMap<_, bool>, _> = fs::read_dir(&context.target)?
            .map(|x| x.map(|ent| (ent.file_name(), false)))
            .collect();
        listing
    })()
    .unwrap_or_default();

    // Algorithm overview:
    //  - Loop over schema entries, which are sorted by their criteria orders
    //  - Match

    for entry in entries {
        match &entry.criteria {
            Match::Fixed(name) => {
                let was_handled = entries_handled.insert(name.into(), true);
                match was_handled {
                    None | Some(false) => {
                        // New | Update
                        let child_path = context.target.join(name);
                        let merged;
                        let schema = match &entry.schema {
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
                    Some(true) => {
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
                let expr = context.lookup(&binding);
                if let Some(expr) = expr {
                    let name: String = context.evaluate(&expr)?;
                    if let Some(pattern) = pattern {
                        if !pattern.is_match(&name) {
                            return Err(anyhow!(
                                "Pattern mismatch: {} against {}",
                                pattern.to_string(),
                                name,
                            ));
                        }
                    }
                    let was_handled = entries_handled.insert(name.clone().into(), true);
                    match was_handled {
                        None | Some(false) => {
                            // New | Update
                            let child_path = context.target.join(name);
                            let merged;
                            let schema = match &entry.schema {
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
                        Some(true) => (), // Earlier rule handled
                    }
                }
                // Otherwise match the pattern against all entries, mark matches as handled,
                // and bind their names to the variable for the child schemas
                else {
                    for (name, handled) in &entries_handled {
                        if let Some(name) = name.to_str() {
                            let matched = match pattern {
                                None => true,
                                Some(ref pattern) => pattern.is_match(name),
                            };
                            if matched {
                                if !handled {
                                    // New
                                } else {
                                    // Update
                                }
                                let child_path = context.target.join(name);
                                let merged;
                                let schema = &match &entry.schema {
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
                                let expr = Expression::new(vec![Token::text(name)]);
                                child_context.bind(binding.clone(), expr);
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
    use crate::schema::{expr::Identifier, meta::MetaBuilder};

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
                    criteria: Match::fixed("place"),
                    schema: Subschema::Referenced {
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

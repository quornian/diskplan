use std::{collections::HashMap, fs, io, path::PathBuf};

use crate::{
    application::context::Context,
    definition::{
        criteria::{Match, MatchCriteria},
        schema::{DirectorySchema, FileSchema, LinkSchema, Schema},
    },
};

pub mod context;
pub mod eval;
pub mod install;
pub mod parse;

#[derive(thiserror::Error, Debug)]
pub enum ApplicationError {
    #[error("IOError occurred during application of schema on: {0}")]
    IOError(PathBuf, #[source] io::Error),

    #[error("Command failed to run (exit code {1}) on: {0}\n  {2}")]
    CommandError(PathBuf, i32, String),

    #[error(transparent)]
    EvaluationError(#[from] eval::EvaluationError),

    #[error("No definition found for {1} under: {0}")]
    DefNotFound(PathBuf, String),

    #[error("Pattern {0} does not match {1}")]
    PatternMismatch(String, String),
}

pub fn apply_tree(context: &context::Context) -> Result<(), ApplicationError> {
    eprintln!("Applying to {}", &context.target.to_str().unwrap());
    match context.schema {
        Schema::File(file_schema) => apply_file(file_schema, context)?,
        Schema::Symlink(link_schema) => apply_link(link_schema, context)?,
        Schema::Directory(dir_schema) => apply_directory(dir_schema, context)?,
        Schema::Use(name) => apply_def_use(name, context)?,
    }
    Ok(())
}

fn apply_def_use(name: &String, context: &Context) -> Result<(), ApplicationError> {
    eprintln!("Looking up definition {}", name);
    let child = context
        .follow(name)
        .ok_or_else(|| ApplicationError::DefNotFound(context.target.clone(), name.clone()))?;
    apply_tree(&child)
}

fn apply_file(file_schema: &FileSchema, context: &Context) -> Result<(), ApplicationError> {
    // Ensure the file exists with the correct permissions and ownership
    // TODO: Consider skipping subprocess call if metadata already matches
    install::install_file(&context.target, file_schema.source(), file_schema.meta())?;
    Ok(())
}

fn apply_link(link_schema: &LinkSchema, context: &Context) -> Result<(), ApplicationError> {
    eprintln!(
        "Not implemented: create_link({:?}, ...)\n  {}",
        link_schema,
        context.target.to_string_lossy()
    );
    Ok(())
}

fn apply_directory(
    directory_schema: &DirectorySchema,
    context: &Context,
) -> Result<(), ApplicationError> {
    // Ensure the directory exists with the correct permissions and ownership
    // TODO: Consider skipping subprocess call if metadata already matches
    install::install_directory(&context.target, directory_schema.meta())?;

    handle_entries(directory_schema.entries(), context)
}

fn handle_entries(
    entries: &Vec<(MatchCriteria, Schema)>,
    context: &Context,
) -> Result<(), ApplicationError> {
    let target = &context.target;
    let map_io_err = |e| ApplicationError::IOError(target.to_owned(), e);

    // Handle entries within this directory
    let mut entries_handled = {
        let listing: Result<HashMap<_, bool>, _> = fs::read_dir(&context.target)
            .map_err(map_io_err)?
            .map(|x| x.map(|ent| (ent.file_name(), false)))
            .collect();
        listing.map_err(map_io_err)?
    };

    // Algorithm overview:
    //  - Loop over schema entries, which are sorted by their criteria orders
    //  - Match

    for (criteria, schema) in entries {
        match criteria.mode() {
            Match::Fixed(name) => {
                let was_handled = entries_handled.insert(name.into(), true);
                match was_handled {
                    None => {
                        // New
                        let child_path = target.join(name);
                        apply_tree(&context.child(child_path, schema))?;
                    }
                    Some(false) => {
                        // Update
                        let child_path = target.join(name);
                        apply_tree(&context.child(child_path, schema))?;
                    }
                    Some(true) => {
                        // Earlier rule handled this, but this is a Fixed match. Seems suspicious...
                        let child_path = target.join(name);
                        eprintln!(
                            "Warning: Fixed({}) rule matches an entry in directory {}, but was already handled by earlier rule",
                            name, child_path.to_string_lossy());
                    }
                }
            }
            Match::Regex { pattern, binding } => {
                // If we have this binding in our variables already, resolve it and use the fixed result
                // issuing an error if the pattern doesn't match (no need to re-bind)
                let name = context.lookup(binding);
                if let Some(name) = name {
                    if !pattern.is_match(name) {
                        return Err(ApplicationError::PatternMismatch(
                            pattern.to_string(),
                            name.to_string(),
                        ));
                    }
                    let was_handled = entries_handled.insert(name.into(), true);
                    match was_handled {
                        None => {
                            // New
                            let child_path = target.join(name);
                            apply_tree(&context.child(child_path, schema))?;
                        }
                        Some(false) => {
                            // Update
                            let child_path = target.join(name);
                            apply_tree(&context.child(child_path, schema))?;
                        }
                        Some(true) => (), // Earlier rule handled
                    }
                }
                // Otherwise match the pattern against all entries, mark matches as handled,
                // and bind their names to the variable for the child schemas
                else {
                    for (name, handled) in &entries_handled {
                        if let Some(name) = name.to_str() {
                            if pattern.is_match(name) {
                                if !handled {
                                    // New
                                    let child_path = target.join(name);
                                    let mut child_context = context.child(child_path, schema);
                                    child_context.bind(binding, name);
                                    apply_tree(&child_context)?;
                                } else {
                                    // Update
                                    let child_path = target.join(name);
                                    let mut child_context = context.child(child_path, schema);
                                    child_context.bind(binding, name);
                                    apply_tree(&child_context)?;
                                }
                            }
                        }
                        // else: Ignore file names we couldn't read
                    }
                }
            }
            Match::Any { binding } => {
                // Finally the Any type matches everything, mark everything as handled,
                // and bind their names to the variable for the child schemas
                for (name, handled) in &entries_handled {
                    if let Some(name) = name.to_str() {
                        if !handled {
                            // New
                            let child_path = target.join(name);
                            let mut child_context = context.child(child_path, schema);
                            child_context.bind(binding, name);
                            apply_tree(&child_context)?;
                        } else {
                            // Update
                            let child_path = target.join(name);
                            let mut child_context = context.child(child_path, schema);
                            child_context.bind(binding, name);
                            apply_tree(&child_context)?;
                        }
                    }
                    // else: Ignore file names we couldn't read
                }
            }
        }
    }
    Ok(())
}

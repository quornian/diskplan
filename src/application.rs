use std::{collections::HashMap, fs, io, path::PathBuf};

use crate::{
    application::context::Context,
    definition::{
        criteria::Match,
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

    #[error("Pattern {0} does not match {1}")]
    PatternMismatch(String, String),
}

pub fn apply_tree(context: &context::Context) -> Result<(), ApplicationError> {
    match context.schema {
        Schema::File(file_schema) => apply_file(file_schema, context)?,
        Schema::Symlink(link_schema) => apply_link(link_schema, context)?,
        Schema::Directory(dir_schema) => apply_directory(dir_schema, context)?,
        Schema::Use(_) => panic!("not implemented"),
    }
    Ok(())
}

fn apply_file(file_schema: &FileSchema, context: &Context) -> Result<(), ApplicationError> {
    eprintln!(
        "Not implemented: create_file({:?}, ...)\n  {}",
        file_schema,
        context.target.to_string_lossy()
    );
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
    let target = &context.target;
    let map_io_err = |e| ApplicationError::IOError(target.to_owned(), e);

    // Ensure the directory exists with the correct permissions and ownership
    // TODO: Consider skipping subprocess call if metadata already matches
    install::install_directory(target, directory_schema.meta())?;

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

    for (criteria, schema) in directory_schema.entries() {
        match criteria.mode() {
            Match::Fixed(name) => {
                let was_handled = entries_handled.insert(name.into(), true);
                match was_handled {
                    None => {
                        // New
                        apply_tree(&context.child(&name, schema))?;
                    }
                    Some(false) => {
                        // Update
                        apply_tree(&context.child(&name, schema))?;
                    }
                    Some(true) => {
                        // Earlier rule handled this, but this is a Fixed match. Seems suspicious...
                        eprintln!("Warning: Fixed({}) rule matches an entry in directory {}, but was already handled by earlier rule", name, target.to_string_lossy());
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
                            apply_tree(&context.child(&name, schema))?;
                        }
                        Some(false) => {
                            // Update
                            apply_tree(&context.child(&name, schema))?;
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
                                    let mut child_context = context.child(&name, schema);
                                    child_context.bind(binding, name);
                                    apply_tree(&child_context)?;
                                } else {
                                    // Update
                                    let mut child_context = context.child(&name, schema);
                                    child_context.bind(binding, name);
                                    apply_tree(&child_context)?;
                                }
                            }
                        }
                        // else: Ignore file names we couldn't read
                    }
                }
            }
            Match::Any { binding } => {}
        }
    }
    Ok(())
}

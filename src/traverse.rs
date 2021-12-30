use std::{borrow::Cow, path::Path};

use anyhow::Result;

mod eval;
mod pattern;

use crate::{
    filesystem::{self, Filesystem},
    schema::{Binding, DirectorySchema, FileSchema, Identifier, Schema, SchemaNode},
    traverse::{eval::evaluate, pattern::CompiledPattern},
};

pub fn traverse<'a, FS>(root: &'a SchemaNode<'_>, filesystem: &FS, target: &str) -> Result<()>
where
    FS: Filesystem,
{
    let mut stack = vec![];
    traverse_over(&root, &mut stack, filesystem, target)
}

enum Scope<'a> {
    Directory(&'a DirectorySchema<'a>),
    Binding(&'a Identifier<'a>, String),
}

// TODO: Error handling: include stack in result and format as traceback
fn traverse_over<'a, FS>(
    node: &'a SchemaNode<'_>,
    stack: &mut Vec<Scope<'a>>,
    filesystem: &FS,
    path: &str,
) -> Result<()>
where
    FS: Filesystem,
{
    match node.schema {
        Schema::Directory(ref directory) => {
            let mut listing = filesystem
                .list_directory(path)
                .unwrap_or_else(|_| vec![])
                .into_iter()
                .map(Some)
                .collect();

            stack.push(Scope::Directory(directory));

            for (binding, child_node) in directory.entries() {
                let pattern = CompiledPattern::compile(child_node.pattern.as_ref(), stack)?;
                for name in marked_matches(&mut listing, binding, pattern) {
                    let child_path = filesystem::join(path, name.as_ref());

                    create(child_node, &child_path, filesystem, stack)?;

                    match binding {
                        Binding::Static(_) => {
                            traverse_over(child_node, stack, filesystem, &child_path)?
                        }
                        Binding::Dynamic(var) => {
                            stack.push(Scope::Binding(var, name.into()));
                            traverse_over(child_node, stack, filesystem, &child_path)?;
                            stack.pop();
                        }
                    }
                }
            }

            stack.pop();
        }
        Schema::File(ref file) => {
            let source = evaluate(file.source(), stack)?;
            println!("{0:1$}#source {2}", "", stack.len(), source);
        }
    }
    Ok(())
}

fn create<FS>(node: &SchemaNode, path: &str, filesystem: &FS, stack: &mut Vec<Scope>) -> Result<()>
where
    FS: Filesystem,
{
    match &node.schema {
        Schema::Directory(_) => {
            if !filesystem.is_directory(path) {
                filesystem.create_directory(path)?;
            }
        }
        Schema::File(file) => {
            if !filesystem.is_file(path) {
                // FIXME: Copy file, don't create one with the contents of the source
                filesystem.create_file(path, evaluate(file.source(), stack)?)?;
            }
        }
    }
    Ok(())
}

fn marked_matches<'a>(
    listing: &mut Vec<Option<String>>,
    binding: &Binding<'a>,
    pattern: CompiledPattern<'a>,
) -> impl Iterator<Item = Cow<'a, str>> {
    let mut matched = Vec::new();

    match binding {
        // Static binding produces a match for that name only and always
        &Binding::Static(name) => matched.push(Cow::Borrowed(name)),
        // Dynamic bindings remove items from the listing pool that match
        &Binding::Dynamic(_) => {
            for entry in listing {
                if let Some(name) = entry {
                    if pattern.matches(name) {
                        let name: String = entry.take().unwrap();
                        matched.push(Cow::Owned(name))
                    }
                }
            }
        }
    };
    matched.into_iter()
}

#[cfg(test)]
mod tests;

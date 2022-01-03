use std::borrow::Cow;

use anyhow::{Context as _, Result};

mod eval;
mod pattern;

use crate::{
    filesystem::{parent, Filesystem, SplitPath},
    schema::{Binding, DirectorySchema, Identifier, Schema, SchemaNode},
    traversal::{eval::evaluate, pattern::CompiledPattern},
};

pub fn traverse<'a, FS>(root: &'a SchemaNode<'_>, filesystem: &FS, target: &str) -> Result<()>
where
    FS: Filesystem,
{
    let mut stack = vec![];
    traverse_over(&root, &mut stack, filesystem, &SplitPath::new(target)?)
        .with_context(|| format!("Traversing over {}", target.to_owned()))
}

#[derive(Debug)]
enum Scope<'a> {
    Directory(&'a DirectorySchema<'a>),
    Binding(&'a Identifier<'a>, String),
}

impl Scope<'_> {
    pub fn as_binding(&self) -> Option<(&Identifier, &String)> {
        match self {
            Scope::Binding(id, value) => Some((id, value)),
            _ => None,
        }
    }
}

// TODO: Error handling: include stack in result and format as traceback
fn traverse_over<'a, FS>(
    node: &'a SchemaNode<'_>,
    stack: &mut Vec<Scope<'a>>,
    filesystem: &FS,
    path: &SplitPath,
) -> Result<()>
where
    FS: Filesystem,
{
    // Create this entry, following symlinks
    create(node, stack, filesystem, &path)?;

    // Traverse over children
    if let Schema::Directory(ref directory) = node.schema {
        let mut listing = filesystem
            .list_directory(path.absolute())
            .unwrap_or_else(|_| vec![])
            .into_iter()
            .map(Some)
            .collect();

        stack.push(Scope::Directory(directory));

        for (binding, child_node) in directory.entries() {
            // Note: Since we don't know the name of the thing we're matching yet, any path
            // variable (e.g. SAME_PATH_NAME) used in the pattern expression will be evaluated
            // using the parent directory
            let pattern = CompiledPattern::compile(child_node.pattern.as_ref(), stack, &path)?;

            for name in marked_matches(&mut listing, binding, pattern) {
                let child_path = path.join(name.as_ref());

                match binding {
                    Binding::Static(_) => traverse_over(child_node, stack, filesystem, &child_path)
                        .with_context(|| format!("Creating {}", child_path.absolute()))?,
                    Binding::Dynamic(var) => {
                        let top = stack.len();
                        stack.push(Scope::Binding(var, name.into()));
                        traverse_over(child_node, stack, filesystem, &child_path).with_context(
                            || {
                                format!(
                                    "Creating {}, setting {}",
                                    child_path.absolute(),
                                    &stack[top]
                                        .as_binding()
                                        .map(|(var, value)| format!("${} = {}", var, value))
                                        .unwrap_or_else(|| "(no binding on stack)".into()),
                                )
                            },
                        )?;
                        stack.pop();
                    }
                }
            }
        }

        stack.pop();
    }
    Ok(())
}

fn create<FS>(node: &SchemaNode, stack: &[Scope], filesystem: &FS, path: &SplitPath) -> Result<()>
where
    FS: Filesystem,
{
    let target;
    let target_str;
    let to_create = if let Some(expr) = &node.symlink {
        target_str = evaluate(expr, stack, path)?;
        target = SplitPath::new(&target_str)
            .with_context(|| format!("Following symlink {} -> {}", path.absolute(), target_str))?;

        // TODO: Come up with a better way to specify parent structure when following symlinks
        if let Some(parent) = parent(&target.absolute()) {
            if !filesystem.exists(parent) {
                eprintln!(
                    "WARNING: Parent directory for symlink target does not exist, creating: {} \
                    (for {} -> {})",
                    parent,
                    path.absolute(),
                    target.absolute()
                );
                filesystem
                    .create_directory_all(parent)
                    .context("Creating parent directories")?;
            }
        }
        filesystem
            .create_symlink(path.absolute(), target.absolute().to_owned())
            .context("Creating as symlink")?;

        target.absolute()
    } else {
        path.absolute()
    };
    match &node.schema {
        Schema::Directory(_) => {
            if !filesystem.is_directory(to_create) {
                filesystem
                    .create_directory(to_create)
                    .context("Creating as directory")?;
            }
        }
        Schema::File(file) => {
            if !filesystem.is_file(to_create) {
                // FIXME: Copy file, don't create one with the contents of the source
                let source = evaluate(file.source(), stack, path)?;
                let source = filesystem.read_file(&source)?;
                filesystem
                    .create_file(to_create, source)
                    .context("Creating as file")?;
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

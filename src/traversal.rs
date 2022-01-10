use std::borrow::Cow;
use std::fmt::Write;

use anyhow::{anyhow, Context as _, Result};

mod eval;
mod pattern;
mod reuse;

use crate::{
    filesystem::{self, Filesystem, SplitPath},
    schema::{Binding, DirectorySchema, Identifier, Schema, SchemaNode},
    traversal::{eval::evaluate, pattern::CompiledPattern},
};

pub fn traverse<'a, FS>(root: &'a SchemaNode<'_>, filesystem: &FS, target: &str) -> Result<()>
where
    FS: Filesystem,
{
    traverse_over(root, None, filesystem, &SplitPath::new(target)?)
}

#[derive(Debug)]
enum Scope<'a> {
    Directory(&'a DirectorySchema<'a>),
    Binding(&'a Identifier<'a>, String),
}

#[derive(Debug)]
pub struct Stack<'a> {
    parent: Option<&'a Stack<'a>>,
    scope: Scope<'a>,
}

impl<'a> Scope<'a> {
    pub fn as_binding(&self) -> Option<(&Identifier<'a>, &String)> {
        match self {
            Scope::Binding(id, value) => Some((id, value)),
            _ => None,
        }
    }
}

fn traverse_over<'a, FS>(
    node: &'a SchemaNode<'_>,
    stack: Option<&'a Stack<'a>>,
    filesystem: &FS,
    path: &SplitPath,
) -> Result<()>
where
    FS: Filesystem,
{
    println!("Path: {}", path.absolute());
    for node in reuse::expand_uses(node, stack)? {
        traverse_into(node, stack, filesystem, path).with_context(|| {
            format!("Into {}\n{}", path.absolute(), summarize_schema_node(node))
        })?;
    }
    Ok(())
}

fn summarize_schema_node(node: &SchemaNode) -> String {
    let mut f = String::new();
    match &node.schema {
        Schema::Directory(ds) => {
            write!(f, "Schema: directory ({} entries)", ds.entries().len()).unwrap()
        }
        Schema::File(fs) => write!(f, "Schema: file (source: {})", fs.source()).unwrap(),
    }
    if let Some(pattern) = &node.pattern {
        write!(f, "(matching: {})", pattern).unwrap()
    }
    f
}

fn traverse_into<'a, FS>(
    node: &'a SchemaNode<'_>,
    stack: Option<&'a Stack<'a>>,
    filesystem: &FS,
    path: &SplitPath,
) -> Result<()>
where
    FS: Filesystem,
{
    // Create this entry, following symlinks
    create(node, stack, filesystem, &path)
        .with_context(|| format!("Create {}", path.absolute()))?;

    // Traverse over children
    if let Schema::Directory(ref directory) = node.schema {
        let mut listing = filesystem
            .list_directory(path.absolute())
            .unwrap_or_else(|_| vec![])
            .into_iter()
            .map(Some)
            .collect();

        let stack = Stack {
            parent: stack,
            scope: Scope::Directory(directory),
        };

        for (binding, child_node) in directory.entries() {
            // Note: Since we don't know the name of the thing we're matching yet, any path
            // variable (e.g. SAME_PATH_NAME) used in the pattern expression will be evaluated
            // using the parent directory
            let pattern =
                CompiledPattern::compile(child_node.pattern.as_ref(), Some(&stack), &path)?;

            for name in marked_matches(&mut listing, binding, pattern) {
                let child_path = path.join(name.as_ref());

                match binding {
                    Binding::Static(_) => {
                        traverse_over(child_node, Some(&stack), filesystem, &child_path)
                            .with_context(|| format!("Over {}", child_path.absolute()))?
                    }
                    Binding::Dynamic(var) => {
                        let stack = Stack {
                            parent: Some(&stack),
                            scope: Scope::Binding(var, name.into()),
                        };
                        traverse_over(child_node, Some(&stack), filesystem, &child_path)
                            .with_context(|| {
                                format!(
                                    "Over {} (with {})",
                                    child_path.absolute(),
                                    &stack
                                        .scope
                                        .as_binding()
                                        .map(|(var, value)| format!("${} = {}", var, value))
                                        .unwrap_or_else(|| "<no binding>".into()),
                                )
                            })?;
                    }
                }
            }
        }
    }
    Ok(())
}

fn create<FS>(
    node: &SchemaNode,
    stack: Option<&Stack>,
    filesystem: &FS,
    path: &SplitPath,
) -> Result<()>
where
    FS: Filesystem,
{
    // References held to data within by `to_create`, but only in the symlink branch
    let link_str;
    let link_target;

    let to_create;
    if let Some(expr) = &node.symlink {
        link_str = evaluate(expr, stack, path)?;
        link_target = SplitPath::new(&link_str)
            .with_context(|| format!("Following symlink {} -> {}", path.absolute(), link_str))?;

        // TODO: Come up with a better way to specify parent structure when following symlinks
        if let Some(parent) = filesystem::parent(&link_target.absolute()) {
            if !filesystem.exists(parent) {
                eprintln!(
                    "WARNING: Parent directory for symlink target does not exist, creating: {} \
                    (for {} -> {})",
                    parent,
                    path.absolute(),
                    link_target.absolute()
                );
                filesystem
                    .create_directory_all(parent)
                    .context("Creating parent directories")?;
            }
        }
        // Create the symlink pointing to its target before (forming the target itself)
        filesystem
            .create_symlink(path.absolute(), link_target.absolute().to_owned())
            .context("As symlink")?;
        // From here on, use the target path for creation. Further traversal will use the original
        // path, and resolving canonical paths through the symlink
        to_create = link_target.absolute();
    } else {
        to_create = path.absolute();
    }

    match &node.schema {
        Schema::Directory(_) => {
            if !filesystem.is_directory(to_create) {
                filesystem
                    .create_directory(to_create)
                    .context("As directory")?;
            }
        }
        Schema::File(file) => {
            if !filesystem.is_file(to_create) {
                let source = evaluate(file.source(), stack, path)?;
                let content = filesystem.read_file(&source)?;
                filesystem
                    .create_file(to_create, content)
                    .context("As file")?;
            }
        }
    }
    Ok(())
}

fn marked_matches<'a>(
    listing: &mut Vec<Option<String>>,
    binding: &Binding<'a>,
    pattern: CompiledPattern,
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

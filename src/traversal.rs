use std::borrow::Cow;
use std::fmt::Write;

use anyhow::{anyhow, Context as _, Result};

mod eval;
mod pattern;
mod reuse;

use crate::{
    filesystem::{normalize, parent, Filesystem, SplitPath},
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

impl Scope<'_> {
    pub fn as_binding(&self) -> Option<(&Identifier, &String)> {
        match self {
            Scope::Binding(id, value) => Some((id, value)),
            _ => None,
        }
    }
}

fn expand_uses<'a>(
    node: &'a SchemaNode,
    stack: Option<&'a Stack>,
) -> Result<Vec<&'a SchemaNode<'a>>> {
    // Expand `node` to itself and any `#use`s within
    let mut use_schemas = Vec::with_capacity(1 + node.uses.len());
    use_schemas.push(node);
    // Include node itself and its #defs in the scope
    let stack: Option<Stack> = match node {
        SchemaNode {
            schema: Schema::Directory(d),
            ..
        } => Some(Stack {
            parent: stack,
            scope: Scope::Directory(d),
        }),
        _ => None,
    };
    for used in &node.uses {
        use_schemas.push(reuse::find_definition(used, stack.as_ref()).ok_or_else(|| {
            anyhow!(
                "No definition (#def) found for {}. Stack:\n{:#?}",
                used,
                stack
            )
        })?);
    }
    Ok(use_schemas)
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
    for node in expand_uses(node, stack)? {
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
            .context("As symlink")?;

        target.absolute()
    } else {
        path.absolute()
    };
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
                // FIXME: Copy file, don't create one with the contents of the source
                let source = evaluate(file.source(), stack, path)?;
                let source = filesystem.read_file(&source)?;
                filesystem
                    .create_file(to_create, source)
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

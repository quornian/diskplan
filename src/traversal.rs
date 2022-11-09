//! A mechanism for traversing a schema and applying its nodes to an underlying
//! filesystem structure
//!
use std::{borrow::Cow, collections::HashMap, fmt::Write};

use anyhow::{anyhow, Context as _, Result};
use camino::Utf8Path;

use crate::{
    filesystem::{Filesystem, SetAttrs, SplitPath},
    schema::{Binding, DirectorySchema, SchemaNode, SchemaType},
};

use self::{
    eval::evaluate,
    pattern::CompiledPattern,
    stack::{Scope, Stack},
};

mod eval;
mod pattern;
mod stack;

/// Apply a Schema tree to the given filesystem starting at the target path
///
pub fn traverse<'a, FS>(
    root: &'a SchemaNode<'_>,
    filesystem: &mut FS,
    target: impl AsRef<Utf8Path>,
) -> Result<()>
where
    FS: Filesystem,
{
    log::debug!("Traversing root {} for {}", root, target.as_ref());
    traverse_node(root, None, filesystem, &SplitPath::new(target)?)
}

fn traverse_node<'a, FS>(
    node: &'a SchemaNode<'_>,
    stack: Option<&'a Stack<'a>>,
    filesystem: &mut FS,
    path: &SplitPath,
) -> Result<()>
where
    FS: Filesystem,
{
    for node in expand_uses(node, stack)? {
        // Create this entry, following symlinks
        create(node, stack, filesystem, path)
            .with_context(|| format!("Create {}", path.absolute()))?;

        // Traverse over children
        if let SchemaType::Directory(ref directory_schema) = node.schema {
            traverse_directory(directory_schema, stack, filesystem, path).with_context(|| {
                format!(
                    "Directory {}\n{}",
                    path.absolute(),
                    summarize_schema_node(node)
                )
            })?;
        }
    }
    Ok(())
}

fn expand_uses<'a>(
    node: &'a SchemaNode,
    stack: Option<&'a Stack>,
) -> Result<Vec<&'a SchemaNode<'a>>> {
    // Expand `node` to itself and any `:use`s within
    let mut use_schemas = Vec::with_capacity(1 + node.uses.len());
    use_schemas.push(node);
    // Include node itself and its :defs in the scope
    let stack: Option<Stack> = match node {
        SchemaNode {
            schema: SchemaType::Directory(d),
            ..
        } => Some(Stack::new(stack, Scope::Directory(d))),
        _ => None,
    };
    for used in &node.uses {
        use_schemas.push(
            stack::find_definition(used, stack.as_ref())
                .ok_or_else(|| anyhow!("No definition (:def) found for {}", used))?,
        );
    }
    Ok(use_schemas)
}

fn summarize_schema_node(node: &SchemaNode) -> String {
    let mut f = String::new();
    match &node.schema {
        SchemaType::Directory(ds) => {
            write!(f, "Schema: directory ({} entries)", ds.entries().len()).unwrap()
        }
        SchemaType::File(fs) => write!(f, "Schema: file (source: {})", fs.source()).unwrap(),
    }
    if let Some(pattern) = &node.match_pattern {
        write!(f, "(matching: {})", pattern).unwrap()
    }
    if let Some(pattern) = &node.avoid_pattern {
        write!(f, "(avoiding: {})", pattern).unwrap()
    }
    f
}

fn traverse_directory<'a, FS>(
    directory_schema: &'a DirectorySchema<'_>,
    stack: Option<&'a Stack<'a>>,
    filesystem: &mut FS,
    path: &SplitPath,
) -> Result<()>
where
    FS: Filesystem,
{
    let stack = Stack::new(stack, Scope::Directory(directory_schema));

    // Collect names of what's on disk
    let on_disk_filenames = filesystem
        .list_directory(path.absolute())
        .unwrap_or_else(|_| vec![]);
    let on_disk_filenames = on_disk_filenames
        .iter()
        .map(AsRef::as_ref)
        .map(Cow::Borrowed);

    // Collect names of fixed and variable schema entries (fixed are sorted first)
    let bound_child_schemas = directory_schema
        .entries()
        .iter()
        .filter_map(|(binding, _)| match *binding {
            Binding::Static(name) => Some(Cow::Borrowed(name)),
            Binding::Dynamic(var) => evaluate(&var.into(), Some(&stack), path)
                .ok()
                .map(Cow::Owned),
        });

    // Use these to build unique mappings, and error if not unique
    let mut mapped: HashMap<Cow<str>, Option<(&Binding, &SchemaNode)>> = on_disk_filenames
        .chain(bound_child_schemas)
        .map(|name| (name, None))
        .collect();
    for (binding, child_node) in directory_schema.entries() {
        // Note: Since we don't know the name of the thing we're matching yet, any path
        // variable (e.g. SAME_PATH_NAME) used in the pattern expression will be evaluated
        // using the parent directory
        let pattern = CompiledPattern::compile(
            child_node.match_pattern.as_ref(),
            child_node.avoid_pattern.as_ref(),
            Some(&stack),
            path,
        )?;

        for (name, have_match) in mapped.iter_mut() {
            log::debug!("Considering {} (have match: {:?})", name, have_match);
            match binding {
                // Static binding produces a match for that name only
                Binding::Static(bound_name) if bound_name == name => match have_match {
                    None => {
                        *have_match = Some((binding, child_node));
                        Ok(())
                    }
                    Some((bound, _)) => Err(anyhow!(
                        "'{}' matches multiple static bindings '{}' and '{}'",
                        name,
                        bound,
                        binding
                    )),
                },
                // Dynamic bindings must match their inner schema pattern
                Binding::Dynamic(_) if pattern.matches(name) => {
                    match have_match {
                        None => {
                            *have_match = Some((binding, child_node));
                            Ok(())
                        }
                        Some((bound, _)) => match bound {
                            Binding::Static(_) => Ok(()), // Keep previous static binding
                            Binding::Dynamic(_) => Err(anyhow!(
                                "'{}' matches multiple dynamic bindings '{}' and '{}' {:?}",
                                name,
                                bound,
                                binding,
                                pattern,
                            )),
                        },
                    }
                }
                _ => Ok(()),
            }?;
        }
    }

    for (name, matched) in mapped {
        if let Some((binding, child_node)) = matched {
            let child_path = path.join(name.as_ref());

            match binding {
                Binding::Static(s) => {
                    log::debug!(
                        "Directory entry {} -> {} for {}",
                        s,
                        child_node,
                        &child_path
                    );
                    traverse_node(child_node, Some(&stack), filesystem, &child_path)
                        .with_context(|| format!("Node {}", child_path.absolute()))?
                }
                Binding::Dynamic(var) => {
                    log::debug!(
                        "Directory entry '{}' (= '{}') -> {} for '{}'",
                        var,
                        name,
                        child_node,
                        &child_path
                    );
                    let stack = Stack::new(Some(&stack), Scope::Binding(var, name.into()));
                    traverse_node(child_node, Some(&stack), filesystem, &child_path).with_context(
                        || {
                            format!(
                                "Node {} (with {})",
                                child_path.absolute(),
                                &stack
                                    .scope()
                                    .as_binding()
                                    .map(|(var, value)| format!("${} = {}", var, value))
                                    .unwrap_or_else(|| "<no binding>".into()),
                            )
                        },
                    )?;
                }
            }
        }
    }
    Ok(())
}

fn create<FS>(
    node: &SchemaNode,
    stack: Option<&Stack>,
    filesystem: &mut FS,
    path: &SplitPath,
) -> Result<()>
where
    FS: Filesystem,
{
    let owner = match &node.attributes.owner {
        Some(expr) => Some(evaluate(expr, stack, path)?),
        None => None,
    };
    let group = match &node.attributes.group {
        Some(expr) => Some(evaluate(expr, stack, path)?),
        None => None,
    };
    let attrs = SetAttrs {
        owner: owner.as_deref(),
        group: group.as_deref(),
        mode: node.attributes.mode.map(Into::into),
    };

    // References held to data within by `to_create`, but only in the symlink branch
    let link_str;
    let link_target;

    let to_create;
    if let Some(expr) = &node.symlink {
        link_str = evaluate(expr, stack, path)?;
        link_target = SplitPath::new(&link_str)
            .with_context(|| format!("Following symlink {} -> {}", path.absolute(), link_str))?;

        // TODO: Come up with a better way to specify parent structure when following symlinks
        if let Some(parent) = link_target.absolute().parent() {
            if !filesystem.exists(parent) {
                eprintln!(
                    "WARNING: Parent directory for symlink target does not exist, creating: {} \
                    (for {} -> {})",
                    parent,
                    path.absolute(),
                    link_target.absolute()
                );
                filesystem
                    .create_directory_all(parent, attrs.clone())
                    .context("Creating parent directories")?;
            }
        }
        // Create the symlink pointing to its target before (forming the target itself)
        // TODO: Consider if symlinks could be allowed to be relative
        filesystem
            .create_symlink(path.absolute(), link_target.absolute())
            .context("As symlink")?;
        // From here on, use the target path for creation. Further traversal will use the original
        // path, and resolving canonical paths through the symlink
        to_create = link_target.absolute();
    } else {
        to_create = path.absolute();
    }

    match &node.schema {
        SchemaType::Directory(_) => {
            if !filesystem.is_directory(to_create) {
                filesystem
                    .create_directory(to_create, attrs)
                    .context("As directory")?;
            } else {
                let dir_attrs = filesystem.attributes(to_create)?;
                if !attrs.matches(&dir_attrs) {
                    filesystem.set_attributes(to_create, attrs)?;
                }
            }
        }
        SchemaType::File(file) => {
            if !filesystem.is_file(to_create) {
                let source = evaluate(file.source(), stack, path)?;
                let content = filesystem.read_file(&source)?;
                filesystem
                    .create_file(to_create, attrs, content)
                    .context("As file")?;
            }
        }
    }
    Ok(())
}

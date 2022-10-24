use std::{borrow::Cow, collections::HashMap, fmt::Write};

use anyhow::{anyhow, Context as _, Result};

mod eval;
mod pattern;
mod reuse;

use crate::{
    filesystem::{self, Filesystem, SetAttrs, SplitPath},
    schema::{Binding, DirectorySchema, Identifier, Schema, SchemaNode},
    traversal::{eval::evaluate, pattern::CompiledPattern},
};

pub fn traverse<'a, FS>(root: &'a SchemaNode<'_>, filesystem: &mut FS, target: &str) -> Result<()>
where
    FS: Filesystem,
{
    traverse_node(root, None, filesystem, &SplitPath::new(target)?)
}

#[derive(Debug)]
pub enum Scope<'a> {
    Directory(&'a DirectorySchema<'a>),
    Binding(&'a Identifier<'a>, String),
}

#[derive(Debug)]
pub struct Stack<'a> {
    parent: Option<&'a Stack<'a>>,
    scope: Scope<'a>,
}

impl<'a> Stack<'a> {
    pub fn new(parent: Option<&'a Stack<'a>>, scope: Scope<'a>) -> Self {
        Stack { parent, scope }
    }
}

impl<'a> Scope<'a> {
    pub fn as_binding(&self) -> Option<(&Identifier<'a>, &String)> {
        match self {
            Scope::Binding(id, value) => Some((id, value)),
            _ => None,
        }
    }
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
    for node in reuse::expand_uses(node, stack)? {
        // Create this entry, following symlinks
        create(node, stack, filesystem, &path)
            .with_context(|| format!("Create {}", path.absolute()))?;

        // Traverse over children
        if let Schema::Directory(ref directory_schema) = node.schema {
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

fn summarize_schema_node(node: &SchemaNode) -> String {
    let mut f = String::new();
    match &node.schema {
        Schema::Directory(ds) => {
            write!(f, "Schema: directory ({} entries)", ds.entries().len()).unwrap()
        }
        Schema::File(fs) => write!(f, "Schema: file (source: {})", fs.source()).unwrap(),
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
        .filter_map(|(binding, _)| match binding {
            Binding::Static(name) => Some(Cow::Borrowed(*name)),
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
            &path,
        )?;

        for (name, have_match) in mapped.iter_mut() {
            match binding {
                // Static binding produces a match for that name only
                &Binding::Static(bound_name) if bound_name == name => match have_match {
                    None => Ok(*have_match = Some((binding, child_node))),
                    Some((bound, _)) => Err(anyhow!(
                        "'{}' matches multiple static bindings '{}' and '{}'",
                        name,
                        bound,
                        binding
                    )),
                },
                // Dynamic bindings must match their inner schema pattern
                &Binding::Dynamic(_) if pattern.matches(name) => {
                    match have_match {
                        None => Ok(*have_match = Some((binding, child_node))),
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
                    traverse_node(child_node, Some(&stack), filesystem, &child_path)
                        .with_context(|| format!("Node {}", child_path.absolute()))?
                }
                Binding::Dynamic(var) => {
                    let stack = Stack::new(Some(&stack), Scope::Binding(var, name.into()));
                    traverse_node(child_node, Some(&stack), filesystem, &child_path).with_context(
                        || {
                            format!(
                                "Node {} (with {})",
                                child_path.absolute(),
                                &stack
                                    .scope
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
                    .create_directory_all(parent, attrs.clone())
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
                    .create_directory(to_create, attrs)
                    .context("As directory")?;
            } else {
                let dir_attrs = filesystem.attributes(to_create)?;
                if !attrs.matches(&dir_attrs) {
                    filesystem.set_attributes(to_create, attrs)?;
                }
            }
        }
        Schema::File(file) => {
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

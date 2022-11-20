//! A mechanism for traversing a schema and applying its nodes to an underlying
//! filesystem structure
//!
use std::{borrow::Cow, collections::HashMap};

use anyhow::{anyhow, Context as _, Result};
use camino::Utf8Path;

use crate::{
    config::Config,
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

pub fn traverse<'a, 's, 't, FS>(
    path: impl AsRef<Utf8Path>,
    config: &'s Config<'t>,
    stack: Option<&'a Stack<'a>>,
    filesystem: &mut FS,
) -> Result<()>
where
    FS: Filesystem,
    's: 't,
{
    let path = path.as_ref();
    if !path.is_absolute() {
        return Err(anyhow!("Path must be absolute: {}", path));
    }
    let (schema, root) = config
        .rooted_schemas()
        .schema_for(path)?
        .ok_or_else(|| anyhow!("No schema for {}", path))?;
    let start_path = SplitPath::new(root, None)?;
    let remaining_path = path
        .strip_prefix(root.path())
        .expect("Located root must prefix path");
    log::debug!(
        r#"Traversing root directory "{}" ("{}" relative path remains)"#,
        start_path,
        remaining_path,
    );
    traverse_node(
        schema,
        &start_path,
        remaining_path,
        config,
        stack,
        filesystem,
    )?;
    // TODO: Figure out how to detect consumption of remaining_path and what still remains after
    // traversal. Use this to create a better error message about how to extend the schema to cover
    // these cases. Or failing that, make continued directory creation allowable.
    if !filesystem.exists(path) {
        return Err(if let Some(stack) = stack {
            anyhow!(
                "{} rooted at \"{}\" failed to produce target path \"{}\" with stack: {}",
                schema,
                root.path(),
                path,
                stack,
            )
        } else {
            anyhow!(
                r#"{} rooted at "{}" failed to produce target path "{}" with empty stack"#,
                schema,
                root.path(),
                path,
            )
        });
    }
    Ok(())
}

fn traverse_node<'a, 's, 't, FS>(
    schema: &SchemaNode<'_>,
    path: &SplitPath,
    remaining_path: &Utf8Path,
    config: &'s Config<'t>,
    stack: Option<&'a Stack<'a>>,
    filesystem: &mut FS,
) -> Result<()>
where
    FS: Filesystem,
    's: 't,
{
    for schema in expand_uses(schema, stack)? {
        log::debug!("Applying: {}", schema);
        // Create this entry, following symlinks
        create(schema, path, config, stack, filesystem)
            .with_context(|| format!("Failed while trying to create {}", &path))?;

        // Traverse over children
        if let SchemaType::Directory(ref directory_schema) = schema.schema {
            traverse_directory(
                directory_schema,
                path,
                remaining_path,
                config,
                stack,
                filesystem,
            )
            .with_context(|| {
                format!(
                    "Failed while trying to apply directory schema to {}: {}",
                    path, schema
                )
            })?;
        }
    }
    Ok(())
}

fn traverse_directory<'a, 's, 't, FS>(
    directory_schema: &DirectorySchema<'_>,
    directory_path: &SplitPath,
    remaining_path: &Utf8Path,
    config: &'s Config<'t>,
    stack: Option<&'a Stack<'a>>,
    filesystem: &mut FS,
) -> Result<()>
where
    FS: Filesystem,
    's: 't,
{
    let stack = Stack::new(stack, Scope::Directory(directory_schema));

    // Pull the front off the relative remaining_path
    let next_remaining = remaining_path
        .as_str()
        .split_once('/')
        .map(|(name, _remaining)| Some(name))
        .unwrap_or(if remaining_path == "" {
            None
        } else {
            Some(remaining_path.as_str())
        });

    // Collect a set of names of
    //  - what's on disk
    //  - the next component of our intended path (next_remaining)
    //  - any static bindings
    //  - any variable bindings for which we have a value from the stack
    let mut names: HashMap<Cow<str>, Option<_>> = HashMap::new();
    let from_key = |key| (key, None);
    names.extend(
        filesystem
            .list_directory(directory_path.absolute())
            .unwrap_or_default()
            .into_iter()
            .map(Cow::Owned)
            .map(from_key),
    );
    names.extend(next_remaining.map(Cow::Borrowed).map(from_key));
    names.extend(
        directory_schema
            .entries()
            .iter()
            .filter_map(|(binding, _)| match *binding {
                Binding::Static(name) => Some(Cow::Borrowed(name)),
                Binding::Dynamic(var) => evaluate(&var.into(), Some(&stack), directory_path)
                    .ok() // Ignore errors here (assume we don't have the variable in scope)
                    .map(Cow::Owned),
            })
            .map(from_key),
    );

    // Use these to build unique mappings, and error if not unique
    for (binding, child_node) in directory_schema.entries() {
        // Note: Since we don't know the name of the thing we're matching yet, any path
        // variable (e.g. SAME_PATH_NAME) used in the pattern expression will be evaluated
        // using the parent directory
        let pattern = CompiledPattern::compile(
            child_node.match_pattern.as_ref(),
            child_node.avoid_pattern.as_ref(),
            Some(&stack),
            directory_path,
        )?;

        for (name, have_match) in names.iter_mut() {
            match binding {
                // Static binding produces a match for that name only
                Binding::Static(bound_name) if bound_name == name => match have_match {
                    None => {
                        *have_match = Some((binding, child_node));
                        Ok(())
                    }
                    Some((bound, _)) => Err(anyhow!(
                        r#""{}" matches multiple static bindings "{}" and "{}""#,
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
                                r#""{}" matches multiple dynamic bindings "{}" and "{}" {:?}"#,
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
            match have_match {
                None => log::trace!(r#"Considered name "{}" but no match found"#, name),
                Some((Binding::Static(_), _)) => {
                    log::trace!(r#"Considered name "{}" and found exact static match"#, name)
                }
                Some((Binding::Dynamic(id), _)) => log::trace!(
                    r#"Considered name "{}", matched {:?}, binding to variable ${{{}}}"#,
                    name,
                    pattern,
                    id.value()
                ),
            }
        }
    }

    for (name, matched) in names {
        let Some((binding, child_schema)) = matched else { continue };
        let name = name.as_ref();
        let child_path = directory_path.join(name);

        // If this name is part of the target path, strip it off the front for further traversal.
        // If it is not, we're no longer completing the target path in this branch
        let child_remaining = remaining_path
            .strip_prefix(name)
            .unwrap_or_else(|_err| Utf8Path::new(""));

        match binding {
            Binding::Static(s) => {
                log::debug!(
                    r#"Traversing static directory entry "{}" at {} ("{}" relative path remains)"#,
                    s,
                    &child_path,
                    child_remaining,
                );
                traverse_node(
                    child_schema,
                    &child_path,
                    child_remaining,
                    config,
                    Some(&stack),
                    filesystem,
                )
                .with_context(|| format!("Failed while trying to process path {}", &child_path))?
            }
            Binding::Dynamic(var) => {
                log::debug!(
                    r#"Traversing variable directory entry ${}="{}" at {} ("{}" relative path remains)"#,
                    var,
                    name,
                    &child_path,
                    child_remaining,
                );
                let stack = Stack::new(Some(&stack), Scope::Binding(var, name.into()));
                traverse_node(
                    child_schema,
                    &child_path,
                    child_remaining,
                    config,
                    Some(&stack),
                    filesystem,
                )
                .with_context(|| {
                    format!(
                        r#"Failed while trying to process path {} (with {})"#,
                        &child_path,
                        &stack
                            .scope()
                            .as_binding()
                            .map(|(var, value)| format!("${} = {}", var, value))
                            .unwrap_or_else(|| "<no binding>".into()),
                    )
                })?;
            }
        }
    }
    Ok(())
}

fn create<'a, 's, 't, FS>(
    schema: &SchemaNode,
    path: &SplitPath,
    config: &'s Config<'t>,
    stack: Option<&'a Stack<'a>>,
    filesystem: &mut FS,
) -> Result<()>
where
    FS: Filesystem,
    's: 't,
{
    let evaluated_owner;
    let owner = match &schema.attributes.owner {
        Some(expr) => {
            evaluated_owner = evaluate(expr, stack, path)?;
            Some(config.map_user(&evaluated_owner))
        }
        None => None,
    };
    let evaluated_group;
    let group = match &schema.attributes.group {
        Some(expr) => {
            evaluated_group = evaluate(expr, stack, path)?;
            Some(config.map_group(&evaluated_group))
        }
        None => None,
    };
    let attrs = SetAttrs {
        owner,
        group,
        mode: schema.attributes.mode.map(Into::into),
    };

    // References held to data within by `to_create`, but only in the symlink branch
    let link_str;
    let link_path;
    let link_target;

    let to_create;
    if let Some(expr) = &schema.symlink {
        link_str = evaluate(expr, stack, path)?;
        link_path = Utf8Path::new(&link_str);
        log::info!("Creating {} -> {}", path, link_path);

        // TODO: Support relative pathed symlinks
        if !link_path.is_absolute() {
            return Err(anyhow!("Relative paths in symlinks are not yet supported"));
        }

        // TODO: Maybe we just need to get the root here
        let rooted = config.rooted_schemas().schema_for(link_path)?;
        let (link_schema, link_root) = rooted.ok_or_else(|| {
            anyhow!(
                "No schema found for symlink target {} -> {}",
                path,
                link_path
            )
        })?;
        link_target = SplitPath::new(link_root, Some(link_path))
            .with_context(|| format!("Following symlink {} -> {}", path, link_path))?;

        // TODO: Think about which schema wins? Target root, or local. Or if this is a link to the local one anyway?!
        if !filesystem.exists(link_target.absolute()) {
            traverse(link_target.absolute(), config, stack, filesystem)?;
            assert!(filesystem.exists(link_target.absolute()));
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
        log::info!("Creating {}", path);
        to_create = path.absolute();
    }

    match &schema.schema {
        SchemaType::Directory(_) => {
            if !filesystem.is_directory(to_create) {
                log::debug!("Make directory: {}", to_create);
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

fn expand_uses<'a, 't>(
    node: &'a SchemaNode<'t>,
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

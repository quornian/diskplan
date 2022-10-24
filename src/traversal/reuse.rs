use anyhow::{anyhow, Result};

use super::{Scope, Stack};
use crate::schema::{Identifier, Schema, SchemaNode};

pub fn expand_uses<'a>(
    node: &'a SchemaNode,
    stack: Option<&'a Stack>,
) -> Result<Vec<&'a SchemaNode<'a>>> {
    // Expand `node` to itself and any `:use`s within
    let mut use_schemas = Vec::with_capacity(1 + node.uses.len());
    use_schemas.push(node);
    // Include node itself and its :defs in the scope
    let stack: Option<Stack> = match node {
        SchemaNode {
            schema: Schema::Directory(d),
            ..
        } => Some(Stack::new(stack, Scope::Directory(d))),
        _ => None,
    };
    for used in &node.uses {
        use_schemas.push(
            find_definition(used, stack.as_ref())
                .ok_or_else(|| anyhow!("No definition (:def) found for {}", used))?,
        );
    }
    Ok(use_schemas)
}

fn find_definition<'a>(
    var: &Identifier<'a>,
    stack: Option<&Stack<'a>>,
) -> Option<&'a SchemaNode<'a>> {
    if let Some(Stack { parent, scope }) = stack {
        match scope {
            Scope::Directory(directory) => directory.get_def(var),
            _ => None,
        }
        .or_else(|| find_definition(var, *parent))
    } else {
        None
    }
}

use crate::schema::{Identifier, SchemaNode};

use super::{Scope, Stack};

pub fn find_definition<'a>(
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

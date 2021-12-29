use std::borrow::Cow;

use anyhow::Result;

mod eval;
mod pattern;

use crate::{
    schema::{Binding, DirectorySchema, Identifier, Schema, SchemaNode},
    traverse::{eval::evaluate, pattern::CompiledPattern},
};

pub fn traverse<'a>(root: &'a SchemaNode<'_>) -> Result<()> {
    let mut stack = Vec::new();
    traverse_over(&root, &mut stack)
}

enum Scope<'a> {
    Directory(&'a DirectorySchema<'a>),
    Binding(&'a Identifier<'a>, String),
}

// TODO: Error handling: include stack in result and format as traceback
fn traverse_over<'a>(node: &'a SchemaNode<'_>, stack: &mut Vec<Scope<'a>>) -> Result<()> {
    match node.schema {
        Schema::Directory(ref directory) => {
            // FIXME: Pull from FS
            let mut listing = "existing1\nexisting2"
                .lines()
                .into_iter()
                .map(String::from)
                .map(Option::Some)
                .collect();

            stack.push(Scope::Directory(directory));

            for (binding, child_node) in directory.entries() {
                let pattern = CompiledPattern::compile(child_node.pattern.as_ref(), stack)?;
                for name in marked_matches(&mut listing, binding, pattern) {
                    println!(
                        "{0:1$}Create {2} (bound to: {3:?})",
                        "",
                        stack.len(),
                        name,
                        binding
                    );
                    match binding {
                        Binding::Static(_) => traverse_over(child_node, stack)?,
                        Binding::Dynamic(var) => {
                            stack.push(Scope::Binding(var, name.into()));
                            traverse_over(child_node, stack)?;
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

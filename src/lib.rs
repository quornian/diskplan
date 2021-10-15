use std::{
    convert::TryFrom,
    path::{Path, PathBuf},
};

use crate::expr::{Expr, Token};

pub mod expr;
pub mod item;
pub mod meta;
pub mod schema;

pub fn read_schema(path: &Path) -> Result<item::Item, item::ItemError> {
    item::Item::from_path(path)
}

pub fn apply_tree(
    root: &PathBuf,
    name: &str,
    item: &item::Item,
    context: &expr::Context,
) -> Result<(), expr::EvaluationError> {
    // Each item may be named with a @variable or text (but only one; not @var_text)
    let expr = Expr::try_from(name)?;
    let token = {
        let mut tokens = expr.tokens().iter();
        let token = tokens
            .next()
            .ok_or_else(|| expr::EvaluationError::NameHasNoTokens(name.to_owned()));
        tokens.next().map_or(Ok(()), |extra| {
            Err(expr::EvaluationError::NameHasMultipleTokens(
                name.to_owned(),
                format!("{:?}", extra),
            ))
        })?;
        token
    };
    // TODO: Use this Token (WIP)

    let name = context.evaluate(&Expr::try_from(name)?)?;
    let mut install_args = vec!["install".to_owned()];
    if let Some(owner) = item.meta().owner() {
        install_args.push(format!("--owner={}", owner));
    }
    if let Some(group) = item.meta().group() {
        install_args.push(format!("--group={}", group));
    }
    if let Some(perms) = item.meta().permissions() {
        install_args.push(format!("--mode={:o}", perms.mode()));
    }
    let action = match item.itemtype() {
        item::ItemType::Directory => {
            let mut path = root.to_owned();
            path.push(name);
            install_args.push("--directory".to_owned());
            install_args.push(String::from(path.to_string_lossy()));
            println!("Run: {:?}", install_args);

            // TODO: Use stack with injected var binding from token/name
            // let child_context = ...
            // for (name, child) in item.children.iter() {
            //     apply_tree(&path, &name, child, stack)?;
            // }
        }
        _ => eprintln!("NOT IMPLEMENTED FOR {:?}", item.itemtype()),
    };
    Ok(())
}

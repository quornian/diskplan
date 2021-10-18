pub mod context;
pub mod eval;
pub mod parse;

// pub fn apply_tree(context: &context::Context) -> Result<(), eval::EvaluationError> {
//     // Each item may be named with a @variable or text (but only one; not @var_text)
//     let expr = Expr::try_from(name)?;
//     let token = {
//         let mut tokens = expr.tokens().iter();
//         let token = tokens
//             .next()
//             .ok_or_else(|| eval::EvaluationError::NameHasNoTokens(name.to_owned()));
//         tokens.next().map_or(Ok(()), |extra| {
//             Err(eval::EvaluationError::NameHasMultipleTokens(
//                 name.to_owned(),
//                 format!("{:?}", extra),
//             ))
//         })?;
//         token
//     };
//     // TODO: Use this Token (WIP)

//     let name = context.evaluate(&Expr::try_from(name)?)?;
//     let mut install_args = vec!["install".to_owned()];
//     if let Some(owner) = item.meta().owner() {
//         install_args.push(format!("--owner={}", owner));
//     }
//     if let Some(group) = item.meta().group() {
//         install_args.push(format!("--group={}", group));
//     }
//     if let Some(perms) = item.meta().permissions() {
//         install_args.push(format!("--mode={:o}", perms.mode()));
//     }
//     let action = match item.itemtype() {
//         Entry::Directory => {
//             let mut path = root.to_owned();
//             path.push(name);
//             install_args.push("--directory".to_owned());
//             install_args.push(String::from(path.to_string_lossy()));
//             println!("Run: {:?}", install_args);

//             // TODO: Use stack with injected var binding from token/name
//             // let child_context = context.child(&name, vars);
//             // for (name, child) in item.children.iter() {
//             //     apply_tree(&path, &name, child, &child_context)?;
//             // }
//         }
//         _ => eprintln!("NOT IMPLEMENTED FOR {:?}", item.itemtype()),
//     };
//     Ok(())
// }

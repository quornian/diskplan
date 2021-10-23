use std::path::Path;

use anyhow::Result;
use clap::{App, Arg};
use diskplan::{
    apply::{gather_actions, Action},
    install,
    schema::expr::{Expression, Token},
};

fn main() -> Result<()> {
    // Parse command line arguments
    let matches = App::new("diskplan")
        .version("1.0")
        .about("Describe and apply filesystem structure")
        .set_term_width(76)
        .arg(
            Arg::with_name("schema")
                .help("The path of the schema to apply")
                .takes_value(true)
                .required(true),
        )
        .arg(
            Arg::with_name("target")
                .help("The root directory on which to apply the schema")
                .takes_value(true)
                .required(true),
        )
        .arg(
            Arg::with_name("let")
                .long("--let")
                .number_of_values(2)
                .value_names(&["variable", "expr"])
                .multiple(true)
                .next_line_help(true)
                .help(
                    "Sets a variable to the given value or expression. \
                     Expressions will be evaluated just-in-time and may \
                     refer to other variables higher in the tree. \
                     This option may be used more than once.",
                ),
        )
        .get_matches();

    let schema = matches.value_of("schema").unwrap();
    let target = matches.value_of("target").unwrap();

    let schema = diskplan::fromfile::schema_from_path(Path::new(schema))?;
    let mut context = diskplan::context::Context::new(&schema, &Path::new(target));

    if let Some(keyvalues) = matches.values_of("let") {
        let keys = keyvalues.clone().into_iter().step_by(2);
        let values = keyvalues.into_iter().skip(1).step_by(2);
        for (key, value) in keys.zip(values) {
            println!("{} = {}", key, value);
            //FIXME: Parse this!
            assert!(!value.contains("$"));
            let expr = Expression::new(vec![Token::text(value)]);
            context.bind(key, expr);
        }
    }

    println!("{:#?}", schema);

    //print_tree(&schema);

    println!("before");
    let actions = gather_actions(&context)?;
    println!("after");

    for action in actions {
        println!("Performing Action: {:?}", action);
        continue;
        match action {
            Action::CreateDirectory { path, meta } => install::install_directory(&path, &meta)?,
            Action::CreateFile { path, source, meta } => {
                install::install_file(&path, &source, &meta)?
            }
            Action::CreateSymlink { path, target } => install::install_link(&path, &target)?,
        }
    }
    Ok(())
}

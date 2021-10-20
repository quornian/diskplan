use std::path::Path;

use anyhow::Result;
use clap::{App, Arg};

use diskplan::{application::apply_tree, definition::schema::print_tree};

fn main() -> Result<()> {
    // Parse command line arguments
    let matches = App::new("diskplan")
        .version("1.0")
        .about("Describe and apply filesystem structure")
        .set_term_width(76)
        .arg(
            Arg::with_name("schema")
                .help("The node schema file to load for testing")
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

    let schema = diskplan::definition::fromdisk::schema_from_path(Path::new(schema))?;
    let mut context = diskplan::application::context::Context::new(&schema, &Path::new(target));

    if let Some(keyvalues) = matches.values_of("let") {
        let keys = keyvalues.clone().into_iter().step_by(2);
        let values = keyvalues.into_iter().skip(1).step_by(2);
        for (key, value) in keys.zip(values) {
            println!("{} = {}", key, value);
            context.bind(key, value);
        }
    }

    println!("{:#?}", schema);
    print_tree(&schema);
    apply_tree(&context)?;
    Ok(())
}

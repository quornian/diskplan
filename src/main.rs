use std::path::Path;

use anyhow::Result;
use clap::{App, Arg};

use diskplan::{application::apply_tree, definition::schema::print_tree};

fn main() -> Result<()> {
    // Parse command line arguments
    let matches = App::new("diskplan")
        .version("1.0")
        .about("Describe and apply filesystem structure")
        .arg(
            Arg::with_name("schema")
                .help("The node schema file to load for testing")
                .takes_value(true), // .required(true),
        )
        .arg(
            Arg::with_name("target")
                .help("The root directory on which to apply the schema")
                .takes_value(true), // .required(true),
        )
        .get_matches();

    let schema = matches.value_of("schema").unwrap_or("mockups/root_5");
    let target = matches.value_of("target").unwrap_or("/tmp/root");

    let schema = diskplan::definition::fromdisk::schema_from_path(Path::new(schema))?;
    let context = diskplan::application::context::Context::new(&schema, &Path::new(target));

    // print_tree(&schema);
    apply_tree(&context)?;
    Ok(())
}

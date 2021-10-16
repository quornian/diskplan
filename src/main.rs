use std::{env::current_dir, path::Path};

use anyhow::{Context, Result};
use clap::{App, Arg};

use diskplan::definition::schema::print_tree;

fn main() -> Result<()> {
    // Parse command line arguments
    let matches = App::new("diskplan")
        .version("1.0")
        .about("Describe and apply filesystem structure")
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
        .get_matches();

    let schema = matches.value_of("schema").expect("<schema> required");
    let target = matches.value_of("target").expect("<target> required");

    let schema = diskplan::definition::fromdisk::item_from_path(Path::new(schema))?;
    // let target = diskplan::context::Context::new(Path::new(target));

    print_tree(&schema);
    // apply_tree(&current_dir()?, ".", &schema, &target)?;
    Ok(())
}

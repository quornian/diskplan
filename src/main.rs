use std::{env::current_dir, path::Path};

use anyhow::{Context, Result};
use clap::{App, Arg};

use diskplan::item::{apply_tree, print_item_tree, Item};

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
        .get_matches();

    let schema_root = matches.value_of("schema").expect("<schema> required");

    // Note: PathBuf::from_str ->
    let node = Item::from_path(Path::new(&schema_root).to_owned())?;
    // print!("{:#?}", node);
    print_item_tree(&node);
    apply_tree(&current_dir()?, ".", &node, &[])?;
    Ok(())
}

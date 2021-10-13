use std::fs::{self, File};
use std::io::Read;
use std::path::Path;

use clap::{App, Arg};

use anyhow::{Context, Result};

use diskplan::meta::{ItemMeta, Permissions};

pub fn load_meta(path: &str) -> Result<ItemMeta, anyhow::Error> {
    fn read_file(path: &str) -> Result<String> {
        let mut file = File::open(path)?;
        let mut data = String::new();
        file.read_to_string(&mut data)?;
        Ok(data)
    }
    Ok(if Path::exists(Path::new(&path)) {
        let meta = read_file(path).context(format!("Failed to read: {}", path))?;
        ItemMeta::from_str(&meta).context(format!("Failed to parse: {}", path))?
    } else {
        ItemMeta::default()
    })
}

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

    for entry in fs::read_dir(schema_root)? {
        let entry = entry?;
        // let path = entry.path();
        let metadata = entry.metadata()?;
        let filename = entry.file_name();
        let filename = filename.to_str().expect("Invalid filename!");
        let filetype = metadata.file_type();

        if filename.ends_with(".meta") {
            continue;
        }

        let meta_path = format!("{}.meta", entry.path().to_str().expect("Invalid path!"));
        let item_meta = load_meta(&meta_path)?;
        eprintln!("Handling: {}:\n  {:?}", filename, item_meta);

        if filetype.is_file() {
        } else if filetype.is_dir() {
        } else if filetype.is_symlink() {
        } else {
            eprintln!("Skipping invalid filetype: {}", filename);
        }
    }
    Ok(())
}

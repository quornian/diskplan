use std::{fs::File, io::Read};

use anyhow::{anyhow, Context as _, Result};
use clap::{App, Arg};

use diskplan::{
    filesystem::{self, Filesystem, SetAttrs},
    schema::parse_schema,
    traversal::traverse,
};

fn main() -> Result<()> {
    // Parse command line arguments
    let matches = App::new("diskplan")
        .version("1.0")
        .about("Describe and apply filesystem structure")
        .set_term_width(72)
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
        .get_matches();

    let schema = matches.value_of("schema").unwrap();
    let target = matches.value_of("target").unwrap();

    let content = std::fs::read_to_string(schema)
        .with_context(|| format!("Failed to load schema from: {}", schema))?;

    let schema_root = parse_schema(&content)
        .map_err(|e| anyhow!("{}", e))
        .with_context(|| format!("Failed to load schema from: {}", schema))?;

    let mut fs = filesystem::MemoryFilesystem::new();

    // if let Some(keyvalues) = matches.values_of("let") {
    //     let keys = keyvalues.clone().into_iter().step_by(2);
    //     let values = keyvalues.into_iter().skip(1).step_by(2);
    //     for (key, value) in keys.zip(values) {
    //         println!("{} = {}", key, value);
    //         //FIXME: Parse this! and Identifier
    //         assert!(!key.contains("$"));
    //         assert!(!value.contains("$"));
    //         let key = Identifier::new(key);
    //         context.bind(key, value.into());
    //     }
    // }
    // let context = context;

    traverse(&schema_root, &mut fs, target)?;

    print_tree("/", &fs, 0)?;

    Ok(())
}

fn print_tree<FS>(path: &str, fs: &FS, depth: usize) -> Result<()>
where
    FS: filesystem::Filesystem,
{
    let (_, name) = filesystem::split(path).ok_or_else(|| anyhow!("No parent: {}", path))?;
    let dir = fs.is_directory(path);
    let attrs = fs.attributes(path)?;
    print_perms(dir, attrs.mode);
    print!(
        " {owner:10} {group:10} {0:indent$}{name}{symbol}",
        "",
        owner = attrs.owner,
        group = attrs.group,
        indent = depth * 2,
        name = name,
        symbol = if dir { "/" } else { "" }
    );
    if let Ok(target) = fs.read_link(path) {
        println!(" -> {}", target);
    } else {
        println!();

        if fs.is_directory(path) {
            for child in fs.list_directory(path)? {
                let child = filesystem::join(path, &child);
                print_tree(&child, fs, depth + 1)?;
            }
        }
    }
    Ok(())
}

fn print_perms(is_dir: bool, mode: u16) {
    print!(
        "{}{}{}{}{}{}{}{}{}{}",
        if is_dir { 'd' } else { '-' },
        if mode & (1 << 8) != 0 { 'r' } else { '-' },
        if mode & (1 << 7) != 0 { 'w' } else { '-' },
        if mode & (1 << 11) != 0 {
            's'
        } else if mode & (1 << 6) != 0 {
            'x'
        } else {
            '-'
        },
        if mode & (1 << 5) != 0 { 'r' } else { '-' },
        if mode & (1 << 4) != 0 { 'w' } else { '-' },
        if mode & (1 << 10) != 0 {
            's'
        } else if mode & (1 << 3) != 0 {
            'x'
        } else {
            '-'
        },
        if mode & (1 << 2) != 0 { 'r' } else { '-' },
        if mode & (1 << 1) != 0 { 'w' } else { '-' },
        if mode & (1 << 9) != 0 {
            't'
        } else if mode & (1 << 0) != 0 {
            'x'
        } else {
            '-'
        },
    );
}

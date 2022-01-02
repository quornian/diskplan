use anyhow::{anyhow, Context as _, Result};
use clap::{App, Arg};
use diskplan::{
    filesystem,
    schema::{parse_schema, Identifier},
    traversal::traverse,
};
use std::{fs::File, io::Read, path::Path};

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
        .arg(
            Arg::with_name("apply")
                .long("--apply")
                .help("Apply the changes")
                .takes_value(false),
        )
        .get_matches();

    let schema = matches.value_of("schema").unwrap();
    let target = matches.value_of("target").unwrap();
    let apply = matches.is_present("apply");

    let content = (|| -> Result<String> {
        let mut file = File::open(schema)?;
        let mut content = String::with_capacity(file.metadata()?.len() as usize);
        file.read_to_string(&mut content)?;
        Ok(content)
    })()
    .with_context(|| format!("Failed to load schema from: {}", schema))?;

    let schema_root = parse_schema(&content)
        .map_err(|e| anyhow!("{}", e))
        .with_context(|| format!("Failed to load schema from: {}", schema))?;

    let fs = filesystem::MemoryFilesystem::new();

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

    traverse(&schema_root, &fs, target)?;

    print_tree("/", &fs, 0)?;

    Ok(())
}

fn print_tree<FS>(path: &str, fs: &FS, depth: usize) -> Result<()>
where
    FS: filesystem::Filesystem,
{
    let (_, name) = filesystem::split(path).ok_or_else(|| anyhow!("No parent: {}", path))?;
    let dir = fs.is_directory(path);
    println!("{0:1$}{2}{3}", "", depth, name, if dir { "/" } else { "" });
    if fs.is_directory(path) {
        for child in fs.list_directory(path)? {
            let child = filesystem::join(path, &child);
            print_tree(&child, fs, depth + 1)?;
        }
    }
    Ok(())
}

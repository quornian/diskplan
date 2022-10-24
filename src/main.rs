use anyhow::{anyhow, Context as _, Result};
use clap::{arg, command, Parser};

use diskplan::{
    filesystem::{self, Filesystem},
    schema::parse_schema,
    traversal::traverse,
};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// The path of the schema to apply
    schema: String,
    /// The root directory on which to apply the schema
    target: String,

    /// Whether to apply the changes (otherwise they are simulated in memory)
    #[arg(long)]
    apply: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let schema = &args.schema;
    let target = &args.target;
    let apply = args.apply;

    let content = std::fs::read_to_string(schema)
        .with_context(|| format!("Failed to load schema from: {}", schema))?;

    let schema_root = parse_schema(&content)
        .map_err(|e| anyhow!("{}", e))
        .with_context(|| format!("Failed to load schema from: {}", schema))?;

    if apply {
        let mut fs = filesystem::DiskFilesystem::new();
        traverse(&schema_root, &mut fs, target)?;
    } else {
        let mut fs = filesystem::MemoryFilesystem::new();
        fs.create_directory_all(target, Default::default())?;
        traverse(&schema_root, &mut fs, target)?;
        print_tree("/", &fs, 0)?;
    }
    Ok(())
}

fn print_tree<FS>(path: &str, fs: &FS, depth: usize) -> Result<()>
where
    FS: filesystem::Filesystem,
{
    let (_, name) = filesystem::split(path).ok_or_else(|| anyhow!("No parent: {}", path))?;
    let dir = fs.is_directory(path);
    let attrs = fs.attributes(path)?;
    print_perms(dir, attrs.mode.value());
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

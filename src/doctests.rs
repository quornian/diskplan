// Note: Included in lib.rs doctests via `include!()` macro
use anyhow::Result;
use regex::Regex;

use diskplan::{
    config::Config,
    filesystem::{Filesystem, MemoryFilesystem, SetAttrs},
    schema::{Root, SchemaNode},
    traversal::traverse,
};

pub fn verify_trees<'s, 't>(
    config: &'s Config<'t>,
    in_tree: &str,
    out_tree: &str,
    target: &str,
) -> Result<()>
where
    's: 't,
{
    let env = env_logger::Env::new().filter("DISKPLAN_LOG");
    env_logger::Builder::from_env(env)
        .filter_level(log::LevelFilter::Debug)
        .format_timestamp(None)
        .init();

    let mut fs = MemoryFilesystem::new();

    // Create initial filesystem from input tree
    for entry in parse_tree(in_tree)? {
        if let Some(target) = entry.link_target {
            fs.create_symlink(&entry.path, target)?;
        } else if entry.is_dir {
            if !fs.exists(&entry.path) {
                fs.create_directory(&entry.path, SetAttrs::default())?;
            }
        } else {
            fs.create_file(&entry.path, SetAttrs::default(), "".to_owned())?;
        }
    }

    // Apply schema
    traverse(target, &config, None, &mut fs)?;

    // Check tree matches expected output tree
    for entry in parse_tree(out_tree)? {
        if let Some(target) = entry.link_target {
            assert_eq!(fs.read_link(&entry.path)?, target.to_owned());
            assert_eq!((&target, fs.exists(&target)), (&target, true));
        } else if entry.is_dir {
            assert_eq!(
                (&entry.path, fs.is_directory(&entry.path)),
                (&entry.path, true)
            );
        } else {
            assert_eq!((&entry.path, fs.is_file(&entry.path)), (&entry.path, true));
        }
    }
    Ok(())
}

struct Entry {
    path: String,
    is_dir: bool,
    link_target: Option<String>,
}

fn parse_tree(tree: &str) -> Result<Vec<Entry>> {
    let line_regex = Regex::new("^([│└├─ ]*)([^/ ]*)(/?)( -> .*)?$")?;
    let mut parts: Vec<_> = (0..10).map(|_| String::new()).collect();
    let mut entries = vec![];

    for line in tree.lines() {
        if let Some(captures) = line_regex.captures(line) {
            let (indent, name, dir, link) =
                (&captures[1], &captures[2], &captures[3], captures.get(4));
            if name == "" && dir == "" {
                continue;
            }

            let level = indent.chars().count() / 4;
            parts[level] = name.to_owned();
            let path = parts[..level + 1].join("/");
            let path = if &path == "" { "/".to_owned() } else { path };
            let (is_dir, link_target) = match (dir, link) {
                (_, Some(link)) => {
                    let target = link.as_str().strip_prefix(" -> ").unwrap();
                    (false, Some(target.to_owned()))
                }
                ("/", None) => (true, None),
                (_, None) => (false, None),
            };
            entries.push(Entry {
                path,
                is_dir,
                link_target,
            });
        } else {
            panic!("Bad line: {}", line);
        }
    }
    Ok(entries)
}

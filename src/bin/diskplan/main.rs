#![doc = include_str!("../../../README.md")]

use anyhow::{anyhow, Result};
use camino::Utf8Path;
use clap::Parser;
use tracing::{span, Level};

mod args;
use args::CommandLineArgs;
use diskplan_config::Config;
use diskplan_filesystem::{self as filesystem, Filesystem};
use diskplan_traversal::{self as traversal, StackFrame, VariableSource};

fn init_logger(verbosity: u8) {
    let sub = tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_file(false)
        .with_line_number(false);
    let (level, pretty) = match verbosity {
        0 => (Level::WARN, false),
        1 => (Level::INFO, false),
        2 => (Level::INFO, true),
        3 => (Level::DEBUG, true),
        _ => (Level::TRACE, true),
    };
    let sub = sub.with_max_level(level);
    if pretty {
        sub.pretty().init();
    } else {
        sub.init();
    }
}

fn main() -> Result<()> {
    let CommandLineArgs {
        target,
        config_file,
        apply,
        verbose,
        usermap,
        groupmap,
        vars,
    } = CommandLineArgs::parse();

    init_logger(verbose);
    let span = span!(Level::DEBUG, "main", target = target.as_str());
    let _guard = span.enter();

    let mut config = Config::new(target, apply);
    config.load(config_file)?;

    if let Some(usermap) = usermap {
        config.apply_user_map(usermap.into())
    }
    if let Some(groupmap) = groupmap {
        config.apply_group_map(groupmap.into())
    }

    let owner = users::get_current_username().unwrap();
    let owner = owner.to_string_lossy();
    let owner = config.map_user(&owner);
    let group = users::get_current_groupname().unwrap();
    let group = group.to_string_lossy();
    let group = config.map_group(&group);
    let mode = 0o755.into();
    let variables = vars
        .map(|vars| VariableSource::Map(vars.into()))
        .unwrap_or_default();
    let stack = StackFrame::stack(&config, variables, owner, group, mode);

    if config.will_apply() {
        let mut fs = filesystem::DiskFilesystem::new();
        traversal::traverse(config.target_path(), &stack, &mut fs)?;
    } else {
        tracing::warn!("Simulating in memory only, use --apply to apply to disk");
        let mut fs = filesystem::MemoryFilesystem::new();
        for root in config.stem_roots() {
            fs.create_directory_all(root.path(), Default::default())?;
        }
        fs.create_directory("/dev", Default::default())?;
        fs.create_file("/dev/null", Default::default(), "".to_owned())?;
        traversal::traverse(config.target_path(), &stack, &mut fs)?;
        tracing::warn!("Displaying in-memory filesystem...");
        for root in config.stem_roots() {
            println!("\n[Root: {}]", root.path());
            print_tree(root.path(), &fs, 0)?;
        }
    }
    Ok(())
}

fn print_tree<FS>(path: impl AsRef<Utf8Path>, fs: &FS, depth: usize) -> Result<()>
where
    FS: filesystem::Filesystem,
{
    let path = path.as_ref();
    let name = path
        .file_name()
        .ok_or_else(|| anyhow!("No file name: {}", path))?;
    let dir = fs.is_directory(path);
    let attrs = fs.attributes(path)?;
    print_perms(dir, attrs.mode.value());
    print!(
        " {owner:10} {group:10} {0:indent$}{name}{symbol}",
        "",
        owner = attrs.owner,
        group = attrs.group,
        indent = depth * 2,
        name = if depth == 0 { path.as_str() } else { name },
        symbol = if dir { "/" } else { "" }
    );
    if let Ok(target) = fs.read_link(path) {
        println!(" -> {target}");
    } else {
        println!();

        if fs.is_directory(path) {
            for child in {
                let mut list = fs.list_directory(path)?;
                list.sort();
                list
            } {
                let child = path.join(&child);
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

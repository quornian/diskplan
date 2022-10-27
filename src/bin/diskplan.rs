use std::path::PathBuf;

use anyhow::{anyhow, Context as _, Result};
use clap::{arg, command, Parser};

use diskplan::{
    filesystem::{self, Filesystem},
    schema::parse_schema,
    traversal::traverse,
};

use crate::config::Config;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// The root directory on which to apply the schema
    target: PathBuf,

    /// The profile to apply
    profile: Option<String>,

    /// The path to the diskplan.toml config file
    #[arg(short, long, default_value = "diskplan.toml")]
    config_file: PathBuf,

    /// Whether to apply the changes (otherwise they are simulated in memory)
    #[arg(long)]
    apply: bool,

    /// Increase verbosity level
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,
}

mod config {
    use std::{collections::HashMap, fmt::Debug, path::Path};

    use anyhow::{anyhow, Context as _, Result};
    use serde::Deserialize;

    #[derive(Deserialize)]
    pub struct Config {
        profiles: HashMap<String, Profile>,
    }

    #[derive(Deserialize)]
    pub struct Profile {
        root: String,
        schema: String,
    }

    impl Config {
        pub fn load<P>(path: P) -> Result<Config>
        where
            P: AsRef<Path> + Debug,
        {
            let config_context = || format!("Reading config file {:?}", path);
            let config = std::fs::read_to_string(&path).with_context(config_context)?;
            toml::from_str(&config).with_context(config_context)
        }

        pub fn get_profile(&self, name: &str) -> Option<&Profile> {
            self.profiles.get(name)
        }

        pub fn profile_for_path(&self, path: &Path) -> Result<&Profile> {
            let matched: Vec<_> = self
                .profiles
                .iter()
                .filter(|(_, profile)| path.starts_with(&profile.root))
                .collect();
            match &matched[..] {
                [(_, profile)] => Ok(&profile),
                [] => Err(anyhow!("No profile has root matching path {:?}", path)),
                _ => Err(anyhow!("Multiple profile roots match path {:?}", path)),
            }
        }
    }

    impl Profile {
        pub fn schema(&self) -> &str {
            &self.schema
        }

        pub fn root(&self) -> &str {
            &self.root
        }
    }
}

fn init_logger(args: &Args) {
    let env = env_logger::Env::new().filter("DISKPLAN_LOG");
    env_logger::Builder::from_env(env)
        .filter_level(match args.verbose {
            0 => log::LevelFilter::Warn,
            1 => log::LevelFilter::Info,
            2 => log::LevelFilter::Debug,
            _ => log::LevelFilter::Trace,
        })
        .format_timestamp(None)
        .init();
}

fn main() -> Result<()> {
    let args = Args::parse();
    init_logger(&args);
    let config = Config::load(&args.config_file)?;

    let profile = match &args.profile {
        Some(profile) => config
            .get_profile(profile)
            .ok_or_else(|| anyhow!("No profile has name {:?}", profile)),
        None => config.profile_for_path(&args.target),
    }
    .with_context(|| anyhow!("Reading config {:?}", args.config_file))?;

    let target = &args.target.to_string_lossy();
    let apply = args.apply;
    let schema = profile.schema();

    log::debug!("Schema: {}", schema);
    log::debug!("Target: {}", target);
    log::debug!("Apply: {}", apply);

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

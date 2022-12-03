use anyhow::Result;
use camino::Utf8PathBuf;
use clap::Parser;

use super::NameMap;

/// Command line arguments
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct CommandLineArgs {
    /// The directory to produce. This must be absolute and begin with one of the configured roots
    pub target: Utf8PathBuf,

    /// The path to the diskplan.toml config file
    #[arg(short, long, default_value = "diskplan.toml")]
    pub config_file: Utf8PathBuf,

    /// Whether to apply the changes (otherwise, only simulate and print)
    #[arg(long)]
    pub apply: bool,

    /// Increase logging verbosity level (0: warn; 1: info; 2: debug; 3: trace)
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// Map user names, for example "root:admin,janine:jfu"
    #[arg(long, value_parser = parse_name_map)]
    pub usermap: Option<NameMap>,

    /// Map groups names
    #[arg(long, value_parser = parse_name_map)]
    pub groupmap: Option<NameMap>,

    /// Set variables that may be used by the schema "variable:value,variable2:value2,..."
    #[arg(long, value_parser = parse_name_map)]
    pub vars: Option<NameMap>,
}

fn parse_name_map(value: &str) -> Result<NameMap> {
    NameMap::try_from(value)
}

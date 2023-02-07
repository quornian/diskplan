use std::{collections::HashMap, ops::Deref};

use anyhow::{anyhow, bail, Result};
use camino::Utf8PathBuf;
use clap::Parser;

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

/// A string-to-string mapping of names to new names that can be parsed
/// from string form `"name1:newname1,name2:newname2"` and used as a lookup
#[derive(Debug, Default, Clone)]
pub struct NameMap(HashMap<String, String>);

impl NameMap {
    /// Returns the mapped name, or the original if no mapping exists
    pub fn map<'a>(&'a self, name: &'a str) -> &'a str {
        self.0.get(name).map(|s| s.deref()).unwrap_or(name)
    }
}

impl TryFrom<&str> for NameMap {
    type Error = anyhow::Error;

    fn try_from(line: &str) -> Result<Self, Self::Error> {
        let mut map = HashMap::new();
        for pair in line.split(',') {
            let mut kv_iter = pair.split(':');
            let key = kv_iter.next().unwrap();
            let value = kv_iter
                .next()
                .ok_or_else(|| anyhow!("Expected ':' separated key value pair"))?;
            if key.is_empty() || value.is_empty() {
                bail!("Key and value must be non-empty");
            }
            if let Some(unexpected) = kv_iter.next() {
                bail!("Unexpected third value \"{}\"", unexpected);
            }
            map.insert(key.to_owned(), value.to_owned());
        }
        Ok(NameMap(map))
    }
}

impl From<NameMap> for HashMap<String, String> {
    fn from(name_map: NameMap) -> Self {
        name_map.0
    }
}

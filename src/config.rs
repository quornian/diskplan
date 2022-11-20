//! Configuration for the system
//!
use std::{collections::HashMap, fmt::Debug, ops::Deref};

use anyhow::{anyhow, Context as _, Result};
use camino::{Utf8Path, Utf8PathBuf};
use clap::Parser;
use serde::Deserialize;

use crate::schema::{Root, RootedSchemas};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// The directory to produce. This must be absolute and begin with one of the configured roots
    pub target: Utf8PathBuf,

    /// The path to the diskplan.toml config file
    #[arg(short, long, default_value = "diskplan.toml")]
    config_file: Utf8PathBuf,

    /// Whether to apply the changes (otherwise, only simulate and print)
    #[arg(long)]
    pub apply: bool,

    /// Increase verbosity level (0: warn; 1: info; 2: debug; 3: trace)
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,

    #[arg(long, value_parser = parse_name_map)]
    usermap: Option<NameMap>,

    #[arg(long, value_parser = parse_name_map)]
    groupmap: Option<NameMap>,
}

fn parse_name_map(value: &str) -> Result<NameMap> {
    NameMap::try_from(value)
}

/// Application configuration
pub struct Config<'t> {
    rooted_schemas: RootedSchemas<'t>,

    user_map: Option<NameMap>,
    group_map: Option<NameMap>,
}

#[derive(Deserialize, Default, Debug, Clone, PartialEq, Eq)]
struct ConfigData {
    /// A map of unique profile names to their individual configurations
    profiles: HashMap<String, ProfileData>,

    /// Schema directory (defaults to directory containing config)
    schema_directory: Option<Utf8PathBuf>,
}

/// Configuration for a single profile
#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
struct ProfileData {
    /// The absolute root directory on which to apply changes
    root: Root,
    /// The path to a schema definition file that describes how files and directories under the
    /// root should be structured (may be absolute or relative to the config file's directory)
    schema: Utf8PathBuf,
}

impl<'t> Config<'t> {
    /// Load a configuration from the specified file
    pub fn from_args(args: &Args) -> Result<Self> {
        let path = &args.config_file;
        let config_context = || format!("Reading config file {:?}", path);
        let config_data = std::fs::read_to_string(path.as_path()).with_context(config_context)?;
        let ConfigData {
            schema_directory,
            profiles,
        } = toml::from_str(&config_data).with_context(config_context)?;

        let schema_directory = schema_directory
            .as_deref()
            .unwrap_or_else(|| path.parent().unwrap_or_else(|| Utf8Path::new(".")));
        let mut rooted_schemas = RootedSchemas::new();
        for (_, profile) in profiles.into_iter() {
            rooted_schemas.add(profile.root, schema_directory.join(profile.schema));
        }

        Ok(Config {
            rooted_schemas,
            user_map: args.usermap.clone(),
            group_map: args.groupmap.clone(),
        })
    }

    /// Access the set of roots and schemas defined by this config
    pub fn rooted_schemas(&self) -> &RootedSchemas<'t> {
        &self.rooted_schemas
    }

    pub fn map_user<'a>(&'a self, name: &'a str) -> &'a str {
        match &self.user_map {
            Some(map) => map.map(name),
            None => name,
        }
    }

    pub fn map_group<'a>(&'a self, name: &'a str) -> &'a str {
        match &self.group_map {
            Some(map) => map.map(name),
            None => name,
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct NameMap(HashMap<String, String>);

impl NameMap {
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
                return Err(anyhow!("Key and value must be non-empty"));
            }
            if let Some(unexpected) = kv_iter.next() {
                return Err(anyhow!("Unexpected third value \"{}\"", unexpected));
            }
            map.insert(key.to_owned(), value.to_owned());
        }
        Ok(NameMap(map))
    }
}

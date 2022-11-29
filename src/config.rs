//! Configuration for the system
//!
use std::{collections::HashMap, fmt::Debug, ops::Deref};

use anyhow::{anyhow, bail, Context as _, Result};
use camino::{Utf8Path, Utf8PathBuf};
use clap::Parser;
use serde::Deserialize;

use crate::schema::{Root, SchemaCache, SchemaNode};

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

    #[arg(long, value_parser = parse_name_map)]
    vars: Option<NameMap>,
}

fn parse_name_map(value: &str) -> Result<NameMap> {
    NameMap::try_from(value)
}

/// Application configuration
#[derive(Default)]
pub struct Config<'t> {
    stems: Stems<'t>,

    user_map: Option<NameMap>,
    group_map: Option<NameMap>,
    vars: Option<NameMap>,
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
    pub fn new() -> Self {
        Default::default()
    }

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
        let mut stems = Stems::new();
        for (_, profile) in profiles.into_iter() {
            stems.add(profile.root, schema_directory.join(profile.schema));
        }

        Ok(Config {
            stems,
            user_map: args.usermap.clone(),
            group_map: args.groupmap.clone(),
            vars: args.vars.clone(),
        })
    }

    pub fn add_stem(&mut self, root: Root, schema_path: impl AsRef<Utf8Path>) {
        self.stems.add(root, schema_path)
    }

    pub fn add_precached_stem(
        &mut self,
        root: Root,
        schema_path: impl AsRef<Utf8Path>,
        schema: SchemaNode<'t>,
    ) {
        self.stems.add_precached(root, schema_path, schema)
    }

    pub fn stem_roots(&self) -> impl Iterator<Item = &Root> {
        self.stems.roots()
    }

    pub fn schema_for<'s, 'p>(
        &'s self,
        path: &'p Utf8Path,
    ) -> Result<Option<(&SchemaNode<'t>, &Root)>>
    where
        's: 't,
    {
        self.stems.schema_for(path)
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

    pub fn vars(&self) -> Option<&NameMap> {
        self.vars.as_ref()
    }
}

#[derive(Debug, Default, Clone)]
pub struct NameMap(HashMap<String, String>);

impl NameMap {
    pub fn map<'a>(&'a self, name: &'a str) -> &'a str {
        self.0.get(name).map(|s| s.deref()).unwrap_or(name)
    }
}

impl From<NameMap> for HashMap<String, String> {
    fn from(map: NameMap) -> Self {
        map.0
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

#[derive(Default)]
struct Stems<'t> {
    /// Maps root path to the schema definition's file path
    path_map: HashMap<Root, Utf8PathBuf>,

    /// A cache of loaded schemas from their definition files
    cache: SchemaCache<'t>,
}

impl<'t> Stems<'t> {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn add(&mut self, root: Root, schema_path: impl AsRef<Utf8Path>) {
        self.path_map.insert(root, schema_path.as_ref().to_owned());
    }

    pub fn add_precached(
        &mut self,
        root: Root,
        schema_path: impl AsRef<Utf8Path>,
        schema: SchemaNode<'t>,
    ) {
        let schema_path = schema_path.as_ref();
        self.cache.inject(schema_path, schema);
        self.add(root, schema_path);
    }

    pub fn roots(&self) -> impl Iterator<Item = &Root> {
        self.path_map.keys()
    }

    pub fn schema_for<'s, 'p>(
        &'s self,
        path: &'p Utf8Path,
    ) -> Result<Option<(&SchemaNode<'t>, &Root)>>
    where
        's: 't,
    {
        let mut longest_candidate = None;
        for (root, schema_path) in self.path_map.iter() {
            if path.starts_with(root.path()) {
                match longest_candidate {
                    None => longest_candidate = Some((root, schema_path)),
                    Some(prev) => {
                        if root.path().as_str().len() > prev.0.path().as_str().len() {
                            longest_candidate = Some((root, schema_path))
                        }
                    }
                }
            }
        }

        Ok(if let Some((root, schema_path)) = longest_candidate {
            log::trace!(
                r#"Schema for path "{}", found root "{}", schema "{}""#,
                path,
                root.path(),
                schema_path
            );
            let schema = self.cache.load(schema_path).with_context(|| {
                format!(
                    "Failed to load schema {} for configured root {} (for target path {})",
                    schema_path,
                    root.path(),
                    path
                )
            })?;
            return Ok(Some((schema, root)));
        } else {
            None
        })
    }
}

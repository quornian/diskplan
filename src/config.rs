//! Configuration for the system
//!
use std::{collections::HashMap, fmt::Debug};

use anyhow::{Context as _, Result};
use camino::{Utf8Path, Utf8PathBuf};
use serde::Deserialize;

use crate::schema::{Root, RootedSchemas};

pub struct Config<'t> {
    rooted_schemas: RootedSchemas<'t>,
}

/// Application configuration
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
    pub fn load<P>(path: P) -> Result<Self>
    where
        P: AsRef<Utf8Path> + Debug,
    {
        let config_context = || format!("Reading config file {:?}", path);
        let config_data = std::fs::read_to_string(path.as_ref()).with_context(config_context)?;
        let ConfigData {
            schema_directory,
            profiles,
        } = toml::from_str(&config_data).with_context(config_context)?;

        let schema_directory = schema_directory
            .as_deref()
            .unwrap_or_else(|| path.as_ref().parent().unwrap_or_else(|| Utf8Path::new(".")));
        let mut rooted_schemas = RootedSchemas::new();
        for (_, profile) in profiles.into_iter() {
            rooted_schemas.add(profile.root, schema_directory.join(profile.schema));
        }
        Ok(Config { rooted_schemas })
    }

    /// Access the set of roots and schemas defined by this config
    pub fn rooted_schemas(&self) -> &RootedSchemas<'t> {
        &self.rooted_schemas
    }
}

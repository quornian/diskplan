use std::collections::HashMap;

use anyhow::{Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use serde::Deserialize;

use crate::Root;

/// Deserialization of diskplan.toml
#[derive(Deserialize, Default, Debug, Clone, PartialEq, Eq)]
pub struct ConfigFile {
    /// A map of unique names to individual stem configurations
    pub stems: HashMap<String, ConfigStem>,

    /// Schema directory (defaults to directory containing config)
    pub schema_directory: Option<Utf8PathBuf>,
}

#[derive(Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
#[serde(try_from = "Utf8PathBuf")]
struct _Root(Root);

impl TryFrom<Utf8PathBuf> for _Root {
    type Error = <Root as TryFrom<Utf8PathBuf>>::Error;
    fn try_from(value: Utf8PathBuf) -> std::result::Result<Self, Self::Error> {
        Root::try_from(value).map(_Root)
    }
}

/// Configuration for a single stem within diskplan.toml
#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ConfigStem {
    root: _Root,
    schema: Utf8PathBuf,
}

impl ConfigStem {
    /// The absolute root directory on which to apply changes
    pub fn root(&self) -> &Root {
        &self.root.0
    }

    /// The path to a schema definition file that describes how files and directories under the
    /// root should be structured (may be absolute or relative to the config file's directory)
    pub fn schema(&self) -> &Utf8Path {
        &self.schema
    }
}

impl ConfigFile {
    /// Load a configuration from the specified file
    ///
    pub fn load(path: impl AsRef<Utf8Path>) -> Result<Self> {
        let path = path.as_ref();
        let config_context = || format!("Reading config file {path:?}");
        let config_data = std::fs::read_to_string(path).with_context(config_context)?;
        config_data.as_str().try_into()
    }
}

impl TryFrom<&str> for ConfigFile {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Ok(toml::from_str(value)?)
    }
}

//! Configuration for the system
//!
use std::{collections::HashMap, fmt::Debug};

use anyhow::{anyhow, Context as _, Result};
use camino::{Utf8Path, Utf8PathBuf};
use serde::Deserialize;

use crate::schema::Root;

/// Application configuration
#[derive(Deserialize, Default, Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// A map of unique profile names to their individual configurations
    profiles: HashMap<String, Profile>,
}

/// Configuration for a single profile
#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    /// The absolute root directory on which to apply changes
    root: Root,
    /// The path to a schema definition file that describes how files and directories under the
    /// root should be structured
    schema: Utf8PathBuf,
}

impl Config {
    /// Load a configuration from the specified file
    pub fn load<P>(path: P) -> Result<Config>
    where
        P: AsRef<Utf8Path> + Debug,
    {
        let config_context = || format!("Reading config file {:?}", path);
        let config = std::fs::read_to_string(path.as_ref()).with_context(config_context)?;
        toml::from_str(&config).with_context(config_context)
    }

    /// Return the [`Profile`] with the given name if one exists
    pub fn get_profile(&self, name: &str) -> Option<&Profile> {
        self.profiles.get(name)
    }

    /// Return the [`Profile`] whose root contains the given path, if one exists
    pub fn profile_for_path(&self, path: &Utf8Path) -> Result<&Profile> {
        let matched: Vec<_> = self
            .profiles
            .iter()
            .filter(|(_, profile)| path.starts_with(profile.root.path()))
            .collect();
        match &matched[..] {
            [(_, profile)] => Ok(profile),
            [] => Err(anyhow!("No profile has root matching path {:?}", path)),
            _ => Err(anyhow!("Multiple profile roots match path {:?}", path)),
        }
    }
}

impl Profile {
    /// The path to a schema definition file that describes how files and directories under the
    /// root should be structured
    pub fn schema(&self) -> &Utf8Path {
        &self.schema
    }

    /// The absolute root directory on which to apply changes
    pub fn root(&self) -> &Root {
        &self.root
    }
}

#[cfg(test)]
mod tests {
    use toml::from_str;

    use super::Config;

    #[test]
    fn root_absolute() {
        let config: Result<Config, _> =
            from_str("[profiles.one]\nschema = \"\"\nroot = \"/absolute/path\"\n");
        assert!(config
            .unwrap()
            .get_profile("one")
            .unwrap()
            .root()
            .path()
            .is_absolute())
    }

    #[test]
    fn root_relative_disallowed() {
        let config: Result<Config, _> =
            from_str("[profiles.one]\nschema = \"\"\nroot = \"relative/path\"\n");
        assert!(config.is_err());
        assert!(config
            .unwrap_err()
            .to_string()
            .contains("path must be absolute"));
    }
}

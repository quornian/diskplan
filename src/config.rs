//! Configuration for the system
//!
use std::{collections::HashMap, fmt::Debug};

use anyhow::{anyhow, Context as _, Result};
use camino::{Utf8Path, Utf8PathBuf};
use serde::Deserialize;

use crate::schema::SchemaNode;

/// Application configuration
#[derive(Deserialize, Default, Debug, Clone, PartialEq)]
pub struct Config {
    /// A map of unique profile names to their individual configurations
    profiles: HashMap<String, Profile>,
}

/// Configuration for a single profile
#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct Profile {
    /// The absolute root directory on which to apply changes
    root: Root,
    /// The path to a schema definition file that describes how files and directories under the
    /// root should be structured
    schema: Utf8PathBuf,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(try_from = "Utf8PathBuf")]
pub struct Root(Utf8PathBuf);

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
            .filter(|(_, profile)| path.starts_with(&profile.root.0))
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

impl Root {
    /// The absolute path of this root
    pub fn path(&self) -> &Utf8Path {
        &self.0
    }
}

impl TryFrom<Utf8PathBuf> for Root {
    type Error = String;

    fn try_from(value: Utf8PathBuf) -> Result<Self, Self::Error> {
        if value.is_absolute() {
            Ok(Root(value))
        } else {
            Err(format!("Invalid root; path must be absolute: {}", value))
        }
    }
}

#[derive(Default)]
pub struct SchemaCache<'a> {
    texts: elsa::FrozenVec<String>,
    schemas: elsa::FrozenVec<Box<SchemaNode<'a>>>,
}

impl<'a> SchemaCache<'a> {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn load<'s>(&'s self, path: impl AsRef<Utf8Path>) -> Result<&'s SchemaNode<'a>>
    where
        's: 'a,
    {
        let text = std::fs::read_to_string(path.as_ref())?;
        let text = self.texts.push_get(text);
        let schema = crate::schema::parse_schema(text)
            // ParseError lifetime is tricky, flattern
            .map_err(|e| anyhow!("{}", e))?;
        Ok(self.schemas.push_get(Box::new(schema)))
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

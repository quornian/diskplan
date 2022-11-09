use std::{collections::HashMap, fmt::Debug};

use anyhow::{anyhow, Context as _, Result};
use camino::{Utf8Path, Utf8PathBuf};
use serde::Deserialize;

#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct Config {
    profiles: HashMap<String, Profile>,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct Profile {
    root: Root,
    schema: Utf8PathBuf,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(try_from = "Utf8PathBuf")]
pub struct Root(Utf8PathBuf);

impl Config {
    pub fn load<P>(path: P) -> Result<Config>
    where
        P: AsRef<Utf8Path> + Debug,
    {
        let config_context = || format!("Reading config file {:?}", path);
        let config = std::fs::read_to_string(path.as_ref()).with_context(config_context)?;
        toml::from_str(&config).with_context(config_context)
    }

    pub fn get_profile(&self, name: &str) -> Option<&Profile> {
        self.profiles.get(name)
    }

    pub fn profile_for_path(&self, path: &Utf8Path) -> Result<&Profile> {
        let matched: Vec<_> = self
            .profiles
            .iter()
            .filter(|(_, profile)| path.starts_with(&profile.root.0))
            .collect();
        match &matched[..] {
            [(_, profile)] => Ok(&profile),
            [] => Err(anyhow!("No profile has root matching path {:?}", path)),
            _ => Err(anyhow!("Multiple profile roots match path {:?}", path)),
        }
    }
}

impl Profile {
    pub fn schema(&self) -> &Utf8Path {
        &self.schema
    }

    pub fn root(&self) -> &Root {
        &self.root
    }
}

impl Root {
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

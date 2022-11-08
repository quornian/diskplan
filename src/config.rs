use std::{collections::HashMap, fmt::Debug};

use anyhow::{anyhow, Context as _, Result};
use camino::Utf8Path;
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

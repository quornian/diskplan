use anyhow::{anyhow, Result};
use camino::{Utf8Path, Utf8PathBuf};
use serde::Deserialize;

use crate::filesystem::is_normalized;

#[derive(Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
#[serde(try_from = "Utf8PathBuf")]
pub struct Root(Utf8PathBuf);

impl Root {
    pub fn new(path: impl AsRef<Utf8Path>) -> Result<Self> {
        path.as_ref().to_owned().try_into()
    }
    /// The absolute path of this root
    pub fn path(&self) -> &Utf8Path {
        &self.0
    }
}

impl AsRef<Utf8Path> for Root {
    fn as_ref(&self) -> &Utf8Path {
        &self.0
    }
}

impl TryFrom<Utf8PathBuf> for Root {
    type Error = anyhow::Error;

    fn try_from(value: Utf8PathBuf) -> Result<Self, Self::Error> {
        if !is_normalized(value.as_str()) {
            return Err(anyhow!("Root must be a normalized path: {}", value));
        }
        if !value.is_absolute() {
            return Err(anyhow!("Invalid root; path must be absolute: {}", value));
        }
        Ok(Root(value))
    }
}

impl TryFrom<&Utf8Path> for Root {
    type Error = anyhow::Error;

    fn try_from(value: &Utf8Path) -> Result<Self, Self::Error> {
        value.to_owned().try_into()
    }
}

impl TryFrom<&str> for Root {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Utf8PathBuf::from(value).try_into()
    }
}

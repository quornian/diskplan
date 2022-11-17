use std::collections::HashMap;

use anyhow::{anyhow, Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use serde::Deserialize;

use crate::{filesystem::is_normalized, schema::SchemaNode};

use super::SchemaCache;

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

pub trait SchemaFor<'p> {
    fn schema_for(
        &self,
        path: impl AsRef<Utf8Path> + 'p,
    ) -> Result<Option<(&SchemaNode<'_>, &'p Utf8Path)>>;
}

#[derive(Default)]
pub struct RootedSchemas<'t> {
    /// Maps root path to the schema definition's file path
    rooted: HashMap<Root, Utf8PathBuf>,

    /// A cache of loaded schemas from their definition files
    cache: SchemaCache<'t>,
}

impl<'t> RootedSchemas<'t> {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn add(&mut self, root: Root, schema_path: impl AsRef<Utf8Path>) {
        self.rooted.insert(root, schema_path.as_ref().to_owned());
    }

    pub fn roots(&self) -> impl Iterator<Item = &Root> {
        self.rooted.keys()
    }

    pub fn schema_for<'s, 'p>(
        &'s self,
        path: &'p Utf8Path,
    ) -> Result<Option<(&SchemaNode<'t>, &Root, &'p Utf8Path)>>
    where
        's: 't,
    {
        for (root, schema_path) in self.rooted.iter() {
            if let Ok(remainder) = path.strip_prefix(root.path()) {
                let schema = self.cache.load(schema_path).with_context(|| {
                    format!(
                        "Failed to load schema for configured root {} (for target path {})",
                        root.path(),
                        path
                    )
                })?;
                return Ok(Some((schema, root, remainder)));
            }
        }
        Ok(None)
    }

    #[cfg(test)]
    pub(crate) fn inject_for_testing(&self, path: impl AsRef<Utf8Path>, schema: SchemaNode<'t>) {
        self.cache.inject_for_testing(path, schema)
    }
}

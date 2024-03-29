use std::{collections::HashMap, sync::Mutex};

use anyhow::{anyhow, Context, Result};
use camino::{Utf8Path, Utf8PathBuf};

use crate::SchemaNode;

/// An append-only cache of schemas ([`SchemaNode`] roots) keyed by their on-disk file path
#[derive(Default)]
pub struct SchemaCache<'a> {
    mapped: Mutex<HashMap<Utf8PathBuf, usize>>,
    texts: elsa::FrozenVec<String>,
    schemas: elsa::FrozenVec<Box<SchemaNode<'a>>>,
}

impl<'a> SchemaCache<'a> {
    /// Constructs an new cache
    pub fn new() -> Self {
        Default::default()
    }

    /// Parses the file at the given `path`, caches the parsed schema, and returns a reference to it
    pub fn load<'s, 'r>(&'s self, path: impl AsRef<Utf8Path>) -> Result<&'r SchemaNode<'a>>
    where
        's: 'a,
    {
        let mut locked = self.mapped.lock().expect("Lock poisoned");

        // Early return for cache hit
        if let Some(index) = locked.get(path.as_ref()) {
            return Ok(&self.schemas[*index]);
        }

        // Cache miss; load text from file and parse it
        let text = self.texts.push_get(
            std::fs::read_to_string(path.as_ref())
                .with_context(|| format!("Failed to load config from: {}", path.as_ref()))?,
        );
        let schema = diskplan_schema::parse_schema(text)
            // ParseError lifetime is tricky, flattern
            .map_err(|e| anyhow!("{}", e))?;
        locked.insert(path.as_ref().to_owned(), self.schemas.len());
        Ok(self.schemas.push_get(Box::new(schema)))
    }

    /// Injects a path to schema mapping into the cache without loading from disk
    ///
    /// This is primarily used for tests
    pub fn inject(&self, path: impl AsRef<Utf8Path>, schema: SchemaNode<'a>) {
        let mut locked = self.mapped.lock().expect("Lock poisoned");
        locked.insert(path.as_ref().to_owned(), self.schemas.len());
        self.schemas.push(Box::new(schema));
    }
}

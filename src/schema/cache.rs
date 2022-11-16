use std::{collections::HashMap, sync::Mutex};

use anyhow::{anyhow, Result};
use camino::{Utf8Path, Utf8PathBuf};

use crate::schema::SchemaNode;

#[derive(Default)]
pub struct SchemaCache<'a> {
    mapped: Mutex<HashMap<Utf8PathBuf, usize>>,
    texts: elsa::FrozenVec<String>,
    schemas: elsa::FrozenVec<Box<SchemaNode<'a>>>,
}

impl<'a> SchemaCache<'a> {
    pub fn new() -> Self {
        Default::default()
    }

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
        let text = self.texts.push_get(std::fs::read_to_string(path.as_ref())?);
        let schema = crate::schema::parse_schema(text)
            // ParseError lifetime is tricky, flattern
            .map_err(|e| anyhow!("{}", e))?;
        locked.insert(path.as_ref().to_owned(), self.schemas.len());
        Ok(self.schemas.push_get(Box::new(schema)))
    }

    #[cfg(test)]
    pub(crate) fn inject_for_testing(&self, path: impl AsRef<Utf8Path>, schema: SchemaNode<'a>) {
        let mut locked = self.mapped.lock().expect("Lock poisoned");
        locked.insert(path.as_ref().to_owned(), self.schemas.len());
        self.schemas.push(Box::new(schema));
    }
}

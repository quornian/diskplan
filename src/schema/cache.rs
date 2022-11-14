use std::{collections::HashMap, sync::Mutex};

use anyhow::{anyhow, Result};

use crate::{config::Root, schema::SchemaNode};

#[derive(Default)]
pub struct SchemaCache<'a> {
    mapped: Mutex<HashMap<Root, usize>>,
    texts: elsa::FrozenVec<String>,
    schemas: elsa::FrozenVec<Box<SchemaNode<'a>>>,
}

impl<'a> SchemaCache<'a> {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn load<'s>(&'s self, path: &Root) -> Result<&'s SchemaNode<'a>>
    where
        's: 'a,
    {
        let mut locked = self.mapped.lock().expect("Lock poisoned");

        // Early return for cache hit
        if let Some(index) = locked.get(path) {
            return Ok(&self.schemas[*index]);
        }

        // Cache miss; load text from file and parse it
        let text = self.texts.push_get(std::fs::read_to_string(path.path())?);
        let schema = crate::schema::parse_schema(text)
            // ParseError lifetime is tricky, flattern
            .map_err(|e| anyhow!("{}", e))?;
        locked.insert(path.clone(), self.schemas.len());
        Ok(self.schemas.push_get(Box::new(schema)))
    }
}

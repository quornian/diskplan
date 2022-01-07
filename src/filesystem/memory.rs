use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
};

use anyhow::{anyhow, Context, Result};

use super::Filesystem;

/// An in-memory representation of a file system
#[derive(Debug)]
pub struct MemoryFilesystem {
    inner: RefCell<Inner>,
}

#[derive(Debug)]
struct Inner {
    map: HashMap<String, Node>,
}

#[derive(Debug)]
enum Node {
    File { content: String },
    Directory { children: Vec<String> },
    Symlink { target: String },
}

impl MemoryFilesystem {
    pub fn new() -> Self {
        let mut map = HashMap::new();
        map.insert("/".into(), Node::Directory { children: vec![] });

        MemoryFilesystem {
            inner: RefCell::new(Inner { map }),
        }
    }

    pub fn to_path_set<'a>(&'a self) -> HashSet<String> {
        self.inner.borrow().map.keys().cloned().collect()
    }

    fn canonical_split<'s>(&self, path: &'s str) -> Result<(String, &'s str)> {
        match super::split(path) {
            None => Err(anyhow!("Cannot create {}", path)),
            Some((parent, name)) => Ok((self.canonicalize(parent)?, name)),
        }
    }
}

impl Filesystem for MemoryFilesystem {
    fn create_directory(&self, path: &str) -> Result<()> {
        let (parent, name) = self
            .canonical_split(path)
            .with_context(|| format!("Splitting {}", path))?;
        let mut inner = self.inner.borrow_mut();
        inner
            .insert_node(&parent, name, Node::Directory { children: vec![] })
            .with_context(|| format!("Creating directory: {}", path))
    }

    fn create_file(&self, path: &str, content: String) -> Result<()> {
        let (parent, name) = self.canonical_split(path)?;
        let mut inner = self.inner.borrow_mut();
        inner
            .insert_node(&parent, name, Node::File { content })
            .with_context(|| format!("Creating file: {}", path))
    }

    fn create_symlink(&self, path: &str, target: String) -> Result<()> {
        let (parent, name) = self.canonical_split(path)?;
        let mut inner = self.inner.borrow_mut();
        inner
            .insert_node(&parent, name, Node::Symlink { target })
            .with_context(|| format!("Creating symlink: {}", path))
    }

    fn exists(&self, path: &str) -> bool {
        match self.canonicalize(path) {
            Ok(path) => self.inner.borrow().map.contains_key(&path),
            _ => false,
        }
    }

    fn is_directory(&self, path: &str) -> bool {
        match self.canonicalize(path) {
            Err(_) => false,
            Ok(path) => match self.inner.borrow().map.get(&path) {
                Some(Node::Directory { .. }) => true,
                _ => false,
            },
        }
    }

    fn is_file(&self, path: &str) -> bool {
        match self.canonicalize(path) {
            Err(_) => false,
            Ok(path) => match self.inner.borrow().map.get(&path) {
                Some(Node::File { .. }) => true,
                _ => false,
            },
        }
    }

    fn is_link(&self, path: &str) -> bool {
        match self.inner.borrow().map.get(path) {
            Some(Node::Symlink { .. }) => true,
            _ => false,
        }
    }

    fn list_directory(&self, path: &str) -> Result<Vec<String>> {
        let path = self.canonicalize(path)?;
        match self.inner.borrow().map.get(&path) {
            None => Err(anyhow!("No such file or directory: {}", path)),
            Some(Node::Directory { children }) => Ok(children.clone()),
            Some(Node::File { .. }) => Err(anyhow!("Tried to list directory of a file")),
            Some(Node::Symlink { .. }) => unreachable!("Canonical"),
        }
        .with_context(|| format!("Listing directory: {}", path))
    }

    fn read_file(&self, path: &str) -> Result<String> {
        let path = self.canonicalize(path)?;
        match self.inner.borrow().map.get(&path) {
            None => Err(anyhow!("No such file or directory: {}", path)),
            Some(Node::File { content }) => Ok(content.clone()),
            Some(Node::Directory { .. }) => Err(anyhow!("Tried to read a directory")),
            Some(Node::Symlink { .. }) => unreachable!("Canonical"),
        }
    }

    fn read_link(&self, path: &str) -> Result<String> {
        let inner = self.inner.borrow();
        match inner.map.get(path) {
            None => Err(anyhow!("No such file or directory: {}", path)),
            Some(Node::Symlink { target }) => Ok(target.clone()),
            Some(_) => Err(anyhow!("Not a symlink: {}", path)),
        }
    }
}

impl Inner {
    /// Inserts a new entry into the filesystem, under the given *canonical* parent
    ///
    /// # Arguments
    ///
    /// * `parent` - A canonical path to the parent directory of the entry
    /// * `name` - The name to give to the new entry
    /// * `node` - The entry itself
    ///
    pub fn insert_node(&mut self, parent: &str, name: &str, node: Node) -> Result<()> {
        // Check it doesn't already exist
        let path = super::join(parent, name);
        if self.map.contains_key(&path) {
            return Err(anyhow!("File exists: {:?}", path));
        }
        let parent_node = self
            .map
            .get_mut(parent)
            .ok_or_else(|| anyhow!("Parent directory not found: {}", parent))?;
        // Insert name into parent
        match parent_node {
            Node::Directory { ref mut children } => children.push(name.into()),
            _ => panic!("Parent not a directory: {}", parent),
        }
        // Insert full path and node into map
        self.map.insert(path, node);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::filesystem::Filesystem;

    use super::MemoryFilesystem;

    #[test]
    fn test_exists() {
        let fs = MemoryFilesystem::new();
        assert!(fs.exists("/"));
        assert!(!fs.exists("/entry"));
        fs.create_directory("/entry").unwrap();
        assert!(fs.exists("/entry"));
    }

    #[test]
    fn test_symlink_make_sub_directory() {
        let fs = MemoryFilesystem::new();
        fs.create_directory("/primary").unwrap();
        fs.create_directory("/secondary").unwrap();
        fs.create_symlink("/primary/link", "/secondary/target".into())
            .unwrap();
        fs.create_directory("/secondary/target").unwrap();
        fs.create_directory("/primary/link/through").unwrap();
        assert!(fs.exists("/primary/link/through"));
    }
}

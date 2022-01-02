use std::{cell::RefCell, collections::HashMap};

use anyhow::{anyhow, Context, Result};

use super::{join, normalize, split, Filesystem};

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
}

impl Filesystem for MemoryFilesystem {
    fn create_directory(&self, path: &str) -> Result<()> {
        let mut inner = self.inner.borrow_mut();
        inner
            .insert_node(path, Node::Directory { children: vec![] })
            .with_context(|| format!("Creating directory: {}", path))
    }

    fn create_file(&self, path: &str, content: String) -> Result<()> {
        let mut inner = self.inner.borrow_mut();
        inner
            .insert_node(path, Node::File { content })
            .with_context(|| format!("Creating file: {}", path))
    }

    fn create_symlink(&self, path: &str, target: String) -> Result<()> {
        let mut inner = self.inner.borrow_mut();
        inner
            .insert_node(path, Node::Symlink { target })
            .with_context(|| format!("Creating symlink: {}", path))
    }

    fn exists(&self, path: &str) -> bool {
        self.inner.borrow().map.contains_key(path)
    }

    fn is_directory(&self, path: &str) -> bool {
        match self.inner.borrow().map.get(path) {
            Some(Node::Directory { .. }) => true,
            _ => false,
        }
    }

    fn is_file(&self, path: &str) -> bool {
        match self.inner.borrow().map.get(path) {
            Some(Node::File { .. }) => true,
            _ => false,
        }
    }

    fn is_link(&self, path: &str) -> bool {
        match self.inner.borrow().map.get(path) {
            Some(Node::Symlink { .. }) => true,
            _ => false,
        }
    }

    fn list_directory(&self, path: &str) -> Result<Vec<String>> {
        let inner = self.inner.borrow();
        match inner.map.get(&inner.dereference(path)?) {
            None => Err(anyhow!("No such file or directory: {}", path)),
            Some(Node::Directory { children }) => Ok(children.clone()),
            Some(Node::File { .. }) => Err(anyhow!("Tried to list directory of a file")),
            Some(Node::Symlink { .. }) => unreachable!("Dereferenced"),
        }
        .with_context(|| format!("Listing directory: {}", path))
    }

    fn read_file(&self, path: &str) -> Result<String> {
        let inner = self.inner.borrow();
        match inner.map.get(&inner.dereference(path)?) {
            None => Err(anyhow!("No such file or directory: {}", path)),
            Some(Node::File { content }) => Ok(content.clone()),
            Some(Node::Directory { .. }) => Err(anyhow!("Tried to read a directory")),
            Some(Node::Symlink { .. }) => unreachable!("Dereferenced"),
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
    fn insert_node(&mut self, path: &str, node: Node) -> Result<()> {
        let path = normalize(path);

        // Check it doesn't already exist
        if self.map.contains_key(path.as_ref()) {
            return Err(anyhow!("File exists: {:?}", path));
        }
        // Get the parent node and file name
        let (parent, name) = split(path.as_ref()).ok_or_else(|| anyhow!("No parent: {}", path))?;
        let parent = self
            .dereference(parent)
            .with_context(|| format!("Dereferencing {}", parent))?;
        let parent_node = self
            .map
            .get_mut(&parent)
            .ok_or_else(|| anyhow!("Outside filesystem: {}", parent))
            .with_context(|| format!("Dereferencing {}", parent))?;
        // Insert name into parent
        match parent_node {
            Node::Directory { ref mut children } => children.push(name.into()),
            _ => panic!("Parent can only be a directory"),
        }
        // Insert full path and node into map
        self.map.insert(join(&parent, name), node);
        Ok(())
    }

    fn dereference(&self, path: &str) -> Result<String> {
        let path = normalize(path);
        match self.map.get(path.as_ref()) {
            Some(Node::Symlink { target }) => self.dereference(target),
            Some(_) => Ok(path.into_owned()),
            None => Err(anyhow!(
                "No such file or directory: {}\n{:#?}",
                path,
                self.map
            )),
        }
    }
}

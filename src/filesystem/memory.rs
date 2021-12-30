use std::{cell::RefCell, collections::HashMap};

use anyhow::{anyhow, Result};

use super::{split, Filesystem};

pub struct MemoryFilesystem {
    inner: RefCell<Inner>,
}

struct Inner {
    map: HashMap<String, Node>,
}

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
        inner.insert_node(path, Node::Directory { children: vec![] })
    }

    fn create_file(&self, path: &str, content: String) -> Result<()> {
        let mut inner = self.inner.borrow_mut();
        inner.insert_node(path, Node::File { content })
    }

    fn create_symlink(&self, path: &str, target: String) -> Result<()> {
        let mut inner = self.inner.borrow_mut();
        inner.insert_node(path, Node::Symlink { target })
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
        match inner
            .dereference(path)
            .and_then(|path| inner.map.get(&path))
        {
            None => Err(anyhow!("No such file or directory: {}", path)),
            Some(Node::Directory { children }) => Ok(children.clone()),
            Some(Node::File { .. }) => Err(anyhow!("Tried to list directory of a file")),
            Some(Node::Symlink { .. }) => unreachable!("Dereferenced"),
        }
    }

    fn read_file(&self, path: &str) -> Result<String> {
        let inner = self.inner.borrow();
        match inner
            .dereference(path)
            .and_then(|path| inner.map.get(&path))
        {
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
        // Check it doesn't already exist
        if self.map.contains_key(path) {
            return Err(anyhow!("File exists: {:?}", path));
        }
        // Get the parent node and file name
        let (parent, name) = split(path).ok_or_else(|| anyhow!("No parent: {}", path))?;
        let parent = self
            .dereference(parent)
            .ok_or_else(|| anyhow!("No such file or directory: {}", parent))?;
        let parent_node = self
            .map
            .get_mut(&parent)
            .ok_or_else(|| anyhow!("Outside filesystem: {}", parent))?;
        // Insert name into parent
        match parent_node {
            Node::Directory { ref mut children } => children.push(name.into()),
            _ => panic!("Parent can only be a directory"),
        }
        // Insert full path and node into map
        self.map.insert(path.into(), node);
        Ok(())
    }

    fn dereference(&self, path: &str) -> Option<String> {
        let mut path: String = path.into();
        loop {
            match self.map.get(&path) {
                Some(Node::Symlink { target }) => path = target.clone(),
                Some(_) => break Some(path),
                None => break None,
            }
        }
    }
}

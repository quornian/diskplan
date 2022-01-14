use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
};

use anyhow::{anyhow, Context, Result};

use super::{Attrs, Filesystem, SetAttrs};

/// An in-memory representation of a file system
#[derive(Debug)]
pub struct MemoryFilesystem {
    map: HashMap<String, Node>,
    uids: HashMap<String, u32>,
    gids: HashMap<String, u32>,
}

#[derive(Debug)]
enum Node {
    File {
        attrs: FSAttrs,
        content: String,
    },
    Directory {
        attrs: FSAttrs,
        children: Vec<String>,
    },
    Symlink {
        target: String,
    },
}

#[derive(Debug)]
struct FSAttrs {
    uid: u32,
    gid: u32,
    mode: u16,
}

impl MemoryFilesystem {
    // Arbitrarily chosen, can overlap
    const UID_BASE: u32 = 5000;
    const GID_BASE: u32 = 9000;
    const ROOT: u32 = 0;
    const NOBODY: u32 = 65534;
    const NOGROUP: u32 = 65534;

    const DEFAULT_OWNER: u32 = Self::ROOT;
    const DEFAULT_GROUP: u32 = Self::ROOT;

    pub fn new() -> Self {
        let mut map = HashMap::new();
        let mut uids = HashMap::new();
        uids.insert("root".to_owned(), Self::ROOT);
        uids.insert("nobody".to_owned(), Self::NOBODY);
        let mut gids = HashMap::new();
        gids.insert("root".to_owned(), Self::ROOT);
        gids.insert("nogroup".to_owned(), Self::NOGROUP);
        map.insert(
            "/".into(),
            Node::Directory {
                attrs: FSAttrs {
                    uid: Self::ROOT,
                    gid: Self::ROOT,
                    mode: super::DEFAULT_DIRECTORY_MODE,
                },
                children: vec![],
            },
        );
        MemoryFilesystem { map, uids, gids }
    }

    pub fn to_path_set<'a>(&'a self) -> HashSet<String> {
        self.map.keys().cloned().collect()
    }
}

impl Filesystem for MemoryFilesystem {
    fn create_directory(&mut self, path: &str, attrs: SetAttrs) -> Result<()> {
        let (parent, name) = self
            .canonical_split(path)
            .with_context(|| format!("Splitting {}", path))?;
        let attrs = self.internal_attrs(attrs, super::DEFAULT_DIRECTORY_MODE)?;
        let children = vec![];
        self.insert_node(&parent, name, Node::Directory { attrs, children })
            .with_context(|| format!("Creating directory: {}", path))
    }

    fn create_file(&mut self, path: &str, attrs: SetAttrs, content: String) -> Result<()> {
        let (parent, name) = self.canonical_split(path)?;
        let attrs = self.internal_attrs(attrs, super::DEFAULT_FILE_MODE)?;
        self.insert_node(&parent, name, Node::File { attrs, content })
            .with_context(|| format!("Creating file: {}", path))
    }

    fn create_symlink(&mut self, path: &str, target: String) -> Result<()> {
        let (parent, name) = self.canonical_split(path)?;
        self.insert_node(&parent, name, Node::Symlink { target })
            .with_context(|| format!("Creating symlink: {}", path))
    }

    fn exists(&self, path: &str) -> bool {
        match self.canonicalize(path) {
            Ok(path) => self.map.contains_key(&path),
            _ => false,
        }
    }

    fn is_directory(&self, path: &str) -> bool {
        match self.canonicalize(path) {
            Err(_) => false,
            Ok(path) => match self.map.get(&path) {
                Some(Node::Directory { .. }) => true,
                _ => false,
            },
        }
    }

    fn is_file(&self, path: &str) -> bool {
        match self.canonicalize(path) {
            Err(_) => false,
            Ok(path) => match self.map.get(&path) {
                Some(Node::File { .. }) => true,
                _ => false,
            },
        }
    }

    fn is_link(&self, path: &str) -> bool {
        match self.map.get(path) {
            Some(Node::Symlink { .. }) => true,
            _ => false,
        }
    }

    fn list_directory(&self, path: &str) -> Result<Vec<String>> {
        let path = self.canonicalize(path)?;
        match self.map.get(&path) {
            None => Err(anyhow!("No such file or directory: {}", path)),
            Some(Node::Directory { children, .. }) => Ok(children.clone()),
            Some(Node::File { .. }) => Err(anyhow!("Tried to list directory of a file")),
            Some(Node::Symlink { .. }) => unreachable!("Canonical"),
        }
        .with_context(|| format!("Listing directory: {}", path))
    }

    fn read_file(&self, path: &str) -> Result<String> {
        let path = self.canonicalize(path)?;
        match self.map.get(&path) {
            None => Err(anyhow!("No such file or directory: {}", path)),
            Some(Node::File { content, .. }) => Ok(content.clone()),
            Some(Node::Directory { .. }) => Err(anyhow!("Tried to read a directory")),
            Some(Node::Symlink { .. }) => unreachable!("Canonical"),
        }
    }

    fn read_link(&self, path: &str) -> Result<String> {
        match self.map.get(path) {
            None => Err(anyhow!("No such file or directory: {}", path)),
            Some(Node::Symlink { target }) => Ok(target.clone()),
            Some(_) => Err(anyhow!("Not a symlink: {}", path)),
        }
    }

    fn attributes(&self, path: &str) -> Result<Attrs> {
        let path = self.canonicalize(path)?;
        let node = self
            .map
            .get(&path)
            .ok_or_else(|| anyhow!("No such file or directory: {}", path))?;
        let attrs = match node {
            Node::Directory { attrs, .. } | Node::File { attrs, .. } => attrs,
            Node::Symlink { .. } => panic!("Non-canonical path: {}", path),
        };
        // Slow inverse lookup
        Ok(Attrs {
            owner: Cow::Borrowed(
                self.uids
                    .iter()
                    .find_map(|(user, &uid)| if uid == attrs.uid { Some(user) } else { None })
                    .ok_or_else(|| anyhow!("UID not found: {}", attrs.uid))?,
            ),
            group: Cow::Borrowed(
                self.gids
                    .iter()
                    .find_map(|(group, &gid)| if gid == attrs.gid { Some(group) } else { None })
                    .ok_or_else(|| anyhow!("GID not found: {}", attrs.gid))?,
            ),
            mode: attrs.mode,
        })
    }

    fn prefetch_uids<'i, I>(&mut self, users: I) -> Result<()>
    where
        I: Iterator<Item = &'i str>,
    {
        // Give each user a unique number, the value is not terribly important so we don't need
        // to check for duplicate entries in the provided iterator
        for user in users {
            self.uids
                .insert(user.to_owned(), Self::UID_BASE + self.uids.len() as u32);
        }
        Ok(())
    }

    fn prefetch_gids<'i, I>(&mut self, groups: I) -> Result<()>
    where
        I: Iterator<Item = &'i str>,
    {
        // Give each group a unique number, the value is not terribly important so we don't need
        // to check for duplicate entries in the provided iterator
        for group in groups {
            self.gids
                .insert(group.to_owned(), Self::GID_BASE + self.gids.len() as u32);
        }
        Ok(())
    }
}

impl MemoryFilesystem {
    fn canonical_split<'s>(&self, path: &'s str) -> Result<(String, &'s str)> {
        match super::split(path) {
            None => Err(anyhow!("Cannot create {}", path)),
            Some((parent, name)) => Ok((self.canonicalize(parent)?, name)),
        }
    }

    fn uid_for(&self, user: &str) -> Result<u32> {
        self.uids.get(user).map(|x| *x).ok_or_else(|| {
            anyhow!(
                "No cached user by the name of {}, make sure to run prefetch_uids",
                user
            )
        })
    }

    fn gid_for(&self, group: &str) -> Result<u32> {
        self.gids.get(group).map(|x| *x).ok_or_else(|| {
            anyhow!(
                "No cached group by the name of {}, make sure to run prefetch_gids",
                group
            )
        })
    }

    fn internal_attrs(&self, attrs: SetAttrs, default_mode: u16) -> Result<FSAttrs> {
        let uid = match attrs.owner {
            Some(owner) => self.uid_for(owner)?,
            None => Self::DEFAULT_OWNER,
        };
        let gid = match attrs.group {
            Some(group) => self.gid_for(group)?,
            None => Self::DEFAULT_GROUP,
        };
        let mode = attrs.mode.unwrap_or(default_mode);
        Ok(FSAttrs { uid, gid, mode })
    }

    /// Inserts a new entry into the filesystem, under the given *canonical* parent
    ///
    /// # Arguments
    ///
    /// * `parent` - A canonical path to the parent directory of the entry
    /// * `name` - The name to give to the new entry
    /// * `node` - The entry itself
    ///
    fn insert_node(&mut self, parent: &str, name: &str, node: Node) -> Result<()> {
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
            Node::Directory {
                ref mut children, ..
            } => children.push(name.into()),
            _ => panic!("Parent not a directory: {}", parent),
        }
        // Insert full path and node into map
        self.map.insert(path, node);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::filesystem::{Filesystem, SetAttrs};

    use super::MemoryFilesystem;

    #[test]
    fn test_exists() {
        let mut fs = MemoryFilesystem::new();
        assert!(fs.exists("/"));
        assert!(!fs.exists("/entry"));
        fs.create_directory("/entry", SetAttrs::default()).unwrap();
        assert!(fs.exists("/entry"));
    }

    #[test]
    fn test_symlink_make_sub_directory() {
        let mut fs = MemoryFilesystem::new();
        fs.create_directory("/primary", SetAttrs::default())
            .unwrap();
        fs.create_directory("/secondary", SetAttrs::default())
            .unwrap();
        fs.create_symlink("/primary/link", "/secondary/target".into())
            .unwrap();
        fs.create_directory("/secondary/target", SetAttrs::default())
            .unwrap();
        fs.create_directory("/primary/link/through", SetAttrs::default())
            .unwrap();
        assert!(fs.exists("/primary/link/through"));
    }
}

use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
};

use anyhow::{anyhow, Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use nix::unistd;
use users::{Groups, Users, UsersCache};

use super::{
    attributes::Mode, Attrs, Filesystem, SetAttrs, DEFAULT_DIRECTORY_MODE, DEFAULT_FILE_MODE,
};

/// An in-memory representation of a file system
pub struct MemoryFilesystem {
    map: HashMap<Utf8PathBuf, Node>,
    users: UsersCache,

    uid: u32,
    gid: u32,
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
        target: Utf8PathBuf,
    },
}

#[derive(Debug)]
struct FSAttrs {
    uid: u32,
    gid: u32,
    mode: u16,
}

impl MemoryFilesystem {
    const ROOT: u32 = 0;
    const DEFAULT_OWNER: u32 = Self::ROOT;
    const DEFAULT_GROUP: u32 = Self::ROOT;

    pub fn new() -> Self {
        let mut map = HashMap::new();
        map.insert(
            "/".into(),
            Node::Directory {
                attrs: FSAttrs {
                    uid: Self::DEFAULT_OWNER,
                    gid: Self::DEFAULT_GROUP,
                    mode: DEFAULT_DIRECTORY_MODE.into(),
                },
                children: vec![],
            },
        );
        MemoryFilesystem {
            map,
            users: UsersCache::new(),
            uid: unistd::getuid().as_raw(),
            gid: unistd::getgid().as_raw(),
        }
    }

    pub fn to_path_set(&self) -> HashSet<&Utf8Path> {
        self.map.keys().map(|i| i.as_ref()).collect()
    }
}

impl Default for MemoryFilesystem {
    fn default() -> Self {
        Self::new()
    }
}

impl Filesystem for MemoryFilesystem {
    fn create_directory(&mut self, path: impl AsRef<Utf8Path>, attrs: SetAttrs) -> Result<()> {
        let path = path.as_ref();
        let (parent, name) = self
            .canonical_split(path)
            .with_context(|| format!("Splitting {}", path))?;
        let attrs = self.internal_attrs(attrs, DEFAULT_DIRECTORY_MODE)?;
        let children = vec![];
        self.insert_node(&parent, name, Node::Directory { attrs, children })
            .with_context(|| format!("Creating directory: {}", path))
    }

    fn create_file(
        &mut self,
        path: impl AsRef<Utf8Path>,
        attrs: SetAttrs,
        content: String,
    ) -> Result<()> {
        let path = path.as_ref();
        let (parent, name) = self.canonical_split(path)?;
        let attrs = self.internal_attrs(attrs, DEFAULT_FILE_MODE)?;
        self.insert_node(&parent, name, Node::File { attrs, content })
            .with_context(|| format!("Creating file: {}", path))
    }

    fn create_symlink(
        &mut self,
        path: impl AsRef<Utf8Path>,
        target: impl AsRef<Utf8Path>,
    ) -> Result<()> {
        let path = path.as_ref();
        let (parent, name) = self.canonical_split(path)?;
        self.insert_node(
            &parent,
            name,
            Node::Symlink {
                target: target.as_ref().to_owned(),
            },
        )
        .with_context(|| format!("Creating symlink: {}", path))
    }

    fn exists(&self, path: impl AsRef<Utf8Path>) -> bool {
        match self.canonicalize(path) {
            Ok(path) => self.map.contains_key(&path),
            _ => false,
        }
    }

    fn is_directory(&self, path: impl AsRef<Utf8Path>) -> bool {
        match self.canonicalize(path) {
            Err(_) => false,
            Ok(path) => matches!(self.map.get(&path), Some(Node::Directory { .. })),
        }
    }

    fn is_file(&self, path: impl AsRef<Utf8Path>) -> bool {
        match self.canonicalize(path) {
            Err(_) => false,
            Ok(path) => matches!(self.map.get(&path), Some(Node::File { .. })),
        }
    }

    fn is_link(&self, path: impl AsRef<Utf8Path>) -> bool {
        matches!(self.map.get(path.as_ref()), Some(Node::Symlink { .. }))
    }

    fn list_directory(&self, path: impl AsRef<Utf8Path>) -> Result<Vec<String>> {
        let path = self.canonicalize(path)?;
        match self.node_from_path(&path)? {
            Node::Directory { children, .. } => Ok(children.clone()),
            Node::File { .. } => Err(anyhow!("Tried to list directory of a file")),
            Node::Symlink { .. } => panic!("Non-canonical path: {}", path),
        }
        .with_context(|| format!("Listing directory: {}", path))
    }

    fn read_file(&self, path: impl AsRef<Utf8Path>) -> Result<String> {
        let path = self.canonicalize(path)?;
        match self.node_from_path(&path)? {
            Node::File { content, .. } => Ok(content.clone()),
            Node::Directory { .. } => Err(anyhow!("Tried to read a directory")),
            Node::Symlink { .. } => panic!("Non-canonical path: {}", path),
        }
    }

    fn read_link(&self, path: impl AsRef<Utf8Path>) -> Result<Utf8PathBuf> {
        match self.node_from_path(&path)? {
            Node::Symlink { target } => Ok(target.clone()),
            _ => Err(anyhow!("Not a symlink: {}", path.as_ref())),
        }
    }

    fn attributes(&self, path: impl AsRef<Utf8Path>) -> Result<Attrs> {
        let path = self.canonicalize(path)?;
        let node = self.node_from_path(&path)?;
        let attrs = match node {
            Node::Directory { attrs, .. } | Node::File { attrs, .. } => attrs,
            Node::Symlink { .. } => panic!("Non-canonical path: {}", path),
        };
        let owner = Cow::Owned(
            self.users
                .get_user_by_uid(attrs.uid)
                .ok_or_else(|| anyhow!("Failed to get user from UID: {}", attrs.uid))?
                .name()
                .to_string_lossy()
                .into_owned(),
        );
        let group = Cow::Owned(
            self.users
                .get_group_by_gid(attrs.gid)
                .ok_or_else(|| anyhow!("Failed to get group from GID: {}", attrs.gid))?
                .name()
                .to_string_lossy()
                .into_owned(),
        );
        let mode = attrs.mode.into();
        Ok(Attrs { owner, group, mode })
    }

    fn set_attributes(&mut self, path: impl AsRef<Utf8Path>, set_attrs: SetAttrs) -> Result<()> {
        let use_default = set_attrs.mode.is_none();
        let mut fs_attrs = self.internal_attrs(set_attrs, 0.into())?;
        let path = self.canonicalize(path)?;
        let node = self
            .map
            .get_mut(&path)
            .ok_or_else(|| anyhow!("No such file or directory: {}", path))?;
        match node {
            Node::Directory { attrs, .. } => {
                if use_default {
                    fs_attrs.mode = DEFAULT_DIRECTORY_MODE.into();
                }
                *attrs = fs_attrs;
                Ok(())
            }
            Node::File { attrs, .. } => {
                if use_default {
                    fs_attrs.mode = DEFAULT_FILE_MODE.into();
                }
                *attrs = fs_attrs;
                Ok(())
            }
            Node::Symlink { .. } => Err(anyhow!("Non-canonical path: {}", path)),
        }
    }
}

impl MemoryFilesystem {
    fn canonical_split<'s>(&self, path: &'s Utf8Path) -> Result<(Utf8PathBuf, &'s str)> {
        match super::split(path) {
            None => Err(anyhow!("Cannot create {}", path)),
            Some((parent, name)) => Ok((self.canonicalize(parent)?, name)),
        }
    }

    fn internal_attrs(&self, attrs: SetAttrs, default_mode: Mode) -> Result<FSAttrs> {
        let uid = match attrs.owner {
            Some(owner) => self
                .users
                .get_user_by_name(owner)
                .ok_or_else(|| anyhow!("No such user: {}", owner))?
                .uid(),
            None => self.uid,
        };
        let gid = match attrs.group {
            Some(group) => self
                .users
                .get_group_by_name(group)
                .ok_or_else(|| anyhow!("No such group: {}", group))?
                .gid(),
            None => self.gid,
        };
        let mode = attrs.mode.unwrap_or(default_mode).into();
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
    fn insert_node(&mut self, parent: impl AsRef<Utf8Path>, name: &str, node: Node) -> Result<()> {
        // Check it doesn't already exist
        let parent = parent.as_ref();
        let path = parent.join(name);
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

    fn node_from_path(&self, path: impl AsRef<Utf8Path>) -> Result<&Node> {
        let path = path.as_ref();
        self.map
            .get(path)
            .ok_or_else(|| anyhow!("No such file or directory: {}", path))
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
        fs.create_symlink("/primary/link", "/secondary/target")
            .unwrap();
        fs.create_directory("/secondary/target", SetAttrs::default())
            .unwrap();
        fs.create_directory("/primary/link/through", SetAttrs::default())
            .unwrap();
        assert!(fs.exists("/primary/link/through"));
    }
}

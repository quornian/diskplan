//! Provides an abstract [`Filesystem`] trait, together with a physical ([`DiskFilesystem`])
//! and virtual ([`MemoryFilesystem`]) implementation.
use std::borrow::Cow;

use anyhow::{anyhow, Result};

mod memory;
mod physical;

pub use memory::MemoryFilesystem;
pub use physical::DiskFilesystem;

#[derive(Debug, Default, Clone, PartialEq)]
pub struct SetAttrs<'a> {
    pub owner: Option<&'a str>,
    pub group: Option<&'a str>,
    pub mode: Option<u16>,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Attrs<'a> {
    pub owner: &'a str,
    pub group: &'a str,
    pub mode: u16,
}

/// Operations of a file system
pub trait Filesystem {
    fn create_directory(&mut self, path: &str, attrs: SetAttrs) -> Result<()>;

    fn create_directory_all(&mut self, path: &str, attrs: SetAttrs) -> Result<()> {
        if let Some((parent, _)) = split(path) {
            if parent != "/" {
                self.create_directory_all(parent, attrs.clone())?;
            }
        }
        if !self.is_directory(path) {
            self.create_directory(path, attrs)?;
        }
        Ok(())
    }

    fn create_file(&mut self, path: &str, attrs: SetAttrs, content: String) -> Result<()>;

    fn create_symlink(&mut self, path: &str, target: String) -> Result<()>;

    fn exists(&self, path: &str) -> bool;

    fn is_directory(&self, path: &str) -> bool;

    fn is_file(&self, path: &str) -> bool;

    fn is_link(&self, path: &str) -> bool;

    fn list_directory(&self, path: &str) -> Result<Vec<String>>;

    fn attributes(&self, path: &str) -> Result<Attrs>;

    fn read_file(&self, path: &str) -> Result<String>;

    fn read_link(&self, path: &str) -> Result<String>;

    fn canonicalize(&self, path: &str) -> Result<String> {
        let path = normalize(path);
        let mut canon = String::with_capacity(path.len());
        if !path.starts_with('/') {
            // TODO: Keep a current_directory to provide relative path support
            return Err(anyhow!("Only absolute paths supported"));
        }
        for part in path[1..].split('/') {
            canon.push('/');
            canon.push_str(part);
            if self.is_link(&canon) {
                canon = self.canonicalize(&self.read_link(&canon)?)?;
            }
        }
        Ok(canon)
    }

    fn prefetch_uids<'i, I>(&mut self, users: I) -> Result<()>
    where
        I: Iterator<Item = &'i str>;

    fn prefetch_gids<'i, I>(&mut self, groups: I) -> Result<()>
    where
        I: Iterator<Item = &'i str>;
}

pub fn name(path: &str) -> &str {
    path.rfind('/')
        .map_or_else(|| path, |index| &path[index + 1..])
}

pub fn parent(path: &str) -> Option<&str> {
    path.rfind('/').map(|index| &path[..index])
}

pub fn join(path: &str, child: &str) -> String {
    format!(
        "{}/{}",
        path.trim_end_matches('/'),
        child.trim_start_matches('/')
    )
}

pub fn split(path: &str) -> Option<(&str, &str)> {
    // TODO: Consider join(parent, "/absolute/child")
    path.rsplit_once('/').map(|(parent, child)| {
        if parent.is_empty() {
            ("/", child)
        } else {
            (parent, child)
        }
    })
}

pub fn normalize(path: &str) -> Cow<'_, str> {
    let mut path = Cow::Borrowed(path);
    while path.contains("//") {
        path = Cow::Owned(path.replace("//", "/"));
    }
    while path.contains("/./") {
        path = Cow::Owned(path.replace("/./", "/"));
    }
    path
}

pub struct SplitPath<'a> {
    root: &'a str,
    full: String,
}

impl<'a> SplitPath<'a> {
    pub fn new(root: &'a str) -> Result<Self> {
        match root.starts_with("/") {
            false => Err(anyhow!("Root must be an absolute path")),
            true => Ok(SplitPath {
                root: root,
                full: root.to_owned(),
            }),
        }
    }

    pub fn root(&self) -> &'a str {
        self.root
    }

    pub fn absolute(&self) -> &str {
        &self.full
    }

    pub fn relative(&self) -> &str {
        self.full.strip_prefix(self.root).unwrap()
    }

    pub fn join(&self, path: &str) -> Self {
        let path = normalize(path);
        SplitPath {
            root: self.root,
            full: join(&self.full, &path),
        }
    }
}

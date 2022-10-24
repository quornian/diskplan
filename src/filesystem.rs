//! Provides an abstract [`Filesystem`] trait, together with a physical ([`DiskFilesystem`])
//! and virtual ([`MemoryFilesystem`]) implementation.
use std::{borrow::Cow, fmt::Display};

use anyhow::{anyhow, Result};

mod attributes;
mod memory;
mod physical;

pub use self::{
    attributes::{Attrs, SetAttrs, DEFAULT_DIRECTORY_MODE, DEFAULT_FILE_MODE},
    memory::MemoryFilesystem,
    physical::DiskFilesystem,
};

impl SetAttrs<'_> {
    pub fn matches(&self, attrs: &Attrs) -> bool {
        let SetAttrs { owner, group, mode } = self;
        owner.map(|owner| owner == attrs.owner).unwrap_or(true)
            && group.map(|group| group == attrs.group).unwrap_or(true)
            && mode.map(|mode| mode == attrs.mode).unwrap_or(true)
    }
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

    fn read_file(&self, path: &str) -> Result<String>;

    fn read_link(&self, path: &str) -> Result<String>;

    fn attributes(&self, path: &str) -> Result<Attrs>;

    fn set_attributes(&mut self, path: &str, attrs: SetAttrs) -> Result<()>;

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

pub fn is_normalized(path: &str) -> bool {
    !((path.ends_with('/') && path != "/") || path.contains("//") || path.contains("/./"))
}

pub fn normalize(path: &str) -> Cow<'_, str> {
    let mut path = Cow::Borrowed(if path == "/" {
        path
    } else {
        path.trim_end_matches('/')
    });
    while path.contains("//") {
        path = Cow::Owned(path.replace("//", "/"));
    }
    while path.contains("/./") {
        path = Cow::Owned(path.replace("/./", "/"));
    }
    path
}

pub struct SplitPath {
    root_len: usize,
    full: String,
}

impl SplitPath {
    pub fn new(root: &str) -> Result<Self> {
        if !is_normalized(root) {
            return Err(anyhow!("Root must be a normalized path: {}", root));
        }
        if !root.starts_with("/") {
            return Err(anyhow!("Root must be an absolute path"));
        }
        Ok(SplitPath {
            root_len: root.len(),
            full: root.to_owned(),
        })
    }

    pub fn root(&self) -> &str {
        &self.full[..self.root_len]
    }

    pub fn absolute(&self) -> &str {
        &self.full
    }

    pub fn relative(&self) -> &str {
        self.full[self.root_len..].trim_start_matches('/')
    }

    pub fn join(&self, path: &str) -> Self {
        let path = normalize(path);
        SplitPath {
            root_len: self.root_len,
            full: join(&self.full, &path),
        }
    }
}

impl Display for SplitPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.full)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_relative() {
        let path = SplitPath::new("/example/path").unwrap();
        assert!(!path.relative().starts_with('/'));
    }
}

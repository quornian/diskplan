//! Provides an abstract [`Filesystem`] trait, together with a physical ([`DiskFilesystem`])
//! and virtual ([`MemoryFilesystem`]) implementation.
#![warn(missing_docs)]

use std::fmt::Display;

use anyhow::{bail, Result};
use camino::{Utf8Component, Utf8Path, Utf8PathBuf};

mod attributes;
mod memory;
mod physical;

use diskplan_config::Root;

pub use self::{
    attributes::{Attrs, Mode, SetAttrs, DEFAULT_DIRECTORY_MODE, DEFAULT_FILE_MODE},
    memory::MemoryFilesystem,
    physical::DiskFilesystem,
};

impl SetAttrs<'_> {
    /// Returns true if this `SetAttrs` matches the given, existing `attrs`
    pub fn matches(&self, attrs: &Attrs) -> bool {
        let SetAttrs { owner, group, mode } = self;
        owner.map(|owner| owner == attrs.owner).unwrap_or(true)
            && group.map(|group| group == attrs.group).unwrap_or(true)
            && mode.map(|mode| mode == attrs.mode).unwrap_or(true)
    }
}

/// Operations of a file system
pub trait Filesystem {
    /// Create a directory at the given path, with any number of attributes set
    fn create_directory(&mut self, path: impl AsRef<Utf8Path>, attrs: SetAttrs) -> Result<()>;

    /// Create a directory and all of its parents
    fn create_directory_all(&mut self, path: impl AsRef<Utf8Path>, attrs: SetAttrs) -> Result<()> {
        let path = path.as_ref();
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

    /// Create a file with the given content and any number of attributes set
    fn create_file(
        &mut self,
        path: impl AsRef<Utf8Path>,
        attrs: SetAttrs,
        content: String,
    ) -> Result<()>;

    /// Create a symlink pointing to the given target
    fn create_symlink(
        &mut self,
        path: impl AsRef<Utf8Path>,
        target: impl AsRef<Utf8Path>,
    ) -> Result<()>;

    /// Returns true if the path exists
    fn exists(&self, path: impl AsRef<Utf8Path>) -> bool;

    /// Returns true if the path is a directory
    fn is_directory(&self, path: impl AsRef<Utf8Path>) -> bool;

    /// Returns true if the path is a regular file
    fn is_file(&self, path: impl AsRef<Utf8Path>) -> bool;

    /// Returns true if the path is a symbolic link
    fn is_link(&self, path: impl AsRef<Utf8Path>) -> bool;

    /// Lists the contents of the given directory
    fn list_directory(&self, path: impl AsRef<Utf8Path>) -> Result<Vec<String>>;

    /// Reads the contents of the given file
    fn read_file(&self, path: impl AsRef<Utf8Path>) -> Result<String>;

    /// Reads the path pointed to by the given symbolic link
    fn read_link(&self, path: impl AsRef<Utf8Path>) -> Result<Utf8PathBuf>;

    /// Returns the attributes of the given file, directory
    ///
    /// If the path is a symlink, the file/directory pointed to by the symlink will be checked
    /// and its attributes returned (i.e. paths are dereferenced)
    fn attributes(&self, path: impl AsRef<Utf8Path>) -> Result<Attrs>;

    /// Sets the attributes of the given file or directory
    ///
    /// If the path is a symlink, the file/directory pointed to by the symlink will be updated
    /// with the given attributes (i.e. paths are dereferenced)
    fn set_attributes(&mut self, path: impl AsRef<Utf8Path>, attrs: SetAttrs) -> Result<()>;

    /// Returns the path after following all symlinks, normalized and absolute
    fn canonicalize(&self, path: impl AsRef<Utf8Path>) -> Result<Utf8PathBuf> {
        let path = path.as_ref();
        if !path.is_absolute() {
            // TODO: Keep a current_directory to provide relative path support
            bail!("Only absolute paths supported");
        }
        let mut canon = Utf8PathBuf::with_capacity(path.as_str().len());
        for part in path.components() {
            if part == Utf8Component::ParentDir {
                let pop = canon.pop();
                assert!(pop);
                continue;
            }
            canon.push(part);
            if self.is_link(Utf8Path::new(&canon)) {
                let link = self.read_link(&canon)?;
                if link.is_absolute() {
                    canon.clear();
                } else {
                    canon.pop();
                }
                canon.push(link);
                canon = self.canonicalize(canon)?;
            }
        }
        Ok(canon)
    }
}

/// Splits the dirname and basename of the path if possible to do so
fn split(path: &Utf8Path) -> Option<(&Utf8Path, &str)> {
    // TODO: Consider join(parent, "/absolute/child")
    path.as_str().rsplit_once('/').map(|(parent, child)| {
        if parent.is_empty() {
            ("/".into(), child)
        } else {
            (parent.into(), child)
        }
    })
}

/// An absolute path that can be split easily into its [`Root`] and relative path parts
pub struct PlantedPath {
    root_len: usize,
    full: Utf8PathBuf,
}

impl PlantedPath {
    /// Creates a planted path from a given root and optional full path
    ///
    /// If no path is given the root's path will be used. If a given path is not prefixed with
    /// the root's path, an error is returned.
    pub fn new(root: &Root, path: Option<&Utf8Path>) -> Result<Self> {
        let path = match path {
            Some(path) => {
                if !path.starts_with(root.path()) {
                    bail!("Path {} must start with root {}", path, root.path());
                }
                path
            }
            None => root.path(),
        };
        Ok(PlantedPath {
            root_len: root.path().as_str().len(),
            full: path.to_owned(),
        })
    }

    /// The absolute path of the root part of this planted path
    pub fn root(&self) -> &Utf8Path {
        self.full.as_str()[..self.root_len].into()
    }

    /// The full, absolute path
    pub fn absolute(&self) -> &Utf8Path {
        &self.full
    }

    /// The path relative to the root
    pub fn relative(&self) -> &Utf8Path {
        self.full.as_str()[self.root_len..]
            .trim_start_matches('/')
            .into()
    }

    /// Produces a new planted path with the given path part appended
    pub fn join(&self, name: impl AsRef<str>) -> Result<Self> {
        let name = name.as_ref();
        if name.contains('/') {
            bail!(
                "Only single path components can be joined to a planted path: {}",
                name
            );
        }
        Ok(PlantedPath {
            root_len: self.root_len,
            full: self.full.join(name),
        })
    }
}

impl Display for PlantedPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.full)
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;

    use super::*;

    #[test]
    fn check_relative() {
        let path = PlantedPath::new(
            &Root::try_from("/example").unwrap(),
            Some(Utf8Path::new("/example/path")),
        )
        .unwrap();
        assert_eq!(path.relative(), "path");
    }

    #[test]
    fn canonicalize() -> Result<()> {
        let path = Utf8Path::new("/");
        let mut fs = MemoryFilesystem::new();
        assert_eq!(fs.canonicalize(path).unwrap(), "/");

        fs.create_directory("/dir", Default::default())?;
        fs.create_symlink("/dir/sym", "../dir2/deeper")?;

        //   /
        //     dir/
        //       sym -> ../dir2/deeper    (Doesn't exist so path is kept)

        assert_eq!(fs.canonicalize("/dir/./sym//final")?, "/dir2/deeper/final");

        fs.create_directory("/dir2", Default::default())?;
        fs.create_directory("/dir2/deeper", Default::default())?;
        fs.create_symlink("/dir2/deeper/final", "/end")?;

        //   /
        //     dir/
        //       sym -> ../dir2/deeper    (Exists, so path is replaced)
        //     dir2/
        //       deeper/
        //         final -> /end

        assert_eq!(fs.canonicalize("/dir/./sym//final")?, "/end");

        assert_eq!(fs.canonicalize("/dir/sym")?, "/dir2/deeper");
        assert_eq!(fs.canonicalize("/dir/sym/.")?, "/dir2/deeper");
        assert_eq!(fs.canonicalize("/dir/sym/..")?, "/dir2");

        Ok(())
    }
}

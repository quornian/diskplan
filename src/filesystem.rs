//! Provides an abstract [`Filesystem`] trait, together with a physical ([`DiskFilesystem`])
//! and virtual ([`MemoryFilesystem`]) implementation.
use std::{borrow::Cow, fmt::Display};

use anyhow::{bail, Result};
use camino::{Utf8Component, Utf8Path, Utf8PathBuf};

mod attributes;
mod memory;
mod physical;

use crate::schema::Root;

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
    fn create_directory(&mut self, path: impl AsRef<Utf8Path>, attrs: SetAttrs) -> Result<()>;

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

    fn create_file(
        &mut self,
        path: impl AsRef<Utf8Path>,
        attrs: SetAttrs,
        content: String,
    ) -> Result<()>;

    fn create_symlink(
        &mut self,
        path: impl AsRef<Utf8Path>,
        target: impl AsRef<Utf8Path>,
    ) -> Result<()>;

    fn exists(&self, path: impl AsRef<Utf8Path>) -> bool;

    fn is_directory(&self, path: impl AsRef<Utf8Path>) -> bool;

    fn is_file(&self, path: impl AsRef<Utf8Path>) -> bool;

    fn is_link(&self, path: impl AsRef<Utf8Path>) -> bool;

    fn list_directory(&self, path: impl AsRef<Utf8Path>) -> Result<Vec<String>>;

    fn read_file(&self, path: impl AsRef<Utf8Path>) -> Result<String>;

    fn read_link(&self, path: impl AsRef<Utf8Path>) -> Result<Utf8PathBuf>;

    fn attributes(&self, path: impl AsRef<Utf8Path>) -> Result<Attrs>;

    fn set_attributes(&mut self, path: impl AsRef<Utf8Path>, attrs: SetAttrs) -> Result<()>;

    fn canonicalize(&self, path: impl AsRef<Utf8Path>) -> Result<Utf8PathBuf> {
        let path = path.as_ref();
        if !path.is_absolute() {
            // TODO: Keep a current_directory to provide relative path support
            bail!("Only absolute paths supported");
        }
        let path = normalize(path);
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

fn normalize(path: &Utf8Path) -> Cow<'_, Utf8Path> {
    let mut path = Cow::Borrowed(if path == "/" {
        path
    } else {
        path.as_str().trim_end_matches('/').into()
    });
    while path.as_str().contains("//") {
        path = Cow::Owned(Utf8PathBuf::from(path.to_string().replace("//", "/")));
    }
    while path.as_str().contains("/./") {
        path = Cow::Owned(Utf8PathBuf::from(path.to_string().replace("/./", "/")));
    }
    path
}

pub struct SplitPath {
    root_len: usize,
    full: Utf8PathBuf,
}

impl SplitPath {
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
        Ok(SplitPath {
            root_len: root.path().as_str().len(),
            full: path.to_owned(),
        })
    }

    pub fn root(&self) -> &Utf8Path {
        self.full.as_str()[..self.root_len].into()
    }

    pub fn absolute(&self) -> &Utf8Path {
        &self.full
    }

    pub fn relative(&self) -> &Utf8Path {
        self.full.as_str()[self.root_len..]
            .trim_start_matches('/')
            .into()
    }

    pub fn join(&self, path: impl AsRef<Utf8Path>) -> Self {
        let path = normalize(path.as_ref());
        SplitPath {
            root_len: self.root_len,
            full: self.full.join(&path),
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
    use anyhow::Result;

    use super::*;

    #[test]
    fn check_relative() {
        let path = SplitPath::new(
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

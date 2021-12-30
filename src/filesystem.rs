//! Provides an abstract [`Filesystem`] trait, together with a physical ([`DiskFilesystem`])
//! amd virtual ([`MemoryFilesystem`]) implementation.
use anyhow::Result;

mod memory;
mod physical;

pub use memory::MemoryFilesystem;
pub use physical::DiskFilesystem;

pub trait Filesystem {
    fn create_directory(&self, path: &str) -> Result<()>;

    fn create_directory_all(&self, path: &str) -> Result<()> {
        if let Some((parent, _)) = split(path) {
            if parent != "/" {
                self.create_directory_all(parent)?;
            }
        }
        if !self.is_directory(path) {
            self.create_directory(path)?;
        }
        Ok(())
    }

    fn create_file(&self, path: &str, content: String) -> Result<()>;

    fn create_symlink(&self, path: &str, target: String) -> Result<()>;

    fn exists(&self, path: &str) -> bool;

    fn is_directory(&self, path: &str) -> bool;

    fn is_file(&self, path: &str) -> bool;

    fn is_link(&self, path: &str) -> bool;

    fn list_directory(&self, path: &str) -> Result<Vec<String>>;

    fn read_file(&self, path: &str) -> Result<String>;

    fn read_link(&self, path: &str) -> Result<String>;
}

pub fn parent(path: &str) -> Option<&str> {
    path.rfind('/').map(|index| &path[..index])
}

pub fn join(path: &str, child: &str) -> String {
    // TODO: Consider join(parent, "/absolute/child")
    format!("{}/{}", path.trim_end_matches('/'), child)
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

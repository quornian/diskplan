use std::fs;

use anyhow::Result;

use super::Filesystem;

/// Access to a real file system
pub struct DiskFilesystem;

impl Filesystem for DiskFilesystem {
    fn create_directory(&self, path: &str) -> Result<()> {
        Ok(fs::create_dir(path)?)
    }

    fn create_file(&self, _path: &str, _content: String) -> Result<()> {
        todo!()
    }

    fn create_symlink(&self, path: &str, target: String) -> Result<()> {
        Ok(std::os::unix::fs::symlink(target, path)?)
    }

    fn exists(&self, path: &str) -> bool {
        fs::metadata(path).is_ok()
    }

    fn is_directory(&self, path: &str) -> bool {
        fs::metadata(path).map(|m| m.is_dir()).unwrap_or(false)
    }

    fn is_file(&self, path: &str) -> bool {
        fs::metadata(path).map(|m| m.is_file()).unwrap_or(false)
    }

    fn is_link(&self, _path: &str) -> bool {
        todo!()
    }

    fn list_directory(&self, path: &str) -> Result<Vec<String>> {
        let mut listing = Vec::new();
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let file_name = entry.file_name();
            listing.push(file_name.to_string_lossy().into_owned());
        }
        Ok(listing)
    }

    fn read_file(&self, _path: &str) -> Result<String> {
        todo!()
    }

    fn read_link(&self, _path: &str) -> Result<String> {
        todo!()
    }
}

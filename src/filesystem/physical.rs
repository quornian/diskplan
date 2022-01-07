use std::{fs, io::Write};

use anyhow::Result;

use super::Filesystem;

/// Access to a real file system
pub struct DiskFilesystem;

impl Filesystem for DiskFilesystem {
    fn create_directory(&self, path: &str) -> Result<()> {
        fs::create_dir(path).map_err(Into::into)
    }

    fn create_file(&self, path: &str, content: String) -> Result<()> {
        let mut file = fs::File::create(path)?;
        file.write_all(content.as_bytes())?;
        Ok(())
    }

    fn create_symlink(&self, path: &str, target: String) -> Result<()> {
        Ok(std::os::unix::fs::symlink(target, path)?)
    }

    fn exists(&self, path: &str) -> bool {
        fs::metadata(path).is_ok()
    }

    fn is_directory(&self, path: &str) -> bool {
        fs::metadata(path)
            .map(|m| m.file_type().is_dir())
            .unwrap_or(false)
    }

    fn is_file(&self, path: &str) -> bool {
        fs::metadata(path)
            .map(|m| m.file_type().is_file())
            .unwrap_or(false)
    }

    fn is_link(&self, path: &str) -> bool {
        fs::symlink_metadata(path)
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false)
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

    fn read_file(&self, path: &str) -> Result<String> {
        fs::read_to_string(path).map_err(Into::into)
    }

    fn read_link(&self, path: &str) -> Result<String> {
        Ok(fs::read_link(path)?.to_string_lossy().into_owned())
    }
}

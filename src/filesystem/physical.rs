use std::{fs, io::Write, os::unix::fs::PermissionsExt};

use anyhow::{anyhow, Result};
use nix::unistd::{Gid, Uid};
use users::{Groups, Users, UsersCache};

use super::{Filesystem, SetAttrs};

/// Access to a real file system
pub struct DiskFilesystem {
    users: UsersCache,
}

impl Filesystem for DiskFilesystem {
    fn create_directory(&mut self, path: &str, attrs: SetAttrs) -> Result<()> {
        fs::create_dir(path)?;
        self.apply_attrs(path, attrs, super::DEFAULT_DIRECTORY_MODE)
    }

    fn create_file(&mut self, path: &str, attrs: SetAttrs, content: String) -> Result<()> {
        let mut file = fs::File::create(path)?;
        file.write_all(content.as_bytes())?;
        self.apply_attrs(path, attrs, super::DEFAULT_FILE_MODE)
    }

    fn create_symlink(&mut self, path: &str, target: String) -> Result<()> {
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

    fn prefetch_uids<'i, I>(&mut self, users: I) -> Result<()>
    where
        I: Iterator<Item = &'i str>,
    {
        for user in users {
            self.users
                .get_user_by_name(user)
                .ok_or_else(|| anyhow!("No such user: {}", user))?;
        }
        Ok(())
    }

    fn prefetch_gids<'i, I>(&mut self, groups: I) -> Result<()>
    where
        I: Iterator<Item = &'i str>,
    {
        for group in groups {
            self.users
                .get_group_by_name(group)
                .ok_or_else(|| anyhow!("No such group: {}", group))?;
        }
        Ok(())
    }
}

impl DiskFilesystem {
    pub fn new() -> Self {
        DiskFilesystem {
            users: UsersCache::new(),
        }
    }

    fn apply_attrs(&self, path: &str, attrs: SetAttrs, default_mode: u16) -> Result<()> {
        let uid = match attrs.owner {
            Some(owner) => Some(Uid::from_raw(
                self.users
                    .get_user_by_name(owner)
                    .ok_or_else(|| anyhow!("No such user: {}", owner))?
                    .uid(),
            )),
            None => None,
        };
        let gid = match attrs.group {
            Some(group) => Some(Gid::from_raw(
                self.users
                    .get_group_by_name(group)
                    .ok_or_else(|| anyhow!("No such group: {}", group))?
                    .gid(),
            )),
            None => None,
        };
        let mode = PermissionsExt::from_mode(attrs.mode.unwrap_or(default_mode) as u32);

        nix::unistd::chown(path, uid, gid)?;
        fs::set_permissions(path, mode)?;
        Ok(())
    }
}

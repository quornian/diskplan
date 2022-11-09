use std::{borrow::Cow, fs, io::Write, os::unix::fs::PermissionsExt};

use anyhow::{anyhow, Result};
use camino::{Utf8Path, Utf8PathBuf};
use nix::{
    sys::stat,
    unistd::{Gid, Uid},
};
use users::{Groups, Users, UsersCache};

use super::{
    attributes::Mode, Attrs, Filesystem, SetAttrs, DEFAULT_DIRECTORY_MODE, DEFAULT_FILE_MODE,
};

/// Access to a real file system
#[derive(Default)]
pub struct DiskFilesystem {
    users: UsersCache,
}

impl Filesystem for DiskFilesystem {
    fn create_directory(&mut self, path: impl AsRef<Utf8Path>, attrs: SetAttrs) -> Result<()> {
        fs::create_dir(path.as_ref())?;
        self.apply_attrs(path, attrs, DEFAULT_DIRECTORY_MODE)
    }

    fn create_file(
        &mut self,
        path: impl AsRef<Utf8Path>,
        attrs: SetAttrs,
        content: String,
    ) -> Result<()> {
        let mut file = fs::File::create(path.as_ref())?;
        file.write_all(content.as_bytes())?;
        self.apply_attrs(path, attrs, DEFAULT_FILE_MODE)
    }

    fn create_symlink(
        &mut self,
        path: impl AsRef<Utf8Path>,
        target: impl AsRef<Utf8Path>,
    ) -> Result<()> {
        // TODO: Not allocate
        Ok(std::os::unix::fs::symlink(target.as_ref(), path.as_ref())?)
    }

    fn exists(&self, path: impl AsRef<Utf8Path>) -> bool {
        fs::metadata(path.as_ref()).is_ok()
    }

    fn is_directory(&self, path: impl AsRef<Utf8Path>) -> bool {
        fs::metadata(path.as_ref())
            .map(|m| m.file_type().is_dir())
            .unwrap_or(false)
    }

    fn is_file(&self, path: impl AsRef<Utf8Path>) -> bool {
        fs::metadata(path.as_ref())
            .map(|m| m.file_type().is_file())
            .unwrap_or(false)
    }

    fn is_link(&self, path: impl AsRef<Utf8Path>) -> bool {
        fs::symlink_metadata(path.as_ref())
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false)
    }

    fn list_directory(&self, path: impl AsRef<Utf8Path>) -> Result<Vec<String>> {
        let mut listing = Vec::new();
        for entry in fs::read_dir(path.as_ref())? {
            let entry = entry?;
            let file_name = entry.file_name();
            listing.push(file_name.to_string_lossy().into_owned());
        }
        Ok(listing)
    }

    fn read_file(&self, path: impl AsRef<Utf8Path>) -> Result<String> {
        fs::read_to_string(path.as_ref()).map_err(Into::into)
    }

    fn read_link(&self, path: impl AsRef<Utf8Path>) -> Result<Utf8PathBuf> {
        Ok(fs::read_link(path.as_ref())?.try_into()?)
    }

    fn attributes(&self, path: impl AsRef<Utf8Path>) -> Result<Attrs> {
        let stat = stat::stat(path.as_ref().as_std_path())?;
        let owner = Cow::Owned(
            self.users
                .get_user_by_uid(stat.st_uid)
                .ok_or_else(|| anyhow!("Failed to get user from UID: {}", stat.st_uid))?
                .name()
                .to_string_lossy()
                .into_owned(),
        );
        let group = Cow::Owned(
            self.users
                .get_group_by_gid(stat.st_gid)
                .ok_or_else(|| anyhow!("Failed to get group from GID: {}", stat.st_gid))?
                .name()
                .to_string_lossy()
                .into_owned(),
        );
        let mode = (stat.st_mode as u16).into();
        Ok(Attrs { owner, group, mode })
    }

    fn set_attributes(&mut self, path: impl AsRef<Utf8Path>, attrs: SetAttrs) -> Result<()> {
        let path = path.as_ref();
        self.apply_attrs(
            path,
            attrs,
            if self.is_directory(path) {
                DEFAULT_DIRECTORY_MODE
            } else {
                DEFAULT_FILE_MODE
            },
        )
    }
}

impl DiskFilesystem {
    pub fn new() -> Self {
        DiskFilesystem {
            users: UsersCache::new(),
        }
    }

    fn apply_attrs(
        &self,
        path: impl AsRef<Utf8Path>,
        attrs: SetAttrs,
        default_mode: Mode,
    ) -> Result<()> {
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
        let mode = PermissionsExt::from_mode(attrs.mode.unwrap_or(default_mode).into());

        nix::unistd::chown(path.as_ref().as_std_path(), uid, gid)?;
        fs::set_permissions(path.as_ref(), mode)?;
        Ok(())
    }
}

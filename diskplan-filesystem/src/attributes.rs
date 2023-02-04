use std::{borrow::Cow, fmt::Debug};

/// The default mode for directories (`0o755` or `rwxr-xr-x`)
pub const DEFAULT_DIRECTORY_MODE: Mode = Mode(0o755);
/// The default mode for files (`0o644` or `rw-r--r--`)
pub const DEFAULT_FILE_MODE: Mode = Mode(0o644);

/// Optional owner, group and UNIX permissions to be set
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SetAttrs<'a> {
    /// An optional owner to set given by name
    pub owner: Option<&'a str>,
    /// An optional group to set given by name
    pub group: Option<&'a str>,
    /// An optional [`Mode`] to set
    pub mode: Option<Mode>,
}

/// Owner, group and UNIX permissions
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attrs<'a> {
    /// The owner of the file or directory
    pub owner: Cow<'a, str>,
    /// The group of the file or directory
    pub group: Cow<'a, str>,
    /// The UNIX permissions of the file or directory
    pub mode: Mode,
}

/// UNIX permissions
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct Mode(u16);

impl Mode {
    /// Returns the inner numeric value of the permissions
    pub fn value(&self) -> u16 {
        self.0
    }
}

impl Debug for Mode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Mode(0o{:03o})", self.0)
    }
}

impl From<u16> for Mode {
    fn from(value: u16) -> Self {
        Mode(value)
    }
}

impl From<Mode> for u16 {
    fn from(mode: Mode) -> Self {
        mode.0
    }
}

impl From<Mode> for u32 {
    fn from(mode: Mode) -> Self {
        mode.0 as u32
    }
}

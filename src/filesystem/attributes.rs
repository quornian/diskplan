use std::{borrow::Cow, fmt::Debug};

pub const DEFAULT_DIRECTORY_MODE: Mode = Mode(0o755);
pub const DEFAULT_FILE_MODE: Mode = Mode(0o644);

#[derive(Debug, Default, Clone, PartialEq)]
pub struct SetAttrs<'a> {
    pub owner: Option<&'a str>,
    pub group: Option<&'a str>,
    pub mode: Option<Mode>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Attrs<'a> {
    pub owner: Cow<'a, str>,
    pub group: Cow<'a, str>,
    pub mode: Mode,
}

#[derive(Clone, PartialEq, Eq)]
pub struct Mode(u16);

impl Mode {
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

use std::{
    convert::{TryFrom, TryInto},
    fmt::Debug,
};

#[derive(thiserror::Error, Debug)]
pub enum MetaError {
    #[error("Permissions string is malformed; expected, e.g. \"0o755\"; got: \"{0}\"")]
    PermissionFormatError(String),

    #[error("Error parsing integer")]
    ParseIntError(#[from] std::num::ParseIntError),
}

#[derive(Debug, Default, PartialEq, Clone)]
pub struct Meta {
    owner: Option<String>,
    group: Option<String>,
    permissions: Option<Permissions>,
}

#[derive(Debug, Default)]
pub struct RawItemMeta {
    pub owner: Option<String>,
    pub group: Option<String>,
    pub permissions: Option<RawPerms>,
}

impl Meta {
    pub fn owner(&self) -> &Option<String> {
        &self.owner
    }
    pub fn group(&self) -> &Option<String> {
        &self.group
    }
    pub fn permissions(&self) -> Option<Permissions> {
        self.permissions
    }
    pub fn is_empty(&self) -> bool {
        match self {
            Meta {
                owner: None,
                group: None,
                permissions: None,
            } => true,
            _ => false,
        }
    }
}

impl TryFrom<RawItemMeta> for Meta {
    type Error = MetaError;
    fn try_from(raw: RawItemMeta) -> Result<Self, MetaError> {
        Ok(Meta {
            owner: raw.owner,
            group: raw.group,
            permissions: match raw.permissions {
                Some(p) => Some(p.try_into()?),
                None => None,
            },
        })
    }
}

#[derive(Clone, Copy, PartialEq)]
pub struct Permissions(u16);

#[derive(Debug)]
pub struct RawPerms(pub String);

impl TryFrom<RawPerms> for Permissions {
    type Error = MetaError;

    fn try_from(raw: RawPerms) -> Result<Self, MetaError> {
        let value = match &raw.0.get(0..2) {
            Some("0o") => match u16::from_str_radix(&raw.0[2..], 8) {
                Ok(n) => n,
                Err(_) => return Err(MetaError::PermissionFormatError(raw.0)),
            },
            _ => return Err(MetaError::PermissionFormatError(raw.0)),
        };
        if value & 0o777 == value {
            Ok(Permissions(value))
        } else {
            Err(MetaError::PermissionFormatError(raw.0))
        }
    }
}

impl From<Permissions> for u16 {
    fn from(perms: Permissions) -> Self {
        perms.0
    }
}

impl Debug for Permissions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Permissions(0o{:03o})", self.0)
    }
}

impl Permissions {
    pub fn mode(&self) -> u16 {
        self.0
    }
}

#[derive(Debug, Default, PartialEq)]
pub struct MetaBuilder {
    meta: Meta,
}

impl MetaBuilder {
    pub fn owner<S: AsRef<str>>(&mut self, s: S) -> &mut Self {
        self.meta.owner = Some(s.as_ref().to_owned());
        self
    }
    pub fn group<S: AsRef<str>>(&mut self, s: S) -> &mut Self {
        self.meta.group = Some(s.as_ref().to_owned());
        self
    }
    pub fn mode(&mut self, mode: u16) -> &mut Self {
        self.meta.permissions = Some(Permissions(mode));
        self
    }
    pub fn merge(&mut self, other: &Meta) -> &mut Self {
        if let Some(owner) = other.owner() {
            self.owner(owner);
        }
        if let Some(group) = other.group() {
            self.group(group);
        }
        if let Some(permissions) = other.permissions() {
            self.mode(permissions.mode());
        }
        self
    }
    pub fn build(&self) -> Meta {
        self.meta.clone()
    }
    pub fn is_empty(&self) -> bool {
        match self.meta {
            Meta {
                owner: None,
                group: None,
                permissions: None,
            } => true,
            _ => false,
        }
    }
}

use serde::{Deserialize, Serialize};
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

    #[error("Error parsing JSON")]
    SerdeJsonError(#[from] serde_json::Error),
}

#[derive(Debug, Default, PartialEq)]
pub struct ItemMeta {
    owner: Option<String>,
    group: Option<String>,
    permissions: Option<Permissions>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct RawItemMeta {
    pub owner: Option<String>,
    pub group: Option<String>,
    pub permissions: Option<RawPerms>,
}

impl ItemMeta {
    pub fn from_str(config: &str) -> Result<ItemMeta, MetaError> {
        let schema: RawItemMeta = serde_json::from_str(config)?;
        let schema: ItemMeta = schema.try_into()?;
        Ok(schema)
    }
    pub fn owner(&self) -> &Option<String> {
        &self.owner
    }
    pub fn group(&self) -> &Option<String> {
        &self.group
    }
    pub fn permissions(&self) -> Option<Permissions> {
        self.permissions
    }
}

impl TryFrom<RawItemMeta> for ItemMeta {
    type Error = MetaError;
    fn try_from(raw: RawItemMeta) -> Result<Self, MetaError> {
        Ok(ItemMeta {
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

#[derive(Debug, Serialize, Deserialize)]
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

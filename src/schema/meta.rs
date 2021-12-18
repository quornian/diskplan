use std::fmt::Debug;

#[derive(Debug, Default, PartialEq, Clone)]
pub struct Meta {
    owner: Option<String>,
    group: Option<String>,
    mode: Option<u16>,
}

impl Meta {
    pub fn owner(&self) -> Option<&str> {
        self.owner.as_deref()
    }
    pub fn group(&self) -> Option<&str> {
        self.group.as_deref()
    }
    pub fn mode(&self) -> Option<u16> {
        self.mode
    }
    pub fn is_empty(&self) -> bool {
        match self {
            Meta {
                owner: None,
                group: None,
                mode: None,
            } => true,
            _ => false,
        }
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
        self.meta.mode = Some(mode);
        self
    }
    pub fn merge(&mut self, other: &Meta) -> &mut Self {
        if let Some(owner) = other.owner() {
            self.owner(owner);
        }
        if let Some(group) = other.group() {
            self.group(group);
        }
        if let Some(mode) = other.mode() {
            self.mode(mode);
        }
        self
    }
    pub fn build(&self) -> Meta {
        self.meta.clone()
    }
    pub fn is_empty(&self) -> bool {
        self.meta.is_empty()
    }
}

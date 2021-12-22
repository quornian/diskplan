use std::fmt::Debug;

#[derive(Debug, Default, PartialEq, Clone)]
pub struct Meta<'t> {
    owner: Option<&'t str>,
    group: Option<&'t str>,
    mode: Option<u16>,
}

impl<'t> Meta<'t> {
    pub fn owner(&self) -> Option<&'t str> {
        self.owner
    }
    pub fn group(&self) -> Option<&'t str> {
        self.group
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
pub struct MetaBuilder<'t> {
    meta: Meta<'t>,
}

impl<'t> MetaBuilder<'t> {
    pub fn owner(mut self, s: &'t str) -> Self {
        self.meta.owner = Some(s);
        self
    }
    pub fn group(mut self, s: &'t str) -> Self {
        self.meta.group = Some(s);
        self
    }
    pub fn mode(mut self, mode: u16) -> Self {
        self.meta.mode = Some(mode);
        self
    }
    pub fn merge(mut self, other: &Meta<'t>) -> Self {
        if let Some(owner) = other.owner() {
            self.meta.owner = Some(owner);
        }
        if let Some(group) = other.group() {
            self.meta.group = Some(group);
        }
        if let Some(mode) = other.mode() {
            self.meta.mode = Some(mode);
        }
        self
    }
    pub fn build(self) -> Meta<'t> {
        self.meta
    }
    pub fn is_empty(&self) -> bool {
        self.meta.is_empty()
    }
}

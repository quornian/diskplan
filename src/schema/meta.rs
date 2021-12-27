use std::fmt::Debug;

#[derive(Debug, Default, PartialEq, Clone)]
pub struct Meta<'t> {
    pub owner: Option<&'t str>,
    pub group: Option<&'t str>,
    pub mode: Option<u16>,
}

impl<'t> Meta<'t> {
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

    pub fn merge(&self, other: &Self) -> Self {
        Meta {
            owner: other.owner.or(self.owner),
            group: other.group.or(self.group),
            mode: other.mode.or(self.mode),
        }
    }
}

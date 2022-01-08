use std::fmt::Debug;

#[derive(Debug, Default, PartialEq, Clone)]
pub struct Attributes<'t> {
    pub owner: Option<&'t str>,
    pub group: Option<&'t str>,
    pub mode: Option<u16>,
}

impl<'t> Attributes<'t> {
    pub fn is_empty(&self) -> bool {
        match self {
            Attributes {
                owner: None,
                group: None,
                mode: None,
            } => true,
            _ => false,
        }
    }
}

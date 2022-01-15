use std::fmt::Debug;

use super::Expression;

#[derive(Debug, Default, PartialEq, Clone)]
pub struct Attributes<'t> {
    pub owner: Option<Expression<'t>>,
    pub group: Option<Expression<'t>>,
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

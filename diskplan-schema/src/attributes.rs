use std::fmt::Debug;

use super::Expression;

/// Owner, group and UNIX permissions
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Attributes<'t> {
    /// The owner to be set, if given
    pub owner: Option<Expression<'t>>,
    /// The group to be set, if given
    pub group: Option<Expression<'t>>,
    /// The UNIX permissions to be set, if given
    pub mode: Option<u16>,
}

impl<'t> Attributes<'t> {
    /// Returns true if no attributes are to be set by this entry
    pub fn is_empty(&self) -> bool {
        matches!(
            self,
            Attributes {
                owner: None,
                group: None,
                mode: None,
            }
        )
    }
}

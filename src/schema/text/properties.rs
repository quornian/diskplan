use super::*;

#[derive(Default)]
pub struct Properties {
    pub match_regex: Option<Expression>,
    pub vars: HashMap<Identifier, Expression>,
    pub defs: HashMap<Identifier, Schema>,
    pub meta: MetaBuilder,
    // Directory only
    pub entries: Vec<SchemaEntry>,
    // File only
    pub source: Option<Expression>,

    // Set if this schema inherits a definition from elsewhere
    pub use_def: Option<Identifier>,
}

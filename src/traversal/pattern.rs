use anyhow::Result;
use regex::Regex;

use crate::{filesystem::SplitPath, schema::Expression};

use super::{eval::evaluate, Stack};

pub(super) enum CompiledPattern {
    Any,
    Regex(regex::Regex),
}

impl CompiledPattern {
    pub fn compile(
        pattern: Option<&Expression>,
        stack: Option<&Stack>,
        path: &SplitPath,
    ) -> Result<CompiledPattern> {
        Ok(match pattern {
            None => CompiledPattern::Any,
            Some(expr) => {
                let pattern = evaluate(expr, stack, path)?;
                Regex::new(&pattern)?; // Ensure it's valid before encasing to avoid injection
                CompiledPattern::Regex(Regex::new(&format!("^(?:{})$", pattern))?)
            }
        })
    }

    pub fn matches(&self, text: &str) -> bool {
        match self {
            &Self::Any => true,
            &Self::Regex(ref regex) => regex.is_match(text),
        }
    }
}

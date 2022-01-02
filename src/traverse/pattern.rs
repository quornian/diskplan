use anyhow::Result;
use regex::Regex;

use crate::{filesystem::SplitPath, schema::Pattern};

use super::{eval::evaluate, Scope};

pub(super) enum CompiledPattern<'a> {
    Any,
    Fixed(&'a str),
    Regex(regex::Regex),
}

impl<'a> CompiledPattern<'a> {
    pub fn compile(
        pattern: Option<&Pattern<'a>>,
        stack: &[Scope],
        path: &SplitPath,
    ) -> Result<CompiledPattern<'a>> {
        Ok(match pattern {
            None => CompiledPattern::Any,
            Some(Pattern::Fixed(fixed)) => CompiledPattern::Fixed(fixed),
            Some(Pattern::Regex(expr)) => {
                let pattern = evaluate(expr, stack, path)?;
                Regex::new(&pattern)?; // Ensure it's valid before encasing to avoid injection
                CompiledPattern::Regex(Regex::new(&format!("^(?:{})$", pattern))?)
            }
        })
    }

    pub fn matches(&self, text: &str) -> bool {
        match self {
            &Self::Any => true,
            &Self::Fixed(fixed) => text == fixed,
            &Self::Regex(ref regex) => regex.is_match(text),
        }
    }
}
